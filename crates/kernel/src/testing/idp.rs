//! An in-process identity provider for tests: serves an OIDC discovery
//! document and a JWKS on `127.0.0.1`, and signs access tokens with Ed25519
//! keys, so scoped endpoints can be tested end to end without a real IdP or
//! network access.
//!
//! It also plays the IdP side of the web BFF's sign-in (a conventional OIDC server's behaviour):
//! [`TestIdp::authorize`] approves an authorization request as the browser
//! would after a login, and its token endpoint redeems codes (PKCE S256,
//! single use) and rotates refresh tokens, for the confidential client
//! [`WEB_CLIENT_ID`] authenticating with `client_secret_post`.
//!
//! Its methods panic when the test environment is broken (no loopback port,
//! no key generation), like a failed assertion.
//!
//! ```no_run
//! # async fn demo(db: sqlx::PgPool) {
//! use campbooks_kernel::testing::TestIdp;
//!
//! let idp = TestIdp::start().await;
//! let state = idp.state(db); // AppState trusting this IdP
//! let token = idp.token(&idp.claims("mail:read"));
//! // send `Authorization: Bearer {token}` to a route built over `state`
//! # }
//! ```

use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
};

use aws_lc_rs::{digest, rand::SecureRandom as _, signature::Ed25519KeyPair};
use axum::{
    Form, Json, Router,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use jsonwebtoken::{
    Algorithm, EncodingKey, Header,
    jwk::{Jwk, PublicKeyUse},
};
use sqlx::PgPool;

use crate::{AppState, config::AuthConfig};

/// Path of the JWKS under the issuer, as an OIDC server serves it.
const JWKS_PATH: &str = "oidc/certs";
/// Lifetime of tokens from [`TestIdp::claims`].
const TOKEN_LIFETIME_SECONDS: u64 = 300;
/// The confidential web client the test IdP knows (`campbooks-web`).
pub const WEB_CLIENT_ID: &str = "campbooks-web";
/// That client's secret.
pub const WEB_CLIENT_SECRET: &str = "test-web-client-secret";

/// A running test identity provider. The server lives until the test's
/// runtime shuts down.
#[derive(Clone)]
pub struct TestIdp {
    shared: Arc<Shared>,
}

impl std::fmt::Debug for TestIdp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestIdp")
            .field("issuer", &self.shared.issuer)
            .finish_non_exhaustive()
    }
}

struct Shared {
    issuer: String,
    /// Keys served in the JWKS.
    published: Mutex<Vec<Jwk>>,
    /// Key new tokens are signed with.
    signing: Mutex<TestKey>,
    jwks_fetches: AtomicUsize,
    discovery_fetches: AtomicUsize,
    /// `Cache-Control: max-age` of the JWKS response; `u64::MAX` = none.
    max_age: AtomicU64,
    /// Answer the JWKS with 503, as an IdP that is down.
    unavailable: AtomicBool,
    /// The `traceparent` of the last request, if it carried one.
    last_traceparent: Mutex<Option<String>>,
    /// Who [`TestIdp::authorize`] signs in.
    user: Mutex<TestUser>,
    /// Issued, not yet redeemed authorization codes.
    codes: Mutex<HashMap<String, Grant>>,
    /// Live refresh tokens (a used one is removed: rotation).
    refresh_tokens: Mutex<HashMap<String, Grant>>,
    /// Every token sent to the revocation endpoint.
    revoked: Mutex<Vec<String>>,
    /// `expires_in` of issued access tokens, in seconds.
    access_lifetime: AtomicU64,
    /// Successful `refresh_token` grants.
    refreshes: AtomicUsize,
    /// IdP sessions started so far (each sign-in gets a new `sid`).
    sessions: AtomicUsize,
    /// Whether code exchanges return a refresh token (an upstream provider
    /// sends none when the person consented before).
    offline: AtomicBool,
    /// Requests to the token endpoint, of any grant, valid or not.
    token_requests: AtomicUsize,
    /// Claims merged into issued ID tokens (`null` removes one).
    id_token_claims: Mutex<serde_json::Map<String, serde_json::Value>>,
}

/// The user [`TestIdp::authorize`] signs in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestUser {
    /// `sub`.
    pub subject: String,
    /// `name`.
    pub name: String,
    /// `email`.
    pub email: String,
}

impl Default for TestUser {
    fn default() -> Self {
        Self {
            subject: "test-user".to_owned(),
            name: "Test User".to_owned(),
            email: "test-user@example.com".to_owned(),
        }
    }
}

/// What an authorization code or a refresh token stands for.
#[derive(Debug, Clone)]
struct Grant {
    user: TestUser,
    /// When the user signed in (`auth_time`): when the code was approved.
    auth_time: u64,
    sid: String,
    scope: String,
    nonce: Option<String>,
    redirect_uri: String,
    code_challenge: String,
}

#[derive(Clone)]
struct TestKey {
    kid: String,
    encoding: EncodingKey,
    jwk: Jwk,
}

impl TestKey {
    fn generate(kid: String) -> Self {
        let pkcs8 = Ed25519KeyPair::generate()
            .and_then(|pair| pair.to_pkcs8v1())
            .unwrap_or_else(|e| panic!("generating an Ed25519 test key: {e}"));
        let encoding = EncodingKey::from_ed_der(pkcs8.as_ref());
        let mut jwk = Jwk::from_encoding_key(&encoding, Algorithm::EdDSA)
            .unwrap_or_else(|e| panic!("deriving the JWK of a test key: {e}"));
        jwk.common.key_id = Some(kid.clone());
        jwk.common.public_key_use = Some(PublicKeyUse::Signature);
        Self { kid, encoding, jwk }
    }
}

impl TestIdp {
    /// Start an IdP on a free loopback port with one published signing key.
    /// Its issuer is `http://127.0.0.1:<port>/auth/v1/` (it has a path, to
    /// exercise issuer paths).
    ///
    /// # Panics
    /// If the test environment is broken (see the module docs).
    pub async fn start() -> Self {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap_or_else(|e| panic!("binding the test IdP: {e}"));
        let addr = listener
            .local_addr()
            .unwrap_or_else(|e| panic!("reading the test IdP address: {e}"));
        let key = TestKey::generate("test-key-1".to_owned());
        let shared = Arc::new(Shared {
            issuer: format!("http://{addr}/auth/v1/"),
            published: Mutex::new(vec![key.jwk.clone()]),
            signing: Mutex::new(key),
            jwks_fetches: AtomicUsize::new(0),
            discovery_fetches: AtomicUsize::new(0),
            max_age: AtomicU64::new(u64::MAX),
            unavailable: AtomicBool::new(false),
            last_traceparent: Mutex::new(None),
            user: Mutex::new(TestUser::default()),
            codes: Mutex::default(),
            refresh_tokens: Mutex::default(),
            revoked: Mutex::default(),
            access_lifetime: AtomicU64::new(TOKEN_LIFETIME_SECONDS),
            refreshes: AtomicUsize::new(0),
            sessions: AtomicUsize::new(0),
            offline: AtomicBool::new(true),
            token_requests: AtomicUsize::new(0),
            id_token_claims: Mutex::default(),
        });
        let app = Router::new()
            .route("/auth/v1/.well-known/openid-configuration", get(discovery))
            .route(&format!("/auth/v1/{JWKS_PATH}"), get(jwks))
            .route("/auth/v1/oidc/token", post(token))
            .route("/auth/v1/oidc/token/revoke", post(revoke))
            .with_state(Arc::clone(&shared));
        tokio::spawn(async move {
            // Ends with the test runtime; nothing to report to.
            let _ = axum::serve(listener, app).await;
        });
        Self { shared }
    }

    /// The issuer URL (with its trailing slash).
    pub fn issuer(&self) -> &str {
        &self.shared.issuer
    }

    /// The JWKS URL.
    pub fn jwks_url(&self) -> String {
        format!("{}{JWKS_PATH}", self.shared.issuer)
    }

    /// The JWKS document this IdP publishes (what its JWKS endpoint serves).
    pub fn jwks_document(&self) -> serde_json::Value {
        serde_json::json!({ "keys": lock(&self.shared.published).clone() })
    }

    /// Settings trusting this IdP, finding the JWKS by discovery.
    pub fn auth_config(&self) -> AuthConfig {
        AuthConfig {
            issuer: Some(self.shared.issuer.clone()),
            ..AuthConfig::default()
        }
    }

    /// App state over `db` whose token verifier trusts this IdP, binding
    /// outbound requests with the test key ([`binding`](super::binding)).
    pub fn state(&self, db: PgPool) -> AppState {
        let mut config = super::config("postgres://test@localhost/test");
        config.auth = self.auth_config();
        AppState::new(db, config).with_binding(super::binding())
    }

    /// Claims of a valid user access token granting `scope` (space-separated
    /// names) plus `openid`, as an OIDC server issues them. Edit the value to test
    /// other cases.
    pub fn claims(&self, scope: &str) -> serde_json::Value {
        let now = jsonwebtoken::get_current_timestamp();
        serde_json::json!({
            "iss": self.shared.issuer,
            "aud": [AuthConfig::default().audience, "campbooks-web"],
            "sub": "test-user",
            "azp": "campbooks-web",
            "scope": format!("openid {scope}").trim(),
            "iat": now,
            "nbf": now,
            "exp": now + TOKEN_LIFETIME_SECONDS,
            "jti": "test-token",
        })
    }

    /// `claims` signed with the current key (`EdDSA`, `typ: at+jwt`).
    ///
    /// # Panics
    /// If the test environment is broken (see the module docs).
    pub fn token(&self, claims: &serde_json::Value) -> String {
        let mut header = Header::new(Algorithm::EdDSA);
        header.typ = Some("at+jwt".to_owned());
        self.sign(header, claims)
    }

    /// `claims` signed with the current key under `header` (its `kid` is set
    /// to the current key's when empty).
    ///
    /// # Panics
    /// If the test environment is broken (see the module docs).
    pub fn sign(&self, mut header: Header, claims: &serde_json::Value) -> String {
        let key = self.signing_key();
        header.kid.get_or_insert(key.kid);
        encode(&header, claims, &key.encoding)
    }

    /// `claims` signed with a fresh key the JWKS does not publish.
    ///
    /// # Panics
    /// If the test environment is broken (see the module docs).
    pub fn unpublished_token(&self, claims: &serde_json::Value) -> String {
        let key = TestKey::generate("unpublished".to_owned());
        let mut header = Header::new(Algorithm::EdDSA);
        header.kid = Some(key.kid);
        encode(&header, claims, &key.encoding)
    }

    /// Rotate: publish a new key next to the old ones and sign with it from
    /// now on. Returns its `kid`.
    ///
    /// # Panics
    /// If the test environment is broken (see the module docs).
    pub fn rotate_key(&self) -> String {
        let mut published = lock(&self.shared.published);
        let key = TestKey::generate(format!("test-key-{}", published.len() + 1));
        published.push(key.jwk.clone());
        let kid = key.kid.clone();
        *lock(&self.shared.signing) = key;
        kid
    }

    /// The `kid` new tokens are signed with.
    pub fn current_kid(&self) -> String {
        self.signing_key().kid
    }

    /// Serve the JWKS with `Cache-Control: max-age=<seconds>` (`None`: no
    /// header).
    pub fn set_max_age(&self, seconds: Option<u64>) {
        self.shared
            .max_age
            .store(seconds.unwrap_or(u64::MAX), Ordering::SeqCst);
    }

    /// Make the JWKS answer 503 (`true`) or work again (`false`). Fetch
    /// attempts are still counted.
    pub fn set_unavailable(&self, unavailable: bool) {
        self.shared.unavailable.store(unavailable, Ordering::SeqCst);
    }

    /// How many times the JWKS was fetched.
    pub fn jwks_fetches(&self) -> usize {
        self.shared.jwks_fetches.load(Ordering::SeqCst)
    }

    /// How many times the discovery document was fetched.
    pub fn discovery_fetches(&self) -> usize {
        self.shared.discovery_fetches.load(Ordering::SeqCst)
    }

    /// The W3C `traceparent` header of the last discovery or JWKS request,
    /// if it carried one.
    pub fn last_traceparent(&self) -> Option<String> {
        lock(&self.shared.last_traceparent).clone()
    }

    /// Sign in as `user` from now on.
    pub fn set_user(&self, user: TestUser) {
        *lock(&self.shared.user) = user;
    }

    /// `expires_in` of the access tokens the token endpoint issues.
    pub fn set_access_token_lifetime(&self, seconds: u64) {
        self.shared.access_lifetime.store(seconds, Ordering::SeqCst);
    }

    /// Whether `authorization_code` grants return a refresh token (default
    /// `true`); `false` plays a provider the person already consented to.
    pub fn set_offline(&self, offline: bool) {
        self.shared.offline.store(offline, Ordering::SeqCst);
    }

    /// Merge `claims` (an object) into every ID token the token endpoint
    /// issues from now on; a `null` value removes that claim. Plays a
    /// provider with its own claims or issuer rule (a tenant's `iss` and
    /// `tid`, no `email_verified`).
    ///
    /// # Panics
    /// If `claims` is not a JSON object.
    pub fn set_id_token_claims(&self, claims: serde_json::Value) {
        let serde_json::Value::Object(claims) = claims else {
            panic!("ID token claims must be an object");
        };
        *lock(&self.shared.id_token_claims) = claims;
    }

    /// Requests to the token endpoint so far, valid or not.
    pub fn token_requests(&self) -> usize {
        self.shared.token_requests.load(Ordering::SeqCst)
    }

    /// Successful `refresh_token` grants so far.
    pub fn refreshes(&self) -> usize {
        self.shared.refreshes.load(Ordering::SeqCst)
    }

    /// Every token sent to the revocation endpoint.
    pub fn revoked_tokens(&self) -> Vec<String> {
        lock(&self.shared.revoked).clone()
    }

    /// Revoke every refresh token, as an admin ending all sessions would.
    pub fn revoke_refresh_tokens(&self) {
        lock(&self.shared.refresh_tokens).clear();
    }

    /// Whether `token` is a live refresh token.
    pub fn is_live_refresh_token(&self, token: &str) -> bool {
        lock(&self.shared.refresh_tokens).contains_key(token)
    }

    /// Approve the authorization request `authorize_url` (built by a relying
    /// party for [`WEB_CLIENT_ID`]) as if the user had signed in, and return
    /// the callback URL the browser is sent to: the request's `redirect_uri`
    /// with `code`, `state` and `iss`.
    ///
    /// # Panics
    /// If the request is not one an OIDC server would accept from `campbooks-web`
    /// (wrong client or response type, no S256 challenge, no state).
    pub fn authorize(&self, authorize_url: &str) -> String {
        let url = reqwest::Url::parse(authorize_url)
            .unwrap_or_else(|e| panic!("authorize URL {authorize_url}: {e}"));
        assert!(
            url.as_str()
                .starts_with(&format!("{}oidc/authorize", self.shared.issuer)),
            "not this IdP's authorization endpoint: {url}"
        );
        let params: HashMap<String, String> = url.query_pairs().into_owned().collect();
        let param = |name: &str| {
            params
                .get(name)
                .cloned()
                .unwrap_or_else(|| panic!("authorize request without `{name}`: {url}"))
        };
        assert_eq!(param("client_id"), WEB_CLIENT_ID);
        assert_eq!(param("response_type"), "code");
        assert_eq!(param("code_challenge_method"), "S256");
        let state = param("state");
        let redirect_uri = param("redirect_uri");
        let sid = format!(
            "test-sid-{}",
            self.shared.sessions.fetch_add(1, Ordering::SeqCst) + 1
        );
        let grant = Grant {
            user: lock(&self.shared.user).clone(),
            auth_time: jsonwebtoken::get_current_timestamp(),
            sid,
            scope: param("scope"),
            nonce: params.get("nonce").cloned(),
            redirect_uri: redirect_uri.clone(),
            code_challenge: param("code_challenge"),
        };
        let code = random_token();
        lock(&self.shared.codes).insert(code.clone(), grant);
        let mut callback = reqwest::Url::parse(&redirect_uri)
            .unwrap_or_else(|e| panic!("redirect_uri {redirect_uri}: {e}"));
        callback
            .query_pairs_mut()
            .append_pair("code", &code)
            .append_pair("state", &state)
            .append_pair("iss", &self.shared.issuer);
        callback.into()
    }

    /// An ID token for `subject` as an OIDC server issues one to [`WEB_CLIENT_ID`]
    /// (`EdDSA`, `typ: JWT`). Edit the claims to test other cases.
    pub fn id_token_claims(&self, subject: &str, nonce: &str, sid: &str) -> serde_json::Value {
        let now = jsonwebtoken::get_current_timestamp();
        serde_json::json!({
            "iss": self.shared.issuer,
            "aud": WEB_CLIENT_ID,
            "azp": WEB_CLIENT_ID,
            "sub": subject,
            "nonce": nonce,
            "sid": sid,
            "iat": now,
            "nbf": now,
            "exp": now + TOKEN_LIFETIME_SECONDS,
        })
    }

    /// `claims` signed as an ID token (`EdDSA`, `typ: JWT`).
    ///
    /// # Panics
    /// If the test environment is broken (see the module docs).
    pub fn id_token(&self, claims: &serde_json::Value) -> String {
        let mut header = Header::new(Algorithm::EdDSA);
        header.typ = Some("JWT".to_owned());
        self.sign(header, claims)
    }

    /// Claims of a back-channel logout token for [`WEB_CLIENT_ID`] ending
    /// the sessions of `subject` and/or `sid`.
    pub fn logout_token_claims(
        &self,
        subject: Option<&str>,
        sid: Option<&str>,
    ) -> serde_json::Value {
        let now = jsonwebtoken::get_current_timestamp();
        let mut claims = serde_json::json!({
            "iss": self.shared.issuer,
            "aud": WEB_CLIENT_ID,
            "iat": now,
            "exp": now + 30,
            "jti": random_token(),
            "events": { crate::auth::BACKCHANNEL_LOGOUT_EVENT: {} },
        });
        if let Some(subject) = subject {
            claims["sub"] = subject.into();
        }
        if let Some(sid) = sid {
            claims["sid"] = sid.into();
        }
        claims
    }

    /// `claims` signed as a logout token (`EdDSA`, `typ: logout+jwt`).
    ///
    /// # Panics
    /// If the test environment is broken (see the module docs).
    pub fn logout_token(&self, claims: &serde_json::Value) -> String {
        let mut header = Header::new(Algorithm::EdDSA);
        header.typ = Some("logout+jwt".to_owned());
        self.sign(header, claims)
    }

    fn signing_key(&self) -> TestKey {
        lock(&self.shared.signing).clone()
    }
}

fn encode(header: &Header, claims: &serde_json::Value, key: &EncodingKey) -> String {
    jsonwebtoken::encode(header, claims, key)
        .unwrap_or_else(|e| panic!("signing a test token: {e}"))
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn note_traceparent(shared: &Shared, headers: &HeaderMap) {
    if let Some(value) = headers.get("traceparent").and_then(|v| v.to_str().ok()) {
        *lock(&shared.last_traceparent) = Some(value.to_owned());
    }
}

async fn discovery(
    State(shared): State<Arc<Shared>>,
    headers: HeaderMap,
) -> Json<serde_json::Value> {
    shared.discovery_fetches.fetch_add(1, Ordering::SeqCst);
    note_traceparent(&shared, &headers);
    let issuer = &shared.issuer;
    Json(serde_json::json!({
        "issuer": issuer,
        "authorization_endpoint": format!("{issuer}oidc/authorize"),
        "token_endpoint": format!("{issuer}oidc/token"),
        "revocation_endpoint": format!("{issuer}oidc/token/revoke"),
        "end_session_endpoint": format!("{issuer}oidc/logout"),
        "jwks_uri": format!("{issuer}{JWKS_PATH}"),
        "backchannel_logout_supported": true,
        "backchannel_logout_session_supported": true,
        "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": ["client_secret_post"],
        "id_token_signing_alg_values_supported": ["EdDSA", "RS256"],
    }))
}

/// A random URL-safe token (256 bits).
fn random_token() -> String {
    let mut bytes = [0_u8; 32];
    aws_lc_rs::rand::SystemRandom::new()
        .fill(&mut bytes)
        .unwrap_or_else(|e| panic!("random bytes for the test IdP: {e}"));
    URL_SAFE_NO_PAD.encode(bytes)
}

/// An OAuth error response (RFC 6749 §5.2).
fn oauth_error(status: StatusCode, error: &str) -> Response {
    (status, Json(serde_json::json!({ "error": error }))).into_response()
}

/// The token endpoint: `authorization_code` and `refresh_token` grants for
/// the web client, authenticated with `client_secret_post`.
async fn token(
    State(shared): State<Arc<Shared>>,
    Form(form): Form<HashMap<String, String>>,
) -> Response {
    shared.token_requests.fetch_add(1, Ordering::SeqCst);
    let field = |name: &str| form.get(name).map(String::as_str).unwrap_or_default();
    if field("client_id") != WEB_CLIENT_ID || field("client_secret") != WEB_CLIENT_SECRET {
        return oauth_error(StatusCode::UNAUTHORIZED, "invalid_client");
    }
    let grant = match field("grant_type") {
        "authorization_code" => {
            let Some(grant) = lock(&shared.codes).remove(field("code")) else {
                return oauth_error(StatusCode::BAD_REQUEST, "invalid_grant");
            };
            let challenge = URL_SAFE_NO_PAD.encode(digest::digest(
                &digest::SHA256,
                field("code_verifier").as_bytes(),
            ));
            if challenge != grant.code_challenge || field("redirect_uri") != grant.redirect_uri {
                return oauth_error(StatusCode::BAD_REQUEST, "invalid_grant");
            }
            grant
        }
        "refresh_token" => {
            let Some(grant) = lock(&shared.refresh_tokens).remove(field("refresh_token")) else {
                return oauth_error(StatusCode::BAD_REQUEST, "invalid_grant");
            };
            shared.refreshes.fetch_add(1, Ordering::SeqCst);
            grant
        }
        _ => return oauth_error(StatusCode::BAD_REQUEST, "unsupported_grant_type"),
    };
    let idp = TestIdp {
        shared: Arc::clone(&shared),
    };
    let lifetime = shared.access_lifetime.load(Ordering::SeqCst);
    let mut access = idp.claims(grant.scope.trim_start_matches("openid").trim());
    access["sub"] = grant.user.subject.clone().into();
    access["exp"] = (jsonwebtoken::get_current_timestamp() + lifetime).into();
    access["jti"] = random_token().into();
    access["auth_time"] = grant.auth_time.into();
    let mut id = idp.id_token_claims(
        &grant.user.subject,
        grant.nonce.as_deref().unwrap_or_default(),
        &grant.sid,
    );
    if grant.nonce.is_none()
        && let Some(id) = id.as_object_mut()
    {
        id.remove("nonce");
    }
    id["name"] = grant.user.name.clone().into();
    id["email"] = grant.user.email.clone().into();
    id["email_verified"] = true.into();
    if let Some(id) = id.as_object_mut() {
        for (name, value) in lock(&shared.id_token_claims).iter() {
            if value.is_null() {
                id.remove(name);
            } else {
                id.insert(name.clone(), value.clone());
            }
        }
    }
    let refresh_token = random_token();
    let mut body = serde_json::json!({
        "access_token": idp.token(&access),
        "token_type": "Bearer",
        "expires_in": lifetime,
        "refresh_token": refresh_token,
        "id_token": idp.id_token(&id),
        "scope": grant.scope,
    });
    if field("grant_type") == "authorization_code" && !shared.offline.load(Ordering::SeqCst) {
        if let Some(body) = body.as_object_mut() {
            body.remove("refresh_token");
        }
        return Json(body).into_response();
    }
    // A refreshed grant carries no nonce (OIDC Core §12.2).
    let next = Grant {
        nonce: None,
        ..grant
    };
    lock(&shared.refresh_tokens).insert(refresh_token, next);
    Json(body).into_response()
}

/// RFC 7009 revocation: always 200; a revoked refresh token stops working.
async fn revoke(
    State(shared): State<Arc<Shared>>,
    Form(form): Form<HashMap<String, String>>,
) -> Response {
    let field = |name: &str| form.get(name).map(String::as_str).unwrap_or_default();
    if field("client_id") != WEB_CLIENT_ID || field("client_secret") != WEB_CLIENT_SECRET {
        return oauth_error(StatusCode::UNAUTHORIZED, "invalid_client");
    }
    let token = field("token").to_owned();
    lock(&shared.refresh_tokens).remove(&token);
    lock(&shared.revoked).push(token);
    StatusCode::OK.into_response()
}

async fn jwks(State(shared): State<Arc<Shared>>, headers: HeaderMap) -> Response {
    shared.jwks_fetches.fetch_add(1, Ordering::SeqCst);
    note_traceparent(&shared, &headers);
    if shared.unavailable.load(Ordering::SeqCst) {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    let keys = lock(&shared.published).clone();
    let mut response = Json(serde_json::json!({ "keys": keys })).into_response();
    let max_age = shared.max_age.load(Ordering::SeqCst);
    if max_age != u64::MAX
        && let Ok(value) = HeaderValue::from_str(&format!("max-age={max_age}"))
    {
        response.headers_mut().insert(header::CACHE_CONTROL, value);
    }
    response
}
