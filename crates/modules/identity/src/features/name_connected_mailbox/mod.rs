//! `identity.name_connected_mailbox` ← `connectors.connection_added`: the
//! durable half of naming a way in's mailbox record with its connection
//! (the round trip names it too, as a fast path, but may die first). Only
//! for the grantor's own tracked way in of that provider account; the row
//! is written if it went missing (another round trip's take-back raced
//! it). Idempotent: a replay names the same connection again.
//!
//! The event alone is no proof that the connection exists: any transaction
//! working in the workspace could write it (campbooks-cloud#211), and a
//! record naming a connection that never was would let a later forged
//! removal of it take the way in. So the record is named only with the
//! connection the connectors module (the grant-intake port) holds for that
//! grantor and provider account there. A port that cannot answer is
//! retried, never guessed at.

use campbooks_kernel::{Event, EventConsumer, JobContext, JobError, tenancy::WorkspaceId};
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
        let retry = |e: campbooks_kernel::Error| JobError::retry(e.to_string());
        let tracked = {
            let mut conn = ctx.pool().acquire().await?;
            mailbox_links::tracks(&mut conn, &added.provider, &subject, grantor)
                .await
                .map_err(retry)?
        };
        if !tracked {
            return Ok(());
        }
        let state = ctx.state().clone();
        let live = state
            .grants
            .connection_of(
                &state,
                &grantor.to_string(),
                WorkspaceId::from_uuid(workspace),
                &added.provider,
                &subject,
            )
            .await
            .map_err(retry)?;
        if live.as_deref() != Some(added.connection.as_str()) {
            tracing::warn!(
                "connection_added names a connection its grantor does not hold; not named"
            );
            return Ok(());
        }
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
        .map_err(retry)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
