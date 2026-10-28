//! `tasks.notice_replies` ← `conversations.member_replied`: the member
//! replied in a conversation, so every open **Do** task of theirs that came
//! from it, made before the reply, remembers the reply and asks "Done?"
//! (campbooks-cloud#396).
//!
//! - Facts only: no message is read. Whether the reply did what the task
//!   asks is the member's answer, never Scout's.
//! - Pay tasks are never marked: a reply does not pay a bill. Neither is a
//!   task whose read said it is done some other way (`done_by = 'elsewhere'`:
//!   sign and send a form, file, upload); a task with no `done_by` (made
//!   before it was read) is marked as before.
//! - Only forward: a task keeps the newest reply, so a repeated or
//!   out-of-order delivery changes nothing.
//! - A reply sent before the task was made (the sync of old sent mail)
//!   marks nothing.

use campbooks_kernel::{
    Event, EventHandler, JobContext, JobError,
    tenancy::{Access, WorkspaceId},
};
use chrono::{SubsecRound, Utc};

use crate::{
    domain::{
        Outcome, SELECT_LOCKED, TaskActor, TaskChange, TaskHistoryDetail, TaskId, TaskRow, finish,
    },
    inbound::MemberReplied,
};

/// Marks the tasks a reply may have done.
pub struct NoticeReplies;

impl EventHandler for NoticeReplies {
    type Event = MemberReplied;
    const NAME: &'static str = "tasks.notice_replies";
    const QUEUE: &'static str = "default";

    async fn handle(ctx: &mut JobContext, event: Event<MemberReplied>) -> Result<(), JobError> {
        let reply = event.payload;
        // Postgres keeps microseconds: compare what it will store, so a
        // repeated delivery finds the reply already kept.
        let sent_at = reply.sent_at.trunc_subsecs(6);
        let workspace: WorkspaceId = reply
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("member_replied names an invalid workspace"))?;
        let state = ctx.state().clone();
        let tx = ctx.tx().await?;
        let ids: Vec<TaskId> = sqlx::query_scalar(
            "SELECT id FROM tasks.tasks
              WHERE workspace_id = $1 AND assignee = $2 AND state = 'open' AND verb = 'do'
                AND source_type = 'conversations.conversation' AND source_id = $3
                AND (done_by IS NULL OR done_by = 'reply')
                AND created_at < $4
                AND (replied_at IS NULL OR replied_at < $4)
              ORDER BY id",
        )
        .bind(workspace.as_uuid())
        .bind(reply.owner.as_uuid())
        .bind(&reply.conversation)
        .bind(sent_at)
        .fetch_all(&mut **tx)
        .await?;
        let access = Access {
            workspace,
            user: Some(reply.owner),
            role: None,
        };
        for id in ids {
            // Locked and read again: another change may have come first.
            let Some(before) = sqlx::query_as::<_, TaskRow>(SELECT_LOCKED)
                .bind(id)
                .fetch_optional(&mut **tx)
                .await?
            else {
                continue;
            };
            if !before.is_open() || before.replied_at.is_some_and(|at| at >= sent_at) {
                continue;
            }
            sqlx::query(
                "UPDATE tasks.tasks SET replied_at = $2, replied_message = $3,
                        revision = revision + 1, updated_at = now()
                  WHERE id = $1",
            )
            .bind(id)
            .bind(sent_at)
            .bind(reply.message)
            .execute(&mut **tx)
            .await?;
            finish(
                &state,
                tx,
                &access,
                id,
                &Outcome {
                    actor: TaskActor::Scout,
                    change: TaskChange::Replied,
                    history: None,
                    detail: TaskHistoryDetail::default(),
                    previous_assignee: None,
                    previous_handed_by: None,
                    before: Some(before),
                },
                Utc::now().date_naive(),
            )
            .await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
