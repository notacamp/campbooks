//! `identity.name_connected_mailbox` ← `connectors.connection_added`: the
//! durable half of naming a way in's mailbox record with its connection
//! (the round trip names it too, as a fast path, but may die first). Only
//! for the grantor's own tracked way in of that provider account; the row
//! is written if it went missing (another round trip's take-back raced
//! it). Idempotent: a replay names the same connection again.

use campbooks_kernel::{Event, EventConsumer, JobContext, JobError};
use chrono::Utc;
use uuid::Uuid;

use crate::{domain::mailbox_links, events::ConnectionAdded};

/// Names the mailbox record a granted connection belongs to.
#[derive(Debug, Clone, Copy, Default)]
pub struct NameConnectedMailbox;

impl EventConsumer for NameConnectedMailbox {
    type Event = ConnectionAdded;
    const NAME: &'static str = "identity.name_connected_mailbox";
    const QUEUE: &'static str = "events";

    async fn handle(
        &self,
        ctx: &mut JobContext,
        event: Event<ConnectionAdded>,
    ) -> Result<(), JobError> {
        let added = event.payload;
        let (Some(subject), Some(grantor)) = (
            added.account_subject,
            added
                .grantor_subject
                .as_deref()
                .and_then(|s| s.parse::<Uuid>().ok()),
        ) else {
            return Ok(());
        };
        let workspace: Uuid = added
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("connection_added names an invalid workspace"))?;
        let tx = ctx.tx().await?;
        mailbox_links::name_from_event(
            tx,
            workspace,
            &added.connection,
            &added.provider,
            &subject,
            grantor,
            Utc::now(),
        )
        .await
        .map_err(|e| JobError::retry(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
