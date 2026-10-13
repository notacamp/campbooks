//! `connectors.request_changes` ← `conversations.changes_requested`: a
//! member's changes are recorded as outbound changes of the inbox's stream,
//! stamped with Gmail's history id (where they sit in Gmail's order of
//! writes), and the stream runs now to make them
//! (`contracts/conversations.changes_requested.v1.schema.json`).

use std::collections::HashSet;

use campbooks_kernel::{
    Event, EventConsumer, JobContext, JobError, db::BeginFor as _, tenancy::WorkspaceId,
};
use uuid::Uuid;

use crate::{
    deps::Deps,
    domain::{
        credentials,
        model::{ConnectionId, Health, InboxId, Provider, StreamId},
        outbound::{self, Mapped, State},
        schedule,
    },
    events::ChangesRequested,
};

/// Records a member's changes for the connector to make.
#[derive(Debug, Clone)]
pub struct RequestChanges {
    deps: Deps,
}

impl RequestChanges {
    /// The consumer, composed with the module's dependencies.
    pub fn new(deps: Deps) -> Self {
        Self { deps }
    }
}

/// The stream that feeds an inbox.
struct Target {
    stream: StreamId,
    connection: ConnectionId,
    provider: Provider,
    paused: bool,
    health: Health,
}

impl EventConsumer for RequestChanges {
    type Event = ChangesRequested;
    const NAME: &'static str = "connectors.request_changes";
    const QUEUE: &'static str = "events";

    async fn handle(
        &self,
        ctx: &mut JobContext,
        event: Event<ChangesRequested>,
    ) -> Result<(), JobError> {
        let workspace: WorkspaceId = event
            .payload
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("changes_requested without a workspace id"))?;
        let now = self.deps.clock.now();
        let mut tx = ctx.pool().begin_for(workspace).await?;
        let target = sqlx::query!(
            r#"SELECT s.id AS "stream: StreamId", s.connection_id AS "connection: ConnectionId",
                      c.provider, s.paused, c.health
                 FROM connectors.streams s
                 JOIN connectors.connections c ON c.id = s.connection_id
                WHERE s.inbox_id = $1 AND s.capability = 'messages'"#,
            event.payload.inbox_id as InboxId,
        )
        .fetch_optional(&mut *tx)
        .await?;
        let Some(row) = target else {
            // Not a connector's inbox, or disconnected meanwhile.
            tx.commit().await?;
            return Ok(());
        };
        let target = Target {
            stream: row.stream,
            connection: row.connection,
            provider: Provider::parse(&row.provider)?,
            paused: row.paused,
            health: Health::parse(&row.health)?,
        };
        let known: HashSet<String> = sqlx::query_scalar!(
            "SELECT label_id FROM connectors.gmail_labels WHERE stream_id = $1 AND kind = 'user'",
            target.stream as StreamId
        )
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .collect();
        tx.commit().await?;

        let stamp = self.stamp(ctx, workspace, &target).await;
        let tx = ctx.tx().await?;
        // Serialise with the stream's run, which checks for changes recorded
        // while it ran under the same lock.
        sqlx::query!(
            "SELECT id FROM connectors.streams WHERE id = $1 FOR UPDATE",
            target.stream as StreamId
        )
        .fetch_one(&mut **tx)
        .await?;
        let mut pending = 0;
        for item in &event.payload.items {
            let (state, reason, add, remove) = match outbound::map(&item.change, &known) {
                // Defence in depth: only the flag labels, categories and
                // user labels are ever written.
                Mapped::Labels { add, remove }
                    if !add.iter().chain(&remove).all(|l| outbound::writable(l)) =>
                {
                    (State::Skipped, Some("unsupported"), Vec::new(), Vec::new())
                }
                Mapped::Labels { add, remove } => (State::Pending, None, add, remove),
                Mapped::Skip(reason) => (State::Skipped, Some(reason), Vec::new(), Vec::new()),
            };
            let inserted = sqlx::query!(
                r#"INSERT INTO connectors.outbound_changes
                       (id, workspace_id, stream_id, request_id, provider_message_id, kind,
                        add_labels, remove_labels, stamp_history_id, state, reason, created_at,
                        done_at)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
                   ON CONFLICT (request_id, provider_message_id) DO NOTHING"#,
                Uuid::now_v7(),
                workspace as WorkspaceId,
                target.stream as StreamId,
                *event.id.as_uuid(),
                item.provider_message_id,
                outbound::kind(&item.change),
                &add,
                &remove,
                stamp,
                state.as_str(),
                reason,
                now,
                (state != State::Pending).then_some(now),
            )
            .execute(&mut **tx)
            .await?
            .rows_affected();
            if inserted > 0 && state == State::Pending {
                pending += 1;
            }
        }
        if pending > 0 && !target.paused {
            schedule::run_now(tx, workspace, target.stream, now).await?;
        }
        tracing::info!(%workspace, stream = %target.stream, pending, "outbound changes recorded");
        Ok(())
    }
}

impl RequestChanges {
    /// Gmail's history id now (1 quota unit), or `None` when Gmail cannot
    /// be asked: the change then applies in arrival order.
    async fn stamp(
        &self,
        ctx: &JobContext,
        workspace: WorkspaceId,
        target: &Target,
    ) -> Option<i64> {
        let google = self.deps.google.as_ref()?;
        if target.health == Health::NeedsReauth {
            return None;
        }
        let rate = self.deps.config.google.units_per_second;
        if !matches!(
            schedule::take_budget(ctx.pool(), target.provider, 1, rate).await,
            Ok(Ok(()))
        ) {
            return None;
        }
        let token = credentials::access_token(ctx.pool(), &self.deps, workspace, target.connection)
            .await
            .ok()?;
        match google.gmail(&token).profile().await {
            Ok(profile) => profile.history_id.parse().ok(),
            Err(err) => {
                tracing::info!(stream = %target.stream, error = %err, "no history stamp for outbound changes");
                None
            }
        }
    }
}

#[cfg(test)]
mod tests;
