# GitHub GraphQL Client

The `github` module owns all communication with the GitHub API. Other modules
import typed structs from `github/projects.rs` and `github/issues.rs` and never
construct URLs, never handle HTTP errors, never parse JSON directly.

This module is the exclusive gateway to GitHub. Everything flows through it.

## Responsibilities

Three things, three things only:

- **Authentication** — read the token from CLI or environment, fail fast if missing
- **Transport** — send GraphQL requests over HTTP, handle retries and rate limits
- **Error translation** — turn network failures and GitHub errors into structured Rust types

The actual GraphQL queries (fields, shapes, variables) live in `projects.rs` and
`issues.rs`. Those modules call back into this module for transport.

## Public API

```rust
pub struct GitHubClient { ... }

impl GitHubClient {
    /// Construct a client. Reads the token from the parameter or the
    /// `GITHUB_TOKEN` env var. Synchronous; does no I/O.
    pub fn new(token: Option<String>, timeout: Duration) -> Result<Self, GitHubError>;

    /// Send a single GraphQL request. Handles retries and rate limits.
    /// Returns the typed `data` field of the response.
    ///
    /// Pagination is the caller's responsibility — see the Pagination section.
    pub async fn query<V: Serialize, T: DeserializeOwned>(
        &self,
        query: &str,
        variables: V,
    ) -> Result<T, GitHubError>;
}
```

Callers construct the client once at startup. The client:
- Owns the HTTP connection pool
- Stores the authentication token
- Stores the timeout duration
- Implements retry logic and rate limit backoff

Errors do not carry caller context (which project, which query name).
Callers wrap errors at the call site if they need it; this keeps the
transport layer focused on transport.

## Authentication

The token comes from two sources in order of priority:

1. `--token` CLI flag — explicit, takes precedence
2. `GITHUB_TOKEN` environment variable — standard convention

If neither is present, `GitHubClient::new()` returns `GitHubError::MissingToken`
before any network I/O occurs. The error message tells the user how to set the
token.

Required scopes:
- `read:project` — read GitHub Projects V2 data
- `repo` — read issue content and blocking relationships on private repositories

For fully public repositories `public_repo` is sufficient.

## Transport

The client uses `reqwest` for HTTP and `tokio` for async runtime. Every GraphQL
query:

1. Serialize variables to JSON
2. POST to `https://api.github.com/graphql` with the Authorization header
3. Parse the response JSON
4. Check HTTP status (4xx/5xx is an error)
5. Check for `errors` field in the response (non-empty is an error)
6. Return `data` on success

The timeout applies to the entire request including retry backoff. If the request
plus retries exceed the timeout, the client returns `GitHubError::Timeout`.

## Retry strategy

Transient errors are retried with exponential backoff and jitter:

- Network failures (timeout, connection refused, DNS, TLS)
- HTTP 5xx (GitHub service errors)
- HTTP 429 (rate limited; see [Rate limiting](#rate-limiting) for the policy)

Retry policy:
- Up to 3 attempts
- Exponential backoff: ~1s, ~2s, ~4s between attempts
- Jitter: ±20% to avoid thundering herd
- Does not exceed the overall request timeout

Non-transient errors (4xx except 429, GraphQL validation errors, auth failures)
fail immediately without retry.

## Rate limiting

GitHub allows 5000 requests per hour for authenticated tokens. When the client
hits the rate limit (HTTP 429):

1. Parse the `X-RateLimit-Reset` header to get the Unix timestamp when the limit resets
2. Calculate seconds to wait
3. Log a message: `"Rate limit exceeded, waiting N seconds before retry"`
4. Sleep until the reset time
5. Retry the request

The client only sleeps if the wait fits within the *remaining* request
timeout. If the reset is further away than the time we have left, it
returns `GitHubError::RateLimited { retry_after }` immediately so the
caller can decide whether to wait or fail. There is no fixed 60-second
threshold — the budget is the timeout.

## Pagination

GitHub's GraphQL API uses cursor-based pagination. The transport does **not**
hide pagination — it sends one request and returns one response. Pagination
loops live in the caller (`projects.rs`, `issues.rs`) where the query and
response shape are known.

Rationale: making pagination transparent in `query()` requires the transport
to know where, in an arbitrary `T`, the `pageInfo` and node list live. That
either forces a `Paginated` trait on every response type or hides query-shape
knowledge inside the transport. Both are worse than a small explicit loop in
the caller, which already owns the query.

The transport provides two reusable types so callers don't redefine them:

```rust
/// Page metadata returned by every GitHub GraphQL connection.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageInfo {
    pub has_next_page: bool,
    pub end_cursor: Option<String>,
}

/// A connection: a list of `nodes` plus `pageInfo`. Embed in your response
/// type to use the standard pagination loop.
#[derive(Debug, Deserialize)]
pub struct Connection<T> {
    pub nodes: Vec<T>,
    #[serde(rename = "pageInfo")]
    pub page_info: PageInfo,
}
```

A caller-side pagination loop looks like this:

```rust
let mut all = Vec::new();
let mut cursor: Option<String> = None;
loop {
    let resp: MyResponse = client
        .query(QUERY, MyVars { after: cursor.clone(), .. })
        .await?;
    let conn = resp.repository.issues; // Connection<Issue>
    all.extend(conn.nodes);
    if !conn.page_info.has_next_page { break; }
    cursor = conn.page_info.end_cursor;
}
```

The query must declare `first: N` for page size and `after: $after` for the
cursor variable. Beyond that, it's the caller's GraphQL.

A generic `paginate(...)` helper in the transport is intentionally not
provided yet — there are only two callers, and the loop pattern is short.
Add a helper if a third caller appears.

## Timeout configuration

Users set a global timeout via:

```bash
skill-tree render --timeout 60
```

Or environment:

```bash
export GITHUB_TIMEOUT=60
```

Default is 30 seconds if neither is set. The timeout applies to the entire
request including retries and rate limit waiting. If the operation exceeds
the timeout, the client returns `GitHubError::Timeout`.

## Error types

```rust
#[derive(Debug, thiserror::Error)]
pub enum GitHubError {
    /// No token in --token or GITHUB_TOKEN environment variable.
    MissingToken,

    /// HTTP client could not be constructed (TLS backend, proxy config, etc.).
    ClientInit(String),

    /// Network-level failure with a sub-category.
    Network { kind: NetworkErrorKind, message: String },

    /// HTTP response with error status code (4xx or 5xx).
    HttpError { status: u16, body: String },

    /// GraphQL response contained errors in the `errors` field.
    GraphQLError(String),

    /// GitHub returned a body we could not interpret: malformed JSON,
    /// or a well-formed envelope with neither `data` nor `errors`.
    InvalidResponse(String),

    /// Rate limit exceeded; see Rate limiting for when the client waits
    /// vs. surfaces this to the caller.
    RateLimited { retry_after: u64 },

    /// Overall budget (timeout) exceeded across attempts.
    Timeout(u64),
}

pub enum NetworkErrorKind { Timeout, Connection, Other(String) }
```

Exit codes (via `GitHubError::exit_code()`):
- 1 — `InvalidResponse` (malformed upstream body; likely a regression)
- 3 — `Network`, `HttpError`, `GraphQLError`, `RateLimited`, `Timeout`
- 4 — `MissingToken`, `ClientInit` (configuration / environment)

Errors do not carry a `context` field. Callers that want to attach which
project or query failed wrap the error at their call site.

## Module structure

```
github/
  mod.rs           — GitHubClient, transport, auth, retry logic
  projects.rs      — ProjectV2 types and query builders
  issues.rs        — Issue types, sub-issues, blocking relationships
```

`projects.rs` and `issues.rs` define the GraphQL queries as `const` strings and
provide typed response structs. They call `client.query()` for transport.

## Example usage

```rust
// Construct the client once at startup. Synchronous, no I/O.
let client = GitHubClient::new(token_from_cli, Duration::from_secs(30))?;

// Single request — no pagination loop:
let project: ProjectMeta = client.query(
    FETCH_PROJECT_META_QUERY,
    FetchProjectMetaVars { owner: "rust-lang", project: 42 },
).await?;

// Paginated fetch — loop lives here in the caller:
let mut all = Vec::new();
let mut cursor: Option<String> = None;
loop {
    let resp: FetchIssuesResponse = client.query(
        FETCH_ISSUES_QUERY,
        FetchIssuesVars { owner: "rust-lang", project: 42, after: cursor.clone() },
    ).await?;
    all.extend(resp.repository.issues.nodes);
    if !resp.repository.issues.page_info.has_next_page { break; }
    cursor = resp.repository.issues.page_info.end_cursor;
}

// The client handles, on each call to query():
// - Rate limits (waits and retries when budget allows)
// - Transient errors (retries with backoff)
// - Overall request timeout
```

## What we are not doing (v2 scope)

- GitHub Enterprise Server (always public github.com)
- GitHub App authentication (token only)
- Per-request timeout override (global timeout only)
- Automatic exponential backoff tuning (fixed schedule)
- Connection pool size configuration