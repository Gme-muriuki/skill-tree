//! GitHub GraphQL API client.
//!
//! This module is the only place in skill-tree that talks to GitHub.
//! Everything else works with the typed structs from [`projects`] and [`issues`].

pub mod issues;
pub mod projects;

use crate::error::{GitHubError, NetworkErrorKind};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// GraphQL primitives
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub(crate) struct GraphQLRequest<'a, V: Serialize> {
    pub query: &'a str,
    pub variables: V,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GraphQLResponse<T> {
    pub data: Option<T>,
    pub errors: Option<Vec<GraphQLErrorResponse>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GraphQLErrorResponse {
    pub message: String,
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// A configured GitHub GraphQL client with built-in retry and rate limit handling.
///
/// Handles network errors, transient failures, rate limiting, and timeouts.
/// Pass `&GitHubClient` to [`projects`] and [`issues`] functions.
pub struct GitHubClient {
    client: Client,
    token: String,
    timeout: Duration,
}

impl GitHubClient {
    const ENDPOINT: &'static str = "https://api.github.com/graphql";
    const MAX_RETRIES: u32 = 3;

    /// Create a new client, reading the token from the parameter or `GITHUB_TOKEN` env var.
    ///
    /// Fails immediately with [`GitHubError::MissingToken`] if neither is present,
    /// before any network I/O occurs.
    pub fn new(token: Option<String>, timeout: Duration) -> Result<Self, GitHubError> {
        let token = token
            .or_else(|| std::env::var("GITHUB_TOKEN").ok())
            .ok_or(GitHubError::MissingToken)?;

        let client = Client::builder()
            .user_agent("skill-tree")
            .timeout(timeout)
            .build()
            .map_err(|e| GitHubError::ClientInit(e.to_string()))?;

        Ok(Self {
            client,
            token,
            timeout,
        })
    }

    /// Send a GraphQL query with automatic retry and rate limit handling.
    ///
    /// Retries transient errors up to 3 times with exponential backoff.
    /// Detects rate limits and waits before retrying when the timeout budget allows.
    /// Fails if the entire operation exceeds the configured timeout.
    pub async fn query<V, T>(&self, query: &str, variables: V) -> Result<T, GitHubError>
    where
        V: Serialize,
        T: for<'de> Deserialize<'de>,
    {
        let start = Instant::now();

        for attempt in 1..=Self::MAX_RETRIES {
            if start.elapsed() >= self.timeout {
                return Err(GitHubError::Timeout(self.timeout.as_secs()));
            }

            let err = match self.query_once(query, &variables).await {
                Ok(response) => return Ok(response),
                Err(err) => err,
            };

            // Last attempt: surface whatever we got, no more retries.
            if attempt == Self::MAX_RETRIES {
                return Err(err);
            }

            // Rate limit: wait if the remaining budget covers it, else fail now.
            if let GitHubError::RateLimited { retry_after } = &err {
                let wait_secs = *retry_after;
                let remaining = self
                    .timeout
                    .as_secs()
                    .saturating_sub(start.elapsed().as_secs());

                if remaining > wait_secs {
                    eprintln!("Rate limited, waiting {wait_secs} seconds...");
                    tokio::time::sleep(Duration::from_secs(wait_secs)).await;
                    continue;
                }
                return Err(err);
            }

            // Transient: back off and retry.
            if Self::is_transient(&err) {
                let backoff = Self::backoff_duration(attempt);
                eprintln!(
                    "Transient error (attempt {}/{}), retrying in {:?}...",
                    attempt,
                    Self::MAX_RETRIES,
                    backoff
                );
                tokio::time::sleep(backoff).await;
                continue;
            }

            // Non-transient: fail fast.
            return Err(err);
        }

        // Loop body always returns or `continue`s on attempts < MAX_RETRIES,
        // and always returns on attempt == MAX_RETRIES.
        unreachable!("retry loop exited without returning")
    }

    /// Send a single GraphQL request without retry logic.
    async fn query_once<V, T>(&self, query: &str, variables: &V) -> Result<T, GitHubError>
    where
        V: Serialize,
        T: for<'de> Deserialize<'de>,
    {
        let request = GraphQLRequest { query, variables };

        let response = self
            .client
            .post(Self::ENDPOINT)
            .bearer_auth(&self.token)
            .json(&request)
            .send()
            .await
            .map_err(Self::classify_reqwest_error)?;

        let status = response.status();
        if !status.is_success() {
            if status.as_u16() == 429 {
                let retry_after = response
                    .headers()
                    .get("X-RateLimit-Reset")
                    .and_then(|h| h.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .and_then(|reset_time| {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .ok()?
                            .as_secs();
                        Some(reset_time.saturating_sub(now))
                    });

                return Err(GitHubError::RateLimited {
                    retry_after: retry_after.unwrap_or(60),
                });
            }

            let body = response.text().await.unwrap_or_default();
            return Err(GitHubError::HttpError {
                status: status.as_u16(),
                body,
            });
        }

        let body: GraphQLResponse<T> = response
            .json()
            .await
            .map_err(Self::classify_reqwest_error)?;

        if let Some(errors) = body.errors {
            let message = errors
                .into_iter()
                .map(|e| e.message)
                .collect::<Vec<_>>()
                .join("; ");
            return Err(GitHubError::GraphQLError(message));
        }

        body.data.ok_or_else(|| {
            GitHubError::InvalidResponse(
                "GraphQL response had neither `data` nor `errors`".to_string(),
            )
        })
    }

    /// Classify a reqwest error. JSON decode failures are reported as
    /// `InvalidResponse`; everything else is a `Network` error.
    fn classify_reqwest_error(err: reqwest::Error) -> GitHubError {
        if err.is_decode() {
            return GitHubError::InvalidResponse(err.to_string());
        }

        let kind = if err.is_timeout() {
            NetworkErrorKind::Timeout
        } else if err.is_connect() {
            NetworkErrorKind::Connection
        } else {
            NetworkErrorKind::Other(err.to_string())
        };

        GitHubError::Network {
            kind,
            message: err.to_string(),
        }
    }

    /// Check if an error is transient and worth retrying.
    fn is_transient(err: &GitHubError) -> bool {
        match err {
            GitHubError::Network { .. } => true,
            GitHubError::HttpError { status, .. } => *status >= 500,
            _ => false,
        }
    }

    /// Exponential backoff with ±20% jitter to avoid thundering herd.
    /// Attempt 1: ~1s, attempt 2: ~2s, attempt 3: ~4s.
    fn backoff_duration(attempt: u32) -> Duration {
        let base_millis = 1000_u64 * 2_u64.pow(attempt - 1);
        let jitter_pct = rand::random::<u64>() % 21; // 0..=20
        let signed = if rand::random::<bool>() {
            base_millis + base_millis * jitter_pct / 100
        } else {
            base_millis - base_millis * jitter_pct / 100
        };
        Duration::from_millis(signed)
    }
}
