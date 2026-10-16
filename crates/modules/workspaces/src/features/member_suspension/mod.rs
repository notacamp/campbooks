//! `workspaces.announce_member_suspended` ← `identity.account_suspended`
//! and `workspaces.announce_member_unsuspended` ←
//! `identity.account_unsuspended`: an account's suspension, and its lift,
//! reach every workspace its user belongs to as
//! [`MemberSuspended`] / [`MemberUnsuspended`], so the modules there (the
//! rules a member taught) handle it in that workspace's transaction.
//!
//! Only this module links an account (a token's `sub`, the user's
//! `idp_subject`) to its user and memberships. The memberships are read as
//! that user (`users_see_their_memberships`), never across workspaces, and
//! each workspace's event is published in a transaction working there. A
//! retry after a partial failure publishes again where it already did;
//! consumers keep the newest `at`, so that is harmless.

#[cfg(test)]
mod tests;

use campbooks_kernel::{
    AppState, Event, EventHandler, JobContext, JobError,
    db::BeginFor as _,
    tenancy::{UserId, WorkspaceId},
};
use chrono::{DateTime, Utc};

use crate::{
    domain::db::set_user,
    events::{AccountSuspended, AccountUnsuspended, MemberSuspended, MemberUnsuspended},
};

/// Announces a suspension in each workspace of the account.
pub struct AnnounceMemberSuspended;

impl EventHandler for AnnounceMemberSuspended {
    type Event = AccountSuspended;
    const NAME: &'static str = "workspaces.announce_member_suspended";
    const QUEUE: &'static str = "events";

    async fn handle(ctx: &mut JobContext, event: Event<AccountSuspended>) -> Result<(), JobError> {
        let at = event.payload.at.unwrap_or(event.occurred_at);
        announce(ctx, event.payload.account, at, true).await
    }
}

/// Announces a lifted suspension in each workspace of the account.
pub struct AnnounceMemberUnsuspended;

impl EventHandler for AnnounceMemberUnsuspended {
    type Event = AccountUnsuspended;
    const NAME: &'static str = "workspaces.announce_member_unsuspended";
    const QUEUE: &'static str = "events";

    async fn handle(
        ctx: &mut JobContext,
        event: Event<AccountUnsuspended>,
    ) -> Result<(), JobError> {
        announce(ctx, event.payload.account, event.payload.at, false).await
    }
}

async fn announce(
    ctx: &mut JobContext,
    account: uuid::Uuid,
    at: DateTime<Utc>,
    suspended: bool,
) -> Result<(), JobError> {
    let state = ctx.state().clone();
    let tx = ctx.tx().await?;
    let Some(user) = sqlx::query_scalar!(
        r#"SELECT id AS "id: UserId" FROM workspaces.users WHERE idp_subject = $1"#,
        account.to_string(),
    )
    .fetch_optional(&mut **tx)
    .await?
    else {
        // Never signed in: a member of nothing.
        return Ok(());
    };
    // Across workspaces, as that user: row-level security applies.
    set_user(tx, user).await?;
    let workspaces = sqlx::query_scalar!(
        r#"SELECT m.workspace_id AS "id: WorkspaceId"
             FROM workspaces.members m
             JOIN workspaces.workspaces w ON w.id = m.workspace_id
            WHERE m.user_id = $1 AND w.deleted_at IS NULL
            ORDER BY m.workspace_id"#,
        user as UserId,
    )
    .fetch_all(&mut **tx)
    .await?;
    for workspace in &workspaces {
        publish(&state, *workspace, user, at, suspended).await?;
    }
    tracing::info!(
        workspaces = workspaces.len(),
        suspended,
        "a member's suspension announced in their workspaces"
    );
    Ok(())
}

/// Publish the change in `workspace`, in a transaction working there.
async fn publish(
    state: &AppState,
    workspace: WorkspaceId,
    user: UserId,
    at: DateTime<Utc>,
    suspended: bool,
) -> Result<(), JobError> {
    let mut tx = state.db.begin_for(workspace).await?;
    let ws = workspace.to_string();
    if suspended {
        let event = MemberSuspended {
            workspace: ws,
            user,
            at,
        };
        state.events.publish(&mut tx, &event).await?;
    } else {
        let event = MemberUnsuspended {
            workspace: ws,
            user,
            at,
        };
        state.events.publish(&mut tx, &event).await?;
    }
    tx.commit().await?;
    Ok(())
}
