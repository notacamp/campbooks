//! The relay (campbooks-cloud#321): publishes the outbox
//! `kernel.tracked_events` to RabbitMQ, with publisher confirms.
//!
//! One relay runs per worker process (`CAMPBOOKS_TRACKING__SINK=amqp`), and
//! several may run at once. Each batch is one transaction in the kernel
//! scope:
//!
//! 1. claim up to `BATCH_SIZE` unpublished rows, oldest id first, locked
//!    `FOR UPDATE SKIP LOCKED` (`kernel.claim_tracked_events`), so another
//!    relay skips them;
//! 2. publish each as one message (`mandatory`, so a missing binding comes
//!    back as a return instead of being dropped) and wait for every confirm,
//!    at most [`CONFIRM_TIMEOUT`];
//! 3. mark them published (`kernel.mark_tracked_events_published`) and
//!    commit. A nack, a return, a broker error or the timeout rolls the
//!    batch back: its rows go out with a later batch. A crash between the
//!    confirms and the commit publishes them twice; subscribers deduplicate
//!    on the event id (`message_id`).
//!
//! A full batch is followed by the next at once; otherwise the relay waits
//! for `NOTIFY kernel_tracked_events` (sent by the outbox's insert trigger)
//! or `POLL_INTERVAL_MS`. While the broker cannot be reached it reconnects
//! with exponential backoff (1 s up to 60 s, with jitter), logging the kind
//! of failure only: never the AMQP URL, its host or its credentials.
//!
//! The message: exchange `EXCHANGE`, routing key `<name>.v<version>`, a JSON
//! body holding the envelope's fields at the top level and the event's body
//! under `body`; `message_id` the event id, `type` its name, `content_type`
//! `application/json`, `timestamp` when it happened, persistent, and header
//! `x-event-version`.

use std::{
    future::Future,
    hash::{BuildHasher as _, Hasher as _},
    sync::Arc,
    time::Duration,
};

use chrono::{DateTime, Utc};
use lapin::{
    BasicProperties, Channel, Confirmation, Connection, ConnectionProperties, ExchangeKind,
    options::{
        BasicPublishOptions, ConfirmSelectOptions, ExchangeDeclareOptions, QueueBindOptions,
        QueueDeclareOptions,
    },
    types::{AMQPValue, FieldTable, ShortString},
};
use serde_json::Value;
use sqlx::{PgPool, postgres::PgListener};
use tokio::{sync::Notify, time::Instant};

use crate::{
    AppState,
    config::{TrackingConfig, TrackingSink},
    db, redact,
    telemetry::{Metrics, PublishOutcome},
};

/// The channel the outbox's insert trigger notifies.
pub const NOTIFY_CHANNEL: &str = "kernel_tracked_events";

/// Longest wait for the broker to confirm a batch; then it is rolled back.
pub const CONFIRM_TIMEOUT: Duration = Duration::from_secs(30);

/// The first pause after the broker failed.
const BACKOFF_FIRST: Duration = Duration::from_secs(1);
/// The longest pause between two attempts.
const BACKOFF_MAX: Duration = Duration::from_mins(1);
/// Pause before listening again after the notification connection failed.
const LISTEN_RETRY: Duration = Duration::from_secs(5);
/// How often the backlog is sampled while metrics are exported.
const SAMPLE_EVERY: Duration = Duration::from_secs(15);
/// Longest wait for the broker to close the connection at shutdown.
const CLOSE_TIMEOUT: Duration = Duration::from_secs(5);
/// What the relay calls itself in the broker's connection list.
const CONNECTION_NAME: &str = "campbooks-tracking-relay";
/// AMQP's persistent delivery mode.
const PERSISTENT: u8 = 2;

/// Why a batch did not go out. Every variant names the kind of failure
/// only: never the URL, a host, a credential or an event's content.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RelayError {
    /// The broker could not be reached, or failed mid-batch: the relay
    /// reconnects.
    #[error("the broker failed ({0})")]
    Broker(&'static str),
    /// The broker refused the batch (a nack, or a message returned as
    /// unroutable): the relay keeps its connection and tries again later.
    #[error("the broker refused the batch ({0})")]
    Refused(&'static str),
    /// The outbox could not be read or marked: a database error by kind
    /// ([`redact::sqlx`]).
    #[error("the outbox could not be read or marked ({0})")]
    Database(String),
    /// A row could not be turned into a message (a name or id too long for
    /// AMQP, a body that does not serialise).
    #[error("an event could not be encoded")]
    Encode,
}

impl RelayError {
    /// The `campbooks.tracking.published` outcome of the batch it failed.
    pub fn outcome(&self) -> PublishOutcome {
        match self {
            Self::Refused(_) => PublishOutcome::Nacked,
            Self::Broker(_) | Self::Database(_) | Self::Encode => PublishOutcome::Error,
        }
    }

    fn database(error: &sqlx::Error) -> Self {
        Self::Database(redact::sqlx(error))
    }
}

/// What one batch did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Batch {
    /// Events published, confirmed and marked.
    pub published: u64,
    /// Whether the batch was full, so more may be waiting.
    pub full: bool,
}

/// A connection to the broker with a confirming channel.
pub struct Publisher {
    connection: Connection,
    channel: Channel,
    exchange: ShortString,
}

impl std::fmt::Debug for Publisher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Publisher")
            .field("exchange", &self.exchange.as_str())
            .finish_non_exhaustive()
    }
}

impl Publisher {
    /// Connect to `config`'s broker, declare the topology when
    /// `DECLARE_TOPOLOGY` is on, and turn publisher confirms on.
    pub async fn connect(config: &TrackingConfig) -> Result<Self, RelayError> {
        let url = config.amqp_url().ok_or(RelayError::Broker("no_url"))?;
        let exchange =
            ShortString::try_new(config.exchange.as_str()).map_err(|_| RelayError::Encode)?;
        let properties =
            ConnectionProperties::default().with_connection_name(CONNECTION_NAME.into());
        let connection = Connection::connect(url, properties).await.map_err(|e| {
            RelayError::Broker(if e.is_amqp_error() {
                "connect_refused"
            } else {
                "connect"
            })
        })?;
        let channel = connection
            .create_channel()
            .await
            .map_err(|_| RelayError::Broker("open_channel"))?;
        if config.declare_topology {
            declare_topology(&channel, config).await?;
        }
        channel
            .confirm_select(ConfirmSelectOptions::default())
            .await
            .map_err(|_| RelayError::Broker("enable_confirms"))?;
        Ok(Self {
            connection,
            channel,
            exchange,
        })
    }

    /// Close the connection, waiting at most a few seconds.
    pub async fn close(self) {
        let closing = self.connection.close(200, "OK".into());
        if tokio::time::timeout(CLOSE_TIMEOUT, closing).await.is_err() {
            tracing::debug!("the broker did not acknowledge the relay's close in time");
        }
    }

    /// Publish `rows` and wait for every confirm: `Ok` only when the broker
    /// acknowledged each one without returning it.
    async fn publish_all(&self, rows: &[OutboxRow]) -> Result<(), RelayError> {
        let mut confirms = Vec::with_capacity(rows.len());
        for row in rows {
            let message = Message::of(row)?;
            let confirm = self
                .channel
                .basic_publish(
                    self.exchange.clone(),
                    message.routing_key,
                    BasicPublishOptions {
                        mandatory: true,
                        immediate: false,
                    },
                    &message.payload,
                    message.properties,
                )
                .await
                .map_err(|_| RelayError::Broker("publish"))?;
            confirms.push(confirm);
        }
        let waited = tokio::time::timeout(CONFIRM_TIMEOUT, async {
            // Every confirm is awaited, so none is left pending on the
            // channel; the first refusal is what the batch reports.
            let mut outcome = Ok(());
            for confirm in confirms {
                let refused = match confirm.await.map_err(|_| RelayError::Broker("confirm"))? {
                    Confirmation::Ack(None) => None,
                    Confirmation::Ack(Some(_)) => Some(RelayError::Refused("unroutable")),
                    Confirmation::Nack(_) => Some(RelayError::Refused("nacked")),
                    Confirmation::NotRequested => Some(RelayError::Broker("unconfirmed")),
                };
                if let Some(refused) = refused
                    && outcome.is_ok()
                {
                    outcome = Err(refused);
                }
            }
            outcome
        })
        .await;
        waited.unwrap_or(Err(RelayError::Broker("confirm_timeout")))
    }
}

/// Declare the topic exchange `EXCHANGE`, the stream `STREAM` (kept for
/// `STREAM_MAX_AGE`) and their binding on every routing key, all durable.
/// Idempotent: declaring what exists, as it exists, changes nothing.
pub(crate) async fn declare_topology(
    channel: &Channel,
    config: &TrackingConfig,
) -> Result<(), RelayError> {
    let exchange =
        ShortString::try_new(config.exchange.as_str()).map_err(|_| RelayError::Encode)?;
    let stream = ShortString::try_new(config.stream.as_str()).map_err(|_| RelayError::Encode)?;
    channel
        .exchange_declare(
            exchange.clone(),
            ExchangeKind::Topic,
            ExchangeDeclareOptions {
                durable: true,
                ..ExchangeDeclareOptions::default()
            },
            FieldTable::default(),
        )
        .await
        .map_err(|_| RelayError::Broker("declare_exchange"))?;
    let mut arguments = FieldTable::default();
    arguments.insert(
        "x-queue-type".into(),
        AMQPValue::LongString("stream".into()),
    );
    arguments.insert(
        "x-max-age".into(),
        AMQPValue::LongString(config.stream_max_age.as_str().into()),
    );
    channel
        .queue_declare(
            stream.clone(),
            QueueDeclareOptions {
                durable: true,
                ..QueueDeclareOptions::default()
            },
            arguments,
        )
        .await
        .map_err(|_| RelayError::Broker("declare_stream"))?;
    channel
        .queue_bind(
            stream,
            exchange,
            "#".into(),
            QueueBindOptions::default(),
            FieldTable::default(),
        )
        .await
        .map_err(|_| RelayError::Broker("bind_stream"))
}

/// One row of the outbox, as the claim returns it.
#[derive(Debug, sqlx::FromRow)]
struct OutboxRow {
    id: uuid::Uuid,
    name: String,
    version: i32,
    occurred_at: DateTime<Utc>,
    envelope: Value,
    body: Value,
}

/// A row as an AMQP message.
struct Message {
    routing_key: ShortString,
    payload: Vec<u8>,
    properties: BasicProperties,
}

impl Message {
    fn of(row: &OutboxRow) -> Result<Self, RelayError> {
        let mut fields = row.envelope.as_object().cloned().unwrap_or_default();
        fields.insert("body".to_owned(), row.body.clone());
        let payload = serde_json::to_vec(&Value::Object(fields)).map_err(|_| RelayError::Encode)?;
        let routing_key = ShortString::try_new(format!("{}.v{}", row.name, row.version))
            .map_err(|_| RelayError::Encode)?;
        let message_id =
            ShortString::try_new(row.id.to_string()).map_err(|_| RelayError::Encode)?;
        let kind = ShortString::try_new(row.name.as_str()).map_err(|_| RelayError::Encode)?;
        let mut headers = FieldTable::default();
        headers.insert("x-event-version".into(), AMQPValue::LongInt(row.version));
        let properties = BasicProperties::default()
            .with_message_id(message_id)
            .with_type(kind)
            .with_content_type("application/json".into())
            .with_timestamp(u64::try_from(row.occurred_at.timestamp()).unwrap_or(0))
            .with_delivery_mode(PERSISTENT)
            .with_headers(headers);
        Ok(Self {
            routing_key,
            payload,
            properties,
        })
    }
}

/// Publishes the outbox to the broker; see the [module](self).
#[derive(Debug, Clone)]
pub struct Relay {
    db: PgPool,
    config: TrackingConfig,
    metrics: Metrics,
    grace: Duration,
}

impl Relay {
    /// The relay of `state`'s process when the sink is `amqp` and a URL is
    /// set; `None` otherwise (`none` and `outbox` publish nothing).
    pub fn from_state(state: &AppState) -> Option<Self> {
        let config = &state.config.tracking;
        (config.sink == TrackingSink::Amqp && config.amqp_url().is_some())
            .then(|| Self::new(state, config.clone()))
    }

    /// A relay over `state`'s database and metrics with `config`, stopping
    /// within the worker's grace period (`CAMPBOOKS_JOBS__SHUTDOWN_GRACE_SECONDS`).
    pub fn new(state: &AppState, config: TrackingConfig) -> Self {
        Self {
            db: state.db.clone(),
            config,
            metrics: state.metrics.clone(),
            grace: Duration::from_secs(state.config.jobs.shutdown_grace_seconds),
        }
    }

    /// Connect to the broker as this relay does.
    pub async fn connect(&self) -> Result<Publisher, RelayError> {
        Publisher::connect(&self.config).await
    }

    /// Claim, publish, confirm and mark one batch, in one transaction; see
    /// the [module](self). Nothing waiting is `Ok` with nothing published.
    pub async fn relay_batch(&self, publisher: &Publisher) -> Result<Batch, RelayError> {
        let size = self.config.batch_size;
        let mut tx = db::begin_kernel(&self.db)
            .await
            .map_err(|e| RelayError::database(&e))?;
        let rows = match sqlx::query_as::<_, OutboxRow>(
            "SELECT id, name, version, occurred_at, envelope, body
               FROM kernel.claim_tracked_events($1)",
        )
        .bind(i32::try_from(size).unwrap_or(i32::MAX))
        .fetch_all(&mut *tx)
        .await
        {
            Ok(rows) => rows,
            Err(e) => {
                let _ = tx.rollback().await;
                return Err(RelayError::database(&e));
            }
        };
        if rows.is_empty() {
            let _ = tx.rollback().await;
            return Ok(Batch::default());
        }
        let count = u64::try_from(rows.len()).unwrap_or(u64::MAX);
        if let Err(e) = publisher.publish_all(&rows).await {
            let _ = tx.rollback().await;
            self.metrics.tracking_published(e.outcome(), count);
            return Err(e);
        }
        let ids: Vec<uuid::Uuid> = rows.iter().map(|row| row.id).collect();
        let marked = async {
            sqlx::query("SELECT kernel.mark_tracked_events_published($1)")
                .bind(&ids[..])
                .execute(&mut *tx)
                .await?;
            tx.commit().await
        }
        .await;
        if let Err(e) = marked {
            // Confirmed but not marked: published again by a later batch.
            self.metrics
                .tracking_published(PublishOutcome::Error, count);
            return Err(RelayError::database(&e));
        }
        self.metrics
            .tracking_published(PublishOutcome::Confirmed, count);
        Ok(Batch {
            published: count,
            full: count >= u64::from(size),
        })
    }

    /// Run until `shutdown` resolves: publish batches as events arrive,
    /// reconnecting with backoff while the broker fails. On shutdown a batch
    /// in flight gets the grace period to finish; past it, it is rolled
    /// back and goes out later.
    pub async fn run(self, shutdown: impl Future<Output = ()> + Send) {
        let wake = Arc::new(Notify::new());
        let listener = tokio::spawn(listen(self.db.clone(), Arc::clone(&wake)));
        tracing::info!(
            exchange = %self.config.exchange,
            declare_topology = self.config.declare_topology,
            batch_size = self.config.batch_size,
            "tracking relay started"
        );
        let poll = Duration::from_millis(self.config.poll_interval_ms);
        let mut publisher: Option<Publisher> = None;
        let mut failures = 0_u32;
        let mut next_sample = Instant::now();
        tokio::pin!(shutdown);

        loop {
            if self.metrics.exported() && Instant::now() >= next_sample {
                self.sample().await;
                next_sample = Instant::now() + SAMPLE_EVERY;
            }
            let next = match publisher.take() {
                None => match self.connect().await {
                    Ok(connected) => {
                        if failures > 0 {
                            tracing::info!("tracking relay reached the broker again");
                        }
                        publisher = Some(connected);
                        Next::Again
                    }
                    Err(error) => {
                        failures = failures.saturating_add(1);
                        let pause = backoff(failures);
                        tracing::warn!(
                            kind = %error,
                            retry_in_ms = millis(pause),
                            "tracking relay cannot reach the broker; events wait in the outbox"
                        );
                        Next::Pause(pause)
                    }
                },
                Some(current) => {
                    let Some((result, stopping)) =
                        self.batch_or_stop(&current, shutdown.as_mut()).await
                    else {
                        publisher = Some(current);
                        break;
                    };
                    let (next, keep) = self.settle(result, stopping, &mut failures);
                    if keep {
                        publisher = Some(current);
                    } else {
                        current.close().await;
                    }
                    next
                }
            };
            match next {
                Next::Again => {}
                Next::Stop => break,
                Next::Pause(pause) => tokio::select! {
                    biased;
                    () = &mut shutdown => break,
                    () = tokio::time::sleep(pause) => {}
                },
                Next::Idle => tokio::select! {
                    biased;
                    () = &mut shutdown => break,
                    () = wake.notified() => {}
                    () = tokio::time::sleep(poll) => {}
                },
            }
        }

        listener.abort();
        if let Some(publisher) = publisher {
            publisher.close().await;
        }
        tracing::info!("tracking relay stopped");
    }

    /// One batch, or, when `shutdown` resolves first, the batch given the
    /// grace period to finish: its result and whether the relay is
    /// stopping. `None` when the grace period ran out (the batch is rolled
    /// back).
    async fn batch_or_stop<S: Future<Output = ()>>(
        &self,
        publisher: &Publisher,
        mut shutdown: std::pin::Pin<&mut S>,
    ) -> Option<(Result<Batch, RelayError>, bool)> {
        let batch = self.relay_batch(publisher);
        tokio::pin!(batch);
        tokio::select! {
            biased;
            result = &mut batch => Some((result, false)),
            () = &mut shutdown => {
                tracing::info!(
                    grace_seconds = self.grace.as_secs(),
                    "tracking relay stopping: finishing its batch"
                );
                if let Ok(result) = tokio::time::timeout(self.grace, &mut batch).await {
                    Some((result, true))
                } else {
                    tracing::warn!("tracking relay stopped mid-batch; the batch goes out later");
                    None
                }
            }
        }
    }

    /// What follows a batch's `result`, and whether its connection is kept
    /// (not after a broker failure).
    fn settle(
        &self,
        result: Result<Batch, RelayError>,
        stopping: bool,
        failures: &mut u32,
    ) -> (Next, bool) {
        let (next, keep) = match result {
            Ok(batch) => {
                *failures = 0;
                (if batch.full { Next::Again } else { Next::Idle }, true)
            }
            Err(error @ (RelayError::Database(_) | RelayError::Encode)) => {
                tracing::warn!(
                    kind = %error,
                    "tracking relay could not publish a batch; it goes out later"
                );
                (
                    Next::Pause(Duration::from_millis(self.config.poll_interval_ms)),
                    true,
                )
            }
            Err(error) => {
                *failures = failures.saturating_add(1);
                let pause = backoff(*failures);
                tracing::warn!(
                    kind = %error,
                    retry_in_ms = millis(pause),
                    "tracking relay could not publish a batch; it goes out later"
                );
                (Next::Pause(pause), !matches!(error, RelayError::Broker(_)))
            }
        };
        (if stopping { Next::Stop } else { next }, keep)
    }

    /// Record the backlog: rows waiting and the age of the oldest.
    async fn sample(&self) {
        let backlog = async {
            let mut tx = db::begin_kernel(&self.db).await?;
            let row: (i64, f64) = sqlx::query_as(
                "SELECT waiting, oldest_seconds FROM kernel.tracked_events_backlog()",
            )
            .fetch_one(&mut *tx)
            .await?;
            tx.commit().await?;
            Ok::<_, sqlx::Error>(row)
        }
        .await;
        match backlog {
            Ok((waiting, oldest)) => self
                .metrics
                .tracking_backlog(u64::try_from(waiting).unwrap_or(0), oldest),
            Err(e) => tracing::warn!(
                error = %redact::sqlx(&e),
                "could not sample the tracked events backlog"
            ),
        }
    }
}

/// What the relay's loop does next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Next {
    /// Another batch at once (the last was full, or it just connected).
    Again,
    /// Wait this long (or for shutdown).
    Pause(Duration),
    /// Wait for a notification or the poll interval (or for shutdown).
    Idle,
    /// Stop.
    Stop,
}

/// `pause` in whole milliseconds, for a log field.
fn millis(pause: Duration) -> u64 {
    u64::try_from(pause.as_millis()).unwrap_or(u64::MAX)
}

/// The pause after `failures` failures in a row: 1 s doubling up to 60 s,
/// each drawn between half that and all of it so relays do not retry in
/// step.
fn backoff(failures: u32) -> Duration {
    let doublings = failures.saturating_sub(1).min(6);
    let ceiling = BACKOFF_FIRST
        .saturating_mul(1_u32 << doublings)
        .min(BACKOFF_MAX);
    let half = ceiling / 2;
    let spread = u64::try_from(half.as_millis()).unwrap_or(0);
    let random = std::collections::hash_map::RandomState::new()
        .build_hasher()
        .finish();
    half + Duration::from_millis(random % spread.saturating_add(1))
}

/// Turn outbox notifications into wake-ups, reconnecting as needed.
async fn listen(db: PgPool, wake: Arc<Notify>) {
    loop {
        let mut listener = match PgListener::connect_with(&db).await {
            Ok(listener) => listener,
            Err(e) => {
                tracing::warn!(error = %redact::sqlx(&e), "cannot listen for tracked events; polling");
                tokio::time::sleep(LISTEN_RETRY).await;
                continue;
            }
        };
        if let Err(e) = listener.listen(NOTIFY_CHANNEL).await {
            tracing::warn!(error = %redact::sqlx(&e), "cannot listen for tracked events; polling");
            tokio::time::sleep(LISTEN_RETRY).await;
            continue;
        }
        loop {
            match listener.recv().await {
                Ok(_) => wake.notify_one(),
                Err(e) => {
                    tracing::warn!(
                        error = %redact::sqlx(&e),
                        "tracked event notifications interrupted; polling"
                    );
                    tokio::time::sleep(LISTEN_RETRY).await;
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn backoff_doubles_up_to_a_minute_with_jitter() {
        for (failures, ceiling) in [
            (1, 1_000),
            (2, 2_000),
            (3, 4_000),
            (7, 60_000),
            (40, 60_000),
        ] {
            for _ in 0..20 {
                let pause = backoff(failures).as_millis();
                assert!(
                    (ceiling / 2..=ceiling).contains(&pause),
                    "{failures} failures: {pause} ms"
                );
            }
        }
    }

    #[test]
    fn a_row_becomes_its_message() {
        let id = uuid::Uuid::now_v7();
        let occurred_at = DateTime::from_timestamp(1_800_000_000, 0).unwrap();
        let row = OutboxRow {
            id,
            name: "test.thing_made".to_owned(),
            version: 2,
            occurred_at,
            envelope: serde_json::json!({"id": id.to_string(), "name": "test.thing_made"}),
            body: serde_json::json!({"size": 3}),
        };
        let message = Message::of(&row).unwrap();
        assert_eq!(message.routing_key.as_str(), "test.thing_made.v2");
        let payload: Value = serde_json::from_slice(&message.payload).unwrap();
        assert_eq!(
            payload,
            serde_json::json!({
                "id": id.to_string(),
                "name": "test.thing_made",
                "body": {"size": 3}
            })
        );
        let properties = &message.properties;
        assert_eq!(
            properties.message_id().as_ref().map(ShortString::as_str),
            Some(id.to_string().as_str())
        );
        assert_eq!(
            properties.kind().as_ref().map(ShortString::as_str),
            Some("test.thing_made")
        );
        assert_eq!(
            properties.content_type().as_ref().map(ShortString::as_str),
            Some("application/json")
        );
        assert_eq!(*properties.timestamp(), Some(1_800_000_000));
        assert_eq!(*properties.delivery_mode(), Some(PERSISTENT));
        assert_eq!(
            properties
                .headers()
                .as_ref()
                .and_then(|h| h.inner().get("x-event-version").cloned()),
            Some(AMQPValue::LongInt(2))
        );
    }

    #[test]
    fn errors_name_their_kind_only() {
        assert_eq!(
            RelayError::Broker("connect").to_string(),
            "the broker failed (connect)"
        );
        assert_eq!(
            RelayError::Refused("nacked").outcome(),
            PublishOutcome::Nacked
        );
        assert_eq!(
            RelayError::Broker("publish").outcome(),
            PublishOutcome::Error
        );
    }
}
