//! GitHub API error types.
//!
//! All failures from GitHub requests are translated into structured
//! `GitHubError` variants. The transport layer does not know about
//! higher-level concerns like which project or owner triggered a call;
//! callers that want that context should wrap these errors at their
//! call site.

use std::fmt;

/// Error returned by the GitHub GraphQL client.
#[derive(Debug, thiserror::Error)]
pub enum GitHubError {
    /// No token found in --token flag or GITHUB_TOKEN environment variable.
    #[error("no GitHub token found. Set GITHUB_TOKEN or use --token flag")]
    MissingToken,

    /// HTTP client could not be constructed (TLS backend, proxy config, etc.).
    #[error("failed to initialize HTTP client: {0}")]
    ClientInit(String),

    /// Network-level failure: timeout, DNS, TLS, connection refused, etc.
    #[error("network error ({kind}): {message}")]
    Network {
        /// Category of network failure.
        kind: NetworkErrorKind,
        /// Human-readable description.
        message: String,
    },

    /// HTTP response with error status code (4xx or 5xx).
    #[error("HTTP {status}: {body}")]
    HttpError {
        /// HTTP status code.
        status: u16,
        /// Full response body.
        body: String,
    },

    /// GraphQL response contained errors in the `errors` field.
    #[error("GraphQL error: {0}")]
    GraphQLError(String),

    /// GitHub returned a body we could not interpret: malformed JSON, or a
    /// well-formed envelope with neither `data` nor `errors`.
    #[error("invalid response body: {0}")]
    InvalidResponse(String),

    /// GitHub rate limit exceeded. Caller should wait before retrying.
    #[error("rate limit exceeded, retry after {retry_after}s")]
    RateLimited {
        /// Seconds to wait before retrying.
        retry_after: u64,
    },

    /// Request exceeded the configured timeout.
    #[error("request timeout after {0}s")]
    Timeout(u64),
}

/// Category of network-level failure.
#[derive(Debug, Clone)]
pub enum NetworkErrorKind {
    /// Request timeout (socket, DNS, or connection timeout).
    Timeout,
    /// Connection refused, reset, or closed unexpectedly.
    Connection,
    /// Other network error not categorized above.
    Other(String),
}

impl fmt::Display for NetworkErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetworkErrorKind::Timeout => write!(f, "timeout"),
            NetworkErrorKind::Connection => write!(f, "connection refused"),
            NetworkErrorKind::Other(s) => write!(f, "{s}"),
        }
    }
}

impl GitHubError {
    /// Return the process exit code for this error.
    ///
    /// - 1: malformed response (likely a bug or upstream regression)
    /// - 3: GitHub API errors (network, HTTP, GraphQL, rate limit, timeout)
    /// - 4: configuration errors (missing token, client init failure)
    pub fn exit_code(&self) -> u8 {
        match self {
            GitHubError::MissingToken | GitHubError::ClientInit(_) => 4,
            GitHubError::Network { .. }
            | GitHubError::HttpError { .. }
            | GitHubError::GraphQLError(_)
            | GitHubError::RateLimited { .. }
            | GitHubError::Timeout(_) => 3,
            GitHubError::InvalidResponse(_) => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_token_exit_code() {
        assert_eq!(GitHubError::MissingToken.exit_code(), 4);
    }

    #[test]
    fn client_init_exit_code() {
        assert_eq!(GitHubError::ClientInit("tls".into()).exit_code(), 4);
    }

    #[test]
    fn network_error_exit_code() {
        let err = GitHubError::Network {
            kind: NetworkErrorKind::Timeout,
            message: "timeout waiting for response".to_string(),
        };
        assert_eq!(err.exit_code(), 3);
    }

    #[test]
    fn http_error_exit_code() {
        let err = GitHubError::HttpError {
            status: 500,
            body: "Internal Server Error".to_string(),
        };
        assert_eq!(err.exit_code(), 3);
    }

    #[test]
    fn graphql_error_exit_code() {
        let err = GitHubError::GraphQLError("Field not found".to_string());
        assert_eq!(err.exit_code(), 3);
    }

    #[test]
    fn rate_limited_exit_code() {
        let err = GitHubError::RateLimited { retry_after: 3600 };
        assert_eq!(err.exit_code(), 3);
    }

    #[test]
    fn timeout_exit_code() {
        let err = GitHubError::Timeout(30);
        assert_eq!(err.exit_code(), 3);
    }

    #[test]
    fn invalid_response_exit_code() {
        let err = GitHubError::InvalidResponse("no data, no errors".into());
        assert_eq!(err.exit_code(), 1);
    }

    #[test]
    fn network_error_kind_display() {
        assert_eq!(NetworkErrorKind::Timeout.to_string(), "timeout");
        assert_eq!(
            NetworkErrorKind::Connection.to_string(),
            "connection refused"
        );
        assert_eq!(
            NetworkErrorKind::Other("custom error".to_string()).to_string(),
            "custom error"
        );
    }
}
