//! `connectors.queue_send` — the conversations module asks for a send
//! (`conversations.send_requested`): record it once, keyed by its send id,
//! hold its document, and enqueue `connectors.send_message`. A repeated
//! delivery records and enqueues nothing new.
//!
//! The event is not taken as proof (campbooks-cloud#226): the conversations
//! module is asked, through the kernel's outbound-requests port, whether
//! the send stands (released, waiting for the provider, its author still
//! allowed to send from that inbox), and the document recorded is the one
//! it stored, never the one the event names. A send that does not stand,
//! or names another inbox, is recorded nowhere.

use campbooks_kernel::{
    Event, EventConsumer, JobContext, JobError, blobs::BlobOwner, jobs::EnqueueOptions,
    tenancy::WorkspaceId,
};
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::{
    domain::model::{InboxId, SendId, StreamId},
    events::SendRequested,
    features::send_message::{GIVE_UP_AFTER, SEND_OWNER, SendMessage, SendMessagePayload},
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
        let stored = state
            .outbound
            .stored_send(&state, tx, workspace, requested.send_id.into())
            .await?;
        let Some(stored) = stored.filter(|s| s.inbox == Uuid::from(requested.inbox_id)) else {
            tracing::warn!(
                %workspace,
                send = %requested.send_id,
                "send_requested names no send that stands; nothing is sent"
            );
            return Ok(());
        };
        let message = stored.message;
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
            json!(message),
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
                &message,
                &BlobOwner::new(SEND_OWNER, requested.send_id),
            )
            .await?;
        // The send, then a check behind it (same ordering key: it runs only
        // once no earlier job for this send is alive) that settles it if
        // that job died, so its draft never stays "sending".
        let order = format!("send:{}", requested.send_id);
        campbooks_kernel::jobs::enqueue_to::<SendMessage>(
            tx,
            &SendMessagePayload {
                send: requested.send_id,
                give_up: false,
            },
            EnqueueOptions::for_workspace(requested.workspace.clone())
                .unique(order.clone())
                .ordered_by(order.clone())
                .priority(10),
        )
        .await?;
        campbooks_kernel::jobs::enqueue_to::<SendMessage>(
            tx,
            &SendMessagePayload {
                send: requested.send_id,
                give_up: true,
            },
            EnqueueOptions::for_workspace(requested.workspace.clone())
                .unique(format!("give_up:{}", requested.send_id))
                .ordered_by(order)
                .run_at(Utc::now() + GIVE_UP_AFTER),
        )
        .await?;
        Ok(())
    }
}
