//! Other modules' reads, run as the member: through
//! `kernel::operations::invoke` with `Caller::Assistant`, by operation id
//! only, so every scope, membership and record check applies as for the
//! member's own request, and only operations that opt the assistant in run.

use axum::http::StatusCode;
use campbooks_kernel::{
    AppState,
    auth::Principal,
    operations::{Caller, Invocation, InvokeError, invoke},
    tenancy::WorkspaceId,
};
use serde_json::Value;

/// Why a read gave nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unread {
    /// The member cannot see it (404), or may not read it (403).
    NotVisible,
    /// The operation refused the input (400, 409, 422…).
    Refused,
    /// It failed, or is not an operation the assistant may call.
    Broken(&'static str),
}

impl Unread {
    /// For the log: a fixed word, never the operation's answer.
    pub const fn kind(self) -> &'static str {
        match self {
            Self::NotVisible => "not_visible",
            Self::Refused => "refused",
            Self::Broken(kind) => kind,
        }
    }
}

/// The member, as the assistant acts for them.
#[derive(Debug, Clone)]
pub struct Member {
    /// Their state (the composed operations).
    pub state: AppState,
    /// Their principal: their scopes, as on their own request.
    pub principal: Principal,
    /// The workspace.
    pub workspace: WorkspaceId,
}

impl Member {
    /// Run read `operation` with `input` (path parameters by name, then the
    /// query) as the member.
    pub async fn read(&self, operation: &str, input: Value) -> Result<Value, Unread> {
        let invocation =
            Invocation::new(Caller::Assistant, self.principal.clone(), operation, input)
                .in_workspace(self.workspace.to_string());
        match invoke(&self.state, invocation).await {
            Ok(output) => Ok(output.body),
            Err(InvokeError::Failed { status, .. })
                if status == StatusCode::NOT_FOUND || status == StatusCode::FORBIDDEN =>
            {
                Err(Unread::NotVisible)
            }
            Err(InvokeError::Failed { status, .. }) if status.is_client_error() => {
                Err(Unread::Refused)
            }
            Err(InvokeError::InvalidInput(_)) => Err(Unread::Refused),
            Err(InvokeError::Failed { .. }) => Err(Unread::Broken("failed")),
            Err(InvokeError::NotAllowed { .. }) => Err(Unread::Broken("not_allowed")),
            Err(InvokeError::UnknownOperation(_)) => Err(Unread::Broken("unknown_operation")),
            Err(InvokeError::NotInstalled) => Err(Unread::Broken("not_installed")),
            Err(InvokeError::ConfirmationRequired(_) | InvokeError::IdempotencyKeyRequired(_)) => {
                Err(Unread::Broken("mutating"))
            }
        }
    }
}
