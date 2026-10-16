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
    auth::{StandingFuture, TokenRevocation, TokenStanding},
    changes::Change,
    config::ChangesConfig,
    jobs::Worker,
    operations,
    tenancy::{ClientActor, UserId},
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

/// Registered clients: each acts for a person, until removed.
#[derive(Debug, Default)]
struct Agents(std::sync::Mutex<std::collections::HashMap<String, ClientActor>>);

impl Agents {
    fn set(&self, client: &str, actor: ClientActor) {
        self.0.lock().unwrap().insert(client.to_owned(), actor);
    }
}

impl campbooks_kernel::tenancy::ClientDirectory for Agents {
    fn acting_for<'a>(
        &'a self,
        _db: &'a PgPool,
        client_id: &'a str,
        _workspace: campbooks_kernel::tenancy::WorkspaceId,
    ) -> campbooks_kernel::tenancy::ClientsFuture<'a> {
        let actor = self
            .0
            .lock()
            .unwrap()
            .get(client_id)
            .cloned()
            .unwrap_or(ClientActor::Unregistered);
        Box::pin(async move { Ok(actor) })
    }
}

/// One server process over the test database.
struct Replica {
    app: Router,
    state: AppState,
    agents: std::sync::Arc<Agents>,
}

impl Replica {
    async fn new(db: &PgPool, idp: &TestIdp, tweak: impl FnOnce(&mut ChangesConfig)) -> Self {
        Self::revocable(db, idp, tweak, None).await
    }

    /// A replica whose verifier also asks `revocation` about people, as the
    /// server's in-process issuer does.
    async fn revocable(
        db: &PgPool,
        idp: &TestIdp,
        tweak: impl FnOnce(&mut ChangesConfig),
        revocation: Option<campbooks_kernel::auth::TokenRevocationPort>,
    ) -> Self {
        let mut config = testing::config("postgres://test@localhost/test");
        config.auth = idp.auth_config();
        config.changes.poll_interval_ms = 200;
        tweak(&mut config.changes);
        let pool = testing::app_role_pool(db).await;
        let blobs = campbooks_kernel::blobs::BlobStore::memory(
            &campbooks_kernel::config::BlobsConfig::default(),
            pool.clone(),
        );
        let agents = std::sync::Arc::new(Agents::default());
        let state = crate::with_ports(
            AppState::new(pool, config)
                .with_blobs(blobs)
                // One instance secret, as every process of an install has.
                .with_signer(campbooks_kernel::crypto::Signer::new(
                    &campbooks_kernel::crypto::SecretKey::from_bytes([7; 32]),
                ))
                .with_registry(crate::registry().expect("valid registrations")),
        )
        .with_client_directory(agents.clone());
        let state = match revocation {
            Some(port) => {
                let auth = state.auth.clone().with_revocation(state.db.clone(), port);
                state.with_auth(auth)
            }
            None => state,
        };
        let operations::Composed { routes, state, .. } = operations::compose(crate::api(), state);
        let app = routes
            .fallback(|| async { campbooks_kernel::Error::NotFound })
            .with_state(state.clone());
        Self { app, state, agents }
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

    /// A heartbeat (or any SSE comment, which the server no longer sends).
    fn is_beat(&self) -> bool {
        self.comment.is_some() || self.is("heartbeat")
    }

    /// The frame's resume token: its SSE `id`, which its `cursor` repeats.
    fn token(&self) -> String {
        let id = self.id.clone().expect("the frame carries a resume token");
        assert_eq!(
            self.json()["cursor"].as_str(),
            Some(id.as_str()),
            "the SSE id is the cursor"
        );
        id
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

    /// The next frame that is not a heartbeat.
    async fn next_event(&mut self) -> Frame {
        loop {
            let frame = self.next().await.expect("the stream is still open");
            if !frame.is_beat() {
                return frame;
            }
        }
    }

    /// The next heartbeat; panics on any other frame.
    async fn next_beat(&mut self) -> Frame {
        let frame = self.next().await.expect("the stream is still open");
        assert!(frame.is("heartbeat"), "expected a heartbeat, got {frame:?}");
        frame
    }

    /// Read whatever arrives until nothing has for `window` (it stays in
    /// `raw`).
    async fn drain(&mut self, window: Duration) {
        while let Ok(Some(_)) = tokio::time::timeout(window, self.read()).await {}
    }

    /// Nothing but heartbeats arrives within `window`.
    async fn quiet_for(&mut self, window: Duration) {
        let deadline = tokio::time::Instant::now() + window;
        loop {
            match tokio::time::timeout_at(deadline, self.read()).await {
                Err(_) => return,
                Ok(Some(frame)) if frame.is_beat() => {}
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

    /// A token of client `id`, bound to `workspace`.
    fn client(&self, id: &str, workspace: &str) -> String {
        let mut claims = self.idp.claims(ALL_SCOPES);
        let object = claims.as_object_mut().unwrap();
        object.remove("sub");
        object.insert("azp".into(), id.into());
        object.insert("workspace_id".into(), workspace.into());
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
    assert_opaque(&frame.token());
}

/// A resume token: sealed, one length, never a number.
fn assert_opaque(token: &str) {
    assert_eq!(token.len(), 60, "{token}");
    assert!(token.parse::<i64>().is_err(), "{token}");
    assert!(
        token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
        "URL-safe: {token}"
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
        ready.json().as_object().unwrap().keys().collect::<Vec<_>>(),
        vec!["cursor"],
        "no head: it would count changes the caller may not see"
    );
    assert_opaque(&ready.token());
    let mut tokens = BTreeSet::from([ready.token()]);

    world.rename(&ana, &ws, "Bakery Lisboa").await;
    world.server.sequence().await;
    let renamed = feed.next_event().await;
    assert!(renamed.is("change"));
    assert_ids_only(&renamed);
    assert_eq!(
        record_of(&renamed),
        ("workspaces.workspace".into(), ws.clone(), "upsert".into())
    );
    tokens.insert(renamed.token());

    let (_, bea) = world.add_member(&ws, "bea", "member").await;
    world.server.sequence().await;
    let joined = feed.next_event().await;
    assert_ids_only(&joined);
    tokens.insert(joined.token());
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
    tokens.insert(left.token());
    assert_eq!(tokens.len(), 4, "a new token on every frame");

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
    let cursor = feed.next_event().await.token();
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
    let missed: Vec<Frame> = vec![
        feed.next_event().await,
        feed.next_event().await,
        feed.next_event().await,
    ];
    let after_rename = missed[0].token();
    let after_bea = missed[1].token();
    let missed: Vec<(String, String, String)> = missed.iter().map(record_of).collect();
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
    let renamed = ("workspaces.workspace".into(), ws.clone(), "upsert".into());
    assert_eq!(record_of(&feed.next_event().await), renamed);

    // `?after=` works for clients that cannot send the header…
    let uri = format!("/api/v1/w/{ws}/changes?after={after_rename}");
    let mut feed = world
        .server
        .subscribe_at(Some(&ana), &uri, None)
        .await
        .unwrap();
    let ready = feed.next_event().await;
    assert!(ready.is("ready"));
    assert_ne!(ready.token(), after_rename, "sealed afresh");
    assert_eq!(record_of(&feed.next_event().await).1, bea.to_string());
    assert_eq!(record_of(&feed.next_event().await).1, cid.to_string());
    // …and the header wins over it.
    let mut feed = world
        .server
        .subscribe_at(Some(&ana), &uri, Some(&after_bea))
        .await
        .unwrap();
    assert!(feed.next_event().await.is("ready"));
    assert_eq!(record_of(&feed.next_event().await).1, cid.to_string());
    assert_eq!(record_of(&feed.next_event().await), renamed);
}

#[sqlx::test(migrations = "../../migrations")]
async fn unknown_and_expired_tokens_reset(db: PgPool) {
    let world = World::with(db, |c| c.retention_hours = 1).await;
    let ana = world.user("ana");
    let ws = world.create_workspace(&ana, "Shop").await;
    world.server.sequence().await;
    let mut feed = world.server.subscribe(Some(&ana), &ws, None).await.unwrap();
    let at_head = feed.next_event().await.token();
    drop(feed);
    let other = world.create_workspace(&ana, "Other shop").await;
    world.server.sequence().await;
    let mut feed = world
        .server
        .subscribe(Some(&ana), &other, None)
        .await
        .unwrap();
    let elsewhere = feed.next_event().await.token();
    drop(feed);
    let mut altered = at_head.clone().into_bytes();
    altered[30] = if altered[30] == b'A' { b'B' } else { b'A' };
    let altered = String::from_utf8(altered).unwrap();

    // Numeric cursors from before tokens, junk, another workspace's token,
    // an altered one: refetch, then go on from a fresh token.
    for token in ["2", "99", "nonsense", "-3", &elsewhere, &altered] {
        let mut feed = world
            .server
            .subscribe(Some(&ana), &ws, Some(token))
            .await
            .unwrap();
        let reset = feed.next_event().await;
        assert!(reset.is("reset"), "{token}: {reset:?}");
        assert_eq!(reset.json()["reason"], "unknown_cursor");
        assert_opaque(&reset.token());
    }

    // A change published two hours ago, delivered now: its token is as old
    // as the change, older than the retention, and resets.
    world.rename(&ana, &ws, "Shop two").await;
    sqlx::query(
        "UPDATE kernel.changes SET occurred_at = now() - interval '2 hours'
          WHERE workspace_id = $1 AND seq IS NULL",
    )
    .bind(&ws)
    .execute(&world.db)
    .await
    .unwrap();
    let mut feed = world
        .server
        .subscribe(Some(&ana), &ws, Some(&at_head))
        .await
        .unwrap();
    assert!(feed.next_event().await.is("ready"));
    world.server.sequence().await;
    let old = feed.next_event().await.token();
    drop(feed);
    let mut feed = world
        .server
        .subscribe(Some(&ana), &ws, Some(&old))
        .await
        .unwrap();
    let reset = feed.next_event().await;
    assert_eq!(reset.json()["reason"], "cursor_expired");

    // Changes after a young token were pruned anyway (a numbering that
    // stalled): reset rather than skip them.
    sqlx::query("DELETE FROM kernel.changes WHERE workspace_id = $1")
        .bind(&ws)
        .execute(&world.db)
        .await
        .unwrap();
    let mut feed = world
        .server
        .subscribe(Some(&ana), &ws, Some(&at_head))
        .await
        .unwrap();
    assert_eq!(feed.next_event().await.json()["reason"], "cursor_expired");
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_long_hidden_backlog_never_resets_a_resume(db: PgPool) {
    let world = World::new(db).await;
    let ana = world.user("ana");
    let ws = world.create_workspace(&ana, "Shop").await;
    world.server.sequence().await;
    // A reader without `workspaces:admin` sees no invitation.
    let reader = world.token("ana", "workspaces:read");
    let mut feed = world
        .server
        .subscribe(Some(&reader), &ws, None)
        .await
        .unwrap();
    let token = feed.next_event().await.token();
    drop(feed);
    // More hidden changes than a page, then one it may see.
    let mut tx = world.db.begin().await.unwrap();
    campbooks_kernel::db::set_workspace(&mut tx, &ws)
        .await
        .unwrap();
    for n in 0..450 {
        world
            .server
            .state
            .events
            .publish(
                &mut tx,
                &Busy {
                    workspace: ws.clone(),
                    n,
                },
            )
            .await
            .unwrap();
    }
    tx.commit().await.unwrap();
    world.rename(&ana, &ws, "Shop two").await;
    world.server.sequence().await;
    let mut feed = world
        .server
        .subscribe(Some(&reader), &ws, Some(&token))
        .await
        .unwrap();
    assert!(feed.next_event().await.is("ready"));
    assert_eq!(
        record_of(&feed.next_event().await),
        ("workspaces.workspace".into(), ws.clone(), "upsert".into())
    );
    feed.quiet_for(Duration::from_millis(300)).await;
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
async fn changes_the_caller_may_not_see_are_skipped_and_a_heartbeat_resumes_past_them(db: PgPool) {
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
    let ready = feed.next_event().await;
    assert!(ready.is("ready"));

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

    // No change frame, only the heartbeat it would have had anyway; its
    // token resumes past them.
    let heartbeat = feed.next_beat().await;
    assert_opaque(&heartbeat.token());
    let mut resumed = world
        .server
        .subscribe(Some(&reader), &ws, Some(&heartbeat.token()))
        .await
        .unwrap();
    assert!(resumed.next_event().await.is("ready"));
    resumed.quiet_for(Duration::from_millis(500)).await;

    // An admin-scoped token does see the invitation (and only that).
    let mut admin = world
        .server
        .subscribe(Some(&ana), &ws, Some(&ready.token()))
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

/// People's standings as a test sets them (absent: in good standing).
#[derive(Debug, Default)]
struct Standings(std::sync::Mutex<std::collections::HashMap<String, TokenStanding>>);

impl TokenRevocation for Standings {
    fn standing<'a>(&'a self, _db: &'a PgPool, account: &'a str) -> StandingFuture<'a> {
        let standing = self
            .0
            .lock()
            .unwrap()
            .get(account)
            .copied()
            .unwrap_or_default();
        Box::pin(async move { Ok(Some(standing)) })
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_stream_ends_within_a_heartbeat_when_its_tokens_are_revoked(db: PgPool) {
    let idp = TestIdp::start().await;
    let standings = std::sync::Arc::new(Standings::default());
    let server = Replica::revocable(
        &db,
        &idp,
        |c| c.heartbeat_seconds = 1,
        Some(standings.clone()),
    )
    .await;
    let world = World { idp, db, server };
    let ana = world.user("ana");
    let ws = world.create_workspace(&ana, "Shop").await;
    world.server.sequence().await;
    let mut feed = world.server.subscribe(Some(&ana), &ws, None).await.unwrap();
    assert!(feed.next_event().await.is("ready"));

    // Ana signs out everywhere (or is suspended): idle, the stream says
    // `revoked` at its next heartbeat and ends.
    standings.0.lock().unwrap().insert(
        "ana".to_owned(),
        TokenStanding {
            suspended: false,
            tokens_not_before: Some(chrono::Utc::now()),
        },
    );
    // Heartbeat comments keep coming until then: bound the wait.
    let revoked = tokio::time::timeout(Duration::from_secs(3), feed.next_event())
        .await
        .unwrap_or_else(|_| panic!("not revoked within a heartbeat: {}", feed.raw));
    assert!(revoked.is("revoked"), "{revoked:?}");
    assert!(feed.next().await.is_none(), "the stream ends");
    let (status, body) = world
        .server
        .subscribe(Some(&ana), &ws, None)
        .await
        .err()
        .unwrap();
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"], "invalid_token");
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

/// A change every member with `workspaces:admin` sees, and that changes
/// nobody's membership: keeps a stream busy.
#[derive(Debug, Serialize, Deserialize)]
struct Busy {
    workspace: String,
    n: u32,
}

impl DomainEvent for Busy {
    const NAME: &'static str = "test.busy";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }

    fn changes(&self) -> Vec<Change> {
        vec![Change::upsert(RecordRef::new(
            "workspaces.invitation",
            &format!("inv-{}", self.n),
        ))]
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_removed_client_loses_its_feed_even_while_busy(db: PgPool) {
    let world = World::with(db, |c| c.heartbeat_seconds = 1).await;
    let ana = world.user("ana");
    let ws = world.create_workspace(&ana, "Shop").await;
    world.add_member(&ws, "bea", "admin").await;
    world
        .server
        .agents
        .set("bea-agent", ClientActor::ActsFor("bea".into()));
    world.server.sequence().await;

    let mut feed = world
        .server
        .subscribe(Some(&world.client("bea-agent", &ws)), &ws, None)
        .await
        .unwrap();
    assert!(feed.next_event().await.is("ready"));
    // Removed (identity revoked its tokens); the stream is busy with
    // changes Bea's client may see, so it never idles into a heartbeat.
    world.server.agents.set("bea-agent", ClientActor::Revoked);
    let mut ended = false;
    for n in 0..15 {
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
                &Busy {
                    workspace: ws.clone(),
                    n,
                },
            )
            .await
            .unwrap();
        tx.commit().await.unwrap();
        world.server.sequence().await;
        tokio::time::sleep(Duration::from_millis(300)).await;
        feed.drain(Duration::from_millis(50)).await;
        if feed.raw.contains("event: revoked") {
            ended = true;
            break;
        }
    }
    assert!(ended, "still streaming: {}", feed.raw);
    let (status, _) = world
        .server
        .subscribe(Some(&world.client("bea-agent", &ws)), &ws, None)
        .await
        .err()
        .unwrap();
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_client_whose_member_left_loses_its_feed(db: PgPool) {
    let world = World::new(db).await;
    let ana = world.user("ana");
    let ws = world.create_workspace(&ana, "Shop").await;
    let (_, bea) = world.add_member(&ws, "bea", "member").await;
    world
        .server
        .agents
        .set("bea-agent", ClientActor::ActsFor("bea".into()));
    world.server.sequence().await;

    let mut feed = world
        .server
        .subscribe(Some(&world.client("bea-agent", &ws)), &ws, None)
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
    assert!(feed.next_event().await.is("revoked"));
    assert!(feed.next().await.is_none(), "the stream ends");
}

// --- scale, limits, liveness ---------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn many_subscribers_each_receive_every_change_once(db: PgPool) {
    let world = World::with(db, |c| {
        c.max_streams_per_principal = 64;
        c.opens_per_window = 0;
    })
    .await;
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
        assert_eq!(
            record_of(&frame),
            ("workspaces.workspace".into(), ws.clone(), "upsert".into())
        );
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
async fn a_caller_opens_a_limited_number_of_streams_per_window(db: PgPool) {
    let world = World::with(db, |c| {
        c.opens_per_window = 2;
        c.opens_window_seconds = 60;
    })
    .await;
    let ana = world.user("ana");
    let ws = world.create_workspace(&ana, "Shop").await;
    world.add_member(&ws, "bea", "member").await;
    for _ in 0..2 {
        drop(world.server.subscribe(Some(&ana), &ws, None).await.unwrap());
    }
    // Closed or not, the third open in the window is refused, with or
    // without a token.
    let (status, problem) = world
        .server
        .subscribe(Some(&ana), &ws, Some("anything"))
        .await
        .err()
        .unwrap();
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(problem["code"], "rate_limited", "{problem}");
    // Per caller.
    let bea = world.user("bea");
    assert!(world.server.subscribe(Some(&bea), &ws, None).await.is_ok());
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_token_resumes_only_for_the_caller_it_was_given_to(db: PgPool) {
    let world = World::new(db).await;
    let ana = world.user("ana");
    let ws = world.create_workspace(&ana, "Shop").await;
    let (bea, _) = world.add_member(&ws, "bea", "member").await;
    world
        .server
        .agents
        .set("ana-agent", ClientActor::ActsFor("ana".into()));
    world.server.sequence().await;
    let mut feed = world.server.subscribe(Some(&ana), &ws, None).await.unwrap();
    let token = feed.next_event().await.token();
    drop(feed);

    // Ana again, on another connection and with other scopes: resumes.
    let narrower = world.token("ana", "workspaces:read");
    let mut feed = world
        .server
        .subscribe(Some(&narrower), &ws, Some(&token))
        .await
        .unwrap();
    assert!(feed.next_event().await.is("ready"));
    // Anyone else, even a client acting for Ana: a token they cannot open.
    for other in [bea, world.client("ana-agent", &ws)] {
        let mut feed = world
            .server
            .subscribe(Some(&other), &ws, Some(&token))
            .await
            .unwrap();
        let reset = feed.next_event().await;
        assert_eq!(reset.json()["reason"], "unknown_cursor", "{reset:?}");
    }
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
    let ready = feed.next().await.unwrap();
    assert!(ready.is("ready"));
    let beat = feed.next_beat().await;
    assert_ne!(beat.token(), ready.token());
    assert_opaque(&beat.token());
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
    let cursor = here.next_event().await.token();
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
    let renamed = ("workspaces.workspace".into(), ws.clone(), "upsert".into());
    assert_eq!(record_of(&frame), renamed);

    // A change sequenced by one process wakes streams on the other, resumed
    // with a token the other sealed.
    let mut live = other
        .subscribe(Some(&ana), &ws, Some(&frame.token()))
        .await
        .unwrap();
    assert!(live.next_event().await.is("ready"));
    world.rename(&ana, &ws, "Shop three").await;
    world.server.sequence().await;
    assert_eq!(record_of(&live.next_event().await), renamed);
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
            account_subject: None,
            grantor_subject: None,
            refreshed: false,
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

// --- nothing about what the caller may not see (campbooks-cloud#131) -------------
//
// The matrix: two workspaces set up alike. Both see the same visible
// activity; one also has activity a given observer may not see (mail in an
// inbox they are not in, someone else's draft or connection, a client's
// draft). Every observer's stream in the busy workspace must match its
// stream in the quiet one: the same frames in the same order with the same
// fields, tokens that are all fresh and of one length, heartbeats on the
// same fixed timer, and resuming from any token they were given replaying
// the same frames. Values that differ between any two runs (record ids,
// event ids, tokens) are compared by where they first appear.

/// The matrix's heartbeat: long enough for a round's work to finish
/// between two.
const MATRIX_BEAT: Duration = Duration::from_secs(2);

/// Whose stream is compared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Observer {
    /// Bea: a member, reader of the shared inbox.
    Member,
    /// Cid: an admin (a manager of the workspace), manager of the shared
    /// inbox.
    Manager,
    /// A client Bea registered: it acts as Bea.
    Agent,
    /// A client bound to the workspace, acting for nobody.
    App,
}

/// Activity the observers may not see.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unseen {
    /// Mail into Hal's inbox, which only Hal is in.
    Mail,
    /// A draft Hal sent (audience: Hal).
    Draft,
    /// A connection Hal added (audience: Hal and the managers).
    Connection,
    /// A draft a client sent in the shared inbox (audience: clients).
    ClientDraft,
}

/// One of the matrix's two workspaces.
struct Space {
    ws: String,
    shared: campbooks_conversations::model::InboxId,
    hals: campbooks_conversations::model::InboxId,
    bea: UserId,
    hal: UserId,
}

/// A stream read by a task of its own, every frame stamped with when it
/// arrived after the stream was asked for.
struct Recorder {
    frames: std::sync::Arc<std::sync::Mutex<Vec<(Duration, Frame)>>>,
    task: tokio::task::JoinHandle<()>,
}

impl Recorder {
    fn start(mut feed: Feed, opened: tokio::time::Instant) -> Self {
        let frames = std::sync::Arc::<std::sync::Mutex<Vec<(Duration, Frame)>>>::default();
        let sink = std::sync::Arc::clone(&frames);
        let task = tokio::spawn(async move {
            while let Some(frame) = feed.read().await {
                sink.lock().unwrap().push((opened.elapsed(), frame));
            }
        });
        Self { frames, task }
    }

    fn beats(&self) -> usize {
        self.frames
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, frame)| frame.is("heartbeat"))
            .count()
    }

    /// Everything up to and including the `n`th heartbeat; the stream stops.
    fn take(self, n: usize) -> Vec<(Duration, Frame)> {
        self.task.abort();
        let frames = self.frames.lock().unwrap().clone();
        let mut seen = 0;
        frames
            .into_iter()
            .take_while(|(_, frame)| {
                let before = seen < n;
                if frame.is("heartbeat") {
                    seen += 1;
                }
                before
            })
            .collect()
    }
}

/// Wait until every recorder has had `n` heartbeats.
async fn until_beats(recorders: &[&Recorder], n: usize) {
    tokio::time::timeout(MATRIX_BEAT * u32::try_from(n + 3).unwrap(), async {
        while recorders.iter().any(|r| r.beats() < n) {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("no heartbeat {n} in time"));
}

impl World {
    /// A client token of `id` for `workspace` with `scopes`.
    fn client_with(&self, id: &str, workspace: &str, scopes: &str) -> String {
        let mut claims = self.idp.claims(scopes);
        let object = claims.as_object_mut().unwrap();
        object.remove("sub");
        object.insert("azp".into(), id.into());
        object.insert("workspace_id".into(), workspace.into());
        self.idp.token(&claims)
    }

    /// A workspace of Ana's with Bea (member), Cid (admin) and Hal
    /// (member); a shared inbox Cid manages and Bea reads; an inbox only
    /// Hal is in.
    async fn space(&self, name: &str) -> Space {
        use campbooks_conversations::{dev, model::InboxRole};
        let ana = self.user("ana");
        let ws = self.create_workspace(&ana, name).await;
        let (_, bea) = self.add_member(&ws, "bea", "member").await;
        let (_, cid) = self.add_member(&ws, "cid", "admin").await;
        let (_, hal) = self.add_member(&ws, "hal", "member").await;
        let workspace = ws.parse().unwrap();
        let shared = dev::create_inbox(
            &self.server.state,
            workspace,
            "shop@example.com",
            &[(cid, InboxRole::Manager), (bea, InboxRole::Reader)],
        )
        .await
        .unwrap();
        let hals = dev::create_inbox(
            &self.server.state,
            workspace,
            "hal@example.com",
            &[(hal, InboxRole::Manager)],
        )
        .await
        .unwrap();
        self.server.sequence().await;
        Space {
            ws,
            shared,
            hals,
            bea,
            hal,
        }
    }

    fn observer_token(&self, observer: Observer, ws: &str) -> String {
        match observer {
            Observer::Member => self.token("bea", READ_ALL),
            Observer::Manager => self.token("cid", READ_ALL),
            Observer::Agent => self.client_with("bea-agent", ws, READ_ALL),
            Observer::App => self.client_with("app", ws, READ_ALL),
        }
    }

    /// Open `observer`'s stream of `ws` and record it.
    async fn record(&self, observer: Observer, ws: &str, after: Option<&str>) -> Recorder {
        let token = self.observer_token(observer, ws);
        let opened = tokio::time::Instant::now();
        let feed = self
            .server
            .subscribe(Some(&token), ws, after)
            .await
            .unwrap_or_else(|e| panic!("{observer:?} may not follow {ws}: {e:?}"));
        Recorder::start(feed, opened)
    }

    /// What everyone in `space` does in round `round`, seen by every
    /// observer that may see it.
    async fn visible_round(&self, space: &Space, round: usize) {
        use campbooks_conversations::{events::DraftSent, model::DraftId};
        match round {
            0 => {
                self.rename(&self.user("ana"), &space.ws, "Bakery").await;
                self.server.sequence().await;
            }
            1 => self.ingest(&space.ws, space.shared, EN).await,
            _ => {
                self.publish(&DraftSent {
                    workspace: space.ws.clone(),
                    inbox: space.shared,
                    draft: DraftId::new(),
                    conversation: None,
                    author: Some(space.bea),
                })
                .await;
            }
        }
    }

    /// `hidden` activity in `space`, in round `round`.
    async fn hidden_round(&self, space: &Space, hidden: &[Unseen], round: usize) {
        use campbooks_connectors::{ConnectionId, Provider, events::ConnectionAdded};
        use campbooks_conversations::{events::DraftSent, model::DraftId};
        for kind in hidden {
            match kind {
                Unseen::Mail => {
                    let mail = if round.is_multiple_of(2) { FR } else { EN };
                    self.ingest(&space.ws, space.hals, mail).await;
                }
                Unseen::Draft => {
                    self.publish(&DraftSent {
                        workspace: space.ws.clone(),
                        inbox: space.hals,
                        draft: DraftId::new(),
                        conversation: None,
                        author: Some(space.hal),
                    })
                    .await;
                }
                Unseen::Connection => {
                    self.publish(&ConnectionAdded {
                        workspace: space.ws.clone(),
                        connection: ConnectionId::new(),
                        provider: Provider::Google,
                        granted_by: space.hal,
                        account_subject: None,
                        grantor_subject: None,
                        refreshed: false,
                    })
                    .await;
                }
                Unseen::ClientDraft => {
                    self.publish(&DraftSent {
                        workspace: space.ws.clone(),
                        inbox: space.shared,
                        draft: DraftId::new(),
                        conversation: None,
                        author: None,
                    })
                    .await;
                }
            }
        }
    }
}

/// `frames` with every value that differs between runs replaced by where
/// it first appears, heartbeats left out; checks every token on the way.
fn canonical(frames: &[(Duration, Frame)]) -> Vec<String> {
    let mut names: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut name = |value: &str| {
        let next = names.len();
        format!("#{}", *names.entry(value.to_owned()).or_insert(next))
    };
    frames
        .iter()
        .filter(|(_, frame)| !frame.is_beat())
        .map(|(_, frame)| {
            let mut data = frame.json();
            if frame.id.is_some() {
                assert_opaque(&frame.token());
                data["cursor"] = "<token>".into();
            }
            if data.get("record").is_some() {
                assert_ids_only(frame);
                let id = data["record"]["id"].as_str().unwrap().to_owned();
                data["record"]["id"] = name(&id).into();
                let version = data["version"].as_str().unwrap().to_owned();
                data["version"] = name(&version).into();
            }
            format!("{} {}", frame.event.as_deref().unwrap_or("<comment>"), data)
        })
        .collect()
}

/// Every token in `frames` is new, and every heartbeat is a token on the
/// fixed timer from the open.
fn assert_tokens_fresh_and_beats_on_time(frames: &[(Duration, Frame)], what: &str) {
    let tokens: Vec<String> = frames.iter().filter_map(|(_, f)| f.id.clone()).collect();
    let distinct: BTreeSet<&String> = tokens.iter().collect();
    assert_eq!(distinct.len(), tokens.len(), "{what}: a token repeated");
    let beats: Vec<Duration> = frames
        .iter()
        .filter(|(_, frame)| frame.is("heartbeat"))
        .map(|(at, frame)| {
            let data = frame.json();
            assert_eq!(
                data.as_object().unwrap().keys().collect::<Vec<_>>(),
                vec!["cursor"],
                "{what}: a heartbeat carries its token only"
            );
            *at
        })
        .collect();
    for (k, at) in beats.iter().enumerate() {
        let due = MATRIX_BEAT * u32::try_from(k + 1).unwrap();
        let off = at.abs_diff(due);
        assert!(
            off < MATRIX_BEAT / 2,
            "{what}: heartbeat {k} at {at:?}, due at {due:?}"
        );
    }
}

/// Follow every observer in a busy and a quiet workspace through three
/// rounds, then resume from the tokens they were given; their streams must
/// not tell the two apart.
async fn assert_blind(db: PgPool, observers: &[Observer], hidden: &[Unseen]) {
    const ROUNDS: usize = 3;
    let world = World::with(db, |c| {
        c.heartbeat_seconds = MATRIX_BEAT.as_secs();
        c.poll_interval_ms = 100;
    })
    .await;
    world
        .server
        .agents
        .set("bea-agent", ClientActor::ActsFor("bea".into()));
    let busy = world.space("Busy").await;
    let quiet = world.space("Quiet").await;

    let mut live = Vec::new();
    for &observer in observers {
        for space in [&busy, &quiet] {
            live.push(world.record(observer, &space.ws, None).await);
        }
    }
    for round in 0..ROUNDS {
        until_beats(&live.iter().collect::<Vec<_>>(), round + 1).await;
        world.hidden_round(&busy, hidden, round).await;
        world.visible_round(&busy, round).await;
        world.hidden_round(&busy, hidden, round + 1).await;
        world.visible_round(&quiet, round).await;
    }
    until_beats(&live.iter().collect::<Vec<_>>(), ROUNDS + 2).await;
    let mut transcripts = Vec::new();
    for recorder in live {
        transcripts.push(recorder.take(ROUNDS + 2));
    }

    for (i, &observer) in observers.iter().enumerate() {
        let (seen, unseen) = (&transcripts[2 * i], &transcripts[2 * i + 1]);
        let what = format!("{observer:?} with {hidden:?}");
        assert_tokens_fresh_and_beats_on_time(seen, &what);
        assert_tokens_fresh_and_beats_on_time(unseen, &what);
        assert_eq!(
            canonical(seen),
            canonical(unseen),
            "{what}: the streams differ\nbusy: {seen:#?}\nquiet: {unseen:#?}"
        );

        // Resuming from any token it was given replays the same.
        let tokens = |frames: &[(Duration, Frame)]| -> Vec<String> {
            frames.iter().filter_map(|(_, f)| f.id.clone()).collect()
        };
        let (seen_tokens, unseen_tokens) = (tokens(seen), tokens(unseen));
        assert_eq!(seen_tokens.len(), unseen_tokens.len(), "{what}");
        let picks = [
            0,
            seen_tokens.len() / 3,
            seen_tokens.len() / 2,
            seen_tokens.len() - 1,
        ];
        let mut resumed = Vec::new();
        for &pick in &picks {
            resumed.push(
                world
                    .record(observer, &busy.ws, Some(&seen_tokens[pick]))
                    .await,
            );
            resumed.push(
                world
                    .record(observer, &quiet.ws, Some(&unseen_tokens[pick]))
                    .await,
            );
        }
        until_beats(&resumed.iter().collect::<Vec<_>>(), 1).await;
        let mut resumed = resumed.into_iter();
        for &pick in &picks {
            let seen = resumed.next().unwrap().take(1);
            let unseen = resumed.next().unwrap().take(1);
            assert_eq!(
                canonical(&seen),
                canonical(&unseen),
                "{what}: resuming from token {pick} differs\nbusy: {seen:#?}\nquiet: {unseen:#?}"
            );
            assert!(seen[0].1.is("ready"), "{what}: {seen:#?}");
        }
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn no_caller_learns_of_mail_or_drafts_in_an_inbox_it_cannot_see(db: PgPool) {
    assert_blind(
        db,
        &[
            Observer::Member,
            Observer::Manager,
            Observer::Agent,
            Observer::App,
        ],
        &[Unseen::Mail, Unseen::Draft],
    )
    .await;
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_member_learns_nothing_of_another_members_connections(db: PgPool) {
    assert_blind(
        db,
        &[Observer::Member, Observer::Agent],
        &[Unseen::Connection],
    )
    .await;
}

#[sqlx::test(migrations = "../../migrations")]
async fn people_learn_nothing_of_drafts_clients_write(db: PgPool) {
    assert_blind(
        db,
        &[Observer::Member, Observer::Manager, Observer::Agent],
        &[Unseen::ClientDraft],
    )
    .await;
}

/// Conversations filed into an inbox, as one event: the catch-up test's
/// changes, many at once.
#[derive(Debug, Serialize, Deserialize)]
struct Filed {
    workspace: String,
    /// `(inbox, conversation id)` in order.
    into: Vec<(String, String)>,
}

impl DomainEvent for Filed {
    const NAME: &'static str = "test.filed";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }

    fn changes(&self) -> Vec<Change> {
        self.into
            .iter()
            .map(|(inbox, id)| {
                Change::upsert(RecordRef::new("conversations.conversation", id))
                    .within(RecordRef::new("conversations.inbox", inbox))
            })
            .collect()
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn catching_up_past_more_hidden_changes_than_a_page_looks_the_same(db: PgPool) {
    let world = World::with(db, |c| c.heartbeat_seconds = MATRIX_BEAT.as_secs()).await;
    let busy = world.space("Busy").await;
    let quiet = world.space("Quiet").await;
    let mut tokens = Vec::new();
    for space in [&busy, &quiet] {
        let mut feed = world
            .server
            .subscribe(
                Some(&world.observer_token(Observer::Member, &space.ws)),
                &space.ws,
                None,
            )
            .await
            .unwrap();
        tokens.push(feed.next_event().await.token());
    }
    // While Bea is away: the same 450 conversations in the shared inbox in
    // both; in the busy one, 500 in Hal's inbox first and up to 9 more
    // before each of hers.
    let markers: Vec<String> = (0..450).map(|n| format!("c-{n}")).collect();
    let mut busy_into: Vec<(String, String)> = (0..500)
        .map(|n| (busy.hals.to_string(), format!("h-{n}")))
        .collect();
    for (n, marker) in markers.iter().enumerate() {
        for h in 0..(n % 10) {
            busy_into.push((busy.hals.to_string(), format!("h-{n}-{h}")));
        }
        busy_into.push((busy.shared.to_string(), marker.clone()));
    }
    let quiet_into = markers
        .iter()
        .map(|marker| (quiet.shared.to_string(), marker.clone()))
        .collect();
    world
        .publish(&Filed {
            workspace: busy.ws.clone(),
            into: busy_into,
        })
        .await;
    world
        .publish(&Filed {
            workspace: quiet.ws.clone(),
            into: quiet_into,
        })
        .await;

    let mut recorders = Vec::new();
    for (space, token) in [&busy, &quiet].into_iter().zip(&tokens) {
        recorders.push(world.record(Observer::Member, &space.ws, Some(token)).await);
    }
    until_beats(&recorders.iter().collect::<Vec<_>>(), 1).await;
    let mut recorders = recorders.into_iter();
    let seen = recorders.next().unwrap().take(1);
    let unseen = recorders.next().unwrap().take(1);
    assert_eq!(canonical(&seen), canonical(&unseen), "the catch-up differs");
    for frames in [&seen, &unseen] {
        let changes: Vec<String> = frames
            .iter()
            .filter(|(_, f)| f.is("change"))
            .map(|(_, f)| record_of(f).1)
            .collect();
        assert_eq!(changes, markers, "every visible change, once, in order");
        // The whole catch-up comes before the first heartbeat.
        let (_, last) = frames.last().unwrap();
        assert!(last.is("heartbeat"), "{last:?}");
        assert_eq!(
            frames.len(),
            markers.len() + 2,
            "ready, the changes, a heartbeat"
        );
        assert_tokens_fresh_and_beats_on_time(frames, "catch-up");
    }
}
