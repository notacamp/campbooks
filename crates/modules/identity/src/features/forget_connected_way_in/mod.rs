//! `identity.forget_connected_way_in` ← `connectors.connection_removed`: a
//! way in that came only from connecting a mailbox goes when its last
//! connection does (campbooks-cloud#117), as Remove on Ways to sign in
//! would: whoever signed in with it is signed out and their refresh tokens
//! are revoked, it is audited, and `identity.way_in_removed` is published.
//!
//! It stays when another recorded connection still uses it, when it was
//! linked by signing up or by Add (not `connect`), or when it is the
//! account's last way in (provider accounts and passkeys). Only connections
//! recorded in `identity.mailbox_links` count (see that module), so a way
//! in some of whose connections are unknown is never removed. Idempotent: a
//! replay finds the record gone.

use campbooks_kernel::{Event, EventConsumer, JobContext, JobError};
use chrono::Utc;
use uuid::Uuid;

use crate::{
    domain::{accounts, mailbox_links, sessions, ways_in},
    events::{ConnectionRemoved, WayInRemoved},
};

/// Removes the way in a removed connection alone brought.
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
        let now = Utc::now();
        let state = ctx.state().clone();
        let tx = ctx.tx().await?;
        let Some(identity) = mailbox_links::forget(tx, &event.payload.connection)
            .await
            .map_err(retry)?
        else {
            return Ok(());
        };
        let found: Option<(Uuid, String, String)> = sqlx::query_as(
            "SELECT account_id, linked_via, provider FROM identity.provider_identities
              WHERE id = $1",
        )
        .bind(identity)
        .fetch_optional(&mut **tx)
        .await?;
        let Some((account, linked_via, provider)) = found else {
            return Ok(());
        };
        if linked_via != "connect" {
            return Ok(());
        }
        // Locks the account: a removal on Ways to sign in can't race this
        // one into leaving none.
        if ways_in::lock_and_count(tx, account).await.map_err(retry)? <= 1 {
            tracing::info!("kept the last way in although its mailbox was disconnected");
            return Ok(());
        }
        let ended = sessions::end_signed_in_with(tx, identity, now)
            .await
            .map_err(retry)?;
        if accounts::remove(tx, identity, account)
            .await
            .map_err(retry)?
            .is_none()
        {
            return Ok(());
        }
        accounts::audit(
            tx,
            Some(account),
            "way_in_removed",
            serde_json::json!({
                "method": "provider", "provider": provider, "way_in": identity,
                "sessions_ended": ended, "reason": "connection_removed",
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
                    account,
                    method: "provider".to_owned(),
                    provider: Some(provider),
                },
            )
            .await
            .map_err(|e| JobError::retry(e.to_string()))?;
        tracing::info!(
            sessions_ended = ended,
            "a way in went with its last mailbox connection"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests;
