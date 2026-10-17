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
    /// The provider answered with a non-success status. Its body is not
    /// kept: a provider's error can echo the request, and so the mail in it.
    #[error("AI provider returned {status}")]
    Status {
        /// HTTP status code.
        status: u16,
    },
    /// The provider's answer did not have the expected shape. The message
    /// says what was wrong and where, never the answer's text.
    #[error("unexpected AI provider response: {0}")]
    InvalidResponse(String),
    /// The role's endpoint is off the instance's network and the role has
    /// not opted in to receiving mail content; nothing was sent.
    #[error(
        "AI role `{role}` points off this instance's network ({host}) and gets no mail content \
         (set CAMPBOOKS_AI__{upper}__EXTERNAL_CONTENT=allow to opt in)",
        upper = role.as_str().to_uppercase()
    )]
    ExternalContentRefused {
        /// The role.
        role: Role,
        /// The endpoint's host, from the configuration.
        host: String,
    },
    /// The `decide` model has not passed its answers check
    /// ([`crate::decide::check`]), so its answers are not used; nothing was
    /// sent.
    #[error("the decide model has not passed its answers check; its answers are not used")]
    AnswersNotTrusted,
    /// The workspace reached its daily AI limit ([`crate::usage`]); nothing
    /// was sent. The caller does what it does without a model.
    #[error("the workspace reached its daily AI limit of {cap} calls; nothing was sent")]
    OverDailyCap {
        /// Calls a workspace may make per UTC day.
        cap: u32,
    },
    /// A metered gateway was called outside any usage scope
    /// ([`crate::usage::Scope`]): a bug in the calling code. Nothing was
    /// sent.
    #[error("an AI call was made outside a usage scope; nothing was sent")]
    Unattributed,
    /// The call could not be counted (the ledger is unavailable), so it was
    /// not made: a broken ledger never lifts the cap. Try again later.
    #[error("the AI call could not be counted; nothing was sent")]
    Metering,
}

impl From<Error> for campbooks_kernel::Error {
    fn from(err: Error) -> Self {
        match err {
            Error::RoleNotConfigured(role) => Self::Unavailable(
                crate::MESSAGES
                    .reason("ai_role_not_configured")
                    .arg("role", role.as_str()),
            ),
            Error::ExternalContentRefused { role, .. } => Self::Unavailable(
                crate::MESSAGES
                    .reason("ai_external_content_refused")
                    .arg("role", role.as_str()),
            ),
            Error::OverDailyCap { cap } => Self::Unavailable(
                crate::MESSAGES
                    .reason("ai_daily_limit_reached")
                    .arg("cap", cap.to_string()),
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
    fn content_refused_is_unavailable_and_explained() {
        let err = campbooks_kernel::Error::from(Error::ExternalContentRefused {
            role: Role::Complete,
            host: "api.example.com".into(),
        });
        assert_eq!(err.code(), "ai_external_content_refused");
        assert!(err.to_problem(Locale::PtPt).detail.contains("complete"));
    }

    #[test]
    fn the_daily_limit_is_unavailable_and_explained() {
        let err = campbooks_kernel::Error::from(Error::OverDailyCap { cap: 5000 });
        assert_eq!(err.code(), "ai_daily_limit_reached");
        assert!(err.to_problem(Locale::En).detail.contains("5000"));
    }

    #[test]
    fn a_status_error_shows_no_body() {
        let err = Error::Status { status: 400 };
        assert_eq!(err.to_string(), "AI provider returned 400");
        assert_eq!(format!("{err:?}"), "Status { status: 400 }");
    }

    #[test]
    fn catalogue_is_at_parity() {
        assert_eq!(crate::MESSAGES.check(), Vec::<String>::new());
    }
}
