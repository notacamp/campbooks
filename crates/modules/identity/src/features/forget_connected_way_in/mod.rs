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
//!
//! The event alone is no proof that the connection went: any transaction
//! working in the workspace could write it (campbooks-cloud#211). Before
//! acting, the handler asks the connectors module (the grant-intake port)
//! for the account's connection of that provider account there; while that
//! is still the named connection, nothing happens. A port that cannot
//! answer is retried, never guessed at.

use campbooks_kernel::{
    AppState, Event, EventConsumer, JobContext, JobError, Result, tenancy::WorkspaceId,
};
use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::{
    domain::{
        accounts,
        mailbox_links::{self, Removed},
        tracked,
    },
    events::{ConnectionRemoved, WayInRemoved},
    tracking::{self, WayInRemoval},
};

/// The plan's provider of a removed way in.
fn provider_of(gone: &Removed) -> Result<tracking::Provider> {
    tracked::provider(&gone.provider)
        .ok_or_else(|| campbooks_kernel::Error::internal("a way in names an unknown provider"))
}

/// Track that `gone` went, for `cause`, in `conn` (an instance
/// transaction: the prune's, or a round trip's take-back).
pub(crate) async fn track_removed(
    state: &AppState,
    conn: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    gone: &Removed,
    cause: WayInRemoval,
) -> Result<()> {
    state
        .tracker
        .track(
            conn,
            &tracking::WayInRemoved {
                way_in_id: gone.way_in,
                account_id: gone.account,
                provider: provider_of(gone)?,
                cause,
                sessions_ended: i64::try_from(gone.sessions_ended).unwrap_or(i64::MAX),
            },
        )
        .await?;
    Ok(())
}

/// Audit way in `gone`'s removal (`reason`) and publish
/// `identity.way_in_removed`, in the transaction that removed it.
pub(crate) async fn announce(
    state: &AppState,
    conn: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    gone: &Removed,
    reason: &str,
    now: DateTime<Utc>,
) -> Result<()> {
    let db: &mut PgConnection = conn;
    accounts::audit(
        db,
        Some(gone.account),
        "way_in_removed",
        serde_json::json!({
            "method": "provider", "provider": gone.provider, "way_in": gone.way_in,
            "sessions_ended": gone.sessions_ended, "reason": reason,
        }),
        now,
    )
    .await?;
    state
        .events
        .publish(
            conn,
            &WayInRemoved {
                account: gone.account,
                method: "provider".to_owned(),
                provider: Some(gone.provider.clone()),
            },
        )
        .await?;
    Ok(())
}

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
        let state = ctx.state().clone();
        let account = {
            let mut conn = ctx.pool().acquire().await?;
            mailbox_links::account_naming(
                &mut conn,
                workspace,
                &removed.connection,
                &provider,
                &subject,
            )
            .await
            .map_err(retry)?
        };
        let Some(account) = account else {
            return Ok(());
        };
        let live = state
            .grants
            .connection_of(
                &state,
                &account.to_string(),
                WorkspaceId::from_uuid(workspace),
                &provider,
                &subject,
            )
            .await
            .map_err(retry)?;
        if live.as_deref() == Some(removed.connection.as_str()) {
            tracing::warn!(
                "connection_removed names a connection that is still connected; not acted on"
            );
            return Ok(());
        }
        let now = Utc::now();
        let tx = ctx.tx().await?;
        let Some(gone) =
            mailbox_links::forget(tx, workspace, &removed.connection, &provider, &subject, now)
                .await
                .map_err(retry)?
        else {
            return Ok(());
        };
        announce(&state, tx, &gone, "connection_removed", now)
            .await
            .map_err(retry)?;
        // The delivery works in the connection's workspace.
        let removed = tracking::ConnectedWayInRemoved {
            workspace: WorkspaceId::from_uuid(workspace),
            way_in_id: gone.way_in,
            account_id: gone.account,
            provider: provider_of(&gone).map_err(retry)?,
            sessions_ended: i64::try_from(gone.sessions_ended).unwrap_or(i64::MAX),
        };
        state.tracker.track(tx, &removed).await?;
        tracing::info!(
            sessions_ended = gone.sessions_ended,
            "a way in went with its last mailbox"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests;
