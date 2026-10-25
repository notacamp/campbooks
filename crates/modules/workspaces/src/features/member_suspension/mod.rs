//! `workspaces.announce_member_suspended` ← `identity.account_suspended`
//! and `workspaces.announce_member_unsuspended` ←
//! `identity.account_unsuspended`: an account's suspension, and its lift,
//! reach every workspace its user belongs to as
//! [`MemberSuspended`](crate::events::MemberSuspended) /
//! [`MemberUnsuspended`](crate::events::MemberUnsuspended), so the modules there (the
//! rules a member taught) handle it in that workspace's transaction.
//!
//! Only this module links an account (a token's `sub`, the user's
//! `idp_subject`) to its user and memberships. The memberships are read as
//! that user (`users_see_their_memberships`), never across workspaces, and
//! each workspace's event is published in a transaction working there.
//!
//! The change is ordered by the account's counter (`seq`,
//! campbooks-cloud#197): one older than the change last announced for the
//! user is dropped; the same one again is announced again (a retry after a
//! partial failure, or a replay), which consumers ignore. The user's row
//! stays locked from reading the change to the end of the job, so a
//! workspace they join meanwhile waits and is then told the change on
//! joining ([`crate::domain::suspension`]).
//!
//! A delivery that dies for good is counted under `suspension_fanout` in
//! Admin → Jobs and logged as "suspension fan-out delivery died".

#[cfg(test)]
mod tests;

use campbooks_kernel::{
    AppState, Event, EventHandler, JobContext, JobError,
    db::BeginFor as _,
    jobs::Watch,
    tenancy::{UserId, WorkspaceId},
};

use crate::{
    domain::{
        db::set_user,
        suspension::{self, Change},
        tracked,
    },
    events::{AccountSuspended, AccountUnsuspended},
    tracking,
};

/// How a dead delivery of the suspension's fan-out is counted and logged:
/// the rules of a suspended member keep acting until it is handled.
pub const SUSPENSION_FANOUT: Watch = Watch {
    code: "suspension_fanout",
    died: "suspension fan-out delivery died",
};

/// Announces a suspension in each workspace of the account.
pub struct AnnounceMemberSuspended;

impl EventHandler for AnnounceMemberSuspended {
    type Event = AccountSuspended;
    const NAME: &'static str = "workspaces.announce_member_suspended";
    const QUEUE: &'static str = "events";
    const WATCH: Option<Watch> = Some(SUSPENSION_FANOUT);

    async fn handle(ctx: &mut JobContext, event: Event<AccountSuspended>) -> Result<(), JobError> {
        let change = Change {
            suspended: true,
            seq: event.payload.seq,
            at: event.payload.at.unwrap_or(event.occurred_at),
        };
        announce(ctx, event.payload.account, change).await
    }
}

/// Announces a lifted suspension in each workspace of the account.
pub struct AnnounceMemberUnsuspended;

impl EventHandler for AnnounceMemberUnsuspended {
    type Event = AccountUnsuspended;
    const NAME: &'static str = "workspaces.announce_member_unsuspended";
    const QUEUE: &'static str = "events";
    const WATCH: Option<Watch> = Some(SUSPENSION_FANOUT);

    async fn handle(
        ctx: &mut JobContext,
        event: Event<AccountUnsuspended>,
    ) -> Result<(), JobError> {
        let change = Change {
            suspended: false,
            seq: event.payload.seq,
            at: event.payload.at,
        };
        announce(ctx, event.payload.account, change).await
    }
}

async fn announce(
    ctx: &mut JobContext,
    account: uuid::Uuid,
    change: Change,
) -> Result<(), JobError> {
    let state = ctx.state().clone();
    let tx = ctx.tx().await?;
    let Some((user, workspaces, fresh)) = record(tx, &account.to_string(), change).await? else {
        return Ok(());
    };
    for workspace in &workspaces {
        publish(&state, *workspace, user, change, fresh).await?;
    }
    tracing::info!(
        workspaces = workspaces.len(),
        suspended = change.suspended,
        "a member's suspension announced in their workspaces"
    );
    Ok(())
}

/// Lock the user whose identity subject is `subject`, record `change` as
/// theirs and list the workspaces to announce it in, with whether it changes
/// whether they are suspended (a replay is announced again, but tracked
/// only once); `None` when there is nothing to announce:
/// no such user (never signed in), or a newer change already announced.
/// The lock is held until `tx` ends, so a workspace the user joins
/// meanwhile is told on joining, and one they leave meanwhile is left
/// first.
pub(crate) async fn record(
    tx: &mut sqlx::PgConnection,
    subject: &str,
    change: Change,
) -> Result<Option<(UserId, Vec<WorkspaceId>, bool)>, JobError> {
    let Some((user, known)) = suspension::lock_subject(tx, subject).await? else {
        // Never signed in: a member of nothing.
        return Ok(None);
    };
    if known.is_some_and(|known| change.is_older_than(&known)) {
        tracing::info!(
            suspended = change.suspended,
            "an older suspension change than the one announced; nothing announced"
        );
        return Ok(None);
    }
    // Tracked only when it changes whether they are suspended: a replay,
    // or a second suspension with no lift between, changes nothing.
    let fresh = known.map_or(change.suspended, |known| {
        known.suspended != change.suspended
    });
    suspension::record(tx, user, change).await?;
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
    .fetch_all(&mut *tx)
    .await?;
    Ok(Some((user, workspaces, fresh)))
}

/// Publish `change` in `workspace`, in a transaction working there, and
/// track it there when it is `fresh` (it changes whether the user is
/// suspended).
pub(crate) async fn publish(
    state: &AppState,
    workspace: WorkspaceId,
    user: UserId,
    change: Change,
    fresh: bool,
) -> Result<(), JobError> {
    let mut tx = state.db.begin_for(workspace).await?;
    suspension::announce(&state.events, &mut tx, &workspace.to_string(), user, change).await?;
    // Listed under the user's lock, so still a member; one who is not any
    // more has nothing to track.
    if fresh && let Some(member) = tracked::member(&mut tx, workspace, user).await? {
        if change.suspended {
            state
                .tracker
                .track(&mut tx, &tracking::MemberSuspended { workspace, member })
                .await?;
        } else {
            state
                .tracker
                .track(&mut tx, &tracking::MemberUnsuspended { workspace, member })
                .await?;
        }
    }
    tx.commit().await?;
    Ok(())
}
