//! Errors without content. A `serde_json` error's message quotes the value
//! it failed on (a subject, an address), and a provider's error can echo
//! the request; job errors (`kernel.jobs.last_error`) and logs keep only
//! what kind of failure it was and where (`campbooks_kernel::redact`).

use campbooks_ai::Error as AiError;
use campbooks_kernel::JobError;

pub use campbooks_kernel::redact::json;

/// A strategy or provider failure as a job outcome, by kind only.
pub fn ai(error: &AiError) -> JobError {
    match error {
        AiError::Status { status } => JobError::retry(format!("decide provider answered {status}")),
        AiError::Transport(_) => JobError::retry("decide provider unreachable"),
        AiError::InvalidResponse(_) => JobError::retry("decide provider gave an unexpected answer"),
        AiError::RoleNotConfigured(_) | AiError::Config(_) => {
            JobError::fatal("the decide role is not configured")
        }
        AiError::ExternalContentRefused { .. } => JobError::fatal(
            "the decide role is off this instance's network and may not receive mail content",
        ),
        AiError::InvalidQuestion(_) => JobError::fatal("a decide question is invalid"),
        AiError::AnswersNotTrusted => {
            JobError::fatal("the decide model has not passed its answers check")
        }
    }
}
