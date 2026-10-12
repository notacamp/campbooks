//! An in-process identity provider for tests: serves an OIDC discovery
//! document and a JWKS on `127.0.0.1`, and signs access tokens with Ed25519
//! keys, so scoped endpoints can be tested end to end without a real IdP or
//! network access.
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

use std::sync::{
    Arc, Mutex, PoisonError,
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};

use aws_lc_rs::signature::Ed25519KeyPair;
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use jsonwebtoken::{
    Algorithm, EncodingKey, Header,
    jwk::{Jwk, PublicKeyUse},
};
use sqlx::PgPool;

use crate::{AppState, config::AuthConfig};

/// Path of the JWKS under the issuer, as Rauthy serves it.
const JWKS_PATH: &str = "oidc/certs";
/// Lifetime of tokens from [`TestIdp::claims`].
const TOKEN_LIFETIME_SECONDS: u64 = 300;

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
    /// Its issuer is `http://127.0.0.1:<port>/auth/v1/` (Rauthy's layout).
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
        });
        let app = Router::new()
            .route("/auth/v1/.well-known/openid-configuration", get(discovery))
            .route(&format!("/auth/v1/{JWKS_PATH}"), get(jwks))
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

    /// Settings trusting this IdP, finding the JWKS by discovery.
    pub fn auth_config(&self) -> AuthConfig {
        AuthConfig {
            issuer: Some(self.shared.issuer.clone()),
            ..AuthConfig::default()
        }
    }

    /// App state over `db` whose token verifier trusts this IdP.
    pub fn state(&self, db: PgPool) -> AppState {
        let mut config = super::config("postgres://test@localhost/test");
        config.auth = self.auth_config();
        AppState::new(db, config)
    }

    /// Claims of a valid user access token granting `scope` (space-separated
    /// names) plus `openid`, as Rauthy issues them. Edit the value to test
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

async fn discovery(State(shared): State<Arc<Shared>>) -> Json<serde_json::Value> {
    shared.discovery_fetches.fetch_add(1, Ordering::SeqCst);
    Json(serde_json::json!({
        "issuer": shared.issuer,
        "jwks_uri": format!("{}{JWKS_PATH}", shared.issuer),
        "token_endpoint": format!("{}oidc/token", shared.issuer),
        "id_token_signing_alg_values_supported": ["EdDSA", "RS256"],
    }))
}

async fn jwks(State(shared): State<Arc<Shared>>) -> Response {
    shared.jwks_fetches.fetch_add(1, Ordering::SeqCst);
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
