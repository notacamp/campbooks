//! Errors without content. A `serde_json` error's message quotes the value
//! it failed on (a subject, an address), and a provider's error can echo
//! the request; job errors (`kernel.jobs.last_error`) and logs keep only
//! what kind of failure it was and where.

use campbooks_ai::Error as AiError;
use campbooks_kernel::JobError;

/// What kind of JSON error, and where: never the offending text.
pub fn json(error: &serde_json::Error) -> String {
    format!(
        "{:?} error at line {}, column {}",
        error.classify(),
        error.line(),
        error.column()
    )
}

/// A strategy or provider failure as a job outcome, by kind only.
pub fn ai(error: &AiError) -> JobError {
    match error {
        AiError::Status { status, .. } => {
            JobError::retry(format!("decide provider answered {status}"))
        }
        AiError::Transport(_) => JobError::retry("decide provider unreachable"),
        AiError::InvalidResponse(_) => JobError::retry("decide provider gave an unexpected answer"),
        AiError::RoleNotConfigured(_) | AiError::Config(_) => {
            JobError::fatal("the decide role is not configured")
        }
        AiError::InvalidQuestion(_) => JobError::fatal("a decide question is invalid"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_errors_keep_no_content() {
        let error =
            serde_json::from_str::<u32>("\"Maya <maya@northwind-studio.example>\"").unwrap_err();
        assert!(
            error.to_string().contains("maya@"),
            "serde quotes the value"
        );
        let redacted = json(&error);
        assert!(!redacted.contains("maya"), "{redacted}");
        assert!(redacted.starts_with("Data error at line 1"), "{redacted}");
    }
}
