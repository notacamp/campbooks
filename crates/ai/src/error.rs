use crate::Role;

/// Errors from the AI gateway.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// No provider is configured for this role.
    #[error("AI role `{0}` is not configured (set CAMPBOOKS_AI__{upper}__BASE_URL and __MODEL)", upper = .0.as_str().to_uppercase())]
    RoleNotConfigured(Role),
    /// The AI configuration is invalid.
    #[error("invalid AI configuration: {0}")]
    Config(String),
    /// The provider could not be reached or timed out.
    #[error("AI provider request failed: {0}")]
    Transport(#[from] reqwest::Error),
    /// The provider answered with a non-success status.
    #[error("AI provider returned {status}: {body}")]
    Status {
        /// HTTP status code.
        status: u16,
        /// Response body (truncated).
        body: String,
    },
    /// The provider's answer did not have the expected shape.
    #[error("unexpected AI provider response: {0}")]
    InvalidResponse(String),
}

impl From<Error> for campbooks_kernel::Error {
    fn from(err: Error) -> Self {
        match err {
            Error::RoleNotConfigured(_) => Self::Unavailable(err.to_string()),
            other => Self::internal(other),
        }
    }
}
