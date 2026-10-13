//! `conversations.ingest_messages` — a connector's change batch arrives:
//! `connectors.messages_changed` names the inbox and the batch blob, and this
//! enqueues `conversations.apply_changes` for it, ordered per inbox, with the
//! batch held until applied (`contracts/connectors.messages_changed.v1.schema.json`).

use campbooks_kernel::{Event, EventHandler, JobContext, JobError};

use crate::{events::MessagesChanged, features::apply_changes};

/// Hands each announced batch to `apply_changes`.
pub struct IngestMessages;

impl EventHandler for IngestMessages {
    type Event = MessagesChanged;
    const NAME: &'static str = "conversations.ingest_messages";
    const QUEUE: &'static str = "sync";

    async fn handle(ctx: &mut JobContext, event: Event<MessagesChanged>) -> Result<(), JobError> {
        let changed = event.payload;
        let state = ctx.state().clone();
        let tx = ctx.tx().await?;
        apply_changes::enqueue(
            &state,
            tx,
            &changed.workspace,
            changed.inbox_id,
            &changed.batch,
        )
        .await?;
        Ok(())
    }
}
