//! The per-caller rate limits end to end: the composed API and the admin
//! router as the server serves them, an in-process identity provider, a real
//! Postgres (`#[sqlx::test]`) for the send class, and a clock the test moves
//! for the in-memory classes. The web BFF's path is covered in `bff::tests`.

use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
};
use campbooks_kernel::{
    AppState,
    auth::{Principal, PrincipalKind, Scope},
    clock::FixedClock,
    config::RateLimitsConfig,
    operations::{self, Caller, Invocation, InvokeError},
    rate_limit::api::ApiLimits,
    testing::{self, TestIdp},
};
use chrono::{DateTime, TimeDelta, Utc};
use http_body_util::BodyExt as _;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt as _;

const SCOPES: &str = "workspaces:read workspaces:admin conversations:send conversations:write";

/// Low limits, so a test reaches them in a few calls: 5 reads and 3
/// writes a minute, 2 sends per 10 minutes, 2 admin calls a minute.
fn low() -> RateLimitsConfig {
    RateLimitsConfig {
        enabled: true,
        reads: 5,
        reads_window_seconds: 60,
        writes: 3,
        writes_window_seconds: 60,
        sends: 2,
        sends_window_seconds: 600,
        admin: 2,
        admin_window_seconds: 60,
    }
}

const WORKSPACE: &str = "0199e0d6-0000-7000-8000-000000000001";
const DRAFT: &str = "0199e0d6-0000-7000-8000-000000000002";

struct Server {
    app: Router,
    state: AppState,
    idp: TestIdp,
    clock: Arc<FixedClock>,
    db: PgPool,
}

#[derive(Debug)]
struct Reply {
    status: StatusCode,
    retry_after: Option<u64>,
    body: Value,
}

impl Reply {
    fn limited(&self) -> bool {
        self.status == StatusCode::TOO_MANY_REQUESTS
    }

    fn assert_limited(&self, retry_after: u64) {
        assert_eq!(
            self.status,
            StatusCode::TOO_MANY_REQUESTS,
            "{:?}",
            self.body
        );
        assert_eq!(self.body["code"], "rate_limited", "{:?}", self.body);
        assert_eq!(self.retry_after, Some(retry_after), "{:?}", self.body);
    }
}

impl Server {
    async fn new(db: &PgPool, limits: &RateLimitsConfig) -> Self {
        let idp = TestIdp::start().await;
        let mut config = testing::config("postgres://test@localhost/test");
        config.auth = idp.auth_config();
        let start = DateTime::parse_from_rfc3339("2026-10-16T09:00:00Z")
            .unwrap()
            .to_utc();
        let clock = Arc::new(FixedClock::new(start));
        let state = crate::with_ports(
            AppState::new(testing::app_role_pool(db).await, config)
                .with_api_limits(ApiLimits::with_clock(limits, clock.clone())),
        );
        let operations::Composed { routes, state, .. } = operations::compose(crate::api(), state);
        let admin = crate::admin_routes(&state, &crate::Composition::default());
        let app = routes
            .merge(admin)
            .fallback(|| async { campbooks_kernel::Error::NotFound })
            .with_state(state.clone());
        Self {
            app,
            state,
            idp,
            clock,
            db: db.clone(),
        }
    }

    /// A person's token from app `azp`.
    fn token(&self, sub: &str, azp: &str) -> String {
        let mut claims = self.idp.claims(&format!("{SCOPES} admin:access"));
        claims["sub"] = sub.into();
        claims["azp"] = azp.into();
        claims["auth_time"] = json!(Utc::now().timestamp());
        self.idp.token(&claims)
    }

    /// A `client_credentials` client's token (its `sub` is its client id).
    fn client_token(&self, client: &str) -> String {
        let mut claims = self.idp.claims(SCOPES);
        claims["sub"] = client.into();
        claims["azp"] = client.into();
        self.idp.token(&claims)
    }

    async fn call(&self, token: Option<&str>, method: &str, uri: &str, body: Value) -> Reply {
        let mut request = Request::builder().method(method).uri(uri);
        if let Some(token) = token {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        let body = if body.is_null() {
            Body::empty()
        } else {
            request = request.header(header::CONTENT_TYPE, "application/json");
            Body::from(body.to_string())
        };
        let response = self
            .app
            .clone()
            .oneshot(request.body(body).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let retry_after = response
            .headers()
            .get(header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok());
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        Reply {
            status,
            retry_after,
            body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        }
    }

    async fn read(&self, token: &str) -> Reply {
        self.call(Some(token), "GET", "/api/v1/workspaces", Value::Null)
            .await
    }

    /// A write (inviting someone into a workspace the caller is not in: 404,
    /// but counted before the handler runs).
    async fn write(&self, token: &str) -> Reply {
        let uri = format!("/api/v1/w/{WORKSPACE}/invitations");
        self.call(
            Some(token),
            "POST",
            &uri,
            json!({ "email": "someone@example.com" }),
        )
        .await
    }

    /// A send (of a draft that does not exist: 404, but counted).
    async fn send(&self, token: &str) -> Reply {
        let uri = format!("/api/v1/w/{WORKSPACE}/drafts/{DRAFT}/send");
        self.call(Some(token), "POST", &uri, json!({ "version": 1 }))
            .await
    }

    async fn admin(&self, token: &str) -> Reply {
        self.call(Some(token), "GET", "/admin/v1/system/jobs", Value::Null)
            .await
    }

    /// Run `operation` as `caller` for person `sub`; the HTTP status it
    /// ended with.
    async fn invoke(&self, caller: Caller, sub: &str, operation: &str, input: Value) -> StatusCode {
        let invocation = Invocation::new(caller, person(sub), operation, input)
            .in_workspace(WORKSPACE)
            .idempotency_key(uuid::Uuid::new_v4().to_string())
            .confirmed();
        match operations::invoke(&self.state, invocation).await {
            Ok(output) => output.status,
            Err(InvokeError::Failed { status, .. }) => status,
            Err(other) => panic!("{other}"),
        }
    }
}

/// Person `sub` with every scope of these tests, as automation or the
/// assistant acts for them.
fn person(sub: &str) -> Principal {
    Principal {
        subject: sub.to_owned(),
        kind: PrincipalKind::User,
        client_id: None,
        scopes: SCOPES.split(' ').map(Scope::new).collect(),
        workspace: None,
        email: None,
        email_verified: false,
        name: None,
        expires_at: None,
        issued_at: None,
        auth_time: None,
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_person_over_the_read_limit_waits_then_reads_again(db: PgPool) {
    let server = Server::new(&db, &low()).await;
    let ana = server.token("ana", "campbooks-web");
    for _ in 0..5 {
        assert!(!server.read(&ana).await.limited());
    }
    // 5 a minute: the next one is allowed every 12 seconds.
    let refused = server.read(&ana).await;
    refused.assert_limited(12);
    assert!(
        refused.body["title"]
            .as_str()
            .is_some_and(|t| !t.is_empty()),
        "{:?}",
        refused.body
    );
    server.clock.advance(TimeDelta::seconds(11));
    server.read(&ana).await.assert_limited(1);
    server.clock.advance(TimeDelta::seconds(1));
    assert!(!server.read(&ana).await.limited());
    server.read(&ana).await.assert_limited(12);
    // A whole window later, the whole burst again.
    server.clock.advance(TimeDelta::minutes(1));
    for _ in 0..5 {
        assert!(!server.read(&ana).await.limited());
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn people_apps_and_clients_do_not_share_a_budget(db: PgPool) {
    let server = Server::new(&db, &low()).await;
    let ana_web = server.token("ana", "campbooks-web");
    for _ in 0..5 {
        server.read(&ana_web).await;
    }
    assert!(server.read(&ana_web).await.limited());
    // Another person.
    let bea = server.token("bea", "campbooks-web");
    assert!(!server.read(&bea).await.limited());
    // The same person through another app.
    let ana_app = server.token("ana", "acme-reader");
    assert!(!server.read(&ana_app).await.limited());
    // An app on its own (client credentials), with its own budget.
    let client = server.client_token("acme-sync");
    for _ in 0..5 {
        assert!(!server.read(&client).await.limited());
    }
    server.read(&client).await.assert_limited(12);
    assert!(!server.read(&bea).await.limited());
}

#[sqlx::test(migrations = "../../migrations")]
async fn writes_and_sends_stop_at_their_own_lower_limits(db: PgPool) {
    let server = Server::new(&db, &low()).await;
    let ana = server.token("ana", "campbooks-web");
    for _ in 0..3 {
        assert_eq!(server.write(&ana).await.status, StatusCode::NOT_FOUND);
    }
    // 3 a minute: one every 20 seconds. Reads are counted apart.
    server.write(&ana).await.assert_limited(20);
    assert!(!server.read(&ana).await.limited());

    // Sends have their own budget (not the writes'), counted in Postgres.
    for _ in 0..2 {
        assert_eq!(server.send(&ana).await.status, StatusCode::NOT_FOUND);
    }
    let refused = server.send(&ana).await;
    assert!(refused.limited(), "{refused:?}");
    assert_eq!(refused.body["code"], "rate_limited");
    assert!(
        refused
            .retry_after
            .is_some_and(|s| (590..=600).contains(&s)),
        "{refused:?}"
    );
    // Per person, across apps.
    let ana_app = server.token("ana", "acme-mailer");
    assert!(server.send(&ana_app).await.limited());
    assert_eq!(
        server.write(&ana_app).await.status,
        StatusCode::NOT_FOUND,
        "the other app's writes are its own"
    );
    let bea = server.token("bea", "campbooks-web");
    assert_eq!(server.send(&bea).await.status, StatusCode::NOT_FOUND);

    // The send window ends (in the database): sending works again.
    sqlx::query("UPDATE kernel.rate_limits SET resets_at = now() - interval '1 second'")
        .execute(&server.db)
        .await
        .unwrap();
    assert_eq!(server.send(&ana).await.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../migrations")]
async fn automation_and_the_assistant_have_their_own_budget_and_never_send(db: PgPool) {
    let server = Server::new(&db, &low()).await;
    let ana = server.token("ana", "campbooks-web");
    for _ in 0..3 {
        server.write(&ana).await;
    }
    assert!(server.write(&ana).await.limited());

    // The person is limited; a rule acting for them is not (yet).
    let archive = json!({ "conversation_id": DRAFT });
    for _ in 0..3 {
        assert_eq!(
            server
                .invoke(
                    Caller::Automation,
                    "ana",
                    "conversations.archive",
                    archive.clone()
                )
                .await,
            StatusCode::NOT_FOUND
        );
    }
    // A looping rule stops at the same ceiling.
    assert_eq!(
        server
            .invoke(
                Caller::Automation,
                "ana",
                "conversations.archive",
                archive.clone()
            )
            .await,
        StatusCode::TOO_MANY_REQUESTS
    );
    // The assistant has a budget of its own.
    for _ in 0..5 {
        assert_ne!(
            server
                .invoke(Caller::Assistant, "ana", "workspaces.list", Value::Null)
                .await,
            StatusCode::TOO_MANY_REQUESTS
        );
    }
    assert_eq!(
        server
            .invoke(Caller::Assistant, "ana", "workspaces.list", Value::Null)
            .await,
        StatusCode::TOO_MANY_REQUESTS
    );
    // The person's reads were never touched.
    assert!(!server.read(&ana).await.limited());

    // Sends are the person's own (campbooks-cloud#242): neither the
    // assistant nor automation may send, so they never reach the send
    // budget, confirmed or not.
    let send = json!({ "draft_id": DRAFT, "version": 1 });
    for caller in [Caller::Assistant, Caller::Automation] {
        let invocation = Invocation::new(
            caller,
            person("ana"),
            "conversations.send_draft",
            send.clone(),
        )
        .in_workspace(WORKSPACE)
        .idempotency_key(uuid::Uuid::new_v4().to_string())
        .confirmed();
        let refused = operations::invoke(&server.state, invocation).await;
        assert!(
            matches!(refused, Err(InvokeError::NotAllowed { .. })),
            "{refused:?}"
        );
    }
    // Ana's own two sends are all hers.
    for _ in 0..2 {
        assert!(!server.send(&ana).await.limited());
    }
    assert!(server.send(&ana).await.limited());
}

#[sqlx::test(migrations = "../../migrations")]
async fn the_admin_api_is_capped(db: PgPool) {
    let server = Server::new(&db, &low()).await;
    let ana = server.token("ana", "campbooks-web");
    // Not an administrator: 404, still counted.
    for _ in 0..2 {
        assert_eq!(server.admin(&ana).await.status, StatusCode::NOT_FOUND);
    }
    server.admin(&ana).await.assert_limited(30);
    // The API is a budget apart.
    assert!(!server.read(&ana).await.limited());
}

#[sqlx::test(migrations = "../../migrations")]
async fn public_operations_and_bad_tokens_are_not_counted(db: PgPool) {
    let server = Server::new(&db, &low()).await;
    for _ in 0..10 {
        let health = server
            .call(None, "GET", "/api/v1/system/version", Value::Null)
            .await;
        assert_eq!(health.status, StatusCode::OK);
    }
    let mut expired = server.idp.claims(SCOPES);
    expired["sub"] = "ana".into();
    expired["exp"] = json!(Utc::now().timestamp() - 3_600);
    let expired = server.idp.token(&expired);
    for _ in 0..10 {
        assert_eq!(server.read(&expired).await.status, StatusCode::UNAUTHORIZED);
    }
    // Ana's budget is whole.
    let ana = server.token("ana", "campbooks-web");
    for _ in 0..5 {
        assert!(!server.read(&ana).await.limited());
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn turned_off_nothing_is_limited(db: PgPool) {
    let server = Server::new(
        &db,
        &RateLimitsConfig {
            enabled: false,
            ..low()
        },
    )
    .await;
    let ana = server.token("ana", "campbooks-web");
    for _ in 0..20 {
        assert!(!server.read(&ana).await.limited());
        assert!(!server.write(&ana).await.limited());
        assert!(!server.send(&ana).await.limited());
        assert!(!server.admin(&ana).await.limited());
    }

    // One class off: writes run free while reads are still limited.
    let server = Server::new(&db, &RateLimitsConfig { writes: 0, ..low() }).await;
    let ana = server.token("ana", "campbooks-web");
    for _ in 0..20 {
        assert!(!server.write(&ana).await.limited());
    }
    for _ in 0..5 {
        server.read(&ana).await;
    }
    assert!(server.read(&ana).await.limited());
}
