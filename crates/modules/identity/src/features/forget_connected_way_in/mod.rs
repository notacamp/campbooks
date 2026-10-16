//! `identity.forget_connected_way_in` ← `connectors.connection_removed`: a
//! way in that came only from connecting a mailbox goes when its last
//! connection does (campbooks-cloud#117), as Remove on Ways to sign in
//! would: whoever signed in with it is signed out and their refresh tokens
//! are revoked, it is audited, and `identity.way_in_removed` is published.
//!
//! It stays when its mailbox is still connected in another workspace, when
//! it was linked by signing up or by Add (not `connect`), or when it is the
//! account's last way in (provider accounts and passkeys). Only mailboxes
//! recorded in `identity.mailbox_links` count (see that module, which also
//! fixes the lock order), so a way in some of whose mailboxes are unknown is
//! never removed. Idempotent: a replay finds the record gone.

use campbooks_kernel::{Event, EventConsumer, JobContext, JobError};
use chrono::Utc;
use uuid::Uuid;

use crate::{
    domain::{accounts, mailbox_links},
    events::{ConnectionRemoved, WayInRemoved},
};

/// Removes the way in a removed mailbox alone brought.
#[derive(Debug, Clone, Copy, Default)]
pub struct ForgetConnectedWayIn;

impl EventConsumer for ForgetConnectedWayIn {
    type Event = ConnectionRemoved;
    const NAME: &'static str = "identity.forget_connected_way_in";
    const QUEUE: &'static str = "events";

    async fn handle(
        &self,
        ctx: &mut JobContext,
        event: Event<ConnectionRemoved>,
    ) -> Result<(), JobError> {
        let retry = |e: campbooks_kernel::Error| JobError::retry(e.to_string());
        let removed = event.payload;
        let (Some(provider), Some(subject)) = (removed.provider, removed.account_subject) else {
            return Ok(());
        };
        let workspace: Uuid = removed
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("connection_removed names an invalid workspace"))?;
        let now = Utc::now();
        let state = ctx.state().clone();
        let tx = ctx.tx().await?;
        let Some(gone) = mailbox_links::forget(tx, workspace, &provider, &subject, now)
            .await
            .map_err(retry)?
        else {
            return Ok(());
        };
        accounts::audit(
            tx,
            Some(gone.account),
            "way_in_removed",
            serde_json::json!({
                "method": "provider", "provider": gone.provider, "way_in": gone.way_in,
                "sessions_ended": gone.sessions_ended, "reason": "connection_removed",
            }),
            now,
        )
        .await
        .map_err(retry)?;
        state
            .events
            .publish(
                tx,
                &WayInRemoved {
                    account: gone.account,
                    method: "provider".to_owned(),
                    provider: Some(gone.provider),
                },
            )
            .await
            .map_err(|e| JobError::retry(e.to_string()))?;
        tracing::info!(
            sessions_ended = gone.sessions_ended,
            "a way in went with its last mailbox"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests;
