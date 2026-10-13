//! `connectors.queue_send` — the conversations module asks for a send
//! (`conversations.send_requested`): record it once, keyed by its send id,
//! hold its document, and enqueue `connectors.send_message`. A repeated
//! delivery records and enqueues nothing new.

use campbooks_kernel::{
    Event, EventConsumer, JobContext, JobError, blobs::BlobOwner, jobs::EnqueueOptions,
    tenancy::WorkspaceId,
};
use serde_json::json;

use crate::{
    domain::model::{InboxId, SendId, StreamId},
    events::SendRequested,
    features::send_message::{SEND_OWNER, SendMessage, SendMessagePayload},
};

/// Records requested sends.
#[derive(Debug, Clone, Copy, Default)]
pub struct QueueSend;

impl EventConsumer for QueueSend {
    type Event = SendRequested;
    const NAME: &'static str = "connectors.queue_send";
    const QUEUE: &'static str = "events";

    async fn handle(
        &self,
        ctx: &mut JobContext,
        event: Event<SendRequested>,
    ) -> Result<(), JobError> {
        let requested = event.payload;
        let workspace: WorkspaceId = requested
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("send_requested names no workspace"))?;
        let state = ctx.state().clone();
        let tx = ctx.tx().await?;
        let stream = sqlx::query_scalar!(
            r#"SELECT id AS "id: StreamId" FROM connectors.streams
                WHERE inbox_id = $1 AND capability = 'messages'"#,
            requested.inbox_id as InboxId,
        )
        .fetch_optional(&mut **tx)
        .await?;
        let inserted = sqlx::query_scalar!(
            r#"INSERT INTO connectors.sends (id, workspace_id, stream_id, inbox_id, document)
               VALUES ($1, $2, $3, $4, $5)
               ON CONFLICT (id) DO NOTHING
               RETURNING id"#,
            requested.send_id as SendId,
            workspace as WorkspaceId,
            stream as Option<StreamId>,
            requested.inbox_id as InboxId,
            json!(requested.message),
        )
        .fetch_optional(&mut **tx)
        .await?;
        if inserted.is_none() {
            return Ok(());
        }
        state
            .blobs
            .attach(
                tx,
                &requested.workspace,
                &requested.message,
                &BlobOwner::new(SEND_OWNER, requested.send_id),
            )
            .await?;
        campbooks_kernel::jobs::enqueue_to::<SendMessage>(
            tx,
            &SendMessagePayload {
                send: requested.send_id,
            },
            EnqueueOptions::for_workspace(requested.workspace.clone())
                .unique(format!("send:{}", requested.send_id))
                .priority(10),
        )
        .await?;
        Ok(())
    }
}
