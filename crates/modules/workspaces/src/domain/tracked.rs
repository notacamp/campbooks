//! The snapshots the module's tracked events carry (ADR 0020), read from its
//! own rows in the transaction of the change, and the plan's enums for its
//! values.
//!
//! Ids, enums, booleans, counts and times only: never a workspace's name, an
//! address (an invitee's or a member's), a person's name, a signup code or
//! its label, or a rejection reason. An invitation is named by its id, a
//! member by their user id.

use campbooks_kernel::{
    Error, Result,
    tenancy::{Role, UserId, WorkspaceId},
};
use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use uuid::Uuid;

use super::{
    model::{Accent, InvitationId, Kind, SignupCodeId},
    signup::{CodeState, RequestStatus},
};
use crate::tracking;

/// The plan's role.
pub fn role(role: Role) -> tracking::Role {
    match role {
        Role::Owner => tracking::Role::Owner,
        Role::Admin => tracking::Role::Admin,
        Role::Member => tracking::Role::Member,
    }
}

/// The plan's status of a signup request.
pub fn request_status(status: RequestStatus) -> tracking::SignupRequestStatus {
    match status {
        RequestStatus::Pending => tracking::SignupRequestStatus::Pending,
        RequestStatus::Approved => tracking::SignupRequestStatus::Approved,
        RequestStatus::Rejected => tracking::SignupRequestStatus::Rejected,
    }
}

/// The plan's state of a signup code.
fn code_state(state: CodeState) -> tracking::SignupCodeState {
    match state {
        CodeState::Active => tracking::SignupCodeState::Active,
        CodeState::UsedUp => tracking::SignupCodeState::UsedUp,
        CodeState::Expired => tracking::SignupCodeState::Expired,
        CodeState::Revoked => tracking::SignupCodeState::Revoked,
    }
}

fn kind(kind: Kind) -> tracking::Kind {
    match kind {
        Kind::Business => tracking::Kind::Business,
        Kind::Personal => tracking::Kind::Personal,
    }
}

fn accent(accent: Accent) -> tracking::Accent {
    match accent {
        Accent::Business => tracking::Accent::Business,
        Accent::Personal => tracking::Accent::Personal,
        Accent::Forest => tracking::Accent::Forest,
    }
}

/// One `workspaces.workspaces` row with its member count.
type WorkspaceRow = (
    Uuid,
    String,
    String,
    i64,
    DateTime<Utc>,
    Option<DateTime<Utc>>,
    Option<DateTime<Utc>>,
);

/// Workspace `id` as it is now (deleted or not), from a transaction
/// working in it.
pub async fn workspace(
    conn: &mut PgConnection,
    id: WorkspaceId,
) -> Result<tracking::WorkspaceSnapshot> {
    let row: Option<WorkspaceRow> = sqlx::query_as(
        "SELECT w.id, w.kind, w.accent,
                (SELECT count(*) FROM workspaces.members m WHERE m.workspace_id = w.id),
                w.created_at, w.deleted_at, w.purge_after
           FROM workspaces.workspaces w WHERE w.id = $1",
    )
    .bind(id)
    .fetch_optional(conn)
    .await?;
    let (id, kind_name, accent_name, member_count, created_at, deleted_at, purge_after) =
        row.ok_or_else(|| Error::internal("a tracked workspace is gone"))?;
    Ok(tracking::WorkspaceSnapshot {
        id,
        kind: kind(kind_name.parse()?),
        accent: accent(accent_name.parse()?),
        member_count,
        created_at,
        deleted_at,
        purge_after,
    })
}

/// Member `user` of `workspace` as they are now, from a transaction working
/// in it; `None` when they are not a member.
pub async fn member(
    conn: &mut PgConnection,
    workspace: WorkspaceId,
    user: UserId,
) -> Result<Option<tracking::MemberSnapshot>> {
    let row: Option<(Uuid, String, bool, DateTime<Utc>)> = sqlx::query_as(
        "SELECT m.user_id, m.role, u.suspended, m.joined_at
           FROM workspaces.members m
           JOIN workspaces.users u ON u.id = m.user_id
          WHERE m.workspace_id = $1 AND m.user_id = $2",
    )
    .bind(workspace)
    .bind(user)
    .fetch_optional(conn)
    .await?;
    row.map(|(user_id, role_name, suspended, joined_at)| {
        Ok(tracking::MemberSnapshot {
            user_id,
            role: role(role_name.parse()?),
            suspended,
            joined_at,
        })
    })
    .transpose()
}

/// [`member`], for a change that must have left them a member.
pub async fn present_member(
    conn: &mut PgConnection,
    workspace: WorkspaceId,
    user: UserId,
) -> Result<tracking::MemberSnapshot> {
    member(conn, workspace, user)
        .await?
        .ok_or_else(|| Error::internal("a tracked member is gone"))
}

/// One `workspaces.invitations` row, without its address or token.
type InvitationRow = (
    Uuid,
    String,
    bool,
    Uuid,
    DateTime<Utc>,
    DateTime<Utc>,
    Option<DateTime<Utc>>,
    Option<Uuid>,
);

/// Invitation `id` as it is now, from a transaction working in its
/// workspace. Never its address.
pub async fn invitation(
    conn: &mut PgConnection,
    id: InvitationId,
) -> Result<tracking::InvitationSnapshot> {
    let row: Option<InvitationRow> = sqlx::query_as(
        "SELECT id, role, expires_at <= now(), invited_by, created_at, expires_at,
                accepted_at, accepted_by
           FROM workspaces.invitations WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(conn)
    .await?;
    let (id, role_name, expired, invited_by, created_at, expires_at, accepted_at, accepted_by) =
        row.ok_or_else(|| Error::internal("a tracked invitation is gone"))?;
    let state = if accepted_at.is_some() {
        tracking::InvitationState::Accepted
    } else if expired {
        tracking::InvitationState::Expired
    } else {
        tracking::InvitationState::Pending
    };
    Ok(tracking::InvitationSnapshot {
        id,
        role: role(role_name.parse()?),
        state,
        invited_by,
        created_at,
        expires_at,
        accepted_at,
        accepted_by,
    })
}

/// One `workspaces.signup_requests` row, without its reason.
type RequestRow = (Uuid, String, bool, DateTime<Utc>, Option<DateTime<Utc>>);

/// The signup request of `user` as it is now. Whether it holds a reason,
/// never the reason.
pub async fn signup_request(
    conn: &mut PgConnection,
    user: UserId,
) -> Result<tracking::SignupRequestSnapshot> {
    let row: Option<RequestRow> = sqlx::query_as(
        "SELECT user_id, status, reason IS NOT NULL, requested_at, decided_at
               FROM workspaces.signup_requests WHERE user_id = $1",
    )
    .bind(user)
    .fetch_optional(conn)
    .await?;
    let (user_id, status, with_reason, requested_at, decided_at) =
        row.ok_or_else(|| Error::internal("a tracked signup request is gone"))?;
    Ok(tracking::SignupRequestSnapshot {
        user_id,
        status: request_status(RequestStatus::parse(&status)?),
        with_reason,
        requested_at,
        decided_at,
    })
}

/// One `workspaces.signup_codes` row, without its digest or label.
type CodeRow = (
    Uuid,
    i32,
    i32,
    Option<DateTime<Utc>>,
    DateTime<Utc>,
    Option<DateTime<Utc>>,
);

/// Signup code `id` as it is now, judged at `now`. Never the code or its
/// label.
pub async fn signup_code(
    conn: &mut PgConnection,
    id: SignupCodeId,
    now: DateTime<Utc>,
) -> Result<tracking::SignupCodeSnapshot> {
    let row: Option<CodeRow> = sqlx::query_as(
        "SELECT id, max_uses, uses, expires_at, created_at, revoked_at
           FROM workspaces.signup_codes WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(conn)
    .await?;
    let (id, max_uses, uses, expires_at, created_at, revoked_at) =
        row.ok_or_else(|| Error::internal("a tracked signup code is gone"))?;
    Ok(tracking::SignupCodeSnapshot {
        id,
        state: code_state(CodeState::of(
            revoked_at.is_some(),
            uses,
            max_uses,
            expires_at,
            now,
        )),
        max_uses: i64::from(max_uses),
        uses: i64::from(uses),
        expires_at,
        created_at,
        revoked_at,
    })
}
