//! [`Tracker::track`]: one tracked event into the outbox, in the caller's
//! transaction.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{Postgres, Transaction};

use super::{
    TrackedEvent,
    context::{self, Actor, Causation},
};
use crate::config::{TrackingConfig, TrackingSink};

/// Writes tracked events. Cheap to clone; one lives in
/// [`AppState`](crate::AppState) (`state.tracker`).
#[derive(Debug, Clone)]
pub struct Tracker {
    sink: TrackingSink,
    role: &'static str,
}

impl Default for Tracker {
    fn default() -> Self {
        Self {
            sink: TrackingSink::None,
            role: DEFAULT_ROLE,
        }
    }
}

/// The server role written into envelopes until the server names its own.
const DEFAULT_ROLE: &str = "all";

impl Tracker {
    /// A tracker for `config` (`CAMPBOOKS_TRACKING__SINK`).
    pub fn new(config: &TrackingConfig) -> Self {
        Self {
            sink: config.sink,
            role: DEFAULT_ROLE,
        }
    }

    /// The same tracker, writing `role` (`all`, `api`, `worker`, `cli`) into
    /// every envelope.
    #[must_use]
    pub fn with_role(mut self, role: &'static str) -> Self {
        self.role = role;
        self
    }

    /// Where events go.
    pub fn sink(&self) -> TrackingSink {
        self.sink
    }

    /// Track `event` in `tx`: it is written to `kernel.tracked_events` with
    /// the envelope the kernel builds from the ambient
    /// [`TrackingContext`](super::TrackingContext), and exists if and only
    /// if `tx` commits. With the sink `none` it returns at once, touching
    /// nothing.
    ///
    /// # Errors
    /// [`sqlx::Error::InvalidArgument`] when the event belongs to a workspace
    /// `tx` does not work in (another, or none), or is instance-wide and `tx`
    /// works in a workspace; the database's.
    pub async fn track<E: TrackedEvent>(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        event: &E,
    ) -> Result<(), sqlx::Error> {
        if self.sink == TrackingSink::None {
            return Ok(());
        }
        let workspace = event.workspace();
        let workspace_text = workspace.as_ref().map(ToString::to_string);
        crate::events::check_workspace(tx, E::NAME, workspace_text.as_deref()).await?;
        let context = context::current_or_outside();
        let id = uuid::Uuid::now_v7();
        let occurred_at = Utc::now();
        let envelope = Envelope {
            id,
            name: E::NAME,
            version: E::VERSION,
            occurred_at,
            workspace_id: workspace_text.as_deref(),
            actor: &context.actor,
            on_behalf_of: context.on_behalf_of.as_ref(),
            operation: context.operation.as_deref(),
            correlation: &context.correlation,
            causation: context.causation.as_ref(),
            role: self.role,
            app_version: env!("CARGO_PKG_VERSION"),
            plan_version: super::PLAN_VERSION,
        };
        let envelope =
            serde_json::to_value(&envelope).map_err(|e| sqlx::Error::Encode(e.into()))?;
        let body = serde_json::to_value(event).map_err(|e| sqlx::Error::Encode(e.into()))?;

        // An instance-wide event from a transaction that set nothing is
        // written in the kernel scope for this one insert, as instance-wide
        // domain events are; a workspace transaction may not add one.
        let lifted = match workspace {
            Some(_) => false,
            None => {
                crate::db::lift_for_instance_rows(
                    tx,
                    &format!("tracked event `{}`", E::NAME),
                    false,
                )
                .await?
            }
        };
        // Nothing is read back: the runtime role may only append.
        sqlx::query(
            "INSERT INTO kernel.tracked_events
                 (id, workspace_id, name, version, occurred_at, envelope, body)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(id)
        .bind(workspace.map(|w| *w.as_uuid()))
        .bind(E::NAME)
        .bind(i32::from(E::VERSION))
        .bind(occurred_at)
        .bind(envelope)
        .bind(body)
        .execute(&mut **tx)
        .await?;
        crate::db::lower(tx, lifted).await?;
        tracing::debug!(event = E::NAME, "tracked");
        Ok(())
    }
}

/// What the kernel writes around every body.
#[derive(Serialize)]
struct Envelope<'a> {
    id: uuid::Uuid,
    name: &'static str,
    version: u16,
    occurred_at: DateTime<Utc>,
    workspace_id: Option<&'a str>,
    actor: &'a Actor,
    on_behalf_of: Option<&'a Actor>,
    operation: Option<&'a str>,
    correlation: &'a str,
    causation: Option<&'a Causation>,
    role: &'static str,
    app_version: &'static str,
    plan_version: u32,
}
