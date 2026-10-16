//! `conversations.pause_suspended_author` ← `workspaces.member_suspended`
//! and `conversations.resume_unsuspended_author` ←
//! `workspaces.member_unsuspended` (campbooks-cloud#147): a rule acts in its
//! author's name, so while the author is suspended their rules read as
//! paused. Nothing on the rule changes: whether its author is suspended is
//! recorded here (`member_suspensions`) and read with the rule, so
//! [`effective_state`](crate::domain::rules::Stored::effective_state) pauses
//! it everywhere (new mail, a running apply run's next batch, previews and
//! suggestions, the list), and lifting the suspension resumes it.
//!
//! Of two changes for one member, the one with the newer `at` wins, whatever
//! order they arrive in; a duplicate changes nothing. Whenever a change is
//! recorded, the member's rules are announced (`rule_changed`) so lists
//! refresh: also when it turns out not to change whether they act, so a
//! suspension and its lift handled at once never leave a list stale.
//!
//! Readers of a rule never learn why its author is away: it reads as paused
//! with `author_unavailable`, the same for any reason a member is held off.

#[cfg(test)]
mod tests;

use campbooks_kernel::{Event, EventHandler, JobContext, JobError, tenancy::UserId};
use chrono::{DateTime, Utc};

use crate::{
    domain::{rule_runs, rules},
    events::{MemberSuspended, MemberUnsuspended},
    model::{RuleId, RuleState},
};

/// Pauses a suspended member's rules.
pub struct PauseSuspendedAuthor;

impl EventHandler for PauseSuspendedAuthor {
    type Event = MemberSuspended;
    const NAME: &'static str = "conversations.pause_suspended_author";
    const QUEUE: &'static str = "default";

    async fn handle(ctx: &mut JobContext, event: Event<MemberSuspended>) -> Result<(), JobError> {
        let MemberSuspended {
            workspace,
            user,
            at,
        } = event.payload;
        record(ctx, &workspace, user, true, at).await
    }
}

/// Resumes the rules of a member whose suspension was lifted.
pub struct ResumeUnsuspendedAuthor;

impl EventHandler for ResumeUnsuspendedAuthor {
    type Event = MemberUnsuspended;
    const NAME: &'static str = "conversations.resume_unsuspended_author";
    const QUEUE: &'static str = "default";

    async fn handle(ctx: &mut JobContext, event: Event<MemberUnsuspended>) -> Result<(), JobError> {
        let MemberUnsuspended {
            workspace,
            user,
            at,
        } = event.payload;
        record(ctx, &workspace, user, false, at).await
    }
}

/// Record that `user` is (or is no longer) suspended as of `at`, unless a
/// newer change is recorded already; announce their rules when it was
/// recorded.
async fn record(
    ctx: &mut JobContext,
    workspace: &str,
    user: UserId,
    suspended: bool,
    at: DateTime<Utc>,
) -> Result<(), JobError> {
    let workspace_id: sqlx::types::Uuid = workspace
        .parse()
        .map_err(|_| JobError::fatal("a member's suspension names an invalid workspace"))?;
    let events = ctx.state().events.clone();
    let tx = ctx.tx().await?;
    let row = sqlx::query_scalar!(
        r#"
        INSERT INTO conversations.member_suspensions (workspace_id, user_id, suspended, as_of)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (workspace_id, user_id) DO UPDATE
           SET suspended = EXCLUDED.suspended, as_of = EXCLUDED.as_of
         WHERE member_suspensions.as_of < EXCLUDED.as_of
        RETURNING suspended
        "#,
        workspace_id,
        user.as_uuid(),
        suspended,
        at,
    )
    .fetch_optional(&mut **tx)
    .await?;
    // Older than what is recorded, or a duplicate.
    if row.is_none() {
        return Ok(());
    }
    let ids = sqlx::query_scalar!(
        r#"SELECT id AS "id: RuleId" FROM conversations.rules
            WHERE taught_by = $1 AND state <> 'removed' ORDER BY id"#,
        user.as_uuid(),
    )
    .fetch_all(&mut **tx)
    .await?;
    for id in &ids {
        let Some(rule) = rules::find(tx, *id).await? else {
            continue;
        };
        let run = rule_runs::latest(tx, rule.id).await?;
        let state: RuleState = rule.effective_state();
        rule_runs::announce(
            tx,
            &events,
            workspace,
            rule.inbox,
            rule.id,
            state,
            run.as_ref(),
        )
        .await?;
    }
    tracing::info!(
        suspended,
        rules = ids.len(),
        "a member's suspension recorded for their rules"
    );
    Ok(())
}
