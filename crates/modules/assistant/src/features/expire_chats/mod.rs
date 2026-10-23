//! Chats are kept 30 days after their last message (campbooks-cloud#305).
//!
//! - `assistant.dispatch_expire_chats`, every day, reads which workspaces
//!   hold a chat to expire (the maintenance scope: ids and times only, read
//!   only, through the table's maintenance policy) and enqueues
//!   `assistant.expire_chats` in each, one pending job per workspace. It
//!   reads nothing else and writes nothing.
//! - `assistant.expire_chats`, in the workspace (row-level security
//!   applies), deletes those chats and their messages.

use std::time::Duration;

use campbooks_kernel::{
    Job, JobContext, JobError,
    jobs::{self, EnqueueOptions},
    tenancy::WorkspaceId,
};
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;

/// How often chats to expire are looked for.
pub const EVERY: Duration = Duration::from_hours(24);
/// Days a chat is kept after its last message.
const KEEP_DAYS: i32 = 30;
/// Workspaces dispatched per tick; the next tick takes the rest.
const DISPATCH_BATCH: i64 = 500;

/// The dispatcher tick.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchExpireChats;

impl Job for DispatchExpireChats {
    const NAME: &'static str = "assistant.dispatch_expire_chats";
    const QUEUE: &'static str = "maintenance";
    const MAX_ATTEMPTS: u32 = 1;

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        // Across workspaces, through the table's maintenance policy (ids and
        // times, read only): the maintenance scope, allowed for this file in
        // `architecture.toml`. Each job is enqueued in its workspace.
        let tx = ctx.maintenance_tx().await?;
        let workspaces: Vec<Uuid> = sqlx::query_scalar(
            "SELECT DISTINCT workspace_id FROM assistant.chats
              WHERE last_message_at < now() - make_interval(days => $1)
              ORDER BY workspace_id LIMIT $2",
        )
        .bind(KEEP_DAYS)
        .bind(DISPATCH_BATCH)
        .fetch_all(&mut **tx)
        .await?;
        for workspace in workspaces {
            let workspace = WorkspaceId::from_uuid(workspace).to_string();
            jobs::enqueue_in(
                tx,
                &workspace,
                &ExpireChats,
                EnqueueOptions::for_workspace(workspace.clone()).unique("assistant.expire_chats"),
            )
            .await?;
        }
        Ok(())
    }
}

/// Deletes, in its workspace, the chats whose last message is more than 30
/// days old, with their messages.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpireChats;

impl Job for ExpireChats {
    const NAME: &'static str = "assistant.expire_chats";
    const QUEUE: &'static str = "default";

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        if ctx.workspace().is_none() {
            return Err(JobError::fatal("expire_chats enqueued outside a workspace"));
        }
        let tx = ctx.tx().await?;
        sqlx::query(
            "DELETE FROM assistant.chat_messages WHERE chat_id IN (
                 SELECT id FROM assistant.chats
                  WHERE last_message_at < now() - make_interval(days => $1))",
        )
        .bind(KEEP_DAYS)
        .execute(&mut **tx)
        .await?;
        let expired = sqlx::query(
            "DELETE FROM assistant.chats WHERE last_message_at < now() - make_interval(days => $1)",
        )
        .bind(KEEP_DAYS)
        .execute(&mut **tx)
        .await?
        .rows_affected();
        tracing::info!(chats = expired, "assistant chats expired");
        Ok(())
    }
}
