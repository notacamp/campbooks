//! The change feed end to end: the composed API (the workspaces module as
//! the producer), the kernel's sequencer run by a worker, a test client
//! reading Server-Sent Events, a real Postgres (`#[sqlx::test]`) and an
//! in-process identity provider.

use std::{collections::BTreeSet, time::Duration};

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
};
use campbooks_kernel::{
    AppState, DomainEvent, RecordRef,
    changes::Change,
    config::ChangesConfig,
    jobs::Worker,
    operations,
    tenancy::UserId,
    testing::{self, TestIdp},
};
use campbooks_workspaces::events::MemberJoined;
use http_body_util::BodyExt as _;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt as _;

const ALL_SCOPES: &str = "workspaces:read workspaces:write workspaces:admin";

/// How long a test waits for a frame it expects.
const WAIT: Duration = Duration::from_secs(10);

/// One server process over the test database.
struct Replica {
    app: Router,
    state: AppState,
}

impl Replica {
    async fn new(db: &PgPool, idp: &TestIdp, tweak: impl FnOnce(&mut ChangesConfig)) -> Self {
        let mut config = testing::config("postgres://test@localhost/test");
        config.auth = idp.auth_config();
        config.changes.poll_interval_ms = 200;
        tweak(&mut config.changes);
        let pool = testing::app_role_pool(db).await;
        let blobs = campbooks_kernel::blobs::BlobStore::memory(
            &campbooks_kernel::config::BlobsConfig::default(),
            pool.clone(),
        );
        let state = crate::with_ports(
            AppState::new(pool, config)
                .with_blobs(blobs)
                .with_registry(crate::registry().expect("valid registrations")),
        );
        let operations::Composed { routes, state, .. } = operations::compose(crate::api(), state);
        let app = routes
            .fallback(|| async { campbooks_kernel::Error::NotFound })
            .with_state(state.clone());
        Self { app, state }
    }

    async fn call(
        &self,
        token: Option<&str>,
        method: &str,
        uri: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut request = Request::builder().method(method).uri(uri);
        if let Some(token) = token {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        let body = match body {
            Some(body) => {
                request = request.header(header::CONTENT_TYPE, "application/json");
                Body::from(body.to_string())
            }
            None => Body::empty(),
        };
        let response = self
            .app
            .clone()
            .oneshot(request.body(body).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    /// Open the feed of `workspace`; `Err` with the status when refused.
    async fn subscribe(
        &self,
        token: Option<&str>,
        workspace: &str,
        last_event_id: Option<&str>,
    ) -> Result<Feed, (StatusCode, Value)> {
        self.subscribe_at(
            token,
            &format!("/api/v1/w/{workspace}/changes"),
            last_event_id,
        )
        .await
    }

    /// Open the feed at `uri` (e.g. with `?after=`).
    async fn subscribe_at(
        &self,
        token: Option<&str>,
        uri: &str,
        last_event_id: Option<&str>,
    ) -> Result<Feed, (StatusCode, Value)> {
        let mut request = Request::get(uri);
        if let Some(token) = token {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        if let Some(id) = last_event_id {
            request = request.header("last-event-id", id);
        }
        let response = self
            .app
            .clone()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        if response.status() != StatusCode::OK {
            let status = response.status();
            let bytes = response.into_body().collect().await.unwrap().to_bytes();
            return Err((
                status,
                serde_json::from_slice(&bytes).unwrap_or(Value::Null),
            ));
        }
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "text/event-stream",
            "an SSE stream"
        );
        assert_eq!(response.headers()["x-accel-buffering"], "no");
        Ok(Feed {
            body: response.into_body(),
            buffer: String::new(),
            raw: String::new(),
        })
    }

    /// Run the sequencer (and anything else queued).
    async fn sequence(&self) {
        Worker::new(self.state.clone()).run_until_idle().await;
    }
}

/// One SSE frame.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Frame {
    event: Option<String>,
    id: Option<String>,
    data: Option<String>,
    comment: Option<String>,
}

impl Frame {
    fn json(&self) -> Value {
        serde_json::from_str(self.data.as_deref().unwrap_or("null")).unwrap()
    }

    fn is(&self, event: &str) -> bool {
        self.event.as_deref() == Some(event)
    }
}

/// A test client reading a stream frame by frame.
struct Feed {
    body: Body,
    buffer: String,
    /// Everything received, for "no personal data" checks.
    raw: String,
}

impl Feed {
    /// The next frame, `None` when the stream ended. Panics after [`WAIT`].
    async fn next(&mut self) -> Option<Frame> {
        tokio::time::timeout(WAIT, self.read())
            .await
            .unwrap_or_else(|_| panic!("no frame within {WAIT:?}; got so far: {}", self.raw))
    }

    async fn read(&mut self) -> Option<Frame> {
        loop {
            if let Some(end) = self.buffer.find("\n\n") {
                let block: String = self.buffer.drain(..end + 2).collect();
                return Some(parse(&block));
            }
            let frame = self.body.frame().await?.ok()?;
            if let Ok(bytes) = frame.into_data() {
                let text = String::from_utf8(bytes.to_vec()).unwrap();
                self.raw.push_str(&text);
                self.buffer.push_str(&text);
            }
        }
    }

    /// The next frame that is not a heartbeat comment.
    async fn next_event(&mut self) -> Frame {
        loop {
            let frame = self.next().await.expect("the stream is still open");
            if frame.comment.is_none() {
                return frame;
            }
        }
    }

    /// Read whatever arrives until nothing has for `window` (it stays in
    /// `raw`).
    async fn drain(&mut self, window: Duration) {
        while let Ok(Some(_)) = tokio::time::timeout(window, self.read()).await {}
    }

    /// Nothing but comments arrives within `window`.
    async fn quiet_for(&mut self, window: Duration) {
        let deadline = tokio::time::Instant::now() + window;
        loop {
            match tokio::time::timeout_at(deadline, self.read()).await {
                Err(_) => return,
                Ok(Some(frame)) if frame.comment.is_some() => {}
                Ok(other) => panic!("expected silence, got {other:?}"),
            }
        }
    }
}

fn parse(block: &str) -> Frame {
    let mut frame = Frame::default();
    for line in block.lines() {
        if let Some(comment) = line.strip_prefix(':') {
            frame.comment = Some(comment.trim().to_owned());
        } else if let Some((field, value)) = line.split_once(':') {
            let value = value.strip_prefix(' ').unwrap_or(value).to_owned();
            match field {
                "event" => frame.event = Some(value),
                "id" => frame.id = Some(value),
                "data" => frame.data = Some(value),
                _ => {}
            }
        }
    }
    frame
}

struct World {
    idp: TestIdp,
    db: PgPool,
    server: Replica,
}

impl World {
    async fn new(db: PgPool) -> Self {
        Self::with(db, |_| {}).await
    }

    async fn with(db: PgPool, tweak: impl FnOnce(&mut ChangesConfig)) -> Self {
        let idp = TestIdp::start().await;
        let server = Replica::new(&db, &idp, tweak).await;
        Self { idp, db, server }
    }

    fn token(&self, sub: &str, scopes: &str) -> String {
        let mut claims = self.idp.claims(scopes);
        claims["sub"] = sub.into();
        claims["email"] = format!("{sub}@example.com").into();
        claims["email_verified"] = true.into();
        claims["name"] = format!("Person {sub}").into();
        self.idp.token(&claims)
    }

    fn user(&self, sub: &str) -> String {
        self.token(sub, ALL_SCOPES)
    }

    async fn create_workspace(&self, token: &str, name: &str) -> String {
        let (status, body) = self
            .server
            .call(
                Some(token),
                "POST",
                "/api/v1/workspaces",
                Some(json!({ "name": name, "kind": "business" })),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        body["id"].as_str().unwrap().to_owned()
    }

    async fn user_id(&self, sub: &str) -> UserId {
        sqlx::query_scalar("SELECT id FROM workspaces.users WHERE idp_subject = $1")
            .bind(sub)
            .fetch_one(&self.db)
            .await
            .unwrap()
    }

    /// Make `sub` a member, as accepting an invitation does: the membership
    /// row and `MemberJoined`, in one transaction.
    async fn add_member(&self, workspace: &str, sub: &str, role: &str) -> (String, UserId) {
        let token = self.user(sub);
        let (status, _) = self
            .server
            .call(Some(&token), "GET", "/api/v1/workspaces", None)
            .await;
        assert_eq!(status, StatusCode::OK);
        let user = self.user_id(sub).await;
        let mut tx = self.db.begin().await.unwrap();
        campbooks_kernel::db::set_workspace(&mut tx, workspace)
            .await
            .unwrap();
        sqlx::query("INSERT INTO workspaces.members (workspace_id, user_id, role) VALUES ($1::uuid, $2, $3)")
            .bind(workspace)
            .bind(user)
            .bind(role)
            .execute(&mut *tx)
            .await
            .unwrap();
        self.server
            .state
            .events
            .publish(
                &mut tx,
                &MemberJoined {
                    workspace: workspace.to_owned(),
                    user,
                    role: role.parse().unwrap(),
                },
            )
            .await
            .unwrap();
        tx.commit().await.unwrap();
        (token, user)
    }

    async fn rename(&self, token: &str, workspace: &str, name: &str) {
        let (status, body) = self
            .server
            .call(
                Some(token),
                "PATCH",
                &format!("/api/v1/w/{workspace}"),
                Some(json!({ "name": name })),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
}

/// Every key a change frame may carry, and nothing else.
fn assert_ids_only(frame: &Frame) {
    let data = frame.json();
    let keys: BTreeSet<&str> = data
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, BTreeSet::from(["cursor", "op", "record", "version"]));
    let record: BTreeSet<&str> = data["record"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(record, BTreeSet::from(["id", "type"]));
    assert_eq!(
        frame.id.as_deref(),
        data["cursor"].as_str(),
        "the SSE id is the cursor"
    );
}

fn record_of(frame: &Frame) -> (String, String, String) {
    let data = frame.json();
    (
        data["record"]["type"].as_str().unwrap().to_owned(),
        data["record"]["id"].as_str().unwrap().to_owned(),
        data["op"].as_str().unwrap().to_owned(),
    )
}

// --- the feed ----------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn members_follow_renames_joins_role_changes_and_departures(db: PgPool) {
    let world = World::new(db).await;
    let ana = world.user("ana");
    let ws = world.create_workspace(&ana, "Corner shop").await;
    world.server.sequence().await;

    let mut feed = world.server.subscribe(Some(&ana), &ws, None).await.unwrap();
    let ready = feed.next_event().await;
    assert!(ready.is("ready"), "{ready:?}");
    assert_eq!(
        ready.json()["cursor"],
        "2",
        "starts at the head: created + owner joined"
    );
    assert_eq!(ready.id.as_deref(), Some("2"));

    world.rename(&ana, &ws, "Bakery Lisboa").await;
    world.server.sequence().await;
    let renamed = feed.next_event().await;
    assert!(renamed.is("change"));
    assert_ids_only(&renamed);
    assert_eq!(
        record_of(&renamed),
        ("workspaces.workspace".into(), ws.clone(), "upsert".into())
    );
    assert_eq!(renamed.id.as_deref(), Some("3"));

    let (_, bea) = world.add_member(&ws, "bea", "member").await;
    world.server.sequence().await;
    let joined = feed.next_event().await;
    assert_ids_only(&joined);
    assert_eq!(
        record_of(&joined),
        ("workspaces.member".into(), bea.to_string(), "upsert".into())
    );

    let (status, _) = world
        .server
        .call(
            Some(&ana),
            "PATCH",
            &format!("/api/v1/w/{ws}/members/{bea}"),
            Some(json!({ "role": "admin" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    world.server.sequence().await;
    assert_eq!(
        record_of(&feed.next_event().await),
        ("workspaces.member".into(), bea.to_string(), "upsert".into())
    );

    let (status, _) = world
        .server
        .call(
            Some(&ana),
            "DELETE",
            &format!("/api/v1/w/{ws}/members/{bea}"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    world.server.sequence().await;
    let left = feed.next_event().await;
    assert_eq!(
        record_of(&left),
        ("workspaces.member".into(), bea.to_string(), "delete".into())
    );
    assert_eq!(
        left.id.as_deref(),
        Some("6"),
        "one cursor per change, no gap"
    );

    // No names, addresses or the workspace's name ever crossed the wire.
    for secret in ["Corner shop", "Bakery", "example.com", "Person"] {
        assert!(
            !feed.raw.contains(secret),
            "`{secret}` leaked: {}",
            feed.raw
        );
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_client_resumes_after_a_disconnect_in_order(db: PgPool) {
    let world = World::new(db).await;
    let ana = world.user("ana");
    let ws = world.create_workspace(&ana, "Shop").await;
    world.server.sequence().await;

    let mut feed = world.server.subscribe(Some(&ana), &ws, None).await.unwrap();
    let cursor = feed.next_event().await.id.unwrap();
    drop(feed);

    // While it is away: three different records change.
    world.rename(&ana, &ws, "Shop two").await;
    let (_, bea) = world.add_member(&ws, "bea", "member").await;
    let (_, cid) = world.add_member(&ws, "cid", "member").await;
    world.server.sequence().await;

    let mut feed = world
        .server
        .subscribe(Some(&ana), &ws, Some(&cursor))
        .await
        .unwrap();
    assert!(feed.next_event().await.is("ready"));
    let missed: Vec<(String, String, String)> = vec![
        record_of(&feed.next_event().await),
        record_of(&feed.next_event().await),
        record_of(&feed.next_event().await),
    ];
    assert_eq!(
        missed,
        vec![
            ("workspaces.workspace".into(), ws.clone(), "upsert".into()),
            ("workspaces.member".into(), bea.to_string(), "upsert".into()),
            ("workspaces.member".into(), cid.to_string(), "upsert".into()),
        ],
        "every missed change, in order"
    );
    // …then live.
    world.rename(&ana, &ws, "Shop three").await;
    world.server.sequence().await;
    assert_eq!(feed.next_event().await.id.as_deref(), Some("6"));

    // `?after=` works for clients that cannot send the header…
    let uri = format!("/api/v1/w/{ws}/changes?after=4");
    let mut feed = world
        .server
        .subscribe_at(Some(&ana), &uri, None)
        .await
        .unwrap();
    assert_eq!(feed.next_event().await.id.as_deref(), Some("4"));
    assert_eq!(feed.next_event().await.id.as_deref(), Some("5"));
    // …and the header wins over it.
    let mut feed = world
        .server
        .subscribe_at(Some(&ana), &uri, Some("5"))
        .await
        .unwrap();
    assert_eq!(feed.next_event().await.id.as_deref(), Some("5"));
    assert_eq!(feed.next_event().await.id.as_deref(), Some("6"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn unknown_and_expired_cursors_reset(db: PgPool) {
    let world = World::new(db).await;
    let ana = world.user("ana");
    let ws = world.create_workspace(&ana, "Shop").await;
    world.rename(&ana, &ws, "Shop two").await;
    world.server.sequence().await;

    for cursor in ["99", "nonsense", "-3"] {
        let mut feed = world
            .server
            .subscribe(Some(&ana), &ws, Some(cursor))
            .await
            .unwrap();
        let reset = feed.next_event().await;
        assert!(reset.is("reset"), "{cursor}: {reset:?}");
        assert_eq!(reset.json()["reason"], "unknown_cursor");
        assert_eq!(reset.json()["cursor"], "3");
        assert_eq!(reset.id.as_deref(), Some("3"));
    }

    // The first changes are pruned: resuming from before them resets.
    sqlx::query("DELETE FROM kernel.changes WHERE workspace_id = $1 AND seq <= 2")
        .bind(&ws)
        .execute(&world.db)
        .await
        .unwrap();
    let mut feed = world
        .server
        .subscribe(Some(&ana), &ws, Some("0"))
        .await
        .unwrap();
    let reset = feed.next_event().await;
    assert_eq!(reset.json()["reason"], "cursor_expired");
    // From 2 nothing is missing.
    let mut feed = world
        .server
        .subscribe(Some(&ana), &ws, Some("2"))
        .await
        .unwrap();
    assert!(feed.next_event().await.is("ready"));
    assert_eq!(feed.next_event().await.id.as_deref(), Some("3"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_client_too_far_behind_resets(db: PgPool) {
    let world = World::with(db, |c| c.max_backlog = 2).await;
    let ana = world.user("ana");
    let ws = world.create_workspace(&ana, "Shop").await;
    world.rename(&ana, &ws, "Shop two").await;
    world.server.sequence().await;
    let mut feed = world
        .server
        .subscribe(Some(&ana), &ws, Some("0"))
        .await
        .unwrap();
    let reset = feed.next_event().await;
    assert_eq!(reset.json()["reason"], "too_far_behind");
}

// --- access ------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn only_members_with_the_scope_may_subscribe(db: PgPool) {
    let world = World::new(db).await;
    let ana = world.user("ana");
    let ws = world.create_workspace(&ana, "Shop").await;

    let (status, problem) = world.server.subscribe(None, &ws, None).await.err().unwrap();
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{problem}");
    let outsider = world.user("zoe");
    let (status, _) = world
        .server
        .subscribe(Some(&outsider), &ws, None)
        .await
        .err()
        .unwrap();
    assert_eq!(status, StatusCode::NOT_FOUND, "a non-member learns nothing");
    let unscoped = world.token("ana", "workspaces:write");
    let (status, _) = world
        .server
        .subscribe(Some(&unscoped), &ws, None)
        .await
        .err()
        .unwrap();
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = world
        .server
        .subscribe(Some(&ana), "not-a-uuid", None)
        .await
        .err()
        .unwrap();
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// A change to a record a subscriber's scopes cannot read, to a record no
/// read operation returns, and one inside a container.
#[derive(Debug, Serialize, Deserialize)]
struct Hidden {
    workspace: String,
}

impl DomainEvent for Hidden {
    const NAME: &'static str = "test.hidden";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }

    fn changes(&self) -> Vec<Change> {
        vec![
            // Read by `workspaces.list_invitations`, scope `workspaces:admin`.
            Change::upsert(RecordRef::new("workspaces.invitation", "inv-1")),
            // No read operation returns it.
            Change::upsert(RecordRef::new("nowhere.thing", "x")),
            // Inside a container; no module answers for containers yet.
            Change::upsert(RecordRef::new("workspaces.member", "m-1"))
                .within(RecordRef::new("workspaces.team", "t-1")),
        ]
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn changes_the_caller_may_not_see_are_skipped_but_advance_the_cursor(db: PgPool) {
    let world = World::with(db, |c| c.heartbeat_seconds = 1).await;
    let ana = world.user("ana");
    let ws = world.create_workspace(&ana, "Shop").await;
    world.server.sequence().await;
    let reader = world.token("ana", "workspaces:read");
    let mut feed = world
        .server
        .subscribe(Some(&reader), &ws, None)
        .await
        .unwrap();
    assert!(feed.next_event().await.is("ready"));

    let mut tx = world.db.begin().await.unwrap();
    campbooks_kernel::db::set_workspace(&mut tx, &ws)
        .await
        .unwrap();
    world
        .server
        .state
        .events
        .publish(
            &mut tx,
            &Hidden {
                workspace: ws.clone(),
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    world.server.sequence().await;

    // No change frame; the heartbeat moves the client's cursor past them.
    let heartbeat = feed.next_event().await;
    assert!(heartbeat.is("heartbeat"), "{heartbeat:?}");
    assert_eq!(heartbeat.json()["cursor"], "5");
    assert_eq!(heartbeat.id.as_deref(), Some("5"));

    // An admin-scoped token does see the invitation (and only that).
    let mut admin = world
        .server
        .subscribe(Some(&ana), &ws, Some("2"))
        .await
        .unwrap();
    assert!(admin.next_event().await.is("ready"));
    assert_eq!(
        record_of(&admin.next_event().await),
        (
            "workspaces.invitation".into(),
            "inv-1".into(),
            "upsert".into()
        )
    );
    admin.quiet_for(Duration::from_millis(500)).await;
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_removed_member_is_revoked_and_then_refused(db: PgPool) {
    let world = World::new(db).await;
    let ana = world.user("ana");
    let ws = world.create_workspace(&ana, "Shop").await;
    let (bea_token, bea) = world.add_member(&ws, "bea", "member").await;
    world.server.sequence().await;

    let mut feed = world
        .server
        .subscribe(Some(&bea_token), &ws, None)
        .await
        .unwrap();
    assert!(feed.next_event().await.is("ready"));
    let (status, _) = world
        .server
        .call(
            Some(&ana),
            "DELETE",
            &format!("/api/v1/w/{ws}/members/{bea}"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    world.server.sequence().await;

    // Membership is resolved again before the page is shown: a former
    // member gets nothing more, not even the change that removed her.
    assert!(feed.next_event().await.is("revoked"));
    assert!(feed.next().await.is_none(), "the stream ends");
    let (status, _) = world
        .server
        .subscribe(Some(&bea_token), &ws, None)
        .await
        .err()
        .unwrap();
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// --- scale, limits, liveness ---------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn many_subscribers_each_receive_every_change_once(db: PgPool) {
    let world = World::with(db, |c| c.max_streams_per_principal = 64).await;
    let ana = world.user("ana");
    let ws = world.create_workspace(&ana, "Shop").await;
    world.server.sequence().await;
    let mut feeds = Vec::new();
    for _ in 0..40 {
        let mut feed = world.server.subscribe(Some(&ana), &ws, None).await.unwrap();
        assert!(feed.next_event().await.is("ready"));
        feeds.push(feed);
    }
    world.rename(&ana, &ws, "Shop two").await;
    world.server.sequence().await;
    let received = futures_util::future::join_all(feeds.iter_mut().map(|feed| async {
        let frame = feed.next_event().await;
        feed.quiet_for(Duration::from_millis(300)).await;
        frame
    }))
    .await;
    for frame in received {
        assert_eq!(frame.id.as_deref(), Some("3"));
    }
    // Forty streams were mid-query a moment ago: close them and their pool
    // before the test database is dropped.
    drop(feeds);
    world.server.state.db.close().await;
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_caller_holds_a_limited_number_of_streams(db: PgPool) {
    let world = World::with(db, |c| c.max_streams_per_principal = 2).await;
    let ana = world.user("ana");
    let ws = world.create_workspace(&ana, "Shop").await;
    let first = world.server.subscribe(Some(&ana), &ws, None).await.unwrap();
    let _second = world.server.subscribe(Some(&ana), &ws, None).await.unwrap();
    let (status, problem) = world
        .server
        .subscribe(Some(&ana), &ws, None)
        .await
        .err()
        .unwrap();
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(problem["code"], "too_many_streams", "{problem}");
    drop(first);
    assert!(world.server.subscribe(Some(&ana), &ws, None).await.is_ok());
}

#[sqlx::test(migrations = "../../migrations")]
async fn idle_streams_beat_and_end_cleanly(db: PgPool) {
    let world = World::with(db, |c| {
        c.heartbeat_seconds = 1;
        c.max_connection_seconds = 3;
    })
    .await;
    let ana = world.user("ana");
    let ws = world.create_workspace(&ana, "Shop").await;
    let mut feed = world.server.subscribe(Some(&ana), &ws, None).await.unwrap();
    assert!(feed.next().await.unwrap().is("ready"));
    let beat = feed.next().await.unwrap();
    assert_eq!(beat.comment.as_deref(), Some("heartbeat"));
    let closing = feed.next_event().await;
    assert!(closing.is("closing"));
    assert_eq!(closing.json()["reason"], "max_age");
    assert!(feed.next().await.is_none(), "the body ends");
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_stream_ends_when_the_access_token_expires(db: PgPool) {
    let world = World::new(db).await;
    let ana = world.user("ana");
    let ws = world.create_workspace(&ana, "Shop").await;
    let mut claims = world.idp.claims(ALL_SCOPES);
    claims["sub"] = "ana".into();
    claims["exp"] = (jsonwebtoken_now() + 2).into();
    let short = world.idp.token(&claims);
    let mut feed = world
        .server
        .subscribe(Some(&short), &ws, None)
        .await
        .unwrap();
    assert!(feed.next_event().await.is("ready"));
    let closing = feed.next_event().await;
    assert_eq!(closing.json()["reason"], "token_expired");
}

fn jsonwebtoken_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[sqlx::test(migrations = "../../migrations")]
async fn cursors_mean_the_same_on_every_replica(db: PgPool) {
    let world = World::new(db).await;
    let other = Replica::new(&world.db, &world.idp, |c| c.poll_interval_ms = 200).await;
    let ana = world.user("ana");
    let ws = world.create_workspace(&ana, "Shop").await;
    world.server.sequence().await;

    let mut here = world.server.subscribe(Some(&ana), &ws, None).await.unwrap();
    let cursor = here.next_event().await.id.unwrap();
    drop(here);
    world.rename(&ana, &ws, "Shop two").await;
    // The other process sequences and serves.
    other.sequence().await;
    let mut there = other
        .subscribe(Some(&ana), &ws, Some(&cursor))
        .await
        .unwrap();
    assert!(there.next_event().await.is("ready"));
    let frame = there.next_event().await;
    assert_eq!(frame.id.as_deref(), Some("3"));

    // A change sequenced by one process wakes streams on the other.
    let mut live = other.subscribe(Some(&ana), &ws, Some("3")).await.unwrap();
    assert!(live.next_event().await.is("ready"));
    world.rename(&ana, &ws, "Shop three").await;
    world.server.sequence().await;
    assert_eq!(live.next_event().await.id.as_deref(), Some("4"));
}

// --- inboxes and private records (campbooks-cloud#117) ---------------------------

/// Scopes that read every record type these tests change.
const READ_ALL: &str = "workspaces:read conversations:read conversations:send connectors:read";

const EN: &str = include_str!("../../../modules/conversations/fixtures/en.json");
const FR: &str = include_str!("../../../modules/conversations/fixtures/fr.json");

impl World {
    async fn ingest(
        &self,
        workspace: &str,
        inbox: campbooks_conversations::model::InboxId,
        json: &str,
    ) {
        let workspace = workspace.parse().unwrap();
        let recorded: Value = serde_json::from_str(json).unwrap();
        let batch =
            campbooks_conversations::dev::record(&self.server.state, workspace, inbox, &recorded)
                .await
                .unwrap();
        campbooks_conversations::dev::submit(&self.server.state, workspace, &batch)
            .await
            .unwrap();
        self.server.sequence().await;
    }

    async fn publish<E: campbooks_kernel::DomainEvent>(&self, event: &E) {
        let mut tx = self.db.begin().await.unwrap();
        if let Some(workspace) = campbooks_kernel::DomainEvent::workspace(event) {
            campbooks_kernel::db::set_workspace(&mut tx, workspace)
                .await
                .unwrap();
        }
        self.server
            .state
            .events
            .publish(&mut tx, event)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        self.server.sequence().await;
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_member_taken_out_of_an_inbox_stops_getting_its_ids(db: PgPool) {
    use campbooks_conversations::{dev, events::InboxChanged, model::InboxRole};

    let world = World::new(db).await;
    let ana = world.user("ana");
    let ws = world.create_workspace(&ana, "Shop").await;
    let ana_id = world.user_id("ana").await;
    let (_, bea_id) = world.add_member(&ws, "bea", "member").await;
    let inbox = dev::create_inbox(
        &world.server.state,
        ws.parse().unwrap(),
        "ana@example.com",
        &[(ana_id, InboxRole::Manager), (bea_id, InboxRole::Reader)],
    )
    .await
    .unwrap();
    world.server.sequence().await;
    let bea = world.token("bea", READ_ALL);
    let mut feed = world.server.subscribe(Some(&bea), &ws, None).await.unwrap();
    assert!(feed.next_event().await.is("ready"));

    world.ingest(&ws, inbox, EN).await;
    let (record_type, _, _) = record_of(&feed.next_event().await);
    assert!(record_type.starts_with("conversations."), "{record_type}");
    // Drain what the first batch announced.
    feed.drain(Duration::from_millis(300)).await;

    // Bea is taken out of the inbox; its next mail must not reach her.
    sqlx::query("DELETE FROM conversations.inbox_members WHERE user_id = $1")
        .bind(bea_id.as_uuid())
        .execute(&world.db)
        .await
        .unwrap();
    world
        .publish(&InboxChanged {
            workspace: ws.clone(),
            inbox,
        })
        .await;
    world.ingest(&ws, inbox, FR).await;
    let reset = feed.next_event().await;
    assert!(reset.is("reset"), "{reset:?}");
    assert_eq!(reset.json()["reason"], "access_changed");
    let seen = feed.raw.clone();
    feed.drain(Duration::from_millis(500)).await;
    let new_messages: Vec<String> = sqlx::query_scalar(
        "SELECT id::text FROM conversations.messages WHERE provider_message_id LIKE 'fr-%'",
    )
    .fetch_all(&world.db)
    .await
    .unwrap();
    assert!(!new_messages.is_empty());
    for id in new_messages {
        assert!(
            !feed.raw.contains(&id),
            "{id} reached a former inbox member"
        );
        assert!(!seen.contains(&id));
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn drafts_and_connections_reach_only_who_may_see_them(db: PgPool) {
    use campbooks_connectors::{ConnectionId, Provider, events::ConnectionAdded};
    use campbooks_conversations::{
        dev,
        events::DraftSent,
        model::{DraftId, InboxRole},
    };

    let world = World::new(db).await;
    let ana = world.user("ana");
    let ws = world.create_workspace(&ana, "Shop").await;
    let ana_id = world.user_id("ana").await;
    let (_, bea_id) = world.add_member(&ws, "bea", "member").await;
    world.add_member(&ws, "cid", "admin").await;
    world.add_member(&ws, "dan", "member").await;
    let inbox = dev::create_inbox(
        &world.server.state,
        ws.parse().unwrap(),
        "ana@example.com",
        &[(ana_id, InboxRole::Manager), (bea_id, InboxRole::Sender)],
    )
    .await
    .unwrap();
    world.server.sequence().await;
    let mut feeds = Vec::new();
    for sub in ["ana", "bea", "cid", "dan"] {
        let token = world.token(sub, READ_ALL);
        let mut feed = world
            .server
            .subscribe(Some(&token), &ws, None)
            .await
            .unwrap();
        assert!(feed.next_event().await.is("ready"));
        feeds.push(feed);
    }
    let draft = DraftId::new();
    world
        .publish(&DraftSent {
            workspace: ws.clone(),
            inbox,
            draft,
            conversation: None,
            author: Some(bea_id),
        })
        .await;
    let connection = ConnectionId::new();
    world
        .publish(&ConnectionAdded {
            workspace: ws.clone(),
            connection,
            provider: Provider::Google,
            granted_by: bea_id,
        })
        .await;
    let [ana_feed, bea_feed, cid_feed, dan_feed] = &mut feeds[..] else {
        unreachable!()
    };
    // Bea: her draft, then her connection.
    assert_eq!(
        record_of(&bea_feed.next_event().await),
        (
            "conversations.draft".into(),
            draft.to_string(),
            "delete".into()
        )
    );
    let connection_change = (
        "connectors.connection".to_owned(),
        connection.to_string(),
        "upsert".to_owned(),
    );
    assert_eq!(record_of(&bea_feed.next_event().await), connection_change);
    // The inbox's manager and the workspace's owner and admins see the
    // connection, never another member's draft.
    for feed in [ana_feed, cid_feed] {
        assert_eq!(record_of(&feed.next_event().await), connection_change);
        feed.drain(Duration::from_millis(300)).await;
        assert!(!feed.raw.contains(&draft.to_string()));
    }
    // Another member sees neither.
    dan_feed.drain(Duration::from_millis(500)).await;
    assert!(!dan_feed.raw.contains(&draft.to_string()));
    assert!(!dan_feed.raw.contains(&connection.to_string()));
}
