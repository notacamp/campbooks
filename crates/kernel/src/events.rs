//! Durable domain events (ADR 0012).
//!
//! Modules never call each other; they publish events and subscribe to the
//! ones they care about. Publishing writes the event to the retained log
//! `kernel.events` **in the caller's transaction**, together with one job per
//! registered consumer (fan-out at publish time). If the transaction rolls
//! back, neither the event nor its deliveries exist; once it commits, every
//! consumer runs it at least once, through the job queue, even across
//! restarts.
//!
//! ```ignore
//! // The publishing module declares the event…
//! #[derive(Serialize, Deserialize)]
//! pub struct MessageReceived { pub workspace: String, pub message: MessageId }
//! impl DomainEvent for MessageReceived {
//!     const NAME: &'static str = "mail.message_received";
//!     const VERSION: u16 = 1;
//!     fn workspace(&self) -> Option<&str> { Some(&self.workspace) }
//! }
//! // …and publishes it with its data change.
//! let mut tx = state.db.begin_for(workspace).await?;
//! /* insert the message */
//! state.events.publish(&mut tx, &MessageReceived { .. }).await?;
//! tx.commit().await?;
//!
//! // It declares the events it publishes, which lists them in `openapi/events.json`
//! // (payload schema from `utoipa::ToSchema`):
//! // Module { background: Background::new().publishes::<MessageReceived>(), … }
//!
//! // A consumer in another module handles it.
//! struct TrackSender;
//! impl EventHandler for TrackSender {
//!     type Event = MessageReceived;
//!     const NAME: &'static str = "parties.track_sender";
//!     const QUEUE: &'static str = "events";
//!     async fn handle(ctx: &mut JobContext, event: Event<MessageReceived>) -> Result<(), JobError> { … }
//! }
//! // Module { background: Background::new().on_event::<TrackSender>(), … }
//! ```

use std::{future::Future, sync::Arc};

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Serialize, de::DeserializeOwned};
use sqlx::{PgPool, Postgres, Transaction};
use utoipa::openapi::{RefOr, schema::Schema};

use crate::{
    changes::{self, Change},
    jobs::{self, JobContext, JobError, Registry},
};

crate::define_id!(
    /// A published domain event, in `kernel.events`.
    EventId
);

/// A fact a module announces to the rest of the system.
///
/// `NAME` is the stable, dotted wire name (`"mail.message_received"`) stored
/// in the event log; never rename it. `VERSION` is the payload's schema
/// version, stored with every event: bump it when the payload changes
/// incompatibly so handlers can tell old events (from a replay) from new ones.
pub trait DomainEvent: Serialize + DeserializeOwned + Send + Sync + 'static {
    /// Stable wire name, `<module>.<event>`.
    const NAME: &'static str;
    /// Payload schema version, starting at 1. Deliberately has no default.
    const VERSION: u16;
    /// Whether every event of this type belongs to a workspace (the default)
    /// or is instance-wide. Listed in `openapi/events.json`; keep
    /// [`workspace`](Self::workspace) consistent with it.
    const WORKSPACE_SCOPED: bool = true;
    /// Whether a transaction working in a workspace may publish an event of
    /// this type that belongs to no workspace ([`workspace`](Self::workspace)
    /// `None`; a workspace handler announcing an account's change).
    /// Meaningless for an event that always names its workspace. Row-level
    /// security refuses every other instance-wide event, and every delivery
    /// to a consumer of one, from a workspace transaction
    /// (campbooks-cloud#207): the event names and consumer kinds it admits are
    /// listed in `kernel.instance_rows_from_workspaces`, by migration, and the
    /// server's tests keep that list equal to these declarations. Because any
    /// workspace transaction may then append one, a consumer must not take
    /// such an event as proof of who published it.
    const PUBLISHED_IN_WORKSPACES: bool = false;
    /// Whether something waits for every consumer to have acted on an event
    /// of this type ([`EventBus::unsettled`]): a delivery that fails for
    /// good is then kept, with the event, until an operator replays or
    /// discards it, instead of being pruned after the failed-job retention.
    const SETTLES: bool = false;

    /// The workspace this event belongs to, or `None` for instance-wide
    /// events. Its deliveries run with that workspace set
    /// (`campbooks.workspace_id`) and count towards its fair share.
    fn workspace(&self) -> Option<&str>;

    /// The records this event's change touched, for the client change feed
    /// ([`changes`](crate::changes)): references and operations only, never
    /// contents. Recorded with the event when it belongs to a workspace.
    /// None by default.
    fn changes(&self) -> Vec<Change> {
        Vec::new()
    }
}

/// A published event as a handler receives it.
#[derive(Debug, Clone)]
pub struct Event<E> {
    /// The event's id in the log; stable across retries and replays, so a
    /// handler can use it as an idempotency key.
    pub id: EventId,
    /// The schema version it was published with (equal to `E::VERSION`).
    pub version: u16,
    /// The workspace it belongs to.
    pub workspace: Option<String>,
    /// When it was published.
    pub occurred_at: DateTime<Utc>,
    /// The event itself.
    pub payload: E,
}

/// A consumer of one event type, registered in a module's
/// [`Background`](crate::jobs::Background) with `.on_event::<H>()`.
///
/// Each delivery is a job of kind [`NAME`](Self::NAME): retried with backoff,
/// leased, fair across workspaces. Delivery is at least once; work done
/// through [`JobContext::tx`] commits atomically with the delivery being
/// marked done, so database-only handlers take effect exactly once.
pub trait EventHandler: Send + Sync + 'static {
    /// The event this handler consumes.
    type Event: DomainEvent;
    /// Stable consumer name, `<module>.<handler>`; the job kind of its
    /// deliveries and the name `campbooks-server events replay` takes.
    const NAME: &'static str;
    /// The queue its deliveries run on.
    const QUEUE: &'static str;
    /// Attempts before a delivery is marked failed (dead).
    const MAX_ATTEMPTS: u32 = jobs::DEFAULT_MAX_ATTEMPTS;
    /// Whether an operator must hear when a delivery dies
    /// ([`Watch`](jobs::Watch)).
    const WATCH: Option<jobs::Watch> = None;

    /// Handle one event.
    fn handle(
        ctx: &mut JobContext,
        event: Event<Self::Event>,
    ) -> impl Future<Output = Result<(), JobError>> + Send;
}

/// A consumer of one event type that owns its dependencies (an AI gateway,
/// settings): the module builds it from what it was composed with and
/// registers it with [`Background::consumer`](crate::jobs::Background::consumer).
/// Otherwise exactly an [`EventHandler`]: same naming, delivery and
/// transaction rules.
pub trait EventConsumer: Send + Sync + 'static {
    /// The event this consumer handles.
    type Event: DomainEvent;
    /// Stable consumer name, `<module>.<handler>`.
    const NAME: &'static str;
    /// The queue its deliveries run on.
    const QUEUE: &'static str;
    /// Attempts before a delivery is marked failed (dead).
    const MAX_ATTEMPTS: u32 = jobs::DEFAULT_MAX_ATTEMPTS;
    /// Whether an operator must hear when a delivery dies
    /// ([`Watch`](jobs::Watch)).
    const WATCH: Option<jobs::Watch> = None;

    /// Handle one event.
    fn handle(
        &self,
        ctx: &mut JobContext,
        event: Event<Self::Event>,
    ) -> impl Future<Output = Result<(), JobError>> + Send;
}

/// Where a replay starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayFrom {
    /// Every event in the log.
    Beginning,
    /// Events published at or after this instant.
    Time(DateTime<Utc>),
    /// This event and every later one (event ids are time-ordered, UUID version 7).
    Event(EventId),
}

/// The durable event bus: publishes events and their deliveries. Cheap to
/// clone; clones share the [`Registry`] of consumers.
#[derive(Clone, Default)]
pub struct EventBus {
    registry: Arc<Registry>,
}

impl std::fmt::Debug for EventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventBus")
            .field("registry", &self.registry)
            .finish()
    }
}

impl EventBus {
    /// A bus delivering to the consumers in `registry`.
    pub fn new(registry: Arc<Registry>) -> Self {
        Self { registry }
    }

    /// The consumers (and jobs and schedules) this bus knows.
    pub fn registry(&self) -> &Arc<Registry> {
        &self.registry
    }

    /// Append `event` to the log and enqueue one delivery per registered
    /// consumer, inside `tx`. Nothing is visible until `tx` commits, and
    /// nothing remains if it rolls back.
    ///
    /// # Errors
    /// [`sqlx::Error::InvalidArgument`] when the event belongs to a workspace
    /// and `tx` does not work in that one
    /// ([`begin_for`](crate::db::BeginFor::begin_for), a job's
    /// [`tx`](crate::JobContext::tx)): a change is announced in the workspace
    /// it was made in, never across, and never from a transaction that set
    /// none (row-level security on the event log refuses the first too).
    pub async fn publish<E: DomainEvent>(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        event: &E,
    ) -> Result<EventId, sqlx::Error> {
        check_workspace(tx, E::NAME, event.workspace()).await?;
        let payload = serde_json::to_value(event).map_err(|e| sqlx::Error::Encode(e.into()))?;
        let consumers = self.registry.consumers_of(E::NAME);
        let kinds: Vec<&str> = consumers.iter().map(|c| c.name).collect();
        let queues: Vec<&str> = consumers.iter().map(|c| c.queue).collect();
        let max_attempts: Vec<i32> = consumers
            .iter()
            .map(|c| jobs::attempts_to_db(c.max_attempts))
            .collect();

        // An instance-wide event (an account's) may be published from a
        // transaction that set no workspace: the kernel lifts it into its
        // scope for the append only (row-level security shows the event log
        // to no transaction that set nothing).
        // From a workspace's transaction only an event declared
        // `PUBLISHED_IN_WORKSPACES` (campbooks-cloud#207).
        let lifted = match event.workspace() {
            Some(_) => false,
            None => {
                crate::db::lift_for_instance_rows(
                    tx,
                    &format!("event `{}`", E::NAME),
                    E::PUBLISHED_IN_WORKSPACES,
                )
                .await?
            }
        };
        let id = sqlx::query_scalar!(
            r#"
            WITH event AS (
                INSERT INTO kernel.events (name, version, workspace_id, payload)
                VALUES ($1, $2, $3, $4)
                RETURNING id, workspace_id
            ), deliveries AS (
                INSERT INTO kernel.jobs
                    (queue, kind, payload, workspace_id, event_id, unique_key, max_attempts,
                     trace_context)
                SELECT c.queue, c.kind, '{}'::jsonb, event.workspace_id, event.id,
                       event.id::text, c.max_attempts, $8
                  FROM event,
                       unnest($5::text[], $6::text[], $7::int[]) AS c(kind, queue, max_attempts)
            )
            SELECT id AS "id!: EventId" FROM event
            "#,
            E::NAME,
            i32::from(E::VERSION),
            event.workspace(),
            payload,
            &kinds as &[&str],
            &queues as &[&str],
            &max_attempts,
            crate::telemetry::current_traceparent(),
        )
        .fetch_one(&mut **tx)
        .await?;
        crate::db::lower(tx, lifted).await?;

        let changes = event.changes();
        if let (false, Some(workspace)) = (changes.is_empty(), event.workspace()) {
            changes::record(tx, id, workspace, &changes).await?;
        }

        let mut notified: Vec<&str> = queues.clone();
        notified.sort_unstable();
        notified.dedup();
        for queue in notified {
            jobs::notify(tx, queue).await?;
        }
        tracing::debug!(
            event = E::NAME,
            event_id = %id,
            consumers = kinds.len(),
            "domain event published"
        );
        Ok(id)
    }

    /// Whether an event `name` of the transaction's workspace whose payload
    /// contains `matching` (`@>`) has a delivery that has not succeeded yet
    /// (pending, running, or failed for good): its consumers have not all
    /// acted on it. Row-level security limits it to `conn`'s workspace.
    ///
    /// # Errors
    /// The database's.
    pub async fn unsettled(
        conn: &mut sqlx::PgConnection,
        name: &str,
        matching: &serde_json::Value,
    ) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar!(
            r#"SELECT EXISTS (
                   SELECT 1 FROM kernel.events e
                     JOIN kernel.jobs j ON j.event_id = e.id
                    WHERE e.name = $1 AND e.payload @> $2 AND j.state <> 'succeeded'
               ) AS "unsettled!""#,
            name,
            matching,
        )
        .fetch_one(conn)
        .await
    }

    /// Re-enqueue deliveries of the logged events of `consumer`'s event type
    /// to that consumer, from `from` on. Events that already have an
    /// unfinished delivery to it are skipped. Returns how many were enqueued.
    pub async fn replay(
        &self,
        db: &PgPool,
        consumer: &str,
        from: ReplayFrom,
    ) -> Result<u64, ReplayError> {
        let info = self
            .registry
            .consumer(consumer)
            .ok_or_else(|| ReplayError::UnknownConsumer(consumer.to_owned()))?;
        let (since_time, since_id) = match from {
            ReplayFrom::Beginning => (None, None),
            ReplayFrom::Time(at) => (Some(at), None),
            ReplayFrom::Event(id) => (None, Some(id)),
        };
        // Across workspaces: every logged event of the type, whoever's.
        let mut tx = crate::db::begin_kernel(db).await?;
        let enqueued = sqlx::query!(
            r#"
            INSERT INTO kernel.jobs
                (queue, kind, payload, workspace_id, event_id, unique_key, max_attempts)
            SELECT $1, $2, '{}'::jsonb, e.workspace_id, e.id, e.id::text, $3
              FROM kernel.events e
             WHERE e.name = $4
               AND ($5::timestamptz IS NULL OR e.occurred_at >= $5)
               AND ($6::uuid IS NULL OR e.id >= $6)
             ORDER BY e.id
            ON CONFLICT (kind, unique_key)
                WHERE unique_key IS NOT NULL AND state IN ('pending', 'running')
                DO NOTHING
            "#,
            info.queue,
            info.name,
            jobs::attempts_to_db(info.max_attempts),
            info.event,
            since_time,
            since_id as Option<EventId>,
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();
        jobs::notify(&mut tx, info.queue).await?;
        tx.commit().await?;
        tracing::info!(consumer, enqueued, "events replayed");
        Ok(enqueued)
    }
}

/// Refuse to publish `event` (of workspace `workspace`) in a transaction set
/// to another workspace, or to none.
async fn check_workspace(
    tx: &mut Transaction<'_, Postgres>,
    event: &str,
    workspace: Option<&str>,
) -> Result<(), sqlx::Error> {
    let Some(workspace) = workspace else {
        return Ok(());
    };
    let current = sqlx::query_scalar!(
        "SELECT nullif(current_setting($1, true), '')",
        crate::db::WORKSPACE_SETTING
    )
    .fetch_one(&mut **tx)
    .await?;
    match current {
        Some(current) if current == workspace => Ok(()),
        Some(_) => Err(sqlx::Error::InvalidArgument(format!(
            "event `{event}` belongs to another workspace than the transaction publishing it"
        ))),
        None => Err(sqlx::Error::InvalidArgument(format!(
            "event `{event}` belongs to a workspace; publish it in a transaction working \
             there (`begin_for`, a job's `tx()`)"
        ))),
    }
}

/// A replay could not run.
#[derive(Debug, thiserror::Error)]
pub enum ReplayError {
    /// No module registers a consumer by that name.
    #[error("no event handler is registered as `{0}`")]
    UnknownConsumer(String),
    /// The database refused.
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// One event type a module publishes, as declared with
/// [`Background::publishes`](crate::jobs::Background::publishes).
#[derive(Debug, Clone, Copy)]
pub struct PublishedEvent {
    /// The declaring module.
    pub module: &'static str,
    /// `DomainEvent::NAME`.
    pub name: &'static str,
    /// `DomainEvent::VERSION`.
    pub version: u16,
    /// `DomainEvent::WORKSPACE_SCOPED`.
    pub workspace_scoped: bool,
    /// `DomainEvent::PUBLISHED_IN_WORKSPACES`.
    pub published_in_workspaces: bool,
    pub(crate) schema: fn() -> RefOr<Schema>,
    pub(crate) schemas: fn(&mut Vec<(String, RefOr<Schema>)>),
}

/// The generated catalogue of events, committed as `openapi/events.json`:
/// what rules can trigger on and projections can read.
#[derive(Clone, Serialize)]
pub struct EventCatalogue {
    /// What the document is.
    pub description: &'static str,
    /// The dialect payload schemas are written in.
    #[serde(rename = "jsonSchemaDialect")]
    pub json_schema_dialect: &'static str,
    /// Every published event, by name.
    pub events: Vec<EventEntry>,
    /// Schemas the payloads reference (`#/components/schemas/<name>`).
    pub components: EventComponents,
}

/// One event of the [`EventCatalogue`].
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventEntry {
    /// Stable wire name, `<module>.<event>`.
    pub name: &'static str,
    /// The publishing module.
    pub module: &'static str,
    /// Payload schema version.
    pub version: u16,
    /// Whether each event belongs to a workspace.
    pub workspace_scoped: bool,
    /// The payload's JSON Schema.
    pub payload: RefOr<Schema>,
}

/// Shared schemas of the [`EventCatalogue`].
#[derive(Clone, Default, Serialize)]
pub struct EventComponents {
    /// By name.
    pub schemas: BTreeMap<String, RefOr<Schema>>,
}

impl EventCatalogue {
    /// The catalogue of every event `registry` knows to be published.
    pub fn new(registry: &Registry) -> Self {
        let mut events: Vec<EventEntry> = registry
            .published()
            .iter()
            .map(|event| EventEntry {
                name: event.name,
                module: event.module,
                version: event.version,
                workspace_scoped: event.workspace_scoped,
                payload: (event.schema)(),
            })
            .collect();
        events.sort_by_key(|event| event.name);
        let mut referenced = Vec::new();
        for event in registry.published() {
            (event.schemas)(&mut referenced);
        }
        Self {
            description: "Domain events Campbooks publishes (ADR 0012, ADR 0013): the triggers \
                automation rules listen to and the inputs projections read. Generated from \
                the modules' DomainEvent types; do not edit.",
            json_schema_dialect: "https://spec.openapis.org/oas/3.1/dialect/base",
            events,
            components: EventComponents {
                schemas: referenced.into_iter().collect(),
            },
        }
    }
}
