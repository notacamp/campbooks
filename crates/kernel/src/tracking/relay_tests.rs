//! The relay against a real Postgres and, where a test needs one, a real
//! RabbitMQ 4 (`CAMPBOOKS_TEST_AMQP_URL`, set in CI; a test that needs the
//! broker returns early, saying so, when it is unset). Each broker test
//! uses an exchange and a stream of its own, so tests sharing a broker never
//! read each other's messages.

use std::{collections::BTreeSet, time::Duration};

use futures_util::StreamExt as _;
use lapin::{
    Connection, ConnectionProperties, ExchangeKind,
    message::Delivery,
    options::{
        BasicAckOptions, BasicConsumeOptions, BasicQosOptions, ExchangeDeclareOptions,
        QueueBindOptions, QueueDeclareOptions, QueueDeleteOptions,
    },
    types::{AMQPValue, FieldTable, ShortString},
};
use secrecy::SecretString;
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{PgPool, postgres::PgListener};

use super::{
    TrackedEvent,
    relay::{NOTIFY_CHANNEL, Relay, RelayError, declare_topology},
};
use crate::{
    AppState,
    config::{TrackingConfig, TrackingSink},
    db::{self, BeginFor as _},
    jobs::{EnqueueOptions, Registry, Worker, enqueue},
    tenancy::WorkspaceId,
    testing,
};

/// A tracked event of a workspace, written by hand for the kernel's own
/// tests (the plan check allows that in test code only).
#[derive(Debug, Clone, Serialize)]
struct ThingRelayed {
    #[serde(skip)]
    workspace: WorkspaceId,
    size: i64,
}

impl TrackedEvent for ThingRelayed {
    const NAME: &'static str = "test.thing_relayed";
    const VERSION: u16 = 1;
    const MODULE: &'static str = "test";
    const WORKSPACE_SCOPED: bool = true;

    fn workspace(&self) -> Option<WorkspaceId> {
        Some(self.workspace)
    }
}

/// The test broker, or `None` (and a line saying the test was skipped).
fn broker() -> Option<String> {
    let url = std::env::var("CAMPBOOKS_TEST_AMQP_URL")
        .ok()
        .filter(|url| !url.trim().is_empty());
    if url.is_none() {
        eprintln!(
            "skipped: CAMPBOOKS_TEST_AMQP_URL is unset (CI sets it; point it at a RabbitMQ 4, \
             e.g. amqp://guest:guest@localhost:5672/%2f, to run this test)"
        );
    }
    url
}

/// Settings for a relay publishing to `url`, on an exchange and a stream
/// named after `test` and nothing else.
fn settings(url: &str, test: &str) -> TrackingConfig {
    let id = uuid::Uuid::now_v7().simple().to_string();
    TrackingConfig {
        sink: TrackingSink::Amqp,
        amqp_url: Some(SecretString::from(url)),
        exchange: format!("test.{test}.{id}"),
        stream: format!("test.{test}.{id}.stream"),
        stream_max_age: "1h".to_owned(),
        declare_topology: true,
        batch_size: 50,
        poll_interval_ms: 50,
        published_retention_hours: 24,
    }
}

/// The kernel's state over the runtime role, with its own work registered.
async fn runtime(db: &PgPool) -> AppState {
    let registry = Registry::build(&[]).unwrap();
    testing::allow_instance_rows(db, &registry).await;
    testing::state(testing::app_role_pool(db).await).with_registry(registry)
}

/// `n` instance-wide rows in the outbox, written as the owner.
async fn outbox_rows(db: &PgPool, n: i32) {
    sqlx::query(
        "INSERT INTO kernel.tracked_events
             (id, workspace_id, name, version, occurred_at, envelope, body)
         SELECT ids.id, NULL, 'test.thing_counted', 1, now(),
                jsonb_build_object('id', ids.id::text, 'name', 'test.thing_counted'),
                jsonb_build_object('n', ids.n)
           FROM (SELECT kernel.uuid_v7() AS id, i AS n FROM generate_series(1, $1) AS i) ids",
    )
    .bind(n)
    .execute(db)
    .await
    .unwrap();
}

/// How many rows wait to be published, read as the owner.
async fn unpublished(db: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM kernel.tracked_events WHERE published_at IS NULL")
        .fetch_one(db)
        .await
        .unwrap()
}

/// Wait until nothing waits to be published, at most `within`.
async fn until_published(db: &PgPool, within: Duration) {
    let deadline = tokio::time::Instant::now() + within;
    while unpublished(db).await > 0 {
        assert!(
            tokio::time::Instant::now() < deadline,
            "{} row(s) still unpublished",
            unpublished(db).await
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Read the stream `stream` from its first message: what arrives until
/// `want` messages are in, then whatever else comes within a short wait
/// (so a test also sees what should not be there).
async fn read_stream(url: &str, stream: &str, want: usize) -> Vec<Delivery> {
    let connection = Connection::connect(url, ConnectionProperties::default())
        .await
        .unwrap();
    let channel = connection.create_channel().await.unwrap();
    channel
        .basic_qos(500, BasicQosOptions::default())
        .await
        .unwrap();
    let mut arguments = FieldTable::default();
    arguments.insert(
        "x-stream-offset".into(),
        AMQPValue::LongString("first".into()),
    );
    let mut consumer = channel
        .basic_consume(
            stream.into(),
            "test-reader".into(),
            BasicConsumeOptions::default(),
            arguments,
        )
        .await
        .unwrap();
    let mut got = Vec::new();
    loop {
        let wait = if got.len() < want {
            Duration::from_secs(15)
        } else {
            Duration::from_millis(500)
        };
        let Ok(Some(Ok(delivery))) = tokio::time::timeout(wait, consumer.next()).await else {
            break;
        };
        delivery
            .acker
            .ack(BasicAckOptions::default())
            .await
            .unwrap();
        got.push(delivery);
    }
    let _ = connection.close(200, "OK".into()).await;
    got
}

fn message_id(delivery: &Delivery) -> String {
    delivery
        .properties
        .message_id()
        .as_ref()
        .map(ShortString::to_string)
        .unwrap_or_default()
}

/// An event tracked in a workspace's transaction reaches the stream with
/// its routing key, its id as `message_id`, its name as `type`, and the
/// envelope and body as JSON; its row is marked published.
#[sqlx::test(migrations = "../../migrations")]
async fn a_tracked_event_reaches_the_stream(db: PgPool) {
    let Some(url) = broker() else { return };
    let state = testing::tracking_outbox(runtime(&db).await);
    let workspace = WorkspaceId::new();
    let mut tx = state.db.begin_for(workspace).await.unwrap();
    state
        .tracker
        .track(&mut tx, &ThingRelayed { workspace, size: 3 })
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let row = testing::tracked_events(&db, ThingRelayed::NAME)
        .await
        .remove(0);

    let config = settings(&url, "reaches");
    let relay = Relay::new(&state, config.clone());
    let publisher = relay.connect().await.unwrap();
    let batch = relay.relay_batch(&publisher).await.unwrap();
    assert_eq!(batch.published, 1);
    assert!(!batch.full);
    assert_eq!(
        relay.relay_batch(&publisher).await.unwrap().published,
        0,
        "nothing left"
    );
    publisher.close().await;

    let published: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT published_at FROM kernel.tracked_events WHERE id = $1")
            .bind(row.id)
            .fetch_one(&db)
            .await
            .unwrap();
    assert!(published.is_some(), "marked published");

    let messages = read_stream(&url, &config.stream, 1).await;
    assert_eq!(messages.len(), 1);
    let message = &messages[0];
    assert_eq!(message.routing_key.as_str(), "test.thing_relayed.v1");
    assert_eq!(message_id(message), row.id.to_string());
    let properties = &message.properties;
    assert_eq!(
        properties.kind().as_ref().map(ShortString::as_str),
        Some(ThingRelayed::NAME)
    );
    assert_eq!(
        properties.content_type().as_ref().map(ShortString::as_str),
        Some("application/json")
    );
    assert_eq!(*properties.delivery_mode(), Some(2));
    assert!(properties.timestamp().is_some_and(|t| t > 0));
    assert_eq!(
        properties
            .headers()
            .as_ref()
            .and_then(|h| h.inner().get("x-event-version").cloned()),
        Some(AMQPValue::LongInt(1))
    );
    let body: Value = serde_json::from_slice(&message.data).unwrap();
    assert_eq!(body["id"], row.id.to_string());
    assert_eq!(body["name"], ThingRelayed::NAME);
    assert_eq!(body["workspace_id"], workspace.to_string());
    assert_eq!(body["actor"], row.envelope["actor"]);
    assert_eq!(body["correlation"], row.envelope["correlation"]);
    assert_eq!(body["body"], json!({"size": 3}));
}

/// A publish that fails (no exchange), is returned (no binding) or is
/// nacked (a queue refusing it) leaves the batch unpublished; once the
/// topology is right, a new connection publishes it.
#[sqlx::test(migrations = "../../migrations")]
async fn a_refused_or_failed_batch_stays_unpublished(db: PgPool) {
    let Some(url) = broker() else { return };
    let state = runtime(&db).await;
    outbox_rows(&db, 3).await;
    let mut config = settings(&url, "refused");
    config.declare_topology = false;
    let relay = Relay::new(&state, config.clone());

    // No exchange at all: the broker closes the channel.
    let publisher = relay.connect().await.unwrap();
    let failed = relay.relay_batch(&publisher).await;
    assert!(matches!(failed, Err(RelayError::Broker(_))), "{failed:?}");
    publisher.close().await;
    assert_eq!(unpublished(&db).await, 3);

    // An exchange with no binding: each message comes back.
    let admin = Connection::connect(&url, ConnectionProperties::default())
        .await
        .unwrap();
    let channel = admin.create_channel().await.unwrap();
    channel
        .exchange_declare(
            config.exchange.as_str().into(),
            ExchangeKind::Topic,
            ExchangeDeclareOptions {
                durable: true,
                ..ExchangeDeclareOptions::default()
            },
            FieldTable::default(),
        )
        .await
        .unwrap();
    let publisher = relay.connect().await.unwrap();
    let returned = relay.relay_batch(&publisher).await;
    assert_eq!(returned, Err(RelayError::Refused("unroutable")));
    assert_eq!(unpublished(&db).await, 3);

    // A queue that takes nothing: the broker nacks.
    let refusing = format!("{}.refusing", config.exchange);
    let mut arguments = FieldTable::default();
    arguments.insert("x-max-length".into(), AMQPValue::LongInt(0));
    arguments.insert(
        "x-overflow".into(),
        AMQPValue::LongString("reject-publish".into()),
    );
    channel
        .queue_declare(
            refusing.as_str().into(),
            QueueDeclareOptions {
                durable: true,
                ..QueueDeclareOptions::default()
            },
            arguments,
        )
        .await
        .unwrap();
    channel
        .queue_bind(
            refusing.as_str().into(),
            config.exchange.as_str().into(),
            "#".into(),
            QueueBindOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();
    let nacked = relay.relay_batch(&publisher).await;
    assert_eq!(nacked, Err(RelayError::Refused("nacked")));
    assert_eq!(unpublished(&db).await, 3);
    publisher.close().await;

    // Put right: the stream bound, the refusing queue gone.
    channel
        .queue_delete(refusing.as_str().into(), QueueDeleteOptions::default())
        .await
        .unwrap();
    declare_topology(&channel, &config).await.unwrap();
    let _ = admin.close(200, "OK".into()).await;
    let publisher = relay.connect().await.unwrap();
    assert_eq!(relay.relay_batch(&publisher).await.unwrap().published, 3);
    publisher.close().await;
    assert_eq!(unpublished(&db).await, 0);
    assert_eq!(read_stream(&url, &config.stream, 3).await.len(), 3);
}

/// While the broker cannot be reached, events wait in the outbox and the
/// relay retries; once it can, every event goes out. The broker "comes
/// back" when a local port starts forwarding to it.
#[sqlx::test(migrations = "../../migrations")]
async fn events_wait_while_the_broker_is_down(db: PgPool) {
    let Some(url) = broker() else { return };
    let state = runtime(&db).await;
    outbox_rows(&db, 5).await;

    // A free local port, closed for now.
    let port = {
        let probe = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        probe.local_addr().unwrap().port()
    };
    let upstream = url::Url::parse(&url).unwrap();
    let upstream_host = upstream.host_str().unwrap().to_owned();
    let upstream_port = upstream.port().unwrap_or(5672);
    let mut through = upstream.clone();
    through.set_host(Some("127.0.0.1")).unwrap();
    through.set_port(Some(port)).unwrap();
    let mut config = settings(through.as_str(), "down");
    config.poll_interval_ms = 100;

    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let relay = tokio::spawn(Relay::new(&state, config.clone()).run(async move {
        let _ = stopped.await;
    }));
    tokio::time::sleep(Duration::from_millis(1_500)).await;
    assert_eq!(
        unpublished(&db).await,
        5,
        "nothing went out while it was down"
    );

    // Back: the port forwards to the broker.
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .unwrap();
    let forward = tokio::spawn(async move {
        while let Ok((mut inbound, _)) = listener.accept().await {
            let (host, port) = (upstream_host.clone(), upstream_port);
            tokio::spawn(async move {
                if let Ok(mut outbound) =
                    tokio::net::TcpStream::connect((host.as_str(), port)).await
                {
                    let _ = tokio::io::copy_bidirectional(&mut inbound, &mut outbound).await;
                }
            });
        }
    });
    outbox_rows(&db, 2).await;
    until_published(&db, Duration::from_secs(60)).await;

    let _ = stop.send(());
    relay.await.unwrap();
    forward.abort();
    let ids: BTreeSet<String> = read_stream(&url, &config.stream, 7)
        .await
        .iter()
        .map(message_id)
        .collect();
    assert_eq!(ids.len(), 7, "every event, once");
}

/// Two relays on one outbox: every row is published, each exactly once.
#[sqlx::test(migrations = "../../migrations")]
async fn two_relays_publish_every_row_once(db: PgPool) {
    let Some(url) = broker() else { return };
    let state = runtime(&db).await;
    outbox_rows(&db, 300).await;
    let mut config = settings(&url, "two");
    config.batch_size = 20;

    let mut relays = Vec::new();
    for _ in 0..2 {
        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        let task = tokio::spawn(Relay::new(&state, config.clone()).run(async move {
            let _ = stopped.await;
        }));
        relays.push((stop, task));
    }
    until_published(&db, Duration::from_secs(60)).await;
    for (stop, task) in relays {
        let _ = stop.send(());
        task.await.unwrap();
    }

    let messages = read_stream(&url, &config.stream, 300).await;
    let ids: BTreeSet<String> = messages.iter().map(message_id).collect();
    assert_eq!(messages.len(), 300, "no row published twice");
    assert_eq!(ids.len(), 300);
}

/// Two claims at once never hold the same row: the second skips what the
/// first locked.
#[sqlx::test(migrations = "../../migrations")]
async fn concurrent_claims_hold_different_rows(db: PgPool) {
    let pool = testing::app_role_pool(&db).await;
    outbox_rows(&db, 5).await;
    let claim = "SELECT id FROM kernel.claim_tracked_events($1)";

    let mut first = db::begin_kernel(&pool).await.unwrap();
    let mine: Vec<uuid::Uuid> = sqlx::query_scalar(claim)
        .bind(3)
        .fetch_all(&mut *first)
        .await
        .unwrap();
    let mut second = db::begin_kernel(&pool).await.unwrap();
    let theirs: Vec<uuid::Uuid> = sqlx::query_scalar(claim)
        .bind(10)
        .fetch_all(&mut *second)
        .await
        .unwrap();
    assert_eq!(mine.len(), 3);
    assert_eq!(theirs.len(), 2, "the two the first did not lock");
    assert!(mine.iter().all(|id| !theirs.contains(id)), "disjoint");
    let mut sorted = mine.clone();
    sorted.sort();
    assert_eq!(mine, sorted, "oldest first");
    first.rollback().await.unwrap();
    second.rollback().await.unwrap();
}

/// The outbox's functions refuse a transaction outside the kernel scope,
/// and the purge one a transaction working in another workspace.
#[sqlx::test(migrations = "../../migrations")]
async fn the_outbox_functions_refuse_other_scopes(db: PgPool) {
    let pool = testing::app_role_pool(&db).await;
    let workspace = WorkspaceId::new();
    for call in [
        "SELECT count(*) FROM kernel.claim_tracked_events(10)",
        "SELECT kernel.mark_tracked_events_published(ARRAY[gen_random_uuid()])",
        "SELECT count(*) FROM kernel.tracked_events_backlog()",
        "SELECT kernel.prune_tracked_events(24, 10)",
    ] {
        for kernel_scope in [false, true] {
            let mut tx = if kernel_scope {
                db::begin_kernel(&pool).await.unwrap()
            } else {
                pool.begin_for(workspace).await.unwrap()
            };
            let result = sqlx::query(call).execute(&mut *tx).await;
            assert_eq!(result.is_ok(), kernel_scope, "{call}: {result:?}");
            let _ = tx.rollback().await;
        }
    }
    let mut tx = pool.begin_for(workspace).await.unwrap();
    let refused = sqlx::query("SELECT kernel.purge_workspace_tracked_events($1, 10)")
        .bind(WorkspaceId::new().to_string())
        .execute(&mut *tx)
        .await;
    assert!(refused.is_err(), "another workspace's purge: {refused:?}");
    let _ = tx.rollback().await;
}

/// Declaring the topology twice changes nothing; with declaring off, the
/// relay declares nothing.
#[sqlx::test(migrations = "../../migrations")]
async fn declaring_is_idempotent_and_optional(db: PgPool) {
    let Some(url) = broker() else { return };
    let state = runtime(&db).await;
    let config = settings(&url, "declare");
    let relay = Relay::new(&state, config.clone());
    let first = relay.connect().await.unwrap();
    let second = relay.connect().await.unwrap();
    first.close().await;
    second.close().await;

    let admin = Connection::connect(&url, ConnectionProperties::default())
        .await
        .unwrap();
    let channel = admin.create_channel().await.unwrap();
    declare_topology(&channel, &config).await.unwrap();
    let mut arguments = FieldTable::default();
    arguments.insert(
        "x-queue-type".into(),
        AMQPValue::LongString("stream".into()),
    );
    let stream = channel
        .queue_declare(
            config.stream.as_str().into(),
            QueueDeclareOptions {
                passive: true,
                durable: true,
                ..QueueDeclareOptions::default()
            },
            arguments,
        )
        .await;
    assert!(stream.is_ok(), "the stream exists: {stream:?}");

    let mut quiet = settings(&url, "quiet");
    quiet.declare_topology = false;
    let publisher = Relay::new(&state, quiet.clone()).connect().await.unwrap();
    publisher.close().await;
    for (what, passive) in [
        (
            "exchange",
            channel
                .exchange_declare(
                    quiet.exchange.as_str().into(),
                    ExchangeKind::Topic,
                    ExchangeDeclareOptions {
                        passive: true,
                        ..ExchangeDeclareOptions::default()
                    },
                    FieldTable::default(),
                )
                .await
                .is_ok(),
        ),
        (
            "stream",
            admin
                .create_channel()
                .await
                .unwrap()
                .queue_declare(
                    quiet.stream.as_str().into(),
                    QueueDeclareOptions {
                        passive: true,
                        ..QueueDeclareOptions::default()
                    },
                    FieldTable::default(),
                )
                .await
                .is_ok(),
        ),
    ] {
        assert!(!passive, "the {what} was declared with declaring off");
    }
    let _ = admin.close(200, "OK".into()).await;
}

/// The prune deletes published rows past their retention and keeps the
/// rest, however old an unpublished row is.
#[sqlx::test(migrations = "../../migrations")]
async fn the_prune_keeps_unpublished_rows(db: PgPool) {
    let state = runtime(&db).await;
    for (n, occurred_days, published_hours) in
        [(1, 3, Some(48)), (2, 1, Some(1)), (3, 10, None::<i32>)]
    {
        sqlx::query(
            "INSERT INTO kernel.tracked_events
                 (id, workspace_id, name, version, occurred_at, envelope, body, published_at)
             VALUES (kernel.uuid_v7(), NULL, 'test.thing_counted', 1,
                     now() - make_interval(days => $2), '{}', jsonb_build_object('n', $1::int),
                     now() - make_interval(hours => $3))",
        )
        .bind(n)
        .bind(occurred_days)
        .bind(published_hours)
        .execute(&db)
        .await
        .unwrap();
    }

    let mut tx = db::begin_kernel(&state.db).await.unwrap();
    enqueue(
        &mut tx,
        &super::PruneTrackedEvents,
        EnqueueOptions::default(),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    Worker::new(state).run_until_idle().await;

    let left: Vec<i32> =
        sqlx::query_scalar("SELECT (body->>'n')::int FROM kernel.tracked_events ORDER BY 1")
            .fetch_all(&db)
            .await
            .unwrap();
    assert_eq!(left, [2, 3], "only the row published two days ago is gone");
}

/// A purged workspace loses its tracked events, published or not; another
/// workspace keeps its own.
#[sqlx::test(migrations = "../../migrations")]
async fn a_purged_workspace_loses_its_tracked_events(db: PgPool) {
    let state = runtime(&db).await;
    let (gone, kept) = (WorkspaceId::new(), WorkspaceId::new());
    for (workspace, published) in [(gone, true), (gone, false), (kept, false)] {
        sqlx::query(
            "INSERT INTO kernel.tracked_events
                 (id, workspace_id, name, version, occurred_at, envelope, body, published_at)
             VALUES (kernel.uuid_v7(), $1, 'test.thing_relayed', 1, now(), '{}', '{}',
                     CASE WHEN $2 THEN now() END)",
        )
        .bind(*workspace.as_uuid())
        .bind(published)
        .execute(&db)
        .await
        .unwrap();
    }

    let purged = crate::purge::purge_workspace(&state.db, &gone.to_string())
        .await
        .unwrap();
    assert_eq!(purged.tracked, 2);
    let left: Vec<Option<uuid::Uuid>> =
        sqlx::query_scalar("SELECT workspace_id FROM kernel.tracked_events")
            .fetch_all(&db)
            .await
            .unwrap();
    assert_eq!(left, [Some(*kept.as_uuid())]);
}

/// An insert into the outbox notifies the relay's channel at commit.
#[sqlx::test(migrations = "../../migrations")]
async fn an_insert_notifies_the_relay(db: PgPool) {
    let mut listener = PgListener::connect_with(&db).await.unwrap();
    listener.listen(NOTIFY_CHANNEL).await.unwrap();
    outbox_rows(&db, 2).await;
    let notification = tokio::time::timeout(Duration::from_secs(5), listener.recv())
        .await
        .expect("a notification within 5 s")
        .unwrap();
    assert_eq!(notification.channel(), NOTIFY_CHANNEL);
    assert_eq!(notification.payload(), "");
}

/// The relay runs only with the sink `amqp` and a URL.
#[sqlx::test(migrations = "../../migrations")]
async fn the_relay_runs_only_for_amqp(db: PgPool) {
    let state = testing::state(db);
    assert!(
        Relay::from_state(&state).is_none(),
        "the default sink is none"
    );
    for (sink, url, runs) in [
        (TrackingSink::Outbox, Some("amqp://localhost"), false),
        (TrackingSink::Amqp, None, false),
        (TrackingSink::Amqp, Some("amqp://localhost"), true),
    ] {
        let mut config = (*state.config).clone();
        config.tracking.sink = sink;
        config.tracking.amqp_url = url.map(SecretString::from);
        let state = AppState::new(state.db.clone(), config);
        assert_eq!(
            Relay::from_state(&state).is_some(),
            runs,
            "{sink:?}, {url:?}"
        );
    }
}

/// The settings' `Debug` never shows the AMQP URL.
#[test]
fn the_settings_never_show_the_url() {
    let config = TrackingConfig {
        amqp_url: Some(SecretString::from(
            "amqp://user:hunter2@broker.example.com/%2f",
        )),
        ..TrackingConfig::default()
    };
    let shown = format!("{config:?}");
    assert!(!shown.contains("hunter2"), "{shown}");
    assert!(!shown.contains("broker.example.com"), "{shown}");
}
