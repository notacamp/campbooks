//! `conversations.forget_member` ← `workspaces.member_left`: a member who
//! leaves (or is removed) loses every inbox membership, their drafts and
//! what they told Scout about people and Scout's explanations of their
//! threads are deleted, and they stop owning the
//! inboxes their connections fed (which the connectors module removes,
//! deleting or keeping their mail). Coming back to the workspace never
//! brings any of it back. Idempotent.

use campbooks_kernel::{Event, EventHandler, JobContext, JobError, tenancy::WorkspaceId};

use crate::{
    events::{DRAFT_RECORD, InboxChanged, MemberLeft},
    model::InboxId,
};

/// Takes a departed member out of the workspace's inboxes.
pub struct ForgetMember;

impl EventHandler for ForgetMember {
    type Event = MemberLeft;
    const NAME: &'static str = "conversations.forget_member";
    const QUEUE: &'static str = "default";

    #[allow(clippy::too_many_lines)]
    async fn handle(ctx: &mut JobContext, event: Event<MemberLeft>) -> Result<(), JobError> {
        let left = event.payload;
        // Only what existed when they left: a member who came back since
        // keeps what they were given after.
        let before = event.occurred_at;
        let workspace: WorkspaceId = left
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("member_left names an invalid workspace"))?;
        let ws = workspace.to_string();
        let state = ctx.state().clone();
        let tx = ctx.tx().await?;
        let drafts = sqlx::query_scalar!(
            "DELETE FROM conversations.drafts WHERE author = $1 AND created_at <= $2 RETURNING id",
            left.user.as_uuid(),
            before,
        )
        .fetch_all(&mut **tx)
        .await?;
        state
            .blobs
            .detach_owners(tx, &ws, DRAFT_RECORD, &drafts)
            .await?;
        // The suggestions they turned down are theirs alone. Rules they
        // taught stay, paused until a manager adopts or removes them.
        sqlx::query!(
            "DELETE FROM conversations.rule_dismissals WHERE user_id = $1 AND dismissed_at <= $2",
            left.user.as_uuid(),
            before,
        )
        .execute(&mut **tx)
        .await?;
        // Their answers to what Scout noticed too.
        sqlx::query!(
            "DELETE FROM conversations.rule_notice_answers WHERE user_id = $1 AND answered_at <= $2",
            left.user.as_uuid(),
            before,
        )
        .execute(&mut **tx)
        .await?;
        // What they told Scout about people (campbooks-cloud#272): theirs
        // alone, and personal data. One changed since they came back stays.
        let notes = sqlx::query(
            "DELETE FROM conversations.person_notes
              WHERE workspace_id = $1 AND user_id = $2 AND updated_at <= $3",
        )
        .bind(workspace.as_uuid())
        .bind(left.user.as_uuid())
        .bind(before)
        .execute(&mut **tx)
        .await?
        .rows_affected();
        let explanations = forget_explanations(tx, workspace, left.user, before).await?;
        // Whatever held their rules off ends with their membership: they
        // can only come back by signing in, so they are not suspended then.
        // Recorded as of their departure, so a suspension that took effect
        // before it and is delivered late changes nothing. A change counted
        // by the account (`seq`, campbooks-cloud#197) is left as it is: it
        // is the truth as of that count, and joining again announces the
        // newest change, which is never older.
        let workspace_id = workspace.as_uuid();
        sqlx::query(
            "INSERT INTO conversations.member_suspensions (workspace_id, user_id, suspended, as_of)
             VALUES ($1, $2, false, $3)
             ON CONFLICT (workspace_id, user_id) DO UPDATE
                SET suspended = false, as_of = EXCLUDED.as_of
              WHERE member_suspensions.seq IS NULL AND member_suspensions.as_of < EXCLUDED.as_of",
        )
        .bind(workspace_id)
        .bind(left.user.as_uuid())
        .bind(before)
        .execute(&mut **tx)
        .await?;
        let mut inboxes = sqlx::query_scalar!(
            r#"DELETE FROM conversations.inbox_members WHERE user_id = $1 AND created_at <= $2
               RETURNING inbox_id AS "inbox: InboxId""#,
            left.user.as_uuid(),
            before,
        )
        .fetch_all(&mut **tx)
        .await?;
        inboxes.extend(
            sqlx::query_scalar!(
                r#"UPDATE conversations.inboxes SET owner = NULL
                    WHERE owner = $1 AND created_at <= $2
                   RETURNING id AS "id: InboxId""#,
                left.user.as_uuid(),
                before,
            )
            .fetch_all(&mut **tx)
            .await?,
        );
        inboxes.sort_unstable();
        inboxes.dedup();
        for inbox in &inboxes {
            state
                .events
                .publish(
                    tx,
                    &InboxChanged {
                        workspace: ws.clone(),
                        inbox: *inbox,
                    },
                )
                .await?;
        }
        tracing::info!(
            inboxes = inboxes.len(),
            drafts = drafts.len(),
            notes,
            explanations,
            "a departed member's inbox access, drafts, notes and explanations removed"
        );
        Ok(())
    }
}

/// Their explanations of threads (campbooks-cloud#407): what Scout wrote
/// about their mail is theirs alone. One asked for since they came back
/// stays.
async fn forget_explanations(
    tx: &mut sqlx::PgConnection,
    workspace: WorkspaceId,
    user: campbooks_kernel::tenancy::UserId,
    before: chrono::DateTime<chrono::Utc>,
) -> Result<u64, JobError> {
    Ok(sqlx::query(
        "DELETE FROM conversations.explanations
          WHERE workspace_id = $1 AND member_id = $2 AND created_at <= $3",
    )
    .bind(workspace.as_uuid())
    .bind(user.as_uuid())
    .bind(before)
    .execute(tx)
    .await?
    .rows_affected())
}
