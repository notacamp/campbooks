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
    rate_limit::api::ApiLimits,
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

/// A session's id, current and previous token hashes, and sealed access,
/// refresh and ID tokens, as stored.
type StoredSession = (
    uuid::Uuid,
    Vec<u8>,
    Option<Vec<u8>>,
    Vec<u8>,
    Option<Vec<u8>>,
    Vec<u8>,
);

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
    harness_limited(db, origin, tweak, |state| state).await
}

/// [`harness_with`], with the app state changed by `with` (the per-caller
/// rate limits, the metrics).
async fn harness_limited(
    db: PgPool,
    origin: &str,
    tweak: impl FnOnce(&mut BffConfig),
    with: impl FnOnce(AppState) -> AppState,
) -> Harness {
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
    let state = with(crate::with_ports(AppState::new(
        testing::app_role_pool(&db).await,
        config,
    )));
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

/// The BFF's cipher under the harness's secret key.
fn cipher() -> campbooks_kernel::crypto::Cipher {
    campbooks_kernel::crypto::Cipher::new(
        &campbooks_kernel::crypto::SecretKey::from_bytes([7; 32]),
        config::KEY_PURPOSE,
    )
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
    // Stored: hashes of the state and the login cookie, the nonce and the
    // verifier sealed; nothing a database reader could replay.
    let (return_to, state_hash, nonce): (String, Vec<u8>, Vec<u8>) =
        sqlx::query_as("SELECT return_to, state_hash, nonce FROM bff.login_transactions")
            .fetch_one(&h.db)
            .await
            .unwrap();
    assert_eq!(return_to, "/inbox?tab=2");
    assert_eq!(state_hash, campbooks_kernel::crypto::hash(&params["state"]));
    assert!(
        !String::from_utf8_lossy(&nonce).contains(params["nonce"].as_str()),
        "the nonce is sealed"
    );
    assert_eq!(
        cipher()
            .open(&nonce, &super::handlers::nonce_context(&state_hash))
            .unwrap(),
        params["nonce"]
    );

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
    // record now expects another one (sealed for it like the real one).
    let state_hash: Vec<u8> = sqlx::query_scalar("SELECT state_hash FROM bff.login_transactions")
        .fetch_one(&h.db)
        .await
        .unwrap();
    sqlx::query("UPDATE bff.login_transactions SET nonce = $1")
        .bind(cipher().seal(
            "another-nonce",
            &super::handlers::nonce_context(&state_hash),
        ))
        .execute(&h.db)
        .await
        .unwrap();
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
    // The ended session's refresh token is revoked; the new one stays live
    // (campbooks-cloud#214).
    let revoked = h.idp.revoked_tokens();
    assert_eq!(revoked.len(), 1, "the earlier session's refresh token");
    assert!(!h.idp.is_live_refresh_token(&revoked[0]));
    assert_eq!(
        browser.get("/bff/api/v1/test/whoami").await.status,
        StatusCode::OK
    );
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
async fn a_session_missing_newer_scopes_ends_and_signs_in_again(db: PgPool) {
    let h = harness(db).await;
    let mut browser = Browser::new(&h.app);
    browser.sign_in(&h.idp, "/").await;
    // A session that asked for every scope keeps going.
    assert_eq!(
        browser.get("/bff/session").await.json()["authenticated"],
        true
    );
    assert_eq!(
        browser.get("/bff/api/v1/test/whoami").await.status,
        StatusCode::OK
    );

    // Signed in before a release added a scope: its tokens never get it, so
    // it ends like an expired session and the app signs in again; its
    // refresh token is revoked.
    exec(&h.db, "UPDATE bff.sessions SET scopes = 'openid'").await;
    let reply = browser.get("/bff/api/v1/test/whoami").await;
    reply.assert_problem(StatusCode::UNAUTHORIZED, "session_required");
    assert!(reply.set_cookie(SESSION).unwrap().contains("Max-Age=0"));
    assert_eq!(count(&h.db, "SELECT count(*) FROM bff.sessions").await, 0);
    let revoked = h.idp.revoked_tokens();
    assert_eq!(revoked.len(), 1, "its refresh token is revoked");
    assert!(!h.idp.is_live_refresh_token(&revoked[0]));

    // The next sign-in asks for every scope, and stays signed in.
    browser.sign_in(&h.idp, "/").await;
    assert_eq!(
        browser.get("/bff/session").await.json()["authenticated"],
        true
    );
    assert_eq!(
        browser.get("/bff/api/v1/test/whoami").await.status,
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_session_from_before_scopes_were_recorded_ends(db: PgPool) {
    let h = harness(db).await;
    let mut browser = Browser::new(&h.app);
    browser.sign_in(&h.idp, "/").await;
    exec(&h.db, "UPDATE bff.sessions SET scopes = NULL").await;
    let reply = browser.get("/bff/session").await;
    assert_eq!(reply.json()["authenticated"], false);
    assert!(reply.set_cookie(SESSION).unwrap().contains("Max-Age=0"));
    assert_eq!(count(&h.db, "SELECT count(*) FROM bff.sessions").await, 0);
    assert_eq!(h.idp.revoked_tokens().len(), 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_sign_in_keeps_the_scopes_it_started_with(db: PgPool) {
    let h = harness(db).await;
    let mut browser = Browser::new(&h.app);
    // Started before a deploy that added a scope, finished after it: the
    // session records what the sign-in asked for, so it ends on use.
    let login = browser.get("/bff/login?return_to=%2F").await;
    exec(&h.db, "UPDATE bff.login_transactions SET scopes = 'openid'").await;
    let callback = h.idp.authorize(login.header(header::LOCATION).unwrap());
    let landed = browser.get(&path_and_query(&callback)).await;
    assert_eq!(landed.status, StatusCode::OK, "{}", landed.body);
    assert_eq!(
        count(
            &h.db,
            "SELECT count(*) FROM bff.sessions WHERE scopes = 'openid'"
        )
        .await,
        1,
        "the session keeps what its sign-in asked for"
    );
    assert_eq!(
        browser.get("/bff/session").await.json()["authenticated"],
        false
    );
}

#[test]
fn asked_for_all_compares_sets() {
    use super::asked_for_all;
    assert!(asked_for_all(
        Some("openid a:read b:write"),
        "b:write openid"
    ));
    assert!(asked_for_all(Some("openid  a:read"), "a:read openid"));
    assert!(!asked_for_all(
        Some("openid a:read"),
        "openid a:read assistant:read"
    ));
    assert!(!asked_for_all(None, "openid"));
    assert!(asked_for_all(Some(""), ""));
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
    // The browser uses its new id: the rotation reached it.
    assert_eq!(
        browser.get("/bff/session").await.json()["authenticated"],
        true
    );
    // Past the grace, the old id is a copied cookie (and either copy may be
    // a thief's): the session ends for both, and its refresh token is
    // revoked at the identity provider.
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
        false,
        "the session ended everywhere"
    );
    assert_eq!(count(&h.db, "SELECT count(*) FROM bff.sessions").await, 0);
    let revoked = h.idp.revoked_tokens();
    assert_eq!(revoked.len(), 1);
    assert!(!h.idp.is_live_refresh_token(&revoked[0]));
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_lost_rotation_gets_another_instead_of_a_sign_out(db: PgPool) {
    let h = harness(db).await;
    let mut browser = Browser::new(&h.app);
    browser.sign_in(&h.idp, "/").await;
    let old = browser.cookie(SESSION).unwrap().to_owned();
    exec(
        &h.db,
        "UPDATE bff.sessions SET rotated_at = now() - interval '2 hours'",
    )
    .await;
    // The response carrying the new id is lost (an aborted fetch, a
    // reload): the browser keeps the old one.
    let rotated = browser.get("/bff/session").await;
    assert!(rotated.set_cookie(SESSION).is_some());
    let lost = browser.cookie(SESSION).unwrap().to_owned();
    browser.jar.insert(SESSION.to_owned(), old.clone());
    exec(
        &h.db,
        "UPDATE bff.sessions SET rotated_at = now() - interval '3 minutes'",
    )
    .await;

    // Minutes later the old id still opens the session, with a fresh one.
    let again = browser.get("/bff/session").await;
    assert_eq!(again.json()["authenticated"], true, "{}", again.body);
    let fresh = browser.cookie(SESSION).unwrap().to_owned();
    assert_ne!(fresh, old);
    assert_ne!(fresh, lost);
    assert_eq!(
        browser.get("/bff/session").await.json()["authenticated"],
        true
    );
    assert!(h.idp.revoked_tokens().is_empty(), "nothing revoked");

    // The lost id is retired: a copy of it presented later is a theft.
    let mut copy = Browser::new(&h.app);
    copy.jar.insert(SESSION.to_owned(), lost);
    assert_eq!(
        copy.get("/bff/session").await.json()["authenticated"],
        false
    );
    assert_eq!(
        browser.get("/bff/session").await.json()["authenticated"],
        false
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_waking_page_reissues_once_and_stays_signed_in(db: PgPool) {
    let h = harness(db).await;
    let mut browser = Browser::new(&h.app);
    browser.sign_in(&h.idp, "/").await;
    let old = browser.cookie(SESSION).unwrap().to_owned();
    exec(
        &h.db,
        "UPDATE bff.sessions SET rotated_at = now() - interval '2 hours'",
    )
    .await;
    // The rotation's response is lost.
    assert!(
        browser
            .get("/bff/session")
            .await
            .set_cookie(SESSION)
            .is_some()
    );
    exec(
        &h.db,
        "UPDATE bff.sessions SET rotated_at = now() - interval '3 minutes'",
    )
    .await;

    // The page wakes and fires several requests with the old cookie at once.
    let mut tabs: Vec<Browser> = (0..4)
        .map(|_| {
            let mut tab = Browser::new(&h.app);
            tab.jar.insert(SESSION.to_owned(), old.clone());
            tab
        })
        .collect();
    let replies =
        futures_util::future::join_all(tabs.iter_mut().map(|tab| tab.get("/bff/session"))).await;
    for reply in &replies {
        assert_eq!(reply.json()["authenticated"], true, "{}", reply.body);
    }
    let fresh: Vec<&Browser> = tabs
        .iter()
        .filter(|tab| tab.cookie(SESSION) != Some(old.as_str()))
        .collect();
    assert_eq!(fresh.len(), 1, "one reissue");
    assert_eq!(
        count(&h.db, "SELECT count(*) FROM bff.retired_tokens").await,
        1,
        "only the lost cookie is retired"
    );
    let mut woken = Browser::new(&h.app);
    woken.jar.insert(
        SESSION.to_owned(),
        fresh[0].cookie(SESSION).unwrap().to_owned(),
    );
    assert_eq!(
        woken.get("/bff/session").await.json()["authenticated"],
        true
    );
    assert!(h.idp.revoked_tokens().is_empty());
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_reissue_says_why_it_did_not(db: PgPool) {
    use campbooks_kernel::crypto::hash;

    use super::store::Reissue;

    let h = harness(db).await;
    let mut browser = Browser::new(&h.app);
    browser.sign_in(&h.idp, "/").await;
    let old = browser.cookie(SESSION).unwrap().to_owned();
    exec(
        &h.db,
        "UPDATE bff.sessions SET rotated_at = now() - interval '2 hours'",
    )
    .await;
    browser.get("/bff/session").await;
    let now = chrono::Utc::now();
    let grace_since = now - super::config::ROTATION_GRACE;
    // As the server calls it: the runtime role, through `bff.reissue_session`.
    let runtime = testing::app_role_pool(&h.db).await;
    let reissue = |previous: &str| {
        let previous = hash(previous);
        let db = runtime.clone();
        async move {
            store::reissue(&db, &previous, &hash("fresh"), now, grace_since)
                .await
                .unwrap()
        }
    };

    // The successor was presented meanwhile: the old token is a copy.
    exec(
        &h.db,
        "UPDATE bff.sessions SET token_delivered = true, rotated_at = now() - interval '3 minutes'",
    )
    .await;
    assert_eq!(reissue(&old).await, Reissue::Delivered);
    // Another request reissued a moment ago: within the new grace.
    exec(
        &h.db,
        "UPDATE bff.sessions SET token_delivered = false, rotated_at = now()",
    )
    .await;
    assert_eq!(reissue(&old).await, Reissue::WithinGrace);
    // Not this session's previous token at all.
    assert_eq!(reissue("someone-else").await, Reissue::Gone);
    // Past the grace, undelivered: reissued.
    exec(
        &h.db,
        "UPDATE bff.sessions SET rotated_at = now() - interval '3 minutes'",
    )
    .await;
    assert_eq!(reissue(&old).await, Reissue::Reissued);
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_cookie_rotations_old_still_ends_the_session(db: PgPool) {
    let h = harness(db).await;
    let mut browser = Browser::new(&h.app);
    browser.sign_in(&h.idp, "/").await;
    let first = browser.cookie(SESSION).unwrap().to_owned();
    // Two rotations while the person keeps using the app: the first token
    // is past even the previous-token grace.
    for _ in 0..2 {
        exec(
            &h.db,
            "UPDATE bff.sessions SET rotated_at = now() - interval '2 hours'",
        )
        .await;
        assert_eq!(
            browser.get("/bff/session").await.json()["authenticated"],
            true
        );
    }
    assert_ne!(browser.cookie(SESSION).unwrap(), first);
    assert_eq!(
        count(&h.db, "SELECT count(*) FROM bff.retired_tokens").await,
        1
    );

    // A copy of the first cookie, presented much later.
    let mut thief = Browser::new(&h.app);
    thief.jar.insert(SESSION.to_owned(), first);
    assert_eq!(
        thief.get("/bff/session").await.json()["authenticated"],
        false
    );
    assert_eq!(
        browser.get("/bff/session").await.json()["authenticated"],
        false,
        "the session ended for both"
    );
    assert_eq!(count(&h.db, "SELECT count(*) FROM bff.sessions").await, 0);
    assert_eq!(
        count(&h.db, "SELECT count(*) FROM bff.retired_tokens").await,
        0,
        "they go with the session"
    );
    let revoked = h.idp.revoked_tokens();
    assert_eq!(revoked.len(), 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn sign_ins_are_rate_limited_per_address(db: PgPool) {
    let h = harness(db).await;
    let from =
        |address: [u8; 4]| {
            let mut request = Request::get("/bff/login").body(Body::empty()).unwrap();
            request.extensions_mut().insert(axum::extract::ConnectInfo(
                std::net::SocketAddr::from((address, 50_000)),
            ));
            request
        };
    for _ in 0..30 {
        let started = send(&h.app, from([203, 0, 113, 7])).await;
        assert_eq!(started.status, StatusCode::SEE_OTHER, "{}", started.body);
    }
    let limited = send(&h.app, from([203, 0, 113, 7])).await;
    assert_eq!(
        limited.status,
        StatusCode::TOO_MANY_REQUESTS,
        "{}",
        limited.body
    );
    assert_eq!(limited.json()["code"], "rate_limited");
    let elsewhere = send(&h.app, from([198, 51, 100, 2])).await;
    assert_eq!(elsewhere.status, StatusCode::SEE_OTHER);
}

#[sqlx::test(migrations = "../../migrations")]
async fn signing_out_everywhere_ends_the_persons_web_sessions(db: PgPool) {
    use campbooks_identity::events::SignedOutEverywhere;

    let h = harness(db).await;
    let account = uuid::Uuid::now_v7();
    h.idp.set_user(TestUser {
        subject: account.to_string(),
        name: "Ana Lima".to_owned(),
        email: "ana@example.com".to_owned(),
    });
    let mut laptop = Browser::new(&h.app);
    laptop.sign_in(&h.idp, "/").await;
    let mut phone = Browser::new(&h.app);
    phone.sign_in(&h.idp, "/").await;
    h.idp.set_user(TestUser {
        subject: "someone-else".to_owned(),
        name: "Sam".to_owned(),
        email: "sam@example.com".to_owned(),
    });
    let mut other = Browser::new(&h.app);
    other.sign_in(&h.idp, "/").await;
    assert_eq!(count(&h.db, "SELECT count(*) FROM bff.sessions").await, 3);

    // The worker runs as the runtime role, like the server's: the handler
    // ends the sessions through `bff.end_account_sessions`.
    let state = AppState::new(
        testing::app_role_pool(&h.db).await,
        testing::config("postgres://test@localhost/test"),
    )
    .with_registry(crate::registry().unwrap());
    let mut tx = state.db.begin().await.unwrap();
    state
        .events
        .publish(&mut tx, &SignedOutEverywhere { account })
        .await
        .unwrap();
    tx.commit().await.unwrap();
    campbooks_kernel::jobs::Worker::new(state)
        .run_until_idle()
        .await;

    for browser in [&mut laptop, &mut phone] {
        assert_eq!(
            browser.get("/bff/session").await.json()["authenticated"],
            false
        );
    }
    assert_eq!(
        other.get("/bff/session").await.json()["authenticated"],
        true,
        "nobody else's"
    );
}

/// A workspace writing an account's `identity.signed_out_everywhere` and its
/// delivery into its own workspace by hand, as the runtime role, ends
/// nobody's sessions: the delivery fails, its handler never runs
/// (campbooks-cloud#207).
#[sqlx::test(migrations = "../../migrations")]
async fn a_sign_out_everywhere_forged_in_a_workspace_ends_nobody(db: PgPool) {
    let h = harness(db).await;
    let victim = uuid::Uuid::now_v7();
    h.idp.set_user(TestUser {
        subject: victim.to_string(),
        name: "Ana Lima".to_owned(),
        email: "ana@example.com".to_owned(),
    });
    let mut laptop = Browser::new(&h.app);
    laptop.sign_in(&h.idp, "/").await;

    let runtime = testing::app_role_pool(&h.db).await;
    let workspace = uuid::Uuid::now_v7().to_string();
    let mut tx = runtime.begin().await.unwrap();
    campbooks_kernel::db::set_workspace(&mut tx, &workspace)
        .await
        .unwrap();
    let job: uuid::Uuid = sqlx::query_scalar(
        "WITH e AS (
             INSERT INTO kernel.events (name, version, workspace_id, payload)
             VALUES ('identity.signed_out_everywhere', 1, $1,
                     jsonb_build_object('account', $2::text))
             RETURNING id)
         INSERT INTO kernel.jobs
             (queue, kind, payload, workspace_id, event_id, unique_key, max_attempts)
         SELECT 'events', 'bff.end_signed_out_sessions', '{}', $1, id, id::text, 3 FROM e
         RETURNING id",
    )
    .bind(&workspace)
    .bind(victim)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let state = AppState::new(
        runtime.clone(),
        testing::config("postgres://test@localhost/test"),
    )
    .with_registry(crate::registry().unwrap());
    campbooks_kernel::jobs::Worker::new(state)
        .run_until_idle()
        .await;
    let job_state: String = sqlx::query_scalar("SELECT state FROM kernel.jobs WHERE id = $1")
        .bind(job)
        .fetch_one(&h.db)
        .await
        .unwrap();
    assert_eq!(job_state, "failed");
    assert_eq!(
        laptop.get("/bff/session").await.json()["authenticated"],
        true,
        "the victim is still signed in"
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

/// Replace the session's held access token with one the API refuses as
/// invalid (as it refuses a token whose sign-in ended: a way in removed,
/// signed out on another device), while the BFF still counts it as valid
/// for minutes (campbooks-cloud#143).
async fn hold_a_refused_token(h: &Harness) {
    let mut claims = h.idp.claims("test:read");
    let past = chrono::Utc::now().timestamp() - 3_600;
    claims["iat"] = json!(past);
    claims["nbf"] = json!(past);
    claims["exp"] = json!(past + 600);
    let token = h.idp.token(&claims);
    let id: uuid::Uuid = sqlx::query_scalar("SELECT id FROM bff.sessions")
        .fetch_one(&h.db)
        .await
        .unwrap();
    sqlx::query("UPDATE bff.sessions SET access_token = $1")
        .bind(cipher().seal(&token, &super::sealed_for(id, "access_token")))
        .execute(&h.db)
        .await
        .unwrap();
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_token_the_api_refuses_is_refreshed_once_and_a_read_goes_through(db: PgPool) {
    let h = harness(db).await;
    let mut browser = Browser::new(&h.app);
    browser.sign_in(&h.idp, "/").await;
    let before = h.idp.refreshes();

    // A read: one forced refresh, then sent again with the fresh token.
    hold_a_refused_token(&h).await;
    let reply = browser.get("/bff/api/v1/test/whoami?x=1").await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    assert_eq!(h.idp.refreshes(), before + 1);
    assert_eq!(
        browser.get("/bff/api/v1/test/whoami").await.status,
        StatusCode::OK
    );
    assert_eq!(h.idp.refreshes(), before + 1, "the fresh token is kept");

    // A write is never sent twice: the refusal comes back, the fresh token
    // is held, and the app's retry goes through.
    hold_a_refused_token(&h).await;
    let reply = browser.post("/bff/api/v1/test/whoami", r#"{"a":1}"#).await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED, "{}", reply.body);
    assert!(
        reply
            .header(header::WWW_AUTHENTICATE)
            .is_some_and(|value| value.contains("error=\"invalid_token\"")),
        "the API's own refusal: {:?}",
        reply.headers
    );
    assert_eq!(h.idp.refreshes(), before + 2);
    let retried = browser.post("/bff/api/v1/test/whoami", r#"{"a":1}"#).await;
    assert_eq!(retried.status, StatusCode::OK, "{}", retried.body);
    assert_eq!(h.idp.refreshes(), before + 2);
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_refused_token_whose_refresh_is_refused_ends_the_session(db: PgPool) {
    let h = harness(db).await;
    let mut browser = Browser::new(&h.app);
    browser.sign_in(&h.idp, "/").await;
    let before = h.idp.token_requests();
    hold_a_refused_token(&h).await;
    h.idp.revoke_refresh_tokens();

    // At once, not when the held token would have expired: the app signs in
    // again. One refresh attempt, no loop.
    let reply = browser.get("/bff/api/v1/test/whoami").await;
    reply.assert_problem(StatusCode::UNAUTHORIZED, "session_required");
    assert_eq!(h.idp.token_requests(), before + 1, "one refresh attempt");
    assert_eq!(count(&h.db, "SELECT count(*) FROM bff.sessions").await, 0);
    assert_eq!(
        browser.get("/bff/session").await.json()["authenticated"],
        false
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
        .publish(
            &mut tx,
            &AccountSuspended {
                account: suspended,
                at: None,
                seq: None,
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let stats = Worker::new(state).run_until_idle().await;
    // The BFF ends the sessions; the workspaces module announces the
    // suspension in the account's workspaces (none here).
    assert_eq!((stats.succeeded, stats.failed), (2, 0), "{stats:?}");

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

// --- the tables' boundary (campbooks-cloud#212) -------------------------------

/// The runtime role the server runs as can neither read, write, empty nor
/// lock the BFF's tables: only the `bff.*` functions reach them.
#[sqlx::test(migrations = "../../migrations")]
async fn the_runtime_role_cannot_touch_the_bff_tables(db: PgPool) {
    let h = harness(db).await;
    let mut browser = Browser::new(&h.app);
    browser.sign_in(&h.idp, "/").await;
    Browser::new(&h.app).get("/bff/login").await;
    exec(
        &h.db,
        "UPDATE bff.sessions SET rotated_at = now() - interval '2 hours'",
    )
    .await;
    for _ in 0..2 {
        browser.get("/bff/session").await;
        exec(
            &h.db,
            "UPDATE bff.sessions SET rotated_at = now() - interval '2 hours'",
        )
        .await;
    }
    for table in [
        "SELECT count(*) FROM bff.sessions",
        "SELECT count(*) FROM bff.login_transactions",
        "SELECT count(*) FROM bff.retired_tokens",
    ] {
        assert!(count(&h.db, table).await > 0, "{table}: a row to protect");
    }

    let runtime = testing::app_role_pool(&h.db).await;
    for sql in [
        "SELECT * FROM bff.sessions",
        "SELECT * FROM bff.login_transactions",
        "SELECT * FROM bff.retired_tokens",
        "UPDATE bff.sessions SET expires_at = now()",
        "UPDATE bff.login_transactions SET expires_at = now()",
        "UPDATE bff.retired_tokens SET retired_at = now()",
        "DELETE FROM bff.sessions",
        "DELETE FROM bff.login_transactions",
        "DELETE FROM bff.retired_tokens",
        "INSERT INTO bff.retired_tokens (token_hash, session_id, retired_at)
             SELECT '\\x00', gen_random_uuid(), now()",
        "TRUNCATE bff.retired_tokens",
        "LOCK TABLE bff.sessions IN ACCESS SHARE MODE",
    ] {
        let mut tx = runtime.begin().await.unwrap();
        let refused = sqlx::query(sql).execute(&mut *tx).await.unwrap_err();
        assert_eq!(
            refused
                .as_database_error()
                .and_then(sqlx::error::DatabaseError::code)
                .as_deref(),
            Some("42501"),
            "{sql}: {refused}"
        );
    }
    assert_eq!(
        browser.get("/bff/session").await.json()["authenticated"],
        true,
        "the session is untouched"
    );
}

/// Every function answers about the one session the presented token hash
/// names, or the one account named, never another account's.
#[sqlx::test(migrations = "../../migrations")]
async fn the_functions_never_reach_another_accounts_session(db: PgPool) {
    use campbooks_kernel::crypto::hash;

    let h = harness(db).await;
    let mut ana = Browser::new(&h.app);
    ana.sign_in(&h.idp, "/").await;
    h.idp.set_user(TestUser {
        subject: "someone-else".to_owned(),
        ..TestUser::default()
    });
    let mut sam = Browser::new(&h.app);
    sam.sign_in(&h.idp, "/").await;
    let ana_hash = hash(ana.cookie(SESSION).unwrap());
    let sam_hash = hash(sam.cookie(SESSION).unwrap());
    let runtime = testing::app_role_pool(&h.db).await;

    let found = store::find_session(&runtime, &ana_hash)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.subject, "test-user");
    assert!(found.current);
    assert_eq!(
        store::find_session(&runtime, &sam_hash)
            .await
            .unwrap()
            .unwrap()
            .subject,
        "someone-else"
    );
    let unknown = hash("nobody's cookie");
    assert!(
        store::find_session(&runtime, &unknown)
            .await
            .unwrap()
            .is_none()
    );
    assert!(!store::is_retired(&runtime, &unknown).await.unwrap());
    assert!(
        !store::rotate(&runtime, &unknown, &hash("fresh"), chrono::Utc::now())
            .await
            .unwrap()
    );
    assert!(
        store::end_session(&runtime, &unknown)
            .await
            .unwrap()
            .is_none()
    );
    {
        let mut tx = store::begin_refresh(&runtime).await.unwrap();
        assert!(
            store::lock_tokens(&mut tx, &unknown)
                .await
                .unwrap()
                .is_none()
        );
    }
    // No function lists sessions: an empty or NULL hash finds nothing.
    for nothing in [&b""[..], &[0_u8; 32][..]] {
        assert!(
            store::find_session(&runtime, nothing)
                .await
                .unwrap()
                .is_none()
        );
    }
    let listed: i64 = sqlx::query_scalar("SELECT count(*) FROM bff.find_session(NULL)")
        .fetch_one(&runtime)
        .await
        .unwrap();
    assert_eq!(listed, 0);

    // One account's sessions end; the other's stay.
    assert_eq!(
        store::delete_for_logout(&runtime, Some("test-user"), None)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        store::delete_for_logout(&runtime, None, None)
            .await
            .unwrap(),
        0
    );
    assert_eq!(ana.get("/bff/session").await.json()["authenticated"], false);
    assert_eq!(sam.get("/bff/session").await.json()["authenticated"], true);
    // Ending by a token hash ends that session only.
    let ended = store::end_session(&runtime, &sam_hash)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(count(&h.db, "SELECT count(*) FROM bff.sessions").await, 0);
    assert!(!ended.id_token.is_empty());
}

/// The functions refuse what the BFF never sends (campbooks-cloud#214): a
/// landing that is not a same-origin path (`//`, a backslash, a space, the
/// BFF's own paths, too long), a use with no time or no later deadline, a
/// refresh that would store nothing, and a session longer than 366 days.
#[sqlx::test(migrations = "../../migrations")]
async fn the_functions_refuse_bad_input(db: PgPool) {
    use chrono::{TimeDelta, Utc};

    const START: &str = "SELECT bff.start_login(sha256(gen_random_uuid()::text::bytea),
                                                sha256(gen_random_uuid()::text::bytea),
                                                '\\x01', '\\x02', $1, now(),
                                                now() + interval '10 minutes', NULL)";
    const TOUCH: &str = "SELECT bff.touch_session($1, $2, $3)";
    const STORE: &str = "SELECT bff.store_session_tokens($1, $2, $3, $4)";
    const CREATE: &str = "SELECT bff.create_session(gen_random_uuid(),
                                                    sha256(gen_random_uuid()::text::bytea),
                                                    'test-user', NULL, NULL, NULL, '\\x01', now(),
                                                    NULL, '\\x02', NULL, now(),
                                                    now() + $1::text::interval,
                                                    now() + $2::text::interval, NULL)";

    let h = harness(db).await;
    let mut browser = Browser::new(&h.app);
    browser.sign_in(&h.idp, "/").await;
    let presented = campbooks_kernel::crypto::hash(browser.cookie(SESSION).unwrap());
    let runtime = testing::app_role_pool(&h.db).await;
    let now = Utc::now();
    let later = now + TimeDelta::hours(1);
    let sealed: &[u8] = b"sealed";

    let long = format!("/{}", "a".repeat(2048));
    for landing in [
        "//evil.example",
        "/\\evil.example",
        "/\\/evil.example",
        "evil.example",
        "",
        "/a b",
        "/a\tb",
        "/\"><script>",
        "/bff",
        "/bff/logout",
        long.as_str(),
    ] {
        invalid(
            sqlx::query(START).bind(landing).execute(&runtime).await,
            landing,
        );
    }
    sqlx::query(START)
        .bind("/inbox?view=all#top")
        .execute(&runtime)
        .await
        .unwrap();

    for (seen, idle_until) in [
        (None, Some(later)),
        (Some(now), None),
        (Some(now), Some(now)),
    ] {
        invalid(
            sqlx::query(TOUCH)
                .bind(&presented)
                .bind(seen)
                .bind(idle_until)
                .execute(&runtime)
                .await,
            "touch_session",
        );
    }
    sqlx::query(TOUCH)
        .bind(&presented)
        .bind(now)
        .bind(later)
        .execute(&runtime)
        .await
        .unwrap();

    for (access, until, refresh) in [
        (None, Some(later), Some(sealed)),
        (Some(sealed), None, Some(sealed)),
        (Some(sealed), Some(later), None),
        (Some(&b""[..]), Some(later), Some(sealed)),
    ] {
        invalid(
            sqlx::query(STORE)
                .bind(&presented)
                .bind(access)
                .bind(until)
                .bind(refresh)
                .execute(&runtime)
                .await,
            "store_session_tokens",
        );
    }

    for (idle, ends) in [
        ("1 hour", "367 days"),
        ("0 seconds", "1 day"),
        ("2 days", "1 day"),
    ] {
        invalid(
            sqlx::query(CREATE)
                .bind(idle)
                .bind(ends)
                .execute(&runtime)
                .await,
            ends,
        );
    }
    sqlx::query(CREATE)
        .bind("1 hour")
        .bind("365 days")
        .execute(&runtime)
        .await
        .unwrap();

    assert_eq!(
        browser.get("/bff/session").await.json()["authenticated"],
        true,
        "the session is untouched"
    );
    assert_eq!(
        browser.get("/bff/api/v1/test/whoami").await.status,
        StatusCode::OK,
        "its tokens too"
    );
}

/// A call the function refused as invalid input (`22023`).
fn invalid(result: sqlx::Result<sqlx::postgres::PgQueryResult>, what: &str) {
    let refused = result.expect_err(what);
    assert_eq!(
        refused
            .as_database_error()
            .and_then(sqlx::error::DatabaseError::code)
            .as_deref(),
        Some("22023"),
        "{what}: {refused}"
    );
}

/// What the BFF keeps at rest: cookies, previous and retired tokens, the
/// state and the login cookie as SHA-256; the access, refresh and ID tokens,
/// the PKCE verifier and the nonce sealed, bound to their row and column.
#[sqlx::test(migrations = "../../migrations")]
async fn nothing_secret_is_stored_in_plain_text(db: PgPool) {
    use campbooks_kernel::crypto::hash;

    let h = harness(db).await;
    let mut browser = Browser::new(&h.app);
    browser.sign_in(&h.idp, "/").await;
    let mut cookies = vec![browser.cookie(SESSION).unwrap().to_owned()];
    // Two rotations: a previous token and a retired one.
    for _ in 0..2 {
        exec(
            &h.db,
            "UPDATE bff.sessions SET rotated_at = now() - interval '2 hours'",
        )
        .await;
        browser.get("/bff/session").await;
        cookies.push(browser.cookie(SESSION).unwrap().to_owned());
    }
    let mut pending = Browser::new(&h.app);
    let login = query(
        pending
            .get("/bff/login")
            .await
            .header(header::LOCATION)
            .unwrap(),
    );
    let login_cookie = pending.cookie(LOGIN).unwrap().to_owned();

    let (id, token_hash, previous, access, refresh, id_token): StoredSession = sqlx::query_as(
        "SELECT id, token_hash, previous_token_hash, access_token, refresh_token, id_token
           FROM bff.sessions",
    )
    .fetch_one(&h.db)
    .await
    .unwrap();
    assert_eq!(token_hash, hash(&cookies[2]));
    assert_eq!(previous, Some(hash(&cookies[1])));
    let retired: Vec<u8> = sqlx::query_scalar("SELECT token_hash FROM bff.retired_tokens")
        .fetch_one(&h.db)
        .await
        .unwrap();
    assert_eq!(retired, hash(&cookies[0]));
    let cipher = cipher();
    for (column, sealed) in [
        ("access_token", access),
        ("refresh_token", refresh.unwrap()),
        ("id_token", id_token),
    ] {
        assert!(!sealed.starts_with(b"eyJ"), "{column} is sealed");
        let opened = cipher
            .open(&sealed, &super::sealed_for(id, column))
            .unwrap();
        assert!(!opened.is_empty(), "{column}");
        // Bound to its row and column: it opens nowhere else.
        assert!(
            cipher
                .open(&sealed, &super::sealed_for(uuid::Uuid::now_v7(), column))
                .is_err()
        );
    }

    let (state_hash, browser_hash, nonce, verifier): (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) =
        sqlx::query_as(
            "SELECT state_hash, browser_hash, nonce, code_verifier FROM bff.login_transactions",
        )
        .fetch_one(&h.db)
        .await
        .unwrap();
    assert_eq!(state_hash, hash(&login["state"]));
    assert_eq!(browser_hash, hash(&login_cookie));
    assert_eq!(
        cipher
            .open(&nonce, &super::handlers::nonce_context(&state_hash))
            .unwrap(),
        login["nonce"]
    );
    let verifier = cipher
        .open(&verifier, &super::handlers::verifier_context(&state_hash))
        .unwrap();
    assert_eq!(
        campbooks_kernel::crypto::pkce_challenge(&verifier),
        login["code_challenge"]
    );

    // No value a request carried appears anywhere, as text or bytes.
    let secrets: Vec<&str> = cookies
        .iter()
        .map(String::as_str)
        .chain([
            login_cookie.as_str(),
            login["state"].as_str(),
            login["nonce"].as_str(),
            verifier.as_str(),
        ])
        .collect();
    for table in ["sessions", "login_transactions", "retired_tokens"] {
        for secret in &secrets {
            // A row as text shows bytea as hex: look for both spellings.
            let found: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                "SELECT count(*) FROM bff.{table} t
                  WHERE strpos(t::text, $1) > 0
                     OR strpos(t::text, encode(convert_to($1, 'UTF8'), 'hex')) > 0"
            )))
            .bind(*secret)
            .fetch_one(&h.db)
            .await
            .unwrap();
            assert_eq!(
                found, 0,
                "bff.{table} holds a request's secret in plain text"
            );
        }
    }
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

    // As the job runs it: the runtime role, through the prune functions.
    let runtime = testing::app_role_pool(&h.db).await;
    let pruned = store::prune(&runtime).await.unwrap();
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

// --- per-caller rate limits ---------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn the_browsers_calls_count_against_the_person(db: PgPool) {
    let limits = campbooks_kernel::config::RateLimitsConfig {
        reads: 3,
        ..campbooks_kernel::config::RateLimitsConfig::default()
    };
    let h = harness_limited(
        db,
        ORIGIN,
        |_| {},
        |state| state.with_api_limits(ApiLimits::new(&limits)),
    )
    .await;
    let mut browser = Browser::new(&h.app);
    browser.sign_in(&h.idp, "/").await;
    for _ in 0..3 {
        let reply = browser.get("/bff/api/v1/test/whoami").await;
        assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    }
    let refused = browser.get("/bff/api/v1/test/whoami").await;
    refused.assert_problem(StatusCode::TOO_MANY_REQUESTS, "rate_limited");
    assert_eq!(refused.header(header::RETRY_AFTER), Some("20"));
    // The same person and web app with their token, directly: one budget.
    let mut claims = h.idp.claims("test:read");
    claims["sub"] = TestUser::default().subject.into();
    let token = h.idp.token(&claims);
    let direct = send(
        &h.app,
        Request::get("/api/v1/test/whoami")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    direct.assert_problem(StatusCode::TOO_MANY_REQUESTS, "rate_limited");
    // Someone else is not affected.
    claims["sub"] = "someone-else".into();
    let other = h.idp.token(&claims);
    let reply = send(
        &h.app,
        Request::get("/api/v1/test/whoami")
            .header(header::AUTHORIZATION, format!("Bearer {other}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
}

#[sqlx::test(migrations = "../../migrations")]
async fn api_calls_are_counted_under_the_api_route(db: PgPool) {
    let telemetry = crate::telemetry::Telemetry::new(
        &campbooks_kernel::config::TelemetryConfig {
            metrics_addr: Some(([127, 0, 0, 1], 0).into()),
            ..Default::default()
        },
        "all",
    )
    .unwrap();
    let metrics = telemetry.metrics();
    let h = harness_limited(db, ORIGIN, |_| {}, |state| state.with_metrics(metrics)).await;
    let mut browser = Browser::new(&h.app);
    browser.sign_in(&h.idp, "/").await;
    let reply = browser.get("/bff/api/v1/test/whoami").await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    let missing = browser.get("/bff/api/v1/nope").await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);

    // Counted under the API route it ran through; an unmatched call under
    // the BFF's own (the request log is tested in tests/request_log.rs).
    let page = telemetry.metrics_router().unwrap();
    let scraped = page
        .oneshot(Request::get("/metrics").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let text = String::from_utf8(
        axum::body::to_bytes(scraped.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    let counted = |route: &str| {
        text.lines().any(|line| {
            line.starts_with("http_server_request_duration_seconds_count{")
                && line.contains(&format!("http_route=\"{route}\""))
        })
    };
    assert!(counted("/api/v1/test/whoami"), "{text}");
    assert!(counted("/bff/api/{*path}"), "the unmatched call: {text}");
}
