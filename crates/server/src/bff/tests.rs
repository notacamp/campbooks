//! The BFF end to end: a simulated browser (cookie jar) against the full
//! application, an in-process [`TestIdp`] for the IdP side, and a real
//! Postgres (`#[sqlx::test]`).

use std::{collections::BTreeMap, sync::Arc};

use axum::{
    Json, Router,
    body::Body,
    http::{HeaderMap, Method, Request, StatusCode, header},
    routing::any,
};
use campbooks_kernel::{
    AppState,
    auth::{ScopeName, Scoped},
    idempotency, operations,
    testing::{self, TestIdp, TestUser, WEB_CLIENT_SECRET},
};
use secrecy::SecretString;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt as _;

use super::{
    Bff, CSRF_HEADER, PruneSessions,
    config::{self, BffConfig, Loaded},
    store,
};

const ORIGIN: &str = "https://app.example.com";
const SESSION: &str = "__Host-campbooks_session";
const LOGIN: &str = "__Host-campbooks_login";

// --- fixtures ----------------------------------------------------------------

/// The scope of the test API route.
struct TestRead;
impl ScopeName for TestRead {
    const NAME: &'static str = "test:read";
    const DESCRIPTION: &'static str = "Read test things.";
}

/// An API route that reports what the API saw.
async fn whoami(
    Scoped { principal, .. }: Scoped<TestRead>,
    method: Method,
    headers: HeaderMap,
    body: String,
) -> Json<Value> {
    Json(json!({
        "subject": principal.subject,
        "client_id": principal.client_id,
        "method": method.as_str(),
        "cookie": headers.contains_key(header::COOKIE),
        "csrf": headers.contains_key(CSRF_HEADER),
        "body": body,
    }))
}

/// What a test reads back from `bff.sessions`.
type SessionRow = (
    Vec<u8>,
    String,
    Option<String>,
    Vec<u8>,
    Option<Vec<u8>>,
    Option<String>,
);

struct Harness {
    app: Router,
    idp: TestIdp,
    db: PgPool,
}

async fn harness(db: PgPool) -> Harness {
    harness_with(db, ORIGIN, |_| {}).await
}

async fn harness_with(db: PgPool, origin: &str, tweak: impl FnOnce(&mut BffConfig)) -> Harness {
    let idp = TestIdp::start().await;
    let mut config = testing::config("postgres://test@localhost/test");
    config.auth = idp.auth_config();
    origin.clone_into(&mut config.public_url);
    // Change streams end quickly, so a test can read one to its end.
    config.changes.heartbeat_seconds = 1;
    config.changes.max_connection_seconds = 2;
    let mut bff = BffConfig {
        client_secret: Some(SecretString::from(WEB_CLIENT_SECRET)),
        ..BffConfig::default()
    };
    tweak(&mut bff);
    let Ok(Loaded::Enabled(settings)) = config::settings(
        &config,
        bff,
        Some(campbooks_kernel::crypto::SecretKey::from_bytes([7; 32])),
        &["test:read", "admin:access"],
    ) else {
        panic!("test BFF settings are valid");
    };
    // The app runs as the runtime role, like the server (row-level security
    // applies); the test's own queries keep the superuser pool.
    let state = crate::with_ports(AppState::new(testing::app_role_pool(&db).await, config));
    let app = app(state, Some(*settings));
    Harness { app, idp, db }
}

/// The full application as [`crate::router`] composes it (operations,
/// idempotency), with the test route in the API.
fn app(state: AppState, settings: Option<config::Settings>) -> Router {
    let operations::Composed {
        routes,
        spec,
        state,
    } = operations::compose(
        crate::api().route("/api/v1/test/whoami", any(whoami)),
        state,
    );
    let api_only: Router = routes
        .clone()
        .fallback(|| async { campbooks_kernel::Error::NotFound })
        .with_state(state.clone());
    let admin = crate::admin_routes(&state, &crate::Composition::default());
    let admin_only: Router = admin
        .clone()
        .fallback(|| async { campbooks_kernel::Error::NotFound })
        .with_state(state.clone());
    let bff = Bff::new(settings, &state, api_only, admin_only, Router::new());
    crate::compose(
        state,
        routes.merge(admin),
        Arc::new(spec),
        &bff,
        Router::new(),
    )
}

#[derive(Debug)]
struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    body: String,
}

impl Reply {
    fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or(Value::Null)
    }

    fn header(&self, name: impl header::AsHeaderName) -> Option<&str> {
        self.headers.get(name).and_then(|v| v.to_str().ok())
    }

    fn set_cookies(&self) -> Vec<String> {
        self.headers
            .get_all(header::SET_COOKIE)
            .iter()
            .map(|v| v.to_str().unwrap().to_owned())
            .collect()
    }

    fn set_cookie(&self, name: &str) -> Option<String> {
        self.set_cookies()
            .into_iter()
            .find(|c| c.starts_with(&format!("{name}=")))
    }

    fn assert_problem(&self, status: StatusCode, code: &str) {
        assert_eq!(self.status, status, "{}", self.body);
        assert_eq!(
            self.header(header::CONTENT_TYPE),
            Some("application/problem+json")
        );
        assert_eq!(self.json()["code"], code, "{}", self.body);
    }
}

/// A browser: a cookie jar over the app.
#[derive(Clone)]
struct Browser {
    app: Router,
    jar: BTreeMap<String, String>,
}

impl Browser {
    fn new(app: &Router) -> Self {
        Self {
            app: app.clone(),
            jar: BTreeMap::new(),
        }
    }

    fn cookie(&self, name: &str) -> Option<&str> {
        self.jar.get(name).map(String::as_str)
    }

    async fn send(&mut self, request: axum::http::request::Builder, body: Body) -> Reply {
        let mut request = request;
        if !self.jar.is_empty() {
            let cookies: Vec<String> = self.jar.iter().map(|(k, v)| format!("{k}={v}")).collect();
            request = request.header(header::COOKIE, cookies.join("; "));
        }
        let reply = send(&self.app, request.body(body).unwrap()).await;
        for cookie in reply.set_cookies() {
            let (pair, attributes) = cookie.split_once(';').unwrap_or((&cookie, ""));
            let (name, value) = pair.split_once('=').unwrap();
            if value.is_empty() || attributes.contains("Max-Age=0") {
                self.jar.remove(name);
            } else {
                self.jar.insert(name.to_owned(), value.to_owned());
            }
        }
        reply
    }

    async fn get(&mut self, uri: &str) -> Reply {
        self.send(Request::get(uri), Body::empty()).await
    }

    /// An unsafe same-origin request as the web client sends it.
    async fn post(&mut self, uri: &str, body: &str) -> Reply {
        self.send(
            Request::post(uri)
                .header(CSRF_HEADER, "1")
                .header("sec-fetch-site", "same-origin")
                .header(header::CONTENT_TYPE, "application/json"),
            Body::from(body.to_owned()),
        )
        .await
    }

    /// `/bff/login` → the IdP approves → `/bff/callback`.
    async fn sign_in(&mut self, idp: &TestIdp, return_to: &str) -> Reply {
        let login = self
            .get(&format!("/bff/login?return_to={}", urlencode(return_to)))
            .await;
        assert_eq!(login.status, StatusCode::SEE_OTHER, "{}", login.body);
        let callback = idp.authorize(login.header(header::LOCATION).unwrap());
        self.get(&path_and_query(&callback)).await
    }
}

async fn send(app: &Router, request: Request<Body>) -> Reply {
    let response = match app.clone().oneshot(request).await {
        Ok(response) => response,
        Err(never) => match never {},
    };
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    Reply {
        status,
        headers,
        body: String::from_utf8(bytes.to_vec()).unwrap(),
    }
}

fn urlencode(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

fn path_and_query(url: &str) -> String {
    let url = url::Url::parse(url).unwrap();
    format!("{}?{}", url.path(), url.query().unwrap_or_default())
}

fn query(url: &str) -> BTreeMap<String, String> {
    url::Url::parse(url)
        .unwrap()
        .query_pairs()
        .into_owned()
        .collect()
}

async fn count(db: &PgPool, sql: &'static str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(db).await.unwrap()
}

async fn exec(db: &PgPool, sql: &'static str) {
    sqlx::query(sql).execute(db).await.unwrap();
}

// --- login ---------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn login_redirects_to_the_idp_with_pkce_state_and_nonce(db: PgPool) {
    let h = harness(db).await;
    let mut browser = Browser::new(&h.app);
    let reply = browser.get("/bff/login?return_to=%2Finbox%3Ftab%3D2").await;

    assert_eq!(reply.status, StatusCode::SEE_OTHER, "{}", reply.body);
    assert_eq!(reply.header(header::CACHE_CONTROL), Some("no-store"));
    let location = reply.header(header::LOCATION).unwrap();
    assert!(
        location.starts_with(&format!("{}oidc/authorize?", h.idp.issuer())),
        "{location}"
    );
    let params = query(location);
    assert_eq!(params["response_type"], "code");
    assert_eq!(params["client_id"], "campbooks-web");
    assert_eq!(params["redirect_uri"], format!("{ORIGIN}/bff/callback"));
    assert_eq!(
        params["scope"],
        "openid profile email offline_access test:read admin:access"
    );
    assert_eq!(params["code_challenge_method"], "S256");
    assert_eq!(params["code_challenge"].len(), 43);
    assert!(params["state"].len() >= 43);
    assert!(params["nonce"].len() >= 43);

    let cookie = reply.set_cookie(LOGIN).unwrap();
    assert!(
        cookie.ends_with("; Path=/; Max-Age=600; HttpOnly; SameSite=Lax; Secure"),
        "{cookie}"
    );
    assert_eq!(
        count(&h.db, "SELECT count(*) FROM bff.login_transactions").await,
        1
    );
    let stored: (String, String) =
        sqlx::query_as("SELECT return_to, nonce FROM bff.login_transactions")
            .fetch_one(&h.db)
            .await
            .unwrap();
    assert_eq!(stored, ("/inbox?tab=2".to_owned(), params["nonce"].clone()));

    // Every sign-in gets its own state, nonce and verifier.
    let again = query(
        browser
            .get("/bff/login")
            .await
            .header(header::LOCATION)
            .unwrap(),
    );
    for name in ["state", "nonce", "code_challenge"] {
        assert_ne!(again[name], params[name], "{name}");
    }
    assert!(!again.contains_key("max_age"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_step_up_login_asks_for_a_recent_sign_in(db: PgPool) {
    let h = harness(db).await;
    let mut browser = Browser::new(&h.app);
    let reply = browser
        .get("/bff/login?max_age=600&return_to=%2Fsettings%2Fsign-in")
        .await;
    assert_eq!(reply.status, StatusCode::SEE_OTHER, "{}", reply.body);
    let params = query(reply.header(header::LOCATION).unwrap());
    assert_eq!(params["max_age"], "600");

    let refused = browser.get("/bff/login?max_age=-1").await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST, "{}", refused.body);
}

#[sqlx::test(migrations = "../../migrations")]
async fn login_refuses_return_to_outside_the_site(db: PgPool) {
    let h = harness(db).await;
    let mut browser = Browser::new(&h.app);
    for bad in [
        "//evil.example.com",
        "https://evil.example.com/x",
        "/\\evil.example.com",
        "inbox",
    ] {
        let reply = browser
            .get(&format!("/bff/login?return_to={}", urlencode(bad)))
            .await;
        reply.assert_problem(StatusCode::BAD_REQUEST, "invalid_return_to");
        assert!(reply.set_cookie(LOGIN).is_none());
    }
    assert_eq!(
        count(&h.db, "SELECT count(*) FROM bff.login_transactions").await,
        0
    );
}

// --- callback ------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn callback_starts_a_session_and_lands_with_a_same_origin_page(db: PgPool) {
    let h = harness(db).await;
    let mut browser = Browser::new(&h.app);
    let reply = browser.sign_in(&h.idp, "/inbox?a=1&b=2").await;

    // Not a redirect: a browser would not send the new Strict cookie on a
    // redirect at the end of a cross-site chain. The page moves on itself.
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    assert_eq!(reply.header(header::LOCATION), None);
    assert_eq!(
        reply.header(header::CONTENT_TYPE),
        Some("text/html; charset=utf-8")
    );
    assert!(
        reply
            .body
            .contains(r#"<meta http-equiv="refresh" content="0;url=/inbox?a=1&amp;b=2">"#),
        "{}",
        reply.body
    );
    assert!(!reply.body.contains("<script"));
    assert_eq!(reply.header(header::CACHE_CONTROL), Some("no-store"));
    assert_eq!(reply.header(header::REFERRER_POLICY), Some("no-referrer"));
    assert!(
        reply
            .header(header::CONTENT_SECURITY_POLICY)
            .unwrap()
            .starts_with("default-src 'none'")
    );

    let session = reply.set_cookie(SESSION).unwrap();
    assert!(
        session.ends_with("; HttpOnly; SameSite=Strict; Secure"),
        "{session}"
    );
    assert!(session.contains("; Path=/; Max-Age=2592000;"), "{session}");
    assert!(!session.contains("Domain"), "{session}");
    let login = reply.set_cookie(LOGIN).unwrap();
    assert!(
        login.contains("Max-Age=0"),
        "the login cookie is cleared: {login}"
    );

    // Stored: a hash of the cookie, never the cookie; tokens encrypted.
    let token = browser.cookie(SESSION).unwrap().to_owned();
    let row: SessionRow = sqlx::query_as(
        "SELECT token_hash, subject, idp_session_id, access_token, refresh_token, user_agent \
             FROM bff.sessions",
    )
    .fetch_one(&h.db)
    .await
    .unwrap();
    assert_eq!(row.0, campbooks_kernel::crypto::hash(&token));
    assert_eq!(row.1, "test-user");
    assert_eq!(row.2.as_deref(), Some("test-sid-1"));
    assert!(!String::from_utf8_lossy(&row.3).contains("eyJ"));
    assert!(row.4.is_some());
    assert_eq!(
        count(&h.db, "SELECT count(*) FROM bff.login_transactions").await,
        0
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn callback_refuses_an_unknown_state(db: PgPool) {
    let h = harness(db).await;
    let mut browser = Browser::new(&h.app);
    let login = browser.get("/bff/login").await;
    let callback = h.idp.authorize(login.header(header::LOCATION).unwrap());
    let params = query(&callback);
    let forged = format!(
        "/bff/callback?code={}&state=forged",
        urlencode(&params["code"])
    );
    let reply = browser.get(&forged).await;
    reply.assert_problem(StatusCode::BAD_REQUEST, "invalid_state");
    assert!(reply.set_cookie(SESSION).is_none());
    assert!(
        browser.cookie(LOGIN).is_none(),
        "the login cookie is cleared anyway"
    );
    browser
        .get("/bff/callback")
        .await
        .assert_problem(StatusCode::BAD_REQUEST, "invalid_state");
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_state_works_once(db: PgPool) {
    let h = harness(db).await;
    let mut browser = Browser::new(&h.app);
    let login = browser.get("/bff/login").await;
    let callback = path_and_query(&h.idp.authorize(login.header(header::LOCATION).unwrap()));
    let login_cookie = browser.cookie(LOGIN).unwrap().to_owned();
    assert_eq!(browser.get(&callback).await.status, StatusCode::OK);

    // Replayed, even with the login cookie put back.
    let mut replay = Browser::new(&h.app);
    replay.jar.insert(LOGIN.to_owned(), login_cookie);
    replay
        .get(&callback)
        .await
        .assert_problem(StatusCode::BAD_REQUEST, "invalid_state");
    assert_eq!(count(&h.db, "SELECT count(*) FROM bff.sessions").await, 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_sign_in_finishes_only_in_the_browser_that_started_it(db: PgPool) {
    let h = harness(db).await;
    // An attacker starts a sign-in and lures the victim to its callback URL
    // (login CSRF): the victim's browser has no matching login cookie.
    let mut attacker = Browser::new(&h.app);
    let login = attacker.get("/bff/login").await;
    let callback = path_and_query(&h.idp.authorize(login.header(header::LOCATION).unwrap()));
    let mut victim = Browser::new(&h.app);
    victim
        .get(&callback)
        .await
        .assert_problem(StatusCode::BAD_REQUEST, "invalid_state");
    assert_eq!(count(&h.db, "SELECT count(*) FROM bff.sessions").await, 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn callback_refuses_an_expired_sign_in(db: PgPool) {
    let h = harness(db).await;
    let mut browser = Browser::new(&h.app);
    let login = browser.get("/bff/login").await;
    let callback = path_and_query(&h.idp.authorize(login.header(header::LOCATION).unwrap()));
    exec(
        &h.db,
        "UPDATE bff.login_transactions SET expires_at = now() - interval '1 second'",
    )
    .await;
    browser
        .get(&callback)
        .await
        .assert_problem(StatusCode::BAD_REQUEST, "invalid_state");
}

#[sqlx::test(migrations = "../../migrations")]
async fn callback_refuses_an_id_token_for_another_nonce(db: PgPool) {
    let h = harness(db).await;
    let mut browser = Browser::new(&h.app);
    let login = browser.get("/bff/login").await;
    let callback = path_and_query(&h.idp.authorize(login.header(header::LOCATION).unwrap()));
    // The IdP's ID token carries the nonce of the request; the sign-in on
    // record now expects another one.
    exec(
        &h.db,
        "UPDATE bff.login_transactions SET nonce = 'another-nonce'",
    )
    .await;
    let reply = browser.get(&callback).await;
    reply.assert_problem(StatusCode::BAD_REQUEST, "invalid_id_token");
    assert!(reply.set_cookie(SESSION).is_none());
    assert_eq!(count(&h.db, "SELECT count(*) FROM bff.sessions").await, 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn callback_reports_idp_errors_and_mix_ups(db: PgPool) {
    let h = harness(db).await;
    let mut browser = Browser::new(&h.app);
    let login = browser.get("/bff/login").await;
    let state = query(login.header(header::LOCATION).unwrap())["state"].clone();
    browser
        .get(&format!(
            "/bff/callback?error=access_denied&state={}",
            urlencode(&state)
        ))
        .await
        .assert_problem(StatusCode::BAD_REQUEST, "login_failed");

    // A response naming another issuer (RFC 9207 mix-up defence).
    let login = browser.get("/bff/login").await;
    let callback = h.idp.authorize(login.header(header::LOCATION).unwrap());
    let params = query(&callback);
    browser
        .get(&format!(
            "/bff/callback?code={}&state={}&iss={}",
            urlencode(&params["code"]),
            urlencode(&params["state"]),
            urlencode("https://evil.example.com/")
        ))
        .await
        .assert_problem(StatusCode::BAD_REQUEST, "login_failed");
    assert_eq!(count(&h.db, "SELECT count(*) FROM bff.sessions").await, 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn signing_in_again_replaces_the_session(db: PgPool) {
    let h = harness(db).await;
    let mut browser = Browser::new(&h.app);
    browser.sign_in(&h.idp, "/").await;
    let first = browser.cookie(SESSION).unwrap().to_owned();
    browser.sign_in(&h.idp, "/").await;
    assert_ne!(browser.cookie(SESSION).unwrap(), first);
    assert_eq!(count(&h.db, "SELECT count(*) FROM bff.sessions").await, 1);
}

// --- session -------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn session_says_who_is_signed_in_and_never_a_token(db: PgPool) {
    let h = harness(db).await;
    h.idp.set_user(TestUser {
        subject: "user-42".into(),
        name: "Ada Lovelace".into(),
        email: "ada@example.com".into(),
    });
    let mut browser = Browser::new(&h.app);
    let anonymous = browser.get("/bff/session").await;
    assert_eq!(anonymous.status, StatusCode::OK);
    assert_eq!(anonymous.json(), json!({ "authenticated": false }));
    assert_eq!(anonymous.header(header::CACHE_CONTROL), Some("no-store"));

    browser.sign_in(&h.idp, "/").await;
    let reply = browser.get("/bff/session").await;
    let body = reply.json();
    assert_eq!(body["authenticated"], true);
    assert_eq!(
        body["user"],
        json!({ "sub": "user-42", "name": "Ada Lovelace", "email": "ada@example.com" })
    );
    assert!(body["expires_at"].is_string());
    let keys: Vec<_> = body.as_object().unwrap().keys().cloned().collect();
    assert_eq!(keys, ["authenticated", "expires_at", "user"]);
    // No JWT, no refresh token, no session id anywhere in the body.
    assert!(!reply.body.contains("eyJ"), "{}", reply.body);
    assert!(!reply.body.contains(browser.cookie(SESSION).unwrap()));
    assert!(!reply.body.to_lowercase().contains("token"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn idle_and_absolute_timeouts_end_a_session(db: PgPool) {
    let h = harness(db).await;
    let mut browser = Browser::new(&h.app);
    browser.sign_in(&h.idp, "/").await;
    exec(
        &h.db,
        "UPDATE bff.sessions SET idle_expires_at = now() - interval '1 second'",
    )
    .await;
    let reply = browser.get("/bff/session").await;
    assert_eq!(reply.json()["authenticated"], false);
    assert!(reply.set_cookie(SESSION).unwrap().contains("Max-Age=0"));

    browser.sign_in(&h.idp, "/").await;
    exec(
        &h.db,
        "UPDATE bff.sessions SET expires_at = now() - interval '1 second'",
    )
    .await;
    assert_eq!(
        browser.get("/bff/session").await.json()["authenticated"],
        false
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn session_ids_rotate_with_a_short_grace(db: PgPool) {
    let h = harness(db).await;
    let mut browser = Browser::new(&h.app);
    browser.sign_in(&h.idp, "/").await;
    let old = browser.cookie(SESSION).unwrap().to_owned();
    assert!(
        browser
            .get("/bff/session")
            .await
            .set_cookie(SESSION)
            .is_none(),
        "not due yet"
    );

    exec(
        &h.db,
        "UPDATE bff.sessions SET rotated_at = now() - interval '2 hours'",
    )
    .await;
    let reply = browser.get("/bff/session").await;
    assert_eq!(reply.json()["authenticated"], true);
    let new = browser.cookie(SESSION).unwrap().to_owned();
    assert_ne!(new, old);
    assert!(
        reply
            .set_cookie(SESSION)
            .unwrap()
            .contains("SameSite=Strict")
    );

    // A request still carrying the old id (in flight during the rotation)
    // works for the grace period, and does not rotate again.
    let mut stale = Browser::new(&h.app);
    stale.jar.insert(SESSION.to_owned(), old.clone());
    let reply = stale.get("/bff/session").await;
    assert_eq!(reply.json()["authenticated"], true);
    assert!(reply.set_cookie(SESSION).is_none());
    exec(
        &h.db,
        "UPDATE bff.sessions SET rotated_at = now() - interval '3 minutes'",
    )
    .await;
    assert_eq!(
        stale.get("/bff/session").await.json()["authenticated"],
        false
    );
    assert_eq!(
        browser.get("/bff/session").await.json()["authenticated"],
        true
    );
}

// --- the API -------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn api_calls_carry_the_sessions_token_in_process(db: PgPool) {
    let h = harness(db).await;
    let mut browser = Browser::new(&h.app);
    browser.sign_in(&h.idp, "/").await;

    let reply = browser.get("/bff/api/v1/test/whoami?x=1").await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    let seen = reply.json();
    assert_eq!(seen["subject"], "test-user");
    assert_eq!(seen["client_id"], "campbooks-web");
    assert_eq!(
        seen["cookie"], false,
        "the API never sees the browser's cookies"
    );

    let reply = browser.post("/bff/api/v1/test/whoami", r#"{"a":1}"#).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    assert_eq!(reply.json()["method"], "POST");
    assert_eq!(reply.json()["body"], r#"{"a":1}"#);
    assert_eq!(reply.json()["csrf"], false);

    // Public API routes and API errors come back as they are.
    assert_eq!(
        browser.get("/bff/api/v1/system/version").await.status,
        StatusCode::OK
    );
    let missing = browser.get("/bff/api/v1/nope").await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
    assert_eq!(
        missing.header(header::CONTENT_TYPE),
        Some("application/problem+json")
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn the_admin_api_is_reached_through_the_bff_by_administrators_only(db: PgPool) {
    let h = harness(db).await;
    let mut browser = Browser::new(&h.app);
    browser
        .get("/bff/admin/v1/system/overview")
        .await
        .assert_problem(StatusCode::UNAUTHORIZED, "session_required");
    browser.sign_in(&h.idp, "/").await;

    // A member: the admin API does not exist for them.
    browser
        .get("/bff/admin/v1/system/overview")
        .await
        .assert_problem(StatusCode::NOT_FOUND, "not_found");

    // The operator grants a profile from the CLI: the next request works.
    let mut conn = h.db.acquire().await.unwrap();
    campbooks_admin::cli::grant(
        &mut conn,
        "test-user",
        &[campbooks_kernel::admin::Permission::Super],
        None,
        chrono::Utc::now(),
    )
    .await
    .unwrap();
    let reply = browser.get("/bff/admin/v1/system/overview").await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    assert_eq!(reply.json()["version"], env!("CARGO_PKG_VERSION"));
    let audited = &campbooks_admin::cli::log(&mut conn, 1).await.unwrap()[0];
    assert_eq!(
        (
            audited.actor.as_str(),
            audited.via.as_str(),
            audited.operation.as_str(),
            audited.status
        ),
        ("test-user", "api", "system.admin_overview", Some(200))
    );

    // Unsafe methods need the CSRF header, as on `/bff/api/*`.
    browser
        .send(
            Request::post("/bff/admin/v1/system/overview"),
            Body::empty(),
        )
        .await
        .assert_problem(StatusCode::FORBIDDEN, "csrf_check_failed");
    // The admin API is not the API: nothing of it under `/bff/api/*`.
    browser
        .get("/bff/api/v1/system/overview")
        .await
        .assert_problem(StatusCode::NOT_FOUND, "not_found");

    // An operation with a path parameter works behind the BFF's own wildcard
    // (`/bff/admin/{*path}`): the handler's `id` is not confused with `path`.
    let dead: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO kernel.jobs (queue, kind, payload, state, attempts, max_attempts, finished_at)
         VALUES ('default', 'test.dead', '{}', 'failed', 3, 3, now()) RETURNING id",
    )
    .fetch_one(&h.db)
    .await
    .unwrap();
    let reply = browser
        .post(&format!("/bff/admin/v1/system/jobs/{dead}/retry"), "")
        .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    assert_eq!(reply.json()["outcome"], "applied");

    // Revoked: gone at once.
    campbooks_admin::cli::revoke(&mut conn, "test-user", &[], chrono::Utc::now())
        .await
        .unwrap();
    browser
        .get("/bff/admin/v1/system/overview")
        .await
        .assert_problem(StatusCode::NOT_FOUND, "not_found");
}

#[sqlx::test(migrations = "../../migrations")]
async fn signing_in_again_passes_max_age_to_the_identity_provider(db: PgPool) {
    let h = harness(db).await;
    let mut browser = Browser::new(&h.app);
    let reply = browser
        .get("/bff/login?return_to=%2Fadmin%2Fsystem&max_age=0")
        .await;
    assert_eq!(reply.status, StatusCode::SEE_OTHER, "{}", reply.body);
    let params = query(reply.header(header::LOCATION).unwrap());
    assert_eq!(params["max_age"], "0");
    let plain = browser.get("/bff/login").await;
    assert!(!query(plain.header(header::LOCATION).unwrap()).contains_key("max_age"));
}

/// `POST /bff/api/v1/workspaces` with a fixed `Idempotency-Key`.
async fn create_workspace(browser: &mut Browser, name: &str) -> Reply {
    browser
        .send(
            Request::post("/bff/api/v1/workspaces")
                .header(CSRF_HEADER, "1")
                .header("sec-fetch-site", "same-origin")
                .header(header::CONTENT_TYPE, "application/json")
                .header(idempotency::HEADER, "bff-create-1"),
            Body::from(json!({ "name": name, "kind": "personal" }).to_string()),
        )
        .await
}

#[sqlx::test(migrations = "../../migrations")]
async fn the_change_feed_streams_through_the_bff(db: PgPool) {
    let h = harness_with(db, ORIGIN, |bff| {
        bff.scopes = Some("openid workspaces:read workspaces:write".to_owned());
    })
    .await;
    let mut browser = Browser::new(&h.app);
    browser.sign_in(&h.idp, "/").await;
    let created = create_workspace(&mut browser, "Household").await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.body);
    let ws = created.json()["id"].as_str().unwrap().to_owned();

    // A GET: no CSRF header needed, as `EventSource` cannot send one. The
    // stream ends after the configured two seconds, so it can be read whole.
    let reply = browser
        .send(
            Request::get(format!("/bff/api/v1/w/{ws}/changes")),
            Body::empty(),
        )
        .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    assert_eq!(
        reply.header(header::CONTENT_TYPE),
        Some("text/event-stream")
    );
    assert!(reply.body.contains("event: ready"), "{}", reply.body);
    assert!(reply.body.contains("event: closing"), "{}", reply.body);

    // Not a member of another workspace: 404, as on the API.
    let other = browser
        .send(
            Request::get(format!("/bff/api/v1/w/{}/changes", uuid::Uuid::now_v7())),
            Body::empty(),
        )
        .await;
    assert_eq!(other.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../migrations")]
async fn api_calls_go_through_the_idempotency_layer(db: PgPool) {
    let h = harness_with(db, ORIGIN, |bff| {
        bff.scopes = Some("openid workspaces:write".to_owned());
    })
    .await;
    let mut browser = Browser::new(&h.app);
    browser.sign_in(&h.idp, "/").await;

    let first = create_workspace(&mut browser, "Household").await;
    assert_eq!(first.status, StatusCode::CREATED, "{}", first.body);
    assert_eq!(first.header(idempotency::REPLAYED_HEADER), None);

    // The same key and request: the stored answer, not a second workspace.
    let again = create_workspace(&mut browser, "Household").await;
    assert_eq!(again.status, StatusCode::CREATED, "{}", again.body);
    assert_eq!(again.header(idempotency::REPLAYED_HEADER), Some("true"));
    assert_eq!(again.json()["id"], first.json()["id"]);
    let workspaces: i64 = sqlx::query_scalar("SELECT count(*) FROM workspaces.workspaces")
        .fetch_one(&h.db)
        .await
        .unwrap();
    assert_eq!(workspaces, 1);

    // The same key for another request is refused.
    let other = create_workspace(&mut browser, "Office").await;
    assert_eq!(
        other.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        other.body
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_browser_supplied_authorization_header_is_replaced(db: PgPool) {
    let h = harness(db).await;
    let mut browser = Browser::new(&h.app);
    browser.sign_in(&h.idp, "/").await;
    let mut claims = h.idp.claims("test:read");
    claims["sub"] = "someone-else".into();
    let forged = h.idp.token(&claims);
    let reply = browser
        .send(
            Request::get("/bff/api/v1/test/whoami")
                .header(header::AUTHORIZATION, format!("Bearer {forged}")),
            Body::empty(),
        )
        .await;
    assert_eq!(reply.json()["subject"], "test-user");
}

#[sqlx::test(migrations = "../../migrations")]
async fn api_without_a_session_is_401_session_required(db: PgPool) {
    let h = harness(db).await;
    let mut browser = Browser::new(&h.app);
    browser
        .get("/bff/api/v1/system/version")
        .await
        .assert_problem(StatusCode::UNAUTHORIZED, "session_required");
    browser.jar.insert(SESSION.to_owned(), "made-up".to_owned());
    let reply = browser.get("/bff/api/v1/test/whoami").await;
    reply.assert_problem(StatusCode::UNAUTHORIZED, "session_required");
    assert!(reply.set_cookie(SESSION).unwrap().contains("Max-Age=0"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn unsafe_requests_need_the_csrf_header_and_the_same_origin(db: PgPool) {
    let h = harness(db).await;
    let mut browser = Browser::new(&h.app);
    browser.sign_in(&h.idp, "/").await;
    let cases: [&[(&str, &str)]; 5] = [
        &[("sec-fetch-site", "same-origin")],
        &[(CSRF_HEADER, "1"), ("sec-fetch-site", "cross-site")],
        &[(CSRF_HEADER, "1"), ("sec-fetch-site", "same-site")],
        &[(CSRF_HEADER, "1"), ("origin", "https://evil.example.com")],
        &[(CSRF_HEADER, "1")],
    ];
    for headers in cases {
        for uri in ["/bff/api/v1/test/whoami", "/bff/logout"] {
            let mut request = Request::post(uri);
            for (name, value) in headers {
                request = request.header(*name, *value);
            }
            let reply = browser.send(request, Body::empty()).await;
            reply.assert_problem(StatusCode::FORBIDDEN, "csrf_check_failed");
        }
    }
    // Without fetch metadata, a matching Origin passes.
    let reply = browser
        .send(
            Request::delete("/bff/api/v1/test/whoami")
                .header(CSRF_HEADER, "1")
                .header(header::ORIGIN, ORIGIN),
            Body::empty(),
        )
        .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    // Safe methods need neither.
    assert_eq!(
        browser.get("/bff/api/v1/test/whoami").await.status,
        StatusCode::OK
    );
    assert_eq!(
        count(&h.db, "SELECT count(*) FROM bff.sessions").await,
        1,
        "logout never ran"
    );
}

// --- refresh -------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn an_expiring_access_token_is_refreshed_and_the_refresh_token_rotates(db: PgPool) {
    let h = harness(db).await;
    let mut browser = Browser::new(&h.app);
    browser.sign_in(&h.idp, "/").await;
    let before: Vec<u8> = sqlx::query_scalar("SELECT refresh_token FROM bff.sessions")
        .fetch_one(&h.db)
        .await
        .unwrap();

    // Not due: no refresh (some issuers end the session on an early one).
    browser.get("/bff/api/v1/test/whoami").await;
    assert_eq!(h.idp.refreshes(), 0);

    exec(
        &h.db,
        "UPDATE bff.sessions SET access_expires_at = now() + interval '20 seconds'",
    )
    .await;
    let reply = browser.get("/bff/api/v1/test/whoami").await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    assert_eq!(h.idp.refreshes(), 1);
    let (after, expires_in): (Vec<u8>, f64) = sqlx::query_as(
        "SELECT refresh_token, extract(epoch FROM access_expires_at - now())::float8 \
         FROM bff.sessions",
    )
    .fetch_one(&h.db)
    .await
    .unwrap();
    assert_ne!(after, before, "the rotated refresh token is stored");
    assert!(expires_in > 200.0, "{expires_in}");

    browser.get("/bff/api/v1/test/whoami").await;
    assert_eq!(h.idp.refreshes(), 1, "fresh again");
}

#[sqlx::test(migrations = "../../migrations")]
async fn concurrent_requests_refresh_once(db: PgPool) {
    let h = harness(db).await;
    let mut browser = Browser::new(&h.app);
    browser.sign_in(&h.idp, "/").await;
    exec(
        &h.db,
        "UPDATE bff.sessions SET access_expires_at = now() - interval '1 second'",
    )
    .await;

    let mut requests = tokio::task::JoinSet::new();
    for _ in 0..8 {
        let mut browser = browser.clone();
        requests.spawn(async move { browser.get("/bff/api/v1/test/whoami").await.status });
    }
    while let Some(status) = requests.join_next().await {
        assert_eq!(status.unwrap(), StatusCode::OK);
    }
    assert_eq!(h.idp.refreshes(), 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_refused_refresh_ends_the_session(db: PgPool) {
    let h = harness(db).await;
    let mut browser = Browser::new(&h.app);
    browser.sign_in(&h.idp, "/").await;
    h.idp.revoke_refresh_tokens();
    exec(
        &h.db,
        "UPDATE bff.sessions SET access_expires_at = now() - interval '1 second'",
    )
    .await;
    let reply = browser.get("/bff/api/v1/test/whoami").await;
    reply.assert_problem(StatusCode::UNAUTHORIZED, "session_required");
    assert!(browser.cookie(SESSION).is_none());
    assert_eq!(count(&h.db, "SELECT count(*) FROM bff.sessions").await, 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_changed_secret_key_ends_sessions_instead_of_failing(db: PgPool) {
    let h = harness(db.clone()).await;
    let mut browser = Browser::new(&h.app);
    browser.sign_in(&h.idp, "/").await;

    // The same database behind a server with another CAMPBOOKS_SECRET_KEY.
    let mut config = testing::config("postgres://test@localhost/test");
    config.auth = h.idp.auth_config();
    ORIGIN.clone_into(&mut config.public_url);
    let bff = BffConfig {
        client_secret: Some(SecretString::from(WEB_CLIENT_SECRET)),
        ..BffConfig::default()
    };
    let Ok(Loaded::Enabled(settings)) = config::settings(
        &config,
        bff,
        Some(campbooks_kernel::crypto::SecretKey::from_bytes([9; 32])),
        &[],
    ) else {
        panic!("valid settings");
    };
    browser.app = app(
        AppState::new(testing::app_role_pool(&db).await, config),
        Some(*settings),
    );
    browser
        .get("/bff/api/v1/test/whoami")
        .await
        .assert_problem(StatusCode::UNAUTHORIZED, "session_required");
    assert_eq!(count(&h.db, "SELECT count(*) FROM bff.sessions").await, 0);
}

// --- logout --------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn logout_revokes_deletes_and_clears(db: PgPool) {
    let h = harness(db).await;
    let mut browser = Browser::new(&h.app);
    browser.sign_in(&h.idp, "/").await;
    let reply = browser.post("/bff/logout", "").await;

    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    assert_eq!(reply.header(header::CACHE_CONTROL), Some("no-store"));
    assert!(reply.set_cookie(SESSION).unwrap().contains("Max-Age=0"));
    let logout_url = reply.json()["logout_url"].as_str().unwrap().to_owned();
    assert!(logout_url.starts_with(&format!("{}oidc/logout?", h.idp.issuer())));
    let params = query(&logout_url);
    assert_eq!(params["client_id"], "campbooks-web");
    assert_eq!(params["post_logout_redirect_uri"], format!("{ORIGIN}/"));
    assert!(
        !logout_url.contains("id_token_hint"),
        "no token reaches the browser"
    );

    assert_eq!(count(&h.db, "SELECT count(*) FROM bff.sessions").await, 0);
    let revoked = h.idp.revoked_tokens();
    assert_eq!(revoked.len(), 1);
    assert!(!h.idp.is_live_refresh_token(&revoked[0]));
    assert_eq!(
        browser.get("/bff/session").await.json()["authenticated"],
        false
    );

    // Without a session it still answers (idempotent).
    assert_eq!(browser.post("/bff/logout", "").await.status, StatusCode::OK);
}

// --- back-channel logout -------------------------------------------------------

async fn backchannel(app: &Router, token: &str) -> Reply {
    send(
        app,
        Request::post("/bff/backchannel-logout")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(format!("logout_token={}", urlencode(token))))
            .unwrap(),
    )
    .await
}

#[sqlx::test(migrations = "../../migrations")]
async fn backchannel_logout_ends_the_named_sessions(db: PgPool) {
    let h = harness(db).await;
    let (mut laptop, mut phone) = (Browser::new(&h.app), Browser::new(&h.app));
    laptop.sign_in(&h.idp, "/").await; // IdP session test-sid-1
    phone.sign_in(&h.idp, "/").await; // test-sid-2

    let token = h.idp.logout_token(
        &h.idp
            .logout_token_claims(Some("test-user"), Some("test-sid-1")),
    );
    let reply = backchannel(&h.app, &token).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    assert_eq!(
        laptop.get("/bff/session").await.json()["authenticated"],
        false
    );
    assert_eq!(
        phone.get("/bff/session").await.json()["authenticated"],
        true
    );

    // `sub` alone: every session of the user.
    let token = h
        .idp
        .logout_token(&h.idp.logout_token_claims(Some("test-user"), None));
    assert_eq!(backchannel(&h.app, &token).await.status, StatusCode::OK);
    assert_eq!(
        phone.get("/bff/session").await.json()["authenticated"],
        false
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_suspended_accounts_web_sessions_end_at_once(db: PgPool) {
    use campbooks_identity::events::AccountSuspended;
    use campbooks_kernel::jobs::Worker;

    let h = harness(db).await;
    let (suspended, other) = (uuid::Uuid::now_v7(), uuid::Uuid::now_v7());
    let mut browsers = Vec::new();
    for account in [suspended, suspended, other] {
        h.idp.set_user(TestUser {
            subject: account.to_string(),
            ..TestUser::default()
        });
        let mut browser = Browser::new(&h.app);
        browser.sign_in(&h.idp, "/").await;
        browsers.push(browser);
    }

    let pool = testing::app_role_pool(&h.db).await;
    let state = crate::with_ports(
        AppState::new(
            pool.clone(),
            testing::config("postgres://test@localhost/test"),
        )
        .with_registry(crate::registry().unwrap()),
    );
    let mut tx = pool.begin().await.unwrap();
    state
        .events
        .publish(&mut tx, &AccountSuspended { account: suspended })
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let stats = Worker::new(state).run_until_idle().await;
    assert_eq!(stats.succeeded, 1, "{stats:?}");

    let mut signed_in = Vec::new();
    for browser in &mut browsers {
        signed_in.push(browser.get("/bff/session").await.json()["authenticated"].clone());
    }
    assert_eq!(signed_in, [false, false, true]);
}

#[sqlx::test(migrations = "../../migrations")]
async fn backchannel_logout_refuses_bad_tokens(db: PgPool) {
    let h = harness(db).await;
    let mut browser = Browser::new(&h.app);
    browser.sign_in(&h.idp, "/").await;
    let id_token = h.idp.id_token(&h.idp.id_token_claims(
        "test-user",
        &campbooks_kernel::crypto::random_token(),
        "test-sid-1",
    ));
    let unsigned = h
        .idp
        .unpublished_token(&h.idp.logout_token_claims(Some("test-user"), None));
    for token in [id_token.as_str(), unsigned.as_str(), "garbage"] {
        backchannel(&h.app, token)
            .await
            .assert_problem(StatusCode::BAD_REQUEST, "invalid_logout_token");
    }
    assert_eq!(
        browser.get("/bff/session").await.json()["authenticated"],
        true
    );
}

// --- pruning -------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn pruning_deletes_ended_sessions_and_stale_sign_ins(db: PgPool) {
    let h = harness(db).await;
    let mut live = Browser::new(&h.app);
    live.sign_in(&h.idp, "/").await;
    let mut idle = Browser::new(&h.app);
    idle.sign_in(&h.idp, "/").await;
    let mut old = Browser::new(&h.app);
    old.sign_in(&h.idp, "/").await;
    let hash = |b: &Browser| campbooks_kernel::crypto::hash(b.cookie(SESSION).unwrap());
    sqlx::query("UPDATE bff.sessions SET idle_expires_at = now() - interval '1 minute' WHERE token_hash = $1")
        .bind(hash(&idle))
        .execute(&h.db)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE bff.sessions SET expires_at = now() - interval '1 minute' WHERE token_hash = $1",
    )
    .bind(hash(&old))
    .execute(&h.db)
    .await
    .unwrap();
    Browser::new(&h.app).get("/bff/login").await;
    Browser::new(&h.app).get("/bff/login").await;
    exec(
        &h.db,
        "UPDATE bff.login_transactions SET expires_at = now() - interval '1 minute' \
         WHERE ctid IN (SELECT ctid FROM bff.login_transactions LIMIT 1)",
    )
    .await;

    let pruned = store::prune(&h.db, chrono::Utc::now()).await.unwrap();
    assert_eq!(pruned, (2, 1));
    assert_eq!(live.get("/bff/session").await.json()["authenticated"], true);
    assert_eq!(
        count(&h.db, "SELECT count(*) FROM bff.login_transactions").await,
        1
    );
}

#[test]
fn pruning_is_scheduled() {
    let registry = crate::registry().unwrap();
    assert!(
        registry
            .schedule_names()
            .contains(&<PruneSessions as campbooks_kernel::Job>::NAME)
    );
}

// --- configuration -------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn plain_http_drops_secure_and_the_host_prefix(db: PgPool) {
    let h = harness_with(db, "http://localhost:8080", |_| {}).await;
    let mut browser = Browser::new(&h.app);
    let login = browser.get("/bff/login").await;
    assert!(login.set_cookie("campbooks_login").is_some());
    let reply = browser.sign_in(&h.idp, "/").await;
    let session = reply.set_cookie("campbooks_session").unwrap();
    assert!(
        session.ends_with("; HttpOnly; SameSite=Strict"),
        "{session}"
    );
    assert_eq!(
        browser.get("/bff/session").await.json()["authenticated"],
        true
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn the_server_reaches_the_idp_at_its_internal_address(db: PgPool) {
    // The issuer the browser uses is unreachable from the server; the
    // internal URL is the real one.
    let idp = TestIdp::start().await;
    let public = "https://auth.example.com/auth/v1/";
    let mut config = testing::config("postgres://test@localhost/test");
    config.auth.issuer = Some(public.to_owned());
    ORIGIN.clone_into(&mut config.public_url);
    let bff = BffConfig {
        client_secret: Some(SecretString::from(WEB_CLIENT_SECRET)),
        idp_internal_url: Some(idp.issuer().to_owned()),
        ..BffConfig::default()
    };
    let Ok(Loaded::Enabled(settings)) = config::settings(
        &config,
        bff,
        Some(campbooks_kernel::crypto::SecretKey::from_bytes([7; 32])),
        &[],
    ) else {
        panic!("valid settings");
    };
    let app = app(AppState::new(db, config), Some(*settings));
    // Discovery names the test IdP's own issuer, not the configured public
    // one, so the BFF refuses to trust it.
    Browser::new(&app)
        .get("/bff/login")
        .await
        .assert_problem(StatusCode::SERVICE_UNAVAILABLE, "idp_unavailable");
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_client_secret_file_is_read_when_needed(db: PgPool) {
    let dir = std::env::temp_dir().join(format!("campbooks-bff-secret-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("campbooks-web.client-secret");
    let path = file.clone();
    let h = harness_with(db, ORIGIN, move |bff| {
        bff.client_secret = None;
        bff.client_secret_file = Some(path);
    })
    .await;
    let mut browser = Browser::new(&h.app);
    // Not written yet (the IdP provisioning runs after the server starts).
    let login = browser.get("/bff/login").await;
    let callback = path_and_query(&h.idp.authorize(login.header(header::LOCATION).unwrap()));
    browser
        .get(&callback)
        .await
        .assert_problem(StatusCode::SERVICE_UNAVAILABLE, "idp_unavailable");
    std::fs::write(&file, format!("{WEB_CLIENT_SECRET}\n")).unwrap();
    let reply = browser.sign_in(&h.idp, "/").await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn without_web_sign_in_the_bff_answers_503() {
    let state = testing::state(testing::unreachable_pool());
    let app = app(state, None);
    let mut browser = Browser::new(&app);
    browser
        .get("/bff/login")
        .await
        .assert_problem(StatusCode::SERVICE_UNAVAILABLE, "bff_not_configured");
    browser
        .get("/bff/api/v1/system/version")
        .await
        .assert_problem(StatusCode::SERVICE_UNAVAILABLE, "bff_not_configured");
    assert_eq!(
        browser.get("/bff/session").await.json(),
        json!({ "authenticated": false })
    );
}

#[test]
fn openapi_documents_every_bff_operation_with_its_security() {
    let spec = serde_json::to_value(crate::openapi()).unwrap();
    let scheme = &spec["components"]["securitySchemes"][super::SESSION_SCHEME];
    assert_eq!(scheme["type"], "apiKey");
    assert_eq!(scheme["in"], "cookie");
    assert_eq!(scheme["name"], SESSION);
    let paths = &spec["paths"];
    assert_eq!(paths["/bff/login"]["get"]["security"], json!([{}]));
    assert_eq!(paths["/bff/callback"]["get"]["security"], json!([{}]));
    assert_eq!(
        paths["/bff/backchannel-logout"]["post"]["security"],
        json!([{}])
    );
    assert_eq!(
        paths["/bff/session"]["get"]["security"],
        json!([{}, { "session": [] }])
    );
    assert_eq!(
        paths["/bff/logout"]["post"]["security"],
        json!([{ "session": [] }])
    );
    for method in ["get", "post", "put", "patch", "delete"] {
        assert_eq!(
            paths["/bff/api/{path}"][method]["security"],
            json!([{ "session": [] }]),
            "{method}"
        );
    }
}
