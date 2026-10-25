//! Tracking against a real Postgres: the outbox is written in the caller's
//! transaction, only into its workspace, only when the sink says so, with
//! the envelope the ambient context makes; jobs carry who caused them.

use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use utoipa_axum::router::OpenApiRouter;

use super::*;
use crate::{
    AppState, DomainEvent, Event, EventHandler, JobContext, JobError, Module,
    db::BeginFor as _,
    jobs::{Background, Registry, Worker},
    tenancy::WorkspaceId,
    testing,
};

/// A tracked event of a workspace, written by hand for the kernel's own
/// tests (the plan check allows that in test code only).
#[derive(Debug, Clone, Serialize)]
struct ThingMade {
    #[serde(skip)]
    workspace: WorkspaceId,
    size: i64,
}

impl TrackedEvent for ThingMade {
    const NAME: &'static str = "test.thing_made";
    const VERSION: u16 = 1;
    const MODULE: &'static str = "test";
    const WORKSPACE_SCOPED: bool = true;

    fn workspace(&self) -> Option<WorkspaceId> {
        Some(self.workspace)
    }
}

/// An instance-wide tracked event.
#[derive(Debug, Clone, Serialize)]
struct InstanceTicked {
    n: i64,
}

impl TrackedEvent for InstanceTicked {
    const NAME: &'static str = "test.instance_ticked";
    const VERSION: u16 = 1;
    const MODULE: &'static str = "test";
    const WORKSPACE_SCOPED: bool = false;

    fn workspace(&self) -> Option<WorkspaceId> {
        None
    }
}

fn outbox(db: PgPool) -> AppState {
    testing::tracking_outbox(AppState::new(
        db,
        testing::config("postgres://test@localhost/test"),
    ))
}

async fn count(db: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM kernel.tracked_events")
        .fetch_one(db)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_committed_change_has_its_event_and_a_rolled_back_one_none(db: PgPool) {
    let state = outbox(testing::app_role_pool(&db).await);
    let workspace = WorkspaceId::new();
    let event = ThingMade { workspace, size: 3 };

    let mut tx = state.db.begin_for(workspace).await.unwrap();
    state.tracker.track(&mut tx, &event).await.unwrap();
    tx.rollback().await.unwrap();
    assert_eq!(count(&db).await, 0);

    let mut tx = state.db.begin_for(workspace).await.unwrap();
    state.tracker.track(&mut tx, &event).await.unwrap();
    tx.commit().await.unwrap();
    let rows = testing::tracked_events(&db, ThingMade::NAME).await;
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.workspace_id, Some(*workspace.as_uuid()));
    assert_eq!(row.body, json!({"size": 3}));
    assert_eq!(row.envelope["id"], row.id.to_string());
    assert_eq!(row.envelope["name"], ThingMade::NAME);
    assert_eq!(row.envelope["version"], 1);
    assert_eq!(row.envelope["role"], "all");
    assert_eq!(row.envelope["plan_version"], PLAN_VERSION);
    assert_eq!(row.envelope["app_version"], env!("CARGO_PKG_VERSION"));
    // Outside any request or job: the process, acting on its own.
    assert_eq!(row.envelope["actor"]["type"], "system");
    assert_eq!(row.envelope["on_behalf_of"], serde_json::Value::Null);
    assert_eq!(row.envelope["operation"], serde_json::Value::Null);
    assert_eq!(row.envelope["correlation"].as_str().unwrap().len(), 32);
}

#[sqlx::test(migrations = "../../migrations")]
async fn another_workspaces_event_is_refused(db: PgPool) {
    let state = outbox(testing::app_role_pool(&db).await);
    let (mine, theirs) = (WorkspaceId::new(), WorkspaceId::new());

    let mut tx = state.db.begin_for(mine).await.unwrap();
    let refused = state
        .tracker
        .track(
            &mut tx,
            &ThingMade {
                workspace: theirs,
                size: 1,
            },
        )
        .await;
    assert!(
        matches!(refused, Err(sqlx::Error::InvalidArgument(_))),
        "{refused:?}"
    );
    tx.commit().await.unwrap();

    // Nor from a transaction that set no workspace.
    let mut tx = state.db.begin().await.unwrap();
    let refused = state
        .tracker
        .track(
            &mut tx,
            &ThingMade {
                workspace: mine,
                size: 1,
            },
        )
        .await;
    assert!(refused.is_err());
    drop(tx);

    // Row-level security refuses it too, written by hand.
    let mut tx = state.db.begin_for(mine).await.unwrap();
    let forged = sqlx::query(
        "INSERT INTO kernel.tracked_events (id, workspace_id, name, version, occurred_at,
                                            envelope, body)
         VALUES (gen_random_uuid(), $1, 'test.thing_made', 1, now(), '{}', '{}')",
    )
    .bind(theirs.as_uuid())
    .execute(&mut *tx)
    .await;
    assert!(forged.is_err());
    drop(tx);
    assert_eq!(count(&db).await, 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn the_runtime_role_only_appends(db: PgPool) {
    let state = outbox(testing::app_role_pool(&db).await);
    let workspace = WorkspaceId::new();
    let mut tx = state.db.begin_for(workspace).await.unwrap();
    state
        .tracker
        .track(&mut tx, &ThingMade { workspace, size: 1 })
        .await
        .unwrap();
    let read = sqlx::query("SELECT 1 FROM kernel.tracked_events")
        .execute(&mut *tx)
        .await;
    assert!(read.is_err(), "the runtime role read the outbox back");
}

#[sqlx::test(migrations = "../../migrations")]
async fn with_the_sink_off_nothing_is_written(db: PgPool) {
    let state = AppState::new(
        testing::app_role_pool(&db).await,
        testing::config("postgres://test@localhost/test"),
    );
    assert_eq!(state.tracker.sink(), crate::config::TrackingSink::None);
    let workspace = WorkspaceId::new();
    let mut tx = state.db.begin_for(workspace).await.unwrap();
    state
        .tracker
        .track(&mut tx, &ThingMade { workspace, size: 1 })
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(count(&db).await, 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn an_instance_wide_event_is_written_from_no_workspace_only(db: PgPool) {
    let state = outbox(testing::app_role_pool(&db).await);
    let mut tx = state.db.begin().await.unwrap();
    state
        .tracker
        .track(&mut tx, &InstanceTicked { n: 1 })
        .await
        .unwrap();
    // The kernel lifted the transaction for that insert only.
    let scope = crate::db::scope(&mut tx).await.unwrap();
    assert!(scope.is_unset(), "{scope:?}");
    tx.commit().await.unwrap();
    let rows = testing::tracked_events(&db, InstanceTicked::NAME).await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].workspace_id, None);

    let mut tx = state.db.begin_for(WorkspaceId::new()).await.unwrap();
    let refused = state.tracker.track(&mut tx, &InstanceTicked { n: 2 }).await;
    assert!(refused.is_err());
}

#[sqlx::test(migrations = "../../migrations")]
async fn the_envelope_comes_from_the_ambient_context(db: PgPool) {
    let state = outbox(testing::app_role_pool(&db).await);
    let workspace = WorkspaceId::new();
    let context = TrackingContext::new(Actor::assistant())
        .on_behalf_of(Actor::user("account-1"))
        .in_operation("test.make")
        .correlated("0123456789abcdef0123456789abcdef");
    scope(context, async {
        let mut tx = state.db.begin_for(workspace).await.unwrap();
        state
            .tracker
            .track(&mut tx, &ThingMade { workspace, size: 2 })
            .await
            .unwrap();
        tx.commit().await.unwrap();
    })
    .await;
    let rows = testing::tracked_events(&db, ThingMade::NAME).await;
    let envelope = &rows[0].envelope;
    assert_eq!(envelope["actor"], json!({"type": "assistant"}));
    assert_eq!(
        envelope["on_behalf_of"],
        json!({"type": "user", "id": "account-1"})
    );
    assert_eq!(envelope["operation"], "test.make");
    assert_eq!(envelope["correlation"], "0123456789abcdef0123456789abcdef");
}

// --- work a request caused ------------------------------------------------------

/// A domain event a request publishes.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
struct Ordered {
    workspace: String,
}

impl DomainEvent for Ordered {
    const NAME: &'static str = "test.ordered";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// Its consumer, which tracks an event.
struct MakeThing;

impl EventHandler for MakeThing {
    type Event = Ordered;
    const NAME: &'static str = "test.make_thing";
    const QUEUE: &'static str = "events";

    async fn handle(ctx: &mut JobContext, event: Event<Ordered>) -> Result<(), JobError> {
        let workspace: WorkspaceId = event
            .payload
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("not a workspace id"))?;
        ctx.track(&ThingMade { workspace, size: 5 }).await
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_delivery_caused_by_a_request_carries_who_and_the_correlation(db: PgPool) {
    let module = Module {
        name: "test",
        scopes: &[],
        router: OpenApiRouter::new(),
        admin_router: None,
        background: Background::new().on_event::<MakeThing>(),
        resources: crate::authz::Resources::default(),
    };
    let registry = Registry::build(&[module]).unwrap();
    let state = outbox(testing::app_role_pool(&db).await).with_registry(registry);
    let workspace = WorkspaceId::new();
    let correlation = "fedcba9876543210fedcba9876543210";
    let request = TrackingContext::new(Actor::user("sam-account"))
        .in_operation("test.order")
        .correlated(correlation);
    let event_id = scope(request, async {
        let mut tx = state.db.begin_for(workspace).await.unwrap();
        let id = state
            .events
            .publish(
                &mut tx,
                &Ordered {
                    workspace: workspace.to_string(),
                },
            )
            .await
            .unwrap();
        tx.commit().await.unwrap();
        id
    })
    .await;

    // The delivery remembers who caused it: ids only.
    let origin: serde_json::Value =
        sqlx::query_scalar("SELECT origin FROM kernel.jobs WHERE event_id = $1")
            .bind(event_id)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(
        origin,
        json!({"actor": {"type": "user", "id": "sam-account"}, "correlation": correlation})
    );

    let stats = Worker::new(state).run_until_idle().await;
    assert_eq!(stats.succeeded, 1, "{stats:?}");
    let rows = testing::tracked_events(&db, ThingMade::NAME).await;
    assert_eq!(rows.len(), 1);
    let envelope = &rows[0].envelope;
    assert_eq!(
        envelope["actor"],
        json!({"type": "system", "id": "test.make_thing"})
    );
    assert_eq!(
        envelope["on_behalf_of"],
        json!({"type": "user", "id": "sam-account"})
    );
    assert_eq!(envelope["correlation"], correlation);
    assert_eq!(
        envelope["causation"],
        json!({"type": "event", "id": event_id.to_string()})
    );
    assert_eq!(envelope["operation"], serde_json::Value::Null);
}

#[test]
fn a_job_with_no_origin_acts_on_its_own() {
    let job = crate::jobs::JobId::new();
    let context = for_job("test.sweep", job, None, None);
    assert_eq!(context.actor, Actor::system("test.sweep"));
    assert_eq!(context.on_behalf_of, None);
    assert_eq!(
        context.causation,
        Some(Causation {
            kind: CausationKind::Job,
            id: job.to_string(),
        })
    );
}

#[tokio::test]
async fn an_origin_names_the_person_never_the_system() {
    assert_eq!(origin(), None);
    let job = TrackingContext::new(Actor::system("test.sweep")).correlated("c");
    let none = scope(job, async { origin() }).await;
    assert_eq!(none, Some(json!({"correlation": "c"})));
    let for_someone = TrackingContext::new(Actor::system("test.sweep"))
        .on_behalf_of(Actor::client(SYNC_APP))
        .correlated("c");
    let remembered = scope(for_someone, async { origin() }).await;
    assert_eq!(
        remembered,
        Some(json!({"actor": {"type": "client", "id": SYNC_APP}, "correlation": "c"}))
    );
}

/// A workspace's app, as Campbooks mints its client id.
const SYNC_APP: &str = "cb_AAAAAAAAAAAAAAAAAAAAAA";

/// A client known by its metadata document: its id is a URL.
const DOCUMENT_CLIENT: &str = "https://agent.example.com/oauth/client.json";

#[test]
fn only_client_ids_campbooks_minted_are_recorded() {
    for minted in [
        "campbooks-web",
        "campbooks-native",
        SYNC_APP,
        "dcr_abcdefghijklmnopqrst-_",
    ] {
        assert_eq!(minted_client_id(minted), Some(minted));
        assert_eq!(Actor::client(minted).id.as_deref(), Some(minted));
    }
    for other in [
        DOCUMENT_CLIENT,
        "cb_short",
        "dcr_abcdefghijklmnopqrst@x",
        "another-idp-client",
        "",
    ] {
        assert_eq!(minted_client_id(other), None, "{other}");
        assert_eq!(Actor::client(other).id, None, "{other}");
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_url_client_id_never_reaches_the_outbox(db: PgPool) {
    let state = outbox(testing::app_role_pool(&db).await);
    let context = TrackingContext::new(Actor::client(DOCUMENT_CLIENT))
        .on_behalf_of(Actor::client(DOCUMENT_CLIENT));
    let mut tx = state.db.begin().await.unwrap();
    scope(
        context,
        state.tracker.track(&mut tx, &InstanceTicked { n: 1 }),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let rows = testing::tracked_events(&db, InstanceTicked::NAME).await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].envelope["actor"], json!({"type": "client"}));
    assert_eq!(rows[0].envelope["on_behalf_of"], json!({"type": "client"}));
    let everything: Vec<String> =
        sqlx::query_scalar("SELECT envelope::text || body::text FROM kernel.tracked_events")
            .fetch_all(&db)
            .await
            .unwrap();
    assert!(
        everything
            .iter()
            .all(|row| !row.contains("agent.example.com")),
        "{everything:?}"
    );
}
