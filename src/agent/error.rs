//! Error type for AI provider calls.

/// Errors returned by [`crate::agent::Provider::ask`].
///
/// The variants distinguish error classes that callers may want to react
/// to differently: auth failures should prompt the user to set a secret;
/// rate limits should back off; malformed responses are bugs to surface;
/// network errors are typically retryable.
#[derive(thiserror::Error, Debug)]
pub enum ProviderError {
    /// Missing or invalid API key. Caller should prompt for credentials.
    #[error("authentication failed: {0}")]
    Auth(String),

    /// HTTP 429 / provider-specific rate-limit signal. Carries the
    /// retry-after hint in seconds when the provider sends one.
    #[error("rate limited (retry after {0}s)")]
    RateLimited(u64),

    /// Non-success HTTP status or provider-reported error.
    #[error("API error: {0}")]
    Api(String),

    /// Transport-level failure (DNS, connection reset, TLS, timeout).
    #[error("network error: {0}")]
    Network(String),

    /// Provider returned a response the client couldn't parse. Indicates
    /// either a Larkline bug or an API change that needs investigation.
    #[error("malformed response: {0}")]
    Malformed(String),

    /// Misconfiguration (missing model name, invalid base URL, etc).
    /// Caller should surface to the user rather than retrying.
    #[error("config error: {0}")]
    Config(String),
}

impl ProviderError {
    /// Whether the caller can usefully retry this error after a delay.
    /// Used by the agent loop to decide between exponential backoff and
    /// surfacing the failure to the user.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        matches!(self, Self::RateLimited(_) | Self::Network(_))
    }
}
