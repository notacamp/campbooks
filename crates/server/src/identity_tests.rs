//! The whole web sign-in with the in-process issuer (ADR 0014): the BFF is an
//! OpenID client of the server's own issuer, reached through its router; a
//! new person signs up with a (fake) Google, lands in the app with a
//! session, creates a workspace, and the mailbox their sign-up granted is
//! handed to the grant-intake port for that workspace.

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, PoisonError},
};

use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Method, Request, StatusCode, header},
};
use campbooks_identity::upstream::google::{Google, GoogleEndpoints};
use campbooks_kernel::{
    AppState,
    clock::SystemClock,
    connections::{AdoptFuture, GrantIntake, ProviderGrant},
    crypto::SecretKey,
    testing::{self, TestIdp, WEB_CLIENT_ID as FAKE_GOOGLE_CLIENT, WEB_CLIENT_SECRET as FAKE_GOOGLE_SECRET},
};
use figment::{Figment, providers::Serialized};
use http_body_util::BodyExt as _;
use secrecy::SecretString;
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt as _;

use crate::bff::config::{self, BffConfig, Loaded};

const ORIGIN: &str = "http://localhost:8080";
const MAIL: &str = "https://mail.example.com/modify";

#[derive(Debug, Default)]
struct Intake {
    adopted: Mutex<Vec<(String, String)>>,
}

impl GrantIntake for Intake {
    fn mailbox_scopes(&self, _provider: &str) -> Vec<String> {
        vec![MAIL.to_owned()]
    }

    fn adopt<'a>(&'a self, _state: &'a AppState, grant: ProviderGrant) -> AdoptFuture<'a> {
        self.adopted
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((grant.workspace.to_string(), grant.address));
        Box::pin(async { Ok("connection-1".to_owned()) })
    }
}

struct Browser {
    app: Router,
    cookies: BTreeMap<String, String>,
}

struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    body: String,
}

impl Reply {
    fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or(Value::Null)
    }

    fn location(&self) -> String {
        self.headers
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_else(|| panic!("no Location ({}): {}", self.status, self.body))
            .to_owned()
    }
}

impl Browser {
    async fn send(&mut self, method: Method, url: &str, body: Option<Value>) -> Reply {
        let path = url.strip_prefix(ORIGIN).unwrap_or(url);
        let mut request = Request::builder().method(method.clone()).uri(path);
        if !self.cookies.is_empty() {
            let jar: Vec<String> = self.cookies.iter().map(|(k, v)| format!("{k}={v}")).collect();
            request = request.header(header::COOKIE, jar.join("; "));
        }
        if method != Method::GET {
            request = request
                .header("x-campbooks-csrf", "1")
                .header("sec-fetch-site", "same-origin")
                .header(header::CONTENT_TYPE, "application/json")
                .header("idempotency-key", uuid::Uuid::now_v7().to_string());
        }
        let body = body.map_or_else(Body::empty, |v| Body::from(v.to_string()));
        let response = self
            .app
            .clone()
            .oneshot(request.body(body).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        for set in headers.get_all(header::SET_COOKIE) {
            let set = set.to_str().unwrap();
            let (name, value) = set.split(';').next().unwrap().split_once('=').unwrap();
            if value.is_empty() {
                self.cookies.remove(name);
            } else {
                self.cookies.insert(name.to_owned(), value.to_owned());
            }
        }
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        Reply {
            status,
            headers,
            body: String::from_utf8_lossy(&bytes).into_owned(),
        }
    }

    async fn get(&mut self, url: &str) -> Reply {
        self.send(Method::GET, url, None).await
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_new_person_signs_up_through_the_bff_and_their_mailbox_reaches_their_workspace(
    db: PgPool,
) {
    let google = TestIdp::start().await;
    let config = testing::config("postgres://test@localhost/test");
    assert!(config.auth.issuer.is_none(), "no other identity provider");
    let secret = SecretKey::from_bytes([5; 32]);
    let scopes: Vec<&str> = crate::scope_catalogue().iter().map(|s| s.name).collect();
    let mut composition = crate::Composition::default();
    composition.identity = campbooks_identity::Deps::from_figment(
        &Figment::from(Serialized::defaults(serde_json::json!({}))),
        &config,
        Some(secret.clone()),
        &scopes,
        Arc::new(SystemClock),
    )
    .unwrap();
    let issuer = composition
        .identity
        .issuer
        .as_mut()
        .and_then(Arc::get_mut)
        .unwrap();
    let base = google.issuer();
    let at = |path: &str| url::Url::parse(&format!("{base}{path}")).unwrap();
    issuer.google = Some(
        Google::new(
            FAKE_GOOGLE_CLIENT.to_owned(),
            campbooks_identity::deps::ClientSecret::Inline(SecretString::from(FAKE_GOOGLE_SECRET)),
            ORIGIN,
            GoogleEndpoints {
                issuer: base.to_owned(),
                authorize: at("oidc/authorize"),
                token: at("oidc/token"),
                revoke: at("oidc/token/revoke"),
                jwks: url::Url::parse(&google.jwks_url()).unwrap(),
            },
        )
        .unwrap(),
    );
    let Ok(Loaded::Enabled(settings)) =
        config::settings(&config, BffConfig::default(), Some(secret), &scopes)
    else {
        panic!("the BFF signs in with the in-process issuer");
    };
    assert!(settings.in_process);
    let intake = Arc::new(Intake::default());
    let state = crate::with_composed_ports(
        AppState::new(testing::app_role_pool(&db).await, config),
        &composition,
    )
    .with_grant_intake(intake.clone());
    let mut browser = Browser {
        app: crate::router_with(state, Some(*settings), &composition),
        cookies: BTreeMap::new(),
    };

    // The web app sends the browser to sign in; the issuer shows its screen.
    let to_issuer = browser.get("/bff/login?return_to=/welcome").await;
    assert!(to_issuer.location().starts_with(&format!("{ORIGIN}/oauth2/authorize?")));
    let to_screen = browser.get(&to_issuer.location()).await;
    let screen = to_screen.location();
    assert!(screen.starts_with("/sign-in?request="), "{screen}");
    let request = screen.trim_start_matches("/sign-in?request=").to_owned();

    // Google, then "Create account".
    let to_google = browser
        .get(&format!("/auth/start/google?request={request}"))
        .await;
    let callback = google.authorize(&to_google.location());
    let to_new = browser.get(&callback).await;
    assert!(to_new.location().starts_with("/sign-in/new"));
    let created = browser.send(Method::POST, "/auth/new-account", None).await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.body);
    let to_bff = created.json()["redirect"].as_str().unwrap().to_owned();
    assert!(to_bff.starts_with(&format!("{ORIGIN}/bff/callback?code=")));

    // The BFF redeems the code in-process and starts the web session.
    let landed = browser.get(&to_bff).await;
    assert_eq!(landed.status, StatusCode::OK, "{}", landed.body);
    assert!(landed.body.contains("/welcome"));
    let session = browser.get("/bff/session").await.json();
    assert_eq!(session["authenticated"], true, "{session}");
    assert_eq!(session["user"]["email"], "test-user@example.com");

    // The mailbox waits for a workspace; creating one and adopting it
    // hands the grant over.
    let held = browser.get("/bff/api/v1/identity/held-grants").await;
    assert_eq!(held.status, StatusCode::OK, "{}", held.body);
    let grant = held.json()["items"][0]["id"].as_str().unwrap().to_owned();
    let workspace = browser
        .send(
            Method::POST,
            "/bff/api/v1/workspaces",
            Some(serde_json::json!({ "name": "Home", "kind": "personal" })),
        )
        .await;
    assert_eq!(workspace.status, StatusCode::CREATED, "{}", workspace.body);
    let workspace = workspace.json()["id"].as_str().unwrap().to_owned();
    let adopted = browser
        .send(
            Method::POST,
            &format!("/bff/api/v1/w/{workspace}/identity/held-grants/{grant}/adopt"),
            None,
        )
        .await;
    assert_eq!(adopted.status, StatusCode::OK, "{}", adopted.body);
    assert_eq!(
        *intake.adopted.lock().unwrap(),
        [(workspace, "test-user@example.com".to_owned())]
    );

    // Signing out ends the issuer's session too, with no confirmation page.
    let out = browser.send(Method::POST, "/bff/logout", None).await;
    let end_session = out.json()["logout_url"].as_str().map(ToOwned::to_owned);
    if let Some(url) = end_session {
        let back = browser.get(&url).await;
        assert_eq!(back.location(), format!("{ORIGIN}/"));
    }
    let again = browser.get(&to_issuer.location()).await;
    assert!(again.location().starts_with("/sign-in?request="));
}
