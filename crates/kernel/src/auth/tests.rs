//! End to end: a route behind `Scoped<S>`, tokens signed by an in-process
//! [`TestIdp`] whose JWKS the verifier fetches over loopback HTTP.

use std::sync::{Arc, LazyLock};

use axum::{
    Json, Router,
    body::Body,
    http::{HeaderMap, Request, StatusCode, header},
    routing::get,
};
use base64::Engine as _;
use chrono::{TimeDelta, Utc};
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde_json::{Value, json};
use tower::ServiceExt as _;

use super::*;
use crate::{
    Error,
    clock::FixedClock,
    config::AuthConfig,
    testing::{self, TestIdp},
};

/// The scope the test route requires.
struct TestRead;
impl ScopeName for TestRead {
    const NAME: &'static str = "test:read";
    const DESCRIPTION: &'static str = "Read test things.";
}

async fn whoami(Scoped { principal, .. }: Scoped<TestRead>) -> Json<Value> {
    Json(json!({
        "subject": principal.subject,
        "kind": format!("{:?}", principal.kind),
        "client_id": principal.client_id,
        "workspace": principal.workspace,
        "scopes": principal.scopes.iter().map(Scope::as_str).collect::<Vec<_>>(),
    }))
}

fn app(verifier: TokenVerifier) -> Router {
    Router::new()
        .route("/whoami", get(whoami))
        .with_state(verifier)
}

/// A verifier trusting `idp` whose key cache runs on a clock the test moves.
fn verifier(idp: &TestIdp) -> (TokenVerifier, Arc<FixedClock>) {
    verifier_for(&idp.auth_config())
}

fn verifier_for(config: &AuthConfig) -> (TokenVerifier, Arc<FixedClock>) {
    let clock = Arc::new(FixedClock::new(Utc::now()));
    (TokenVerifier::with_clock(config, clock.clone()), clock)
}

struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    json: Value,
}

impl Reply {
    fn challenge(&self) -> &str {
        self.headers
            .get(header::WWW_AUTHENTICATE)
            .map_or("", |v| v.to_str().unwrap())
    }

    fn assert_rejected(&self, code: &str) {
        assert_eq!(self.status, StatusCode::UNAUTHORIZED, "{}", self.json);
        assert_eq!(
            self.headers[header::CONTENT_TYPE],
            "application/problem+json"
        );
        assert_eq!(self.json["code"], code);
    }

    fn assert_invalid(&self) {
        self.assert_rejected("invalid_token");
        assert_eq!(self.challenge(), r#"Bearer error="invalid_token""#);
    }
}

async fn call(app: &Router, authorization: Option<&str>) -> Reply {
    let mut request = Request::get("/whoami");
    if let Some(value) = authorization {
        request = request.header(header::AUTHORIZATION, value);
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    Reply {
        status,
        headers,
        json: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    }
}

async fn call_with(app: &Router, token: &str) -> Reply {
    call(app, Some(&format!("Bearer {token}"))).await
}

fn b64(value: &Value) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(value.to_string())
}

// --- the happy path ----------------------------------------------------------

#[tokio::test]
async fn valid_token_reaches_the_handler() {
    let idp = TestIdp::start().await;
    let app = app(verifier(&idp).0);
    let reply = call_with(&app, &idp.token(&idp.claims("test:read"))).await;

    assert_eq!(reply.status, StatusCode::OK, "{}", reply.json);
    assert_eq!(
        reply.json,
        json!({
            "subject": "test-user",
            "kind": "User",
            "client_id": "campbooks-web",
            "workspace": null,
            "scopes": ["openid", "test:read"],
        })
    );
    assert_eq!(idp.discovery_fetches(), 1, "JWKS URL found by discovery");
    assert_eq!(idp.jwks_fetches(), 1);
}

#[tokio::test]
async fn works_through_app_state() {
    let idp = TestIdp::start().await;
    let app = Router::new()
        .route("/whoami", get(whoami))
        .with_state(idp.state(testing::unreachable_pool()));
    let reply = call_with(&app, &idp.token(&idp.claims("test:read"))).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.json);
}

#[tokio::test]
async fn audience_may_be_a_single_string() {
    let idp = TestIdp::start().await;
    let app = app(verifier(&idp).0);
    let mut claims = idp.claims("test:read");
    claims["aud"] = json!("campbooks-api");
    assert_eq!(
        call_with(&app, &idp.token(&claims)).await.status,
        StatusCode::OK
    );
}

#[tokio::test]
async fn header_typ_jwt_and_absent_are_accepted() {
    let idp = TestIdp::start().await;
    let app = app(verifier(&idp).0);
    let claims = idp.claims("test:read");
    for typ in [Some("JWT"), None] {
        let mut header = Header::new(Algorithm::EdDSA);
        header.typ = typ.map(ToOwned::to_owned);
        let reply = call_with(&app, &idp.sign(header, &claims)).await;
        assert_eq!(reply.status, StatusCode::OK, "{typ:?}: {}", reply.json);
    }
}

#[tokio::test]
async fn clock_skew_within_leeway_is_tolerated() {
    let idp = TestIdp::start().await;
    let app = app(verifier(&idp).0);
    let now = jsonwebtoken::get_current_timestamp();
    let mut claims = idp.claims("test:read");
    claims["exp"] = json!(now - 30);
    claims["nbf"] = json!(now + 30);
    claims["iat"] = json!(now + 30);
    assert_eq!(
        call_with(&app, &idp.token(&claims)).await.status,
        StatusCode::OK
    );
}

// --- who the principal is -----------------------------------------------------

#[tokio::test]
async fn client_credentials_token_uses_azp_as_subject() {
    let idp = TestIdp::start().await;
    let app = app(verifier(&idp).0);
    let mut claims = idp.claims("test:read");
    let fields = claims.as_object_mut().unwrap();
    fields.remove("sub");
    fields.insert("azp".into(), json!("acme-sync"));
    fields.insert("aud".into(), json!(["acme-sync", "campbooks-api"]));

    let reply = call_with(&app, &idp.token(&claims)).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.json);
    assert_eq!(reply.json["subject"], "acme-sync");
    assert_eq!(reply.json["kind"], "Client");
    assert_eq!(reply.json["client_id"], "acme-sync");
}

#[tokio::test]
async fn workspace_claim_at_root_or_under_custom() {
    let idp = TestIdp::start().await;
    let app = app(verifier(&idp).0);
    let client_claims = || {
        let mut claims = idp.claims("test:read");
        let fields = claims.as_object_mut().unwrap();
        fields.remove("sub");
        fields.insert("azp".into(), json!("acme-sync"));
        claims
    };

    let mut root = client_claims();
    root["workspace_id"] = json!("ws-root");
    let reply = call_with(&app, &idp.token(&root)).await;
    assert_eq!(reply.json["workspace"], "ws-root");

    let mut custom = client_claims();
    custom["custom"] = json!({ "workspace_id": "ws-custom" });
    let reply = call_with(&app, &idp.token(&custom)).await;
    assert_eq!(reply.json["workspace"], "ws-custom");

    let mut conflicting = root.clone();
    conflicting["custom"] = json!({ "workspace_id": "ws-other" });
    call_with(&app, &idp.token(&conflicting))
        .await
        .assert_invalid();
}

#[tokio::test]
async fn user_tokens_never_carry_a_workspace() {
    let idp = TestIdp::start().await;
    let app = app(verifier(&idp).0);
    let mut claims = idp.claims("test:read");
    claims["workspace_id"] = json!("ws-chosen-by-user");
    let reply = call_with(&app, &idp.token(&claims)).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.json);
    assert_eq!(reply.json["kind"], "User");
    assert!(reply.json["workspace"].is_null(), "{}", reply.json);
}

// --- 401: no or bad token -----------------------------------------------------

#[tokio::test]
async fn issuer_unset_answers_auth_not_configured() {
    let idp = TestIdp::start().await;
    let app = app(TokenVerifier::new(&AuthConfig::default()));
    for authorization in [
        None,
        Some(format!("Bearer {}", idp.token(&idp.claims("test:read")))),
    ] {
        let reply = call(&app, authorization.as_deref()).await;
        reply.assert_rejected("auth_not_configured");
        assert_eq!(reply.challenge(), "Bearer");
    }
    assert_eq!(idp.jwks_fetches(), 0);
}

#[tokio::test]
async fn missing_token_is_401_with_bare_challenge() {
    let idp = TestIdp::start().await;
    let app = app(verifier(&idp).0);
    for authorization in [None, Some("Basic dXNlcjpwYXNz")] {
        let reply = call(&app, authorization).await;
        reply.assert_rejected("missing_token");
        assert_eq!(reply.challenge(), "Bearer");
    }
}

#[tokio::test]
async fn malformed_tokens_are_invalid() {
    let idp = TestIdp::start().await;
    let app = app(verifier(&idp).0);
    for authorization in ["Bearer ", "Bearer not-a-jwt", "Bearer a.b.c", "Bearer a.b"] {
        call(&app, Some(authorization)).await.assert_invalid();
    }
}

#[tokio::test]
async fn wrong_issuer_is_invalid() {
    let idp = TestIdp::start().await;
    let app = app(verifier(&idp).0);
    for iss in [
        json!("https://idp.example.com/auth/v1/"),
        json!(idp.issuer().trim_end_matches('/')),
    ] {
        let mut claims = idp.claims("test:read");
        claims["iss"] = iss;
        call_with(&app, &idp.token(&claims)).await.assert_invalid();
    }
    let mut claims = idp.claims("test:read");
    claims.as_object_mut().unwrap().remove("iss");
    call_with(&app, &idp.token(&claims)).await.assert_invalid();
}

#[tokio::test]
async fn wrong_or_missing_audience_is_invalid() {
    let idp = TestIdp::start().await;
    let app = app(verifier(&idp).0);
    for aud in [
        json!("campbooks-web"),
        json!(["campbooks-web", "other-api"]),
        json!([]),
    ] {
        let mut claims = idp.claims("test:read");
        claims["aud"] = aud;
        call_with(&app, &idp.token(&claims)).await.assert_invalid();
    }
    let mut claims = idp.claims("test:read");
    claims.as_object_mut().unwrap().remove("aud");
    call_with(&app, &idp.token(&claims)).await.assert_invalid();
}

#[tokio::test]
async fn expired_token_says_so() {
    let idp = TestIdp::start().await;
    let app = app(verifier(&idp).0);
    let mut claims = idp.claims("test:read");
    claims["exp"] = json!(jsonwebtoken::get_current_timestamp() - 3600);
    let reply = call_with(&app, &idp.token(&claims)).await;
    reply.assert_rejected("token_expired");
    assert_eq!(reply.challenge(), r#"Bearer error="invalid_token""#);

    claims.as_object_mut().unwrap().remove("exp");
    call_with(&app, &idp.token(&claims)).await.assert_invalid();
}

#[tokio::test]
async fn not_yet_valid_or_issued_in_the_future_is_invalid() {
    let idp = TestIdp::start().await;
    let app = app(verifier(&idp).0);
    let later = jsonwebtoken::get_current_timestamp() + 3600;
    for claim in ["nbf", "iat"] {
        let mut claims = idp.claims("test:read");
        claims[claim] = json!(later);
        claims["exp"] = json!(later + 300);
        call_with(&app, &idp.token(&claims)).await.assert_invalid();
    }
}

#[tokio::test]
async fn disallowed_or_mismatched_algorithms_are_invalid() {
    let idp = TestIdp::start().await;
    let app = app(verifier(&idp).0);
    let claims = idp.claims("test:read");
    let kid = idp.current_kid();

    // HMAC with a guessable secret, under the real key's kid.
    let mut hs = Header::new(Algorithm::HS256);
    hs.kid = Some(kid.clone());
    let hs = jsonwebtoken::encode(&hs, &claims, &EncodingKey::from_secret(b"secret")).unwrap();
    call_with(&app, &hs).await.assert_invalid();

    // `alg: none`, and RS256 claimed for the Ed25519 key.
    for alg in ["none", "RS256"] {
        let header = json!({ "alg": alg, "typ": "JWT", "kid": kid });
        let forged = format!("{}.{}.c2lnbmF0dXJl", b64(&header), b64(&claims));
        call_with(&app, &forged).await.assert_invalid();
    }
}

#[tokio::test]
async fn wrong_typ_or_missing_kid_is_invalid() {
    let idp = TestIdp::start().await;
    let app = app(verifier(&idp).0);
    let claims = idp.claims("test:read");

    let mut id_token = Header::new(Algorithm::EdDSA);
    id_token.typ = Some("id_token+jwt".to_owned());
    call_with(&app, &idp.sign(id_token, &claims))
        .await
        .assert_invalid();

    let token = idp.token(&claims);
    let (_, rest) = token.split_once('.').unwrap();
    let no_kid = format!(
        "{}.{rest}",
        b64(&json!({ "alg": "EdDSA", "typ": "at+jwt" }))
    );
    call_with(&app, &no_kid).await.assert_invalid();
}

#[tokio::test]
async fn tampered_claims_are_invalid() {
    let idp = TestIdp::start().await;
    let app = app(verifier(&idp).0);
    let token = idp.token(&idp.claims("test:read"));
    let parts: Vec<&str> = token.split('.').collect();
    let mut claims = idp.claims("test:read admin:all");
    claims["sub"] = json!("someone-else");
    let tampered = format!("{}.{}.{}", parts[0], b64(&claims), parts[2]);
    call_with(&app, &tampered).await.assert_invalid();
}

// --- 403: valid token, wrong scope -------------------------------------------

#[tokio::test]
async fn missing_scope_is_403_naming_it() {
    let idp = TestIdp::start().await;
    let app = app(verifier(&idp).0);
    let reply = call_with(&app, &idp.token(&idp.claims("test:write other:read"))).await;
    assert_eq!(reply.status, StatusCode::FORBIDDEN);
    assert_eq!(reply.json["code"], "insufficient_scope");
    assert_eq!(
        reply.challenge(),
        r#"Bearer error="insufficient_scope", scope="test:read""#
    );
}

// --- signing keys: discovery, rotation, caching, outages ---------------------

#[tokio::test]
async fn configured_jwks_url_skips_discovery() {
    let idp = TestIdp::start().await;
    let config = AuthConfig {
        jwks_url: Some(idp.jwks_url()),
        ..idp.auth_config()
    };
    let app = app(verifier_for(&config).0);
    let reply = call_with(&app, &idp.token(&idp.claims("test:read"))).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.json);
    assert_eq!(idp.discovery_fetches(), 0);
    assert_eq!(idp.jwks_fetches(), 1);
}

#[tokio::test]
async fn keys_are_fetched_once_and_shared() {
    let idp = TestIdp::start().await;
    let app = app(verifier(&idp).0);
    let token = idp.token(&idp.claims("test:read"));
    let mut calls = tokio::task::JoinSet::new();
    for _ in 0..8 {
        let (app, token) = (app.clone(), token.clone());
        calls.spawn(async move { call_with(&app, &token).await.status });
    }
    while let Some(status) = calls.join_next().await {
        assert_eq!(status.unwrap(), StatusCode::OK);
    }
    call_with(&app, &token).await;
    assert_eq!(idp.jwks_fetches(), 1, "single-flight, then cached");
}

#[tokio::test]
async fn key_rotation_triggers_one_refresh_then_succeeds() {
    let idp = TestIdp::start().await;
    let (verifier, clock) = verifier(&idp);
    let app = app(verifier);
    let old_token = idp.token(&idp.claims("test:read"));
    assert_eq!(call_with(&app, &old_token).await.status, StatusCode::OK);
    assert_eq!(idp.jwks_fetches(), 1);

    // The IdP rotates; the cache does not know the new kid yet.
    clock.advance(TimeDelta::seconds(31));
    idp.rotate_key();
    let new_token = idp.token(&idp.claims("test:read"));
    let reply = call_with(&app, &new_token).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.json);
    assert_eq!(idp.jwks_fetches(), 2, "exactly one refresh");

    // Both keys are now cached; nothing else is fetched.
    assert_eq!(call_with(&app, &new_token).await.status, StatusCode::OK);
    assert_eq!(call_with(&app, &old_token).await.status, StatusCode::OK);
    assert_eq!(idp.jwks_fetches(), 2);
}

#[tokio::test]
async fn unknown_kid_is_invalid_and_refreshes_at_most_every_30_seconds() {
    let idp = TestIdp::start().await;
    let (verifier, clock) = verifier(&idp);
    let app = app(verifier);
    let claims = idp.claims("test:read");
    let forged = idp.unpublished_token(&claims);

    call_with(&app, &forged).await.assert_invalid();
    assert_eq!(idp.jwks_fetches(), 1, "the first fetch");
    clock.advance(TimeDelta::seconds(10));
    call_with(&app, &forged).await.assert_invalid();
    call_with(&app, &idp.unpublished_token(&claims))
        .await
        .assert_invalid();
    assert_eq!(idp.jwks_fetches(), 1, "no refresh inside the cooldown");

    clock.advance(TimeDelta::seconds(21));
    call_with(&app, &forged).await.assert_invalid();
    assert_eq!(idp.jwks_fetches(), 2, "one refresh after it");
}

#[tokio::test]
async fn keys_expire_after_max_age() {
    let idp = TestIdp::start().await;
    idp.set_max_age(Some(120));
    let (verifier, clock) = verifier(&idp);
    let app = app(verifier);
    let token = idp.token(&idp.claims("test:read"));

    call_with(&app, &token).await;
    clock.advance(TimeDelta::seconds(119));
    call_with(&app, &token).await;
    assert_eq!(idp.jwks_fetches(), 1);
    clock.advance(TimeDelta::seconds(1));
    assert_eq!(call_with(&app, &token).await.status, StatusCode::OK);
    assert_eq!(idp.jwks_fetches(), 2);
}

#[tokio::test]
async fn failed_refresh_keeps_the_last_good_keys() {
    let idp = TestIdp::start().await;
    idp.set_max_age(Some(60));
    let (verifier, clock) = verifier(&idp);
    let app = app(verifier);
    let token = idp.token(&idp.claims("test:read"));
    assert_eq!(call_with(&app, &token).await.status, StatusCode::OK);

    idp.set_unavailable(true);
    clock.advance(TimeDelta::seconds(61));
    let reply = call_with(&app, &token).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.json);
    assert_eq!(idp.jwks_fetches(), 2, "the refresh was attempted");
}

#[tokio::test]
async fn unreachable_idp_is_503_not_401() {
    let idp = TestIdp::start().await;
    let token = idp.token(&idp.claims("test:read"));
    let config = AuthConfig {
        issuer: Some(idp.issuer().to_owned()),
        // Nothing listens on port 1.
        jwks_url: Some("http://127.0.0.1:1/certs".to_owned()),
        ..AuthConfig::default()
    };
    let reply = call_with(&app(verifier_for(&config).0), &token).await;
    assert_eq!(
        reply.status,
        StatusCode::SERVICE_UNAVAILABLE,
        "{}",
        reply.json
    );
    assert_eq!(
        reply.headers[header::CONTENT_TYPE],
        "application/problem+json"
    );
}

#[tokio::test]
async fn discovery_must_name_the_configured_issuer() {
    let idp = TestIdp::start().await;
    let token = idp.token(&idp.claims("test:read"));
    // Same server, but configured without the trailing slash: the discovery
    // document's issuer no longer matches, so no keys are trusted.
    let config = AuthConfig {
        issuer: Some(idp.issuer().trim_end_matches('/').to_owned()),
        ..AuthConfig::default()
    };
    let reply = call_with(&app(verifier_for(&config).0), &token).await;
    assert_eq!(
        reply.status,
        StatusCode::SERVICE_UNAVAILABLE,
        "{}",
        reply.json
    );
    assert_eq!(idp.jwks_fetches(), 0);
}

// --- ID tokens and logout tokens (the web BFF is the relying party) ----------

const CLIENT: &str = testing::WEB_CLIENT_ID;

fn invalid(result: &Result<impl std::fmt::Debug, Error>) {
    assert!(
        matches!(result, Err(Error::Unauthorized(AuthFailure::InvalidToken))),
        "{result:?}"
    );
}

/// The login nonce the ID-token tests sign in with, random per run (a literal
/// reads as a hard-coded cryptographic value to static analysis).
fn nonce() -> &'static str {
    static NONCE: LazyLock<String> = LazyLock::new(|| uuid::Uuid::now_v7().to_string());
    &NONCE
}

/// A nonce other than [`nonce`].
fn other_nonce() -> &'static str {
    static OTHER: LazyLock<String> = LazyLock::new(|| uuid::Uuid::now_v7().to_string());
    &OTHER
}

#[tokio::test]
async fn id_token_yields_the_user() {
    let idp = TestIdp::start().await;
    let (verifier, _) = verifier(&idp);
    let mut claims = idp.id_token_claims("user-1", nonce(), "sid-1");
    claims["given_name"] = "Ada".into();
    claims["family_name"] = "Lovelace".into();
    claims["email"] = "ada@example.com".into();
    let id = verifier
        .verify_id_token(&idp.id_token(&claims), CLIENT, nonce())
        .await
        .unwrap();
    assert_eq!(
        id,
        IdToken {
            subject: "user-1".into(),
            session_id: Some("sid-1".into()),
            name: Some("Ada Lovelace".into()),
            email: Some("ada@example.com".into()),
            email_verified: false,
            auth_time: None,
        }
    );

    claims["email_verified"] = true.into();
    claims["auth_time"] = 1_700_000_000.into();
    let id = verifier
        .verify_id_token(&idp.id_token(&claims), CLIENT, nonce())
        .await
        .unwrap();
    assert!(id.email_verified);
    assert_eq!(id.auth_time, Some(1_700_000_000));
    claims["email_verified"] = "true".into();
    let id = verifier
        .verify_id_token(&idp.id_token(&claims), CLIENT, nonce())
        .await
        .unwrap();
    assert!(id.email_verified);
    claims["email_verified"] = "yes".into();
    let id = verifier
        .verify_id_token(&idp.id_token(&claims), CLIENT, nonce())
        .await
        .unwrap();
    assert!(!id.email_verified);

    claims["name"] = "Countess".into();
    let id = verifier
        .verify_id_token(&idp.id_token(&claims), CLIENT, nonce())
        .await
        .unwrap();
    assert_eq!(id.name.as_deref(), Some("Countess"));
}

#[tokio::test]
async fn id_token_must_carry_the_sign_ins_nonce() {
    let idp = TestIdp::start().await;
    let (verifier, _) = verifier(&idp);
    let claims = idp.id_token_claims("user-1", nonce(), "sid-1");
    invalid(
        &verifier
            .verify_id_token(&idp.id_token(&claims), CLIENT, other_nonce())
            .await,
    );
    let mut no_nonce = claims.clone();
    no_nonce.as_object_mut().unwrap().remove("nonce");
    invalid(
        &verifier
            .verify_id_token(&idp.id_token(&no_nonce), CLIENT, nonce())
            .await,
    );
}

#[tokio::test]
async fn id_token_must_be_for_this_client() {
    let idp = TestIdp::start().await;
    let (verifier, _) = verifier(&idp);
    let claims = idp.id_token_claims("user-1", nonce(), "s");
    invalid(
        &verifier
            .verify_id_token(&idp.id_token(&claims), "other-client", nonce())
            .await,
    );

    let mut foreign_azp = claims.clone();
    foreign_azp["azp"] = "other-client".into();
    invalid(
        &verifier
            .verify_id_token(&idp.id_token(&foreign_azp), CLIENT, nonce())
            .await,
    );

    // Several audiences need `azp` to say which one asked.
    let mut many = claims.clone();
    many["aud"] = json!([CLIENT, "campbooks-api"]);
    many.as_object_mut().unwrap().remove("azp");
    invalid(
        &verifier
            .verify_id_token(&idp.id_token(&many), CLIENT, nonce())
            .await,
    );
    many["azp"] = CLIENT.into();
    assert!(
        verifier
            .verify_id_token(&idp.id_token(&many), CLIENT, nonce())
            .await
            .is_ok()
    );

    // An access token is not an ID token: no nonce, and its `typ` is wrong.
    let access = idp.token(&idp.claims("test:read"));
    invalid(&verifier.verify_id_token(&access, CLIENT, nonce()).await);
}

#[tokio::test]
async fn id_token_must_be_current_and_name_a_subject() {
    let idp = TestIdp::start().await;
    let (verifier, _) = verifier(&idp);
    let mut expired = idp.id_token_claims("user-1", nonce(), "s");
    expired["exp"] = (Utc::now() - TimeDelta::hours(1)).timestamp().into();
    assert!(matches!(
        verifier
            .verify_id_token(&idp.id_token(&expired), CLIENT, nonce())
            .await,
        Err(Error::Unauthorized(AuthFailure::ExpiredToken))
    ));

    let mut no_iat = idp.id_token_claims("user-1", nonce(), "s");
    no_iat.as_object_mut().unwrap().remove("iat");
    invalid(
        &verifier
            .verify_id_token(&idp.id_token(&no_iat), CLIENT, nonce())
            .await,
    );

    let blank = idp.id_token_claims(" ", nonce(), "s");
    invalid(
        &verifier
            .verify_id_token(&idp.id_token(&blank), CLIENT, nonce())
            .await,
    );

    let unpublished = idp.unpublished_token(&idp.id_token_claims("user-1", nonce(), "s"));
    invalid(
        &verifier
            .verify_id_token(&unpublished, CLIENT, nonce())
            .await,
    );
}

#[tokio::test]
async fn logout_token_names_the_sessions_to_end() {
    let idp = TestIdp::start().await;
    let (verifier, _) = verifier(&idp);
    let both = idp.logout_token(&idp.logout_token_claims(Some("user-1"), Some("sid-1")));
    assert_eq!(
        verifier.verify_logout_token(&both, CLIENT).await.unwrap(),
        LogoutToken {
            subject: Some("user-1".into()),
            session_id: Some("sid-1".into()),
        }
    );
    let sid_only = idp.logout_token(&idp.logout_token_claims(None, Some("sid-1")));
    assert_eq!(
        verifier
            .verify_logout_token(&sid_only, CLIENT)
            .await
            .unwrap()
            .subject,
        None
    );
}

#[tokio::test]
async fn malformed_logout_tokens_are_rejected() {
    let idp = TestIdp::start().await;
    let (verifier, _) = verifier(&idp);
    let good = idp.logout_token_claims(Some("user-1"), Some("sid-1"));

    let neither = idp.logout_token_claims(None, None);
    let mut with_nonce = good.clone();
    with_nonce["nonce"] = "n".into();
    let mut no_event = good.clone();
    no_event["events"] = json!({});
    let mut no_iat = good.clone();
    no_iat.as_object_mut().unwrap().remove("iat");
    let mut other_client = good.clone();
    other_client["aud"] = "other-client".into();
    for claims in [neither, with_nonce, no_event, no_iat, other_client] {
        invalid(
            &verifier
                .verify_logout_token(&idp.logout_token(&claims), CLIENT)
                .await,
        );
    }
    // An ID token is not a logout token.
    let id = idp.id_token(&idp.id_token_claims("user-1", nonce(), "s"));
    invalid(&verifier.verify_logout_token(&id, CLIENT).await);
}

#[tokio::test]
async fn oidc_tokens_need_a_configured_issuer() {
    let verifier = TokenVerifier::new(&AuthConfig::default());
    assert!(matches!(
        verifier.verify_id_token("a.b.c", CLIENT, nonce()).await,
        Err(Error::Unauthorized(AuthFailure::NotConfigured))
    ));
    assert!(matches!(
        verifier.verify_logout_token("a.b.c", CLIENT).await,
        Err(Error::Unauthorized(AuthFailure::NotConfigured))
    ));
}

// --- an issuer in this process ------------------------------------------------

/// Hands the verifier `idp`'s keys directly, counting reads.
#[derive(Debug)]
struct Local {
    idp: TestIdp,
    reads: std::sync::atomic::AtomicUsize,
    broken: std::sync::atomic::AtomicBool,
}

impl KeySource for Local {
    fn jwks(&self) -> KeySourceFuture<'_> {
        use std::sync::atomic::Ordering;
        self.reads.fetch_add(1, Ordering::SeqCst);
        let broken = self.broken.load(Ordering::SeqCst);
        let document = self.idp.jwks_document();
        Box::pin(async move {
            if broken {
                Err("database unavailable".to_owned())
            } else {
                Ok(document)
            }
        })
    }
}

#[tokio::test]
async fn in_process_issuer_keys_are_read_without_http() {
    use std::sync::atomic::Ordering;
    let idp = TestIdp::start().await;
    let local = Arc::new(Local {
        idp: idp.clone(),
        reads: 0.into(),
        broken: false.into(),
    });
    let clock = Arc::new(FixedClock::new(Utc::now()));
    let verifier = TokenVerifier::in_process_with_clock(
        &AuthConfig::default(),
        idp.issuer(),
        local.clone(),
        clock.clone(),
    );
    assert_eq!(verifier.issuer(), Some(idp.issuer()));
    let app = app(verifier);

    let reply = call_with(&app, &idp.token(&idp.claims("test:read"))).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.json);
    call_with(&app, &idp.token(&idp.claims("test:read"))).await;
    assert_eq!(local.reads.load(Ordering::SeqCst), 1, "cached");
    assert_eq!(idp.jwks_fetches() + idp.discovery_fetches(), 0, "no HTTP");

    // A new key is picked up on the first token that names it.
    idp.rotate_key();
    clock.advance(TimeDelta::seconds(31));
    let reply = call_with(&app, &idp.token(&idp.claims("test:read"))).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.json);
    assert_eq!(local.reads.load(Ordering::SeqCst), 2);

    // A failing source keeps the last good keys.
    local.broken.store(true, Ordering::SeqCst);
    clock.advance(TimeDelta::hours(1));
    let reply = call_with(&app, &idp.token(&idp.claims("test:read"))).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.json);
}

#[tokio::test]
async fn in_process_issuer_that_never_answered_is_503() {
    let idp = TestIdp::start().await;
    let local = Arc::new(Local {
        idp: idp.clone(),
        reads: 0.into(),
        broken: true.into(),
    });
    let verifier = TokenVerifier::in_process(&AuthConfig::default(), idp.issuer(), local);
    let reply = call_with(&app(verifier), &idp.token(&idp.claims("test:read"))).await;
    assert_eq!(reply.status, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn the_in_process_issuer_requires_at_jwt_on_access_tokens() {
    let idp = TestIdp::start().await;
    let local = Arc::new(Local {
        idp: idp.clone(),
        reads: 0.into(),
        broken: false.into(),
    });
    let in_process = app(TokenVerifier::in_process(
        &AuthConfig::default(),
        idp.issuer(),
        local,
    ));
    assert_eq!(
        call_with(&in_process, &idp.token(&idp.claims("test:read")))
            .await
            .status,
        StatusCode::OK
    );
    // An ID token (`JWT`), or no `typ` at all, never passes, whatever its aud.
    call_with(&in_process, &idp.id_token(&idp.claims("test:read")))
        .await
        .assert_invalid();
    let mut untyped = Header::new(Algorithm::EdDSA);
    untyped.typ = None;
    untyped.kid = Some(idp.current_kid());
    call_with(&in_process, &idp.sign(untyped, &idp.claims("test:read")))
        .await
        .assert_invalid();

    // An outside identity provider may still say `JWT` (Rauthy 0.36).
    let (remote, _) = verifier(&idp);
    assert_eq!(
        call_with(&app(remote), &idp.id_token(&idp.claims("test:read")))
            .await
            .status,
        StatusCode::OK
    );
}
