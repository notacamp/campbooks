//! The security log: who did what to whose access, recorded in the
//! transaction of the change.

use campbooks_kernel::tenancy::{UserId, WorkspaceId};
use sqlx::PgConnection;

use super::model::SecurityEventId;

/// What happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityKind {
    /// A user signed in for the first time.
    SignedIn,
    /// A user created a workspace.
    WorkspaceCreated,
    /// A member's role changed.
    RoleChanged,
    /// Someone was invited.
    InvitationCreated,
    /// An invitation was accepted.
    InvitationAccepted,
    /// A member was removed.
    MemberRemoved,
    /// A member left.
    MemberLeft,
    /// The workspace was deleted.
    WorkspaceDeleted,
    /// A user asked to create a workspace on an instance whose signup needs
    /// approval.
    SignupRequested,
}

impl SecurityKind {
    /// The stored name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SignedIn => "signed_in",
            Self::WorkspaceCreated => "workspace_created",
            Self::RoleChanged => "role_changed",
            Self::InvitationCreated => "invitation_created",
            Self::InvitationAccepted => "invitation_accepted",
            Self::MemberRemoved => "member_removed",
            Self::MemberLeft => "member_left",
            Self::WorkspaceDeleted => "workspace_deleted",
            Self::SignupRequested => "signup_requested",
        }
    }
}

/// Record one event. `metadata` holds ids and roles only: never tokens,
/// emails or names.
pub async fn record(
    conn: &mut PgConnection,
    workspace: Option<WorkspaceId>,
    user: Option<UserId>,
    kind: SecurityKind,
    metadata: serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO workspaces.security_events (id, workspace_id, user_id, kind, metadata)
         VALUES ($1, $2, $3, $4, $5)",
        SecurityEventId::new() as SecurityEventId,
        workspace as Option<WorkspaceId>,
        user as Option<UserId>,
        kind.as_str(),
        metadata,
    )
    .execute(conn)
    .await?;
    Ok(())
}
