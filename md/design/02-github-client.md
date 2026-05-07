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
    /// Construct a client. Reads token from --token or GITHUB_TOKEN env var.
    /// Fails immediately if neither is present.
    pub async fn new(token: Option<String>, timeout: Duration) -> Result<Self, GitHubError>;

    /// Send a GraphQL query. Handles retries, rate limits, pagination.
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
- Follows pagination cursors transparently

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

- Network timeouts
- HTTP 5xx (GitHub service errors)
- HTTP 429 (rate limited)
- Temporary DNS failures

Retry policy:
- Up to 3 attempts
- Exponential backoff: 1s, 2s, 4s between attempts
- Jitter: ±10% to avoid thundering herd
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

If the reset time is more than 60 seconds away, return `GitHubError::RateLimited`
with the retry-after value instead of waiting. The caller can decide whether to
retry or fail.

## Pagination

GitHub's GraphQL API uses cursor-based pagination. A query response looks like:

```json
{
  "data": {
    "repository": {
      "issues": {
        "edges": [ ... ],
        "pageInfo": {
          "hasNextPage": true,
          "endCursor": "Y3Vyc29yOjEw"
        }
      }
    }
  }
}
```

The client's `query()` function:
- Accepts a query string with a `$after` variable for the cursor
- Returns only the `data` field (not the pageInfo)
- Automatically follows `hasNextPage`, fetching all pages
- Accumulates results into a single response
- Is transparent to the caller — they see one response with all data

The query must use `first: N` to set page size and `after: $after` for the cursor.
The client handles following cursors automatically.

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
    #[error("no GitHub token found. Set GITHUB_TOKEN or use --token flag")]
    MissingToken,

    /// Network failure: timeout, DNS, TLS error, connection refused, etc.
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    /// HTTP response with 4xx or 5xx status.
    #[error("HTTP {status}: {body}")]
    HttpError { status: u16, body: String },

    /// GraphQL response contained errors (in the errors field).
    #[error("GraphQL error: {0}")]
    GraphQLError(String),

    /// Rate limit exceeded and reset is >60 seconds away.
    #[error("rate limit exceeded, retry after {retry_after} seconds")]
    RateLimited { retry_after: u64 },

    /// Request exceeded the configured timeout.
    #[error("request timeout after {0}s")]
    Timeout(u64),

    /// JSON parsing or serialization failure.
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}
```

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
// Construct the client once at startup
let client = GitHubClient::new(token_from_cli, Duration::from_secs(30)).await?;

// Use it to fetch project data
let issues: ProjectIssues = client.query(
    FETCH_ISSUES_QUERY,
    FetchIssuesVars { owner: "rust-lang", ... }
).await?;

// The client handles:
// - Pagination (fetches all pages automatically)
// - Rate limits (waits and retries)
// - Transient errors (retries with backoff)
// - Timeouts (fails if exceeded)
```

## What we are not doing (v2 scope)

- GitHub Enterprise Server (always public github.com)
- GitHub App authentication (token only)
- Per-request timeout override (global timeout only)
- Automatic exponential backoff tuning (fixed schedule)
- Connection pool size configuration