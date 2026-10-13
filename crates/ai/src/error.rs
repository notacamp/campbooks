use crate::Role;

/// Errors from the AI gateway.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// No provider is configured for this role.
    #[error("AI role `{0}` is not configured (set CAMPBOOKS_AI__{upper}__BASE_URL and __MODEL)", upper = .0.as_str().to_uppercase())]
    RoleNotConfigured(Role),
    /// A `decide` call asked questions that cannot be answered (too many
    /// options, duplicate ids, …). A bug in the calling module.
    #[error("invalid decide question: {0}")]
    InvalidQuestion(String),
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
            Error::RoleNotConfigured(role) => Self::Unavailable(
                crate::MESSAGES
                    .reason("ai_role_not_configured")
                    .arg("role", role.as_str()),
            ),
            other => Self::internal(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use campbooks_kernel::i18n::Locale;

    use super::*;

    #[test]
    fn an_unconfigured_role_is_unavailable_and_explained() {
        let err = campbooks_kernel::Error::from(Error::RoleNotConfigured(Role::Vision));
        assert_eq!(err.code(), "ai_role_not_configured");
        assert!(err.to_problem(Locale::Fr).detail.contains("vision"));
    }

    #[test]
    fn catalogue_is_at_parity() {
        assert_eq!(crate::MESSAGES.check(), Vec::<String>::new());
    }
}
