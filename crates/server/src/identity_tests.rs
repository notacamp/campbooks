//! The whole web sign-in with the in-process issuer (ADR 0014): the BFF is an
//! OIDC client of the server's own issuer, reached through its router; a
//! new person signs up with a (fake) Google, lands in the app with a
//! session, creates a workspace, and the mailbox their sign-up granted
//! becomes that workspace's connection, through the connectors module's
//! grant intake. Run with `--nocapture` to see each step narrated.

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
    blobs::BlobStore,
    clock::SystemClock,
    crypto::SecretKey,
    testing::{
        self, TestIdp, WEB_CLIENT_ID as FAKE_GOOGLE_CLIENT, WEB_CLIENT_SECRET as FAKE_GOOGLE_SECRET,
    },
};
use figment::{Figment, providers::Serialized};
use http_body_util::BodyExt as _;
use secrecy::SecretString;
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt as _;

use crate::bff::config::{self, BffConfig, Loaded};

const ORIGIN: &str = "http://localhost:8080";
/// Gmail's scope: what the connectors module asks for in the sign-up consent.
const GMAIL: &str = "https://www.googleapis.com/auth/gmail.modify";

/// One narrated step of the walk-through (shown with `--nocapture`).
fn step(text: &str) {
    println!("  \u{2192} {text}");
}

/// Every log line written while a test runs (all levels).
#[derive(Clone, Default)]
struct Logs(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Logs {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Logs {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap_or_else(PoisonError::into_inner)).into_owned()
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
            let jar: Vec<String> = self
                .cookies
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect();
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
    let logs = Logs::default();
    let writer = logs.clone();
    let _logging = tracing::subscriber::set_default(
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_writer(move || writer.clone())
            .finish(),
    );
    let google = TestIdp::start().await;
    let config = testing::config("postgres://test@localhost/test");
    assert!(config.auth.issuer.is_none(), "no other identity provider");
    let secret = SecretKey::from_bytes([5; 32]);
    let scopes: Vec<&str> = crate::scope_catalogue().iter().map(|s| s.name).collect();
    let google_settings = Figment::from(Serialized::defaults(serde_json::json!({
        "google": { "client_id": FAKE_GOOGLE_CLIENT, "client_secret": FAKE_GOOGLE_SECRET },
    })));
    let mut composition = crate::Composition {
        connectors: campbooks_connectors::Deps::from_figment(
            &google_settings,
            &config,
            Some(secret.clone()),
        )
        .unwrap(),
        identity: campbooks_identity::Deps::from_figment(
            &Figment::from(Serialized::defaults(serde_json::json!({}))),
            &config,
            Some(secret.clone()),
            &scopes,
            Arc::new(SystemClock),
        )
        .unwrap(),
        ..crate::Composition::default()
    };
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
    let pool = testing::app_role_pool(&db).await;
    let blobs = BlobStore::memory(&config.blobs, pool.clone());
    let registry = crate::registry_with(&composition).unwrap();
    let state = crate::with_composed_ports(
        AppState::new(pool, config)
            .with_blobs(blobs)
            .with_registry(registry),
        &composition,
    );
    let mut browser = Browser {
        app: crate::router_with(state, Some(*settings), &composition),
        cookies: BTreeMap::new(),
    };

    println!("\nSign up with Google through the web app, Campbooks as its own issuer:");
    // The web app sends the browser to sign in; the issuer shows its screen.
    let to_issuer = browser.get("/bff/login?return_to=/welcome").await;
    assert!(
        to_issuer
            .location()
            .starts_with(&format!("{ORIGIN}/oauth2/authorize?"))
    );
    let to_screen = browser.get(&to_issuer.location()).await;
    let screen = to_screen.location();
    assert!(screen.starts_with("/sign-in?request="), "{screen}");
    let request = screen.trim_start_matches("/sign-in?request=").to_owned();
    step("/bff/login -> /oauth2/authorize (PKCE S256) -> nobody signed in -> /sign-in");

    // Google, then "Create account".
    let to_google = browser
        .get(&format!("/auth/start/google?request={request}"))
        .await;
    let asked = to_google.location();
    assert!(
        asked.contains(&url::form_urlencoded::byte_serialize(GMAIL.as_bytes()).collect::<String>())
    );
    assert!(asked.contains("access_type=offline"));
    step("one Google consent: openid email profile + gmail.modify, offline access");
    let callback = google.authorize(&asked);
    let to_new = browser.get(&callback).await;
    assert!(to_new.location().starts_with("/sign-in/new"));
    step("Google account unknown here -> /sign-in/new (nothing created yet)");
    let created = browser.send(Method::POST, "/auth/new-account", None).await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.body);
    let to_bff = created.json()["redirect"].as_str().unwrap().to_owned();
    assert!(to_bff.starts_with(&format!("{ORIGIN}/bff/callback?code=")));
    step("Create account -> back to the web app with a one-time code");

    // The BFF redeems the code in-process and starts the web session.
    let landed = browser.get(&to_bff).await;
    assert_eq!(landed.status, StatusCode::OK, "{}", landed.body);
    assert!(landed.body.contains("/welcome"));
    let session = browser.get("/bff/session").await.json();
    assert_eq!(session["authenticated"], true, "{session}");
    assert_eq!(session["user"]["email"], "test-user@example.com");
    step("BFF redeems the code through the issuer's router: web session for test-user@example.com");

    // The mailbox waits for a workspace; creating one and adopting it
    // makes it that workspace's connection.
    let held = browser.get("/bff/api/v1/identity/held-grants").await;
    assert_eq!(held.status, StatusCode::OK, "{}", held.body);
    let grant = held.json()["items"][0].clone();
    assert_eq!(grant["address"], "test-user@example.com");
    let grant = grant["id"].as_str().unwrap().to_owned();
    step("held mailbox: test-user@example.com, waiting for a workspace (7 days at most)");
    let workspace = browser
        .send(
            Method::POST,
            "/bff/api/v1/workspaces",
            Some(serde_json::json!({ "name": "Home", "kind": "personal" })),
        )
        .await;
    assert_eq!(workspace.status, StatusCode::CREATED, "{}", workspace.body);
    let workspace = workspace.json()["id"].as_str().unwrap().to_owned();
    step("first workspace \"Home\" created");
    let adopted = browser
        .send(
            Method::POST,
            &format!("/bff/api/v1/w/{workspace}/identity/held-grants/{grant}/adopt"),
            None,
        )
        .await;
    assert_eq!(adopted.status, StatusCode::OK, "{}", adopted.body);
    let connection = adopted.json()["connection"].as_str().unwrap().to_owned();
    let connections = browser
        .get(&format!("/bff/api/v1/w/{workspace}/connections"))
        .await
        .json();
    let listed = &connections["connections"][0];
    assert_eq!(listed["id"], connection);
    assert_eq!(listed["address"], "test-user@example.com");
    assert_eq!(listed["capabilities"], serde_json::json!(["messages"]));
    assert_eq!(
        browser.get("/bff/api/v1/identity/held-grants").await.json()["items"],
        serde_json::json!([])
    );
    step("adopted -> connectors made it the workspace's Gmail connection; no consent asked");

    // Signing out ends the issuer's session too, with no confirmation page.
    let out = browser.send(Method::POST, "/bff/logout", None).await;
    let url = out.json()["logout_url"].as_str().unwrap().to_owned();
    let back = browser.get(&url).await;
    assert_eq!(back.location(), format!("{ORIGIN}/"));
    let again = browser.get(&to_issuer.location()).await;
    assert!(again.location().starts_with("/sign-in?request="));
    step("sign out -> issuer session ended, no confirmation page");

    // Nothing secret or personal reached a log line: no code, cookie,
    // token or address.
    let text = logs.text();
    assert!(!text.is_empty(), "the flow logged something");
    let google_code = callback
        .split("code=")
        .nth(1)
        .unwrap()
        .split('&')
        .next()
        .unwrap();
    let our_code = to_bff
        .split("code=")
        .nth(1)
        .unwrap()
        .split('&')
        .next()
        .unwrap();
    for secret in [
        google_code,
        our_code,
        "test-user@example.com",
        FAKE_GOOGLE_SECRET,
    ] {
        assert!(
            !text.contains(secret),
            "a log line holds a secret or an address"
        );
    }
    for cookie in browser.cookies.values() {
        assert!(!text.contains(cookie.as_str()), "a log line holds a cookie");
    }
    step("every log line checked: no code, cookie, secret or address");
}
