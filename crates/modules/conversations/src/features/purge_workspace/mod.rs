//! Deleting a workspace's conversations when the workspace is deleted.
//!
//! `workspaces.workspace_deleting` hides the workspace at once (every request
//! to it answers 404); its data is kept until `purge_after`, when
//! `conversations.purge_workspace` deletes this module's rows. The blobs
//! (bodies, attachments, participants documents) are purged by the kernel
//! with the workspace's other blobs.

use campbooks_kernel::{
    Event, EventHandler, Job, JobContext, JobError, jobs::EnqueueOptions, tenancy::WorkspaceId,
};
use serde::{Deserialize, Serialize};

use crate::events::WorkspaceDeleting;

/// Schedules the purge for the end of the grace period.
pub struct ScheduleWorkspacePurge;

impl EventHandler for ScheduleWorkspacePurge {
    type Event = WorkspaceDeleting;
    const NAME: &'static str = "conversations.schedule_workspace_purge";
    const QUEUE: &'static str = "default";

    async fn handle(ctx: &mut JobContext, event: Event<WorkspaceDeleting>) -> Result<(), JobError> {
        let deleting = event.payload;
        let options = EnqueueOptions::for_workspace(deleting.workspace.clone())
            .unique(format!("conversations.purge:{}", deleting.workspace))
            .run_at(deleting.purge_after);
        ctx.enqueue(&PurgeWorkspace, options).await?;
        Ok(())
    }
}

/// Deletes every row of the job's workspace in this module's schema.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct PurgeWorkspace;

impl Job for PurgeWorkspace {
    const NAME: &'static str = "conversations.purge_workspace";
    const QUEUE: &'static str = "default";

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        let workspace = ctx
            .workspace()
            .and_then(|w| w.parse::<WorkspaceId>().ok())
            .ok_or_else(|| JobError::fatal("purge enqueued outside a workspace"))?;
        let tx = ctx.tx().await?;
        // Inboxes cascade to everything else; corrections and decisions go
        // with their conversations and messages.
        let deleted = sqlx::query!(
            "DELETE FROM conversations.inboxes WHERE workspace_id = $1",
            workspace.as_uuid()
        )
        .execute(&mut **tx)
        .await?
        .rows_affected();
        // Not tied to an inbox: whether each member is suspended.
        sqlx::query!(
            "DELETE FROM conversations.member_suspensions WHERE workspace_id = $1",
            workspace.as_uuid()
        )
        .execute(&mut **tx)
        .await?;
        // Nor what members told Scout about people (campbooks-cloud#272).
        sqlx::query("DELETE FROM conversations.person_notes WHERE workspace_id = $1")
            .bind(workspace.as_uuid())
            .execute(&mut **tx)
            .await?;
        // Nor their signatures (campbooks-cloud#393).
        sqlx::query("DELETE FROM conversations.signatures WHERE workspace_id = $1")
            .bind(workspace.as_uuid())
            .execute(&mut **tx)
            .await?;
        tracing::info!(
            inboxes = deleted,
            "conversations purged for a deleted workspace"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use campbooks_kernel::tenancy::WorkspaceId;
    use chrono::Utc;
    use sqlx::PgPool;

    use crate::{events::WorkspaceDeleting, testkit::Kit};

    #[sqlx::test(migrations = "../../../migrations")]
    async fn a_deleted_workspace_s_conversations_are_purged_after_the_grace_period(db: PgPool) {
        let kit = Kit::new(db.clone()).await;
        let gone = WorkspaceId::new();
        let kept = WorkspaceId::new();
        for workspace in [gone, kept] {
            let inbox = kit.inbox(workspace, "sam@example.com", &[]).await;
            kit.ingest(workspace, inbox, "en").await;
            // A member suspended there.
            sqlx::query(
                "INSERT INTO conversations.member_suspensions
                     (workspace_id, user_id, suspended, as_of)
                 VALUES ($1, gen_random_uuid(), true, now())",
            )
            .bind(workspace.as_uuid())
            .execute(&db)
            .await
            .unwrap();
            // What a member told Scout about a person.
            sqlx::query(
                "INSERT INTO conversations.person_notes (id, workspace_id, user_id, address, note)
                 VALUES (gen_random_uuid(), $1, gen_random_uuid(), 'marta@clientes.example',
                         'My landlord. Formal, in Portuguese.')",
            )
            .bind(workspace.as_uuid())
            .execute(&db)
            .await
            .unwrap();
            // A member's signature for the inbox.
            sqlx::query(
                "INSERT INTO conversations.signatures (workspace_id, inbox_id, user_id, text)
                 SELECT $1, id, gen_random_uuid(), 'Sam Rivers' FROM conversations.inboxes
                  WHERE workspace_id = $1",
            )
            .bind(workspace.as_uuid())
            .execute(&db)
            .await
            .unwrap();
        }
        let mut tx = kit.state.db.begin().await.unwrap();
        campbooks_kernel::db::set_workspace(&mut tx, &gone.to_string())
            .await
            .unwrap();
        kit.state
            .events
            .publish(
                &mut tx,
                &WorkspaceDeleting {
                    workspace: gone.to_string(),
                    purge_after: Utc::now(),
                },
            )
            .await
            .unwrap();
        tx.commit().await.unwrap();
        kit.run_jobs().await;

        let left: Vec<(String, i64)> = sqlx::query_as(
            "SELECT workspace_id::text, count(*) FROM conversations.messages GROUP BY 1",
        )
        .fetch_all(&db)
        .await
        .unwrap();
        assert_eq!(left, [(kept.to_string(), 4)]);
        let suspensions: Vec<String> =
            sqlx::query_scalar("SELECT workspace_id::text FROM conversations.member_suspensions")
                .fetch_all(&db)
                .await
                .unwrap();
        assert_eq!(suspensions, [kept.to_string()]);
        let notes: Vec<String> =
            sqlx::query_scalar("SELECT workspace_id::text FROM conversations.person_notes")
                .fetch_all(&db)
                .await
                .unwrap();
        assert_eq!(notes, [kept.to_string()]);
        let signatures: Vec<String> =
            sqlx::query_scalar("SELECT workspace_id::text FROM conversations.signatures")
                .fetch_all(&db)
                .await
                .unwrap();
        assert_eq!(signatures, [kept.to_string()]);
    }
}
