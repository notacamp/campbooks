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
use campbooks_identity::upstream::{
    Provider,
    client::{Endpoints, IssuerRule, Server, Upstream},
};
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

pub(crate) const ORIGIN: &str = "http://localhost:8080";
/// Gmail's scope: what the connectors module asks for in the sign-up consent.
const GMAIL: &str = "https://www.googleapis.com/auth/gmail.modify";

/// One narrated step of the walk-through (shown with `--nocapture`).
pub(crate) fn step(text: &str) {
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

pub(crate) struct Browser {
    app: Router,
    cookies: BTreeMap<String, String>,
}

pub(crate) struct Reply {
    pub(crate) status: StatusCode,
    pub(crate) headers: HeaderMap,
    pub(crate) body: String,
}

impl Reply {
    pub(crate) fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or(Value::Null)
    }

    pub(crate) fn location(&self) -> String {
        self.headers
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_else(|| panic!("no Location ({}): {}", self.status, self.body))
            .to_owned()
    }
}

impl Browser {
    /// Another browser: no cookies.
    pub(crate) fn forget_cookies(&mut self) {
        self.cookies.clear();
    }

    pub(crate) async fn send(&mut self, method: Method, url: &str, body: Option<Value>) -> Reply {
        let key = uuid::Uuid::now_v7().to_string();
        self.send_keyed(method, url, body, &key).await
    }

    /// [`send`](Self::send) with the `Idempotency-Key` `key` (on anything
    /// but a GET).
    pub(crate) async fn send_keyed(
        &mut self,
        method: Method,
        url: &str,
        body: Option<Value>,
        key: &str,
    ) -> Reply {
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
                .header("idempotency-key", key);
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

    pub(crate) async fn get(&mut self, url: &str) -> Reply {
        self.send(Method::GET, url, None).await
    }

    /// A request with `headers` and a raw `body`, no cookies (an outside
    /// client, not a browser).
    pub(crate) async fn raw(
        &self,
        method: Method,
        url: &str,
        headers: &[(&str, &str)],
        body: Option<String>,
    ) -> Reply {
        let mut request = Request::builder().method(method).uri(url);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let response = self
            .app
            .clone()
            .oneshot(
                request
                    .body(body.map_or_else(Body::empty, Body::from))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        Reply {
            status,
            headers,
            body: String::from_utf8_lossy(&bytes).into_owned(),
        }
    }

    /// A form POST from an outside client.
    async fn form(&self, url: &str, pairs: &[(&str, &str)]) -> Reply {
        let body = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(pairs.iter().copied())
            .finish();
        self.raw(
            Method::POST,
            url,
            &[("content-type", "application/x-www-form-urlencoded")],
            Some(body),
        )
        .await
    }
}

/// The server with the in-process issuer, a (fake) Google and the web BFF,
/// and a browser with no cookies; and the issuer (to mint tokens no client
/// could get).
pub(crate) async fn world(db: PgPool) -> (Browser, TestIdp, Arc<campbooks_identity::deps::Issuer>) {
    let google = TestIdp::start().await;
    let mut config = testing::config("postgres://test@localhost/test");
    assert!(config.auth.issuer.is_none(), "no other identity provider");
    // Plain-http public URL (the tests' localhost): development only.
    config.env = campbooks_kernel::config::Environment::Development;
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
            // MCP agents may register themselves here (off by default).
            &Figment::from(Serialized::defaults(serde_json::json!({
                "oauth": { "dynamic_registration": "on" },
            }))),
            &config,
            Some(secret.clone()),
            &crate::scope_catalogue(),
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
    issuer.upstreams = vec![
        Upstream::new(
            Provider::Google,
            FAKE_GOOGLE_CLIENT.to_owned(),
            campbooks_identity::deps::ClientSecret::Inline(SecretString::from(FAKE_GOOGLE_SECRET)),
            campbooks_identity::upstream::redirect_uri(Provider::Google, ORIGIN, None).unwrap(),
            Endpoints {
                authorize: at("oidc/authorize"),
                issuers: IssuerRule::Fixed(Server {
                    issuer: base.to_owned(),
                    token: at("oidc/token"),
                    jwks: url::Url::parse(&google.jwks_url()).unwrap(),
                }),
            },
        )
        .unwrap(),
    ];
    let Ok(Loaded::Enabled(settings)) =
        config::settings(&config, BffConfig::default(), Some(secret), &scopes)
    else {
        panic!("the BFF signs in with the in-process issuer");
    };
    assert!(settings.in_process);
    let issuer = composition.identity.issuer.clone().unwrap();
    let pool = testing::app_role_pool(&db).await;
    let blobs = BlobStore::memory(&config.blobs, pool.clone());
    let registry = crate::registry_with(&composition).unwrap();
    let state = crate::with_composed_ports(
        AppState::new(pool, config)
            .with_blobs(blobs)
            .with_registry(registry),
        &composition,
    );
    let browser = Browser {
        app: crate::router_with(state, Some(*settings), &composition),
        cookies: BTreeMap::new(),
    };

    (browser, google, issuer)
}

/// An access token like `access` (its account, session, client and
/// sign-in time) with `scope`, bound to `workspace` when given.
async fn minted_token(
    db: &PgPool,
    issuer: &campbooks_identity::deps::Issuer,
    access: &str,
    workspace: Option<&str>,
    scope: &str,
) -> String {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    use campbooks_identity::domain::{accounts, tokens};

    let claims: Value = serde_json::from_slice(
        &URL_SAFE_NO_PAD
            .decode(access.split('.').nth(1).unwrap())
            .unwrap(),
    )
    .unwrap();
    let account: uuid::Uuid = claims["sub"].as_str().unwrap().parse().unwrap();
    let grant = tokens::Grant {
        client_id: claims["client_id"].as_str().unwrap().to_owned(),
        account_id: account,
        session_id: claims["sid"].as_str().and_then(|s| s.parse().ok()),
        scope: scope.to_owned(),
        auth_time: chrono::Utc::now(),
        resource: format!("{ORIGIN}/api"),
        workspace: workspace.map(|w| w.parse().unwrap()),
    };
    let profile = accounts::profile(db, account).await.unwrap().unwrap();
    let signer = issuer.keys.signer(db).await.unwrap();
    tokens::Mint {
        issuer: &issuer.url,
        signer: &signer,
        grant: &grant,
        profile: &profile,
        amr: &[],
        family: None,
        consent: None,
        now: chrono::Utc::now(),
    }
    .access_token()
    .unwrap()
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
    let (mut browser, google, _) = world(db.clone()).await;

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

    // Signing out ends the issuer's session too, with no confirmation page,
    // even for a session from before a release that asks for more scopes
    // (one that would otherwise be retired, the issuer's session kept).
    sqlx::query("UPDATE bff.sessions SET scopes = 'openid'")
        .execute(&db)
        .await
        .unwrap();
    let out = browser.send(Method::POST, "/bff/logout", None).await;
    // The BFF ended the issuer session itself, with the ID token it keeps:
    // nothing for the browser to visit.
    assert_eq!(out.json()["logout_url"], serde_json::Value::Null);
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

/// Sign up through the web app as the fake Google's user and land with a
/// web session.
pub(crate) async fn sign_up_through_bff(browser: &mut Browser, google: &TestIdp) {
    let to_issuer = browser.get("/bff/login?return_to=/welcome").await;
    let screen = browser.get(&to_issuer.location()).await.location();
    let request = screen.trim_start_matches("/sign-in?request=").to_owned();
    let to_google = browser
        .get(&format!("/auth/start/google?request={request}"))
        .await;
    let to_new = browser.get(&google.authorize(&to_google.location())).await;
    assert!(to_new.location().starts_with("/sign-in/new"));
    let created = browser.send(Method::POST, "/auth/new-account", None).await;
    let to_bff = created.json()["redirect"].as_str().unwrap().to_owned();
    assert_eq!(browser.get(&to_bff).await.status, StatusCode::OK);
}

/// Create a workspace through the web app; its id.
pub(crate) async fn create_workspace(browser: &mut Browser, name: &str) -> String {
    let created = browser
        .send(
            Method::POST,
            "/bff/api/v1/workspaces",
            Some(serde_json::json!({ "name": name, "kind": "business" })),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.body);
    created.json()["id"].as_str().unwrap().to_owned()
}

#[sqlx::test(migrations = "../../migrations")]
async fn an_agent_finds_the_issuer_registers_is_allowed_and_calls_one_workspace(db: PgPool) {
    let logs = Logs::default();
    let writer = logs.clone();
    let _logging = tracing::subscriber::set_default(
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_writer(move || writer.clone())
            .finish(),
    );
    let (mut browser, google, issuer) = world(db.clone()).await;
    println!("\nAn MCP agent connects to Campbooks (RFC 9728, 7591, 8707), a person allows it:");
    sign_up_through_bff(&mut browser, &google).await;
    let home = create_workspace(&mut browser, "Home").await;
    let work = create_workspace(&mut browser, "Work").await;
    step("Ana signed up through the web app; workspaces Home and Work");

    // The agent knows only the API's URL.
    let challenge = browser
        .raw(Method::GET, &format!("/api/v1/w/{home}/inboxes"), &[], None)
        .await;
    assert_eq!(challenge.status, StatusCode::UNAUTHORIZED);
    let www = challenge.headers[header::WWW_AUTHENTICATE]
        .to_str()
        .unwrap();
    let metadata_url = format!("{ORIGIN}/.well-known/oauth-protected-resource/api");
    assert_eq!(www, format!(r#"Bearer resource_metadata="{metadata_url}""#));
    step(&format!("401 WWW-Authenticate: {www}"));
    let resource = browser
        .raw(Method::GET, &metadata_url, &[], None)
        .await
        .json();
    assert_eq!(resource["resource"], format!("{ORIGIN}/api"));
    assert_eq!(resource["authorization_servers"][0], ORIGIN);
    let server = browser
        .raw(
            Method::GET,
            "/.well-known/oauth-authorization-server",
            &[],
            None,
        )
        .await
        .json();
    step(&format!(
        "protected resource {} -> issuer {} -> registration {}",
        resource["resource"], server["issuer"], server["registration_endpoint"]
    ));

    // It registers itself: a public client, unverified.
    let callback = "http://127.0.0.1:33418/callback";
    let registered = browser
        .raw(
            Method::POST,
            "/oauth2/register",
            &[("content-type", "application/json")],
            Some(
                serde_json::json!({
                    "client_name": "Desk Agent",
                    "redirect_uris": [callback],
                    "grant_types": ["authorization_code", "refresh_token"],
                    "scope": "conversations:read workspaces:read offline_access identity:write workspaces:write",
                })
                .to_string(),
            ),
        )
        .await;
    assert_eq!(
        registered.status,
        StatusCode::CREATED,
        "{}",
        registered.body
    );
    let client = registered.json()["client_id"].as_str().unwrap().to_owned();
    assert_eq!(
        registered.json()["scope"],
        "conversations:read workspaces:read"
    );
    step(&format!(
        "POST /oauth2/register -> {client} (public, PKCE; identity:* and workspaces:write dropped)"
    ));

    // The person's browser: signed in already, so straight to the consent
    // screen, which names the app as unverified and asks for a workspace.
    let verifier = campbooks_kernel::crypto::random_token();
    let mut authorize = url::Url::parse(&format!("{ORIGIN}/oauth2/authorize")).unwrap();
    authorize
        .query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", &client)
        .append_pair("redirect_uri", callback)
        .append_pair("scope", "conversations:read workspaces:read offline_access")
        .append_pair("state", "agent-state")
        .append_pair(
            "code_challenge",
            &campbooks_kernel::crypto::pkce_challenge(&verifier),
        )
        .append_pair("code_challenge_method", "S256")
        .append_pair("resource", &format!("{ORIGIN}/api"));
    let to_consent = browser.get(authorize.as_str()).await.location();
    assert!(to_consent.starts_with("/consent?request="), "{to_consent}");
    let request = to_consent
        .trim_start_matches("/consent?request=")
        .to_owned();
    let shown = browser
        .get(&format!("/auth/consent/{request}"))
        .await
        .json();
    assert_eq!(shown["client"]["verified"], false);
    assert_eq!(shown["client"]["device"], true);
    let names: Vec<&str> = shown["workspaces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Home", "Work"]);
    step("consent screen: \"Desk Agent\" (unverified, an app on this device), pick a workspace");
    let allowed = browser
        .send(
            Method::POST,
            &format!("/auth/consent/{request}"),
            Some(serde_json::json!({ "decision": "allow", "workspace_id": home })),
        )
        .await;
    assert_eq!(allowed.status, StatusCode::OK, "{}", allowed.body);
    let back = url::Url::parse(allowed.json()["redirect"].as_str().unwrap()).unwrap();
    let code = back
        .query_pairs()
        .find(|(k, _)| k == "code")
        .unwrap()
        .1
        .into_owned();
    step("Allow in Home -> code to the agent's loopback callback");

    let tokens = browser
        .form(
            "/oauth2/token",
            &[
                ("grant_type", "authorization_code"),
                ("code", &code),
                ("redirect_uri", callback),
                ("code_verifier", &verifier),
                ("client_id", &client),
                ("resource", &format!("{ORIGIN}/api")),
            ],
        )
        .await;
    assert_eq!(tokens.status, StatusCode::OK, "{}", tokens.body);
    let access = tokens.json()["access_token"].as_str().unwrap().to_owned();
    let bearer = format!("Bearer {access}");
    step("token: aud = <origin>/api, workspace_id = Home");
    let in_home = browser
        .raw(
            Method::GET,
            &format!("/api/v1/w/{home}/inboxes"),
            &[("authorization", &bearer)],
            None,
        )
        .await;
    assert_eq!(in_home.status, StatusCode::OK, "{}", in_home.body);
    let in_work = browser
        .raw(
            Method::GET,
            &format!("/api/v1/w/{work}/inboxes"),
            &[("authorization", &bearer)],
            None,
        )
        .await;
    assert_eq!(in_work.status, StatusCode::NOT_FOUND);
    step("GET Home's inboxes -> 200; Work's -> 404 (bound to Home)");

    // Every route outside a workspace: the bound token sees Home only, and
    // reaches nothing account-wide.
    let call = |method: Method, path: String, body: Option<&'static str>| {
        let browser = &browser;
        let bearer = bearer.clone();
        async move {
            let mut headers = vec![("authorization", bearer.as_str())];
            if body.is_some() {
                headers.push(("content-type", "application/json"));
            }
            browser
                .raw(method, &path, &headers, body.map(ToOwned::to_owned))
                .await
        }
    };
    let listed = call(Method::GET, "/api/v1/workspaces".to_owned(), None).await;
    assert_eq!(listed.status, StatusCode::OK, "{}", listed.body);
    let names: Vec<String> = listed.json()["workspaces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["name"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(names, ["Home"]);
    for (method, path, body, expected) in [
        (
            Method::POST,
            "/api/v1/workspaces".to_owned(),
            Some(r#"{"name":"Sneaky","kind":"business"}"#),
            StatusCode::FORBIDDEN,
        ),
        (
            Method::POST,
            "/api/v1/workspaces/invitations/accept".to_owned(),
            Some(r#"{"token":"x"}"#),
            StatusCode::FORBIDDEN,
        ),
        (
            Method::GET,
            "/api/v1/identity/consents".to_owned(),
            None,
            StatusCode::FORBIDDEN,
        ),
        (
            Method::GET,
            "/api/v1/identity/ways-in".to_owned(),
            None,
            StatusCode::FORBIDDEN,
        ),
        (
            Method::GET,
            "/api/v1/identity/held-grants".to_owned(),
            None,
            StatusCode::FORBIDDEN,
        ),
        (
            Method::GET,
            "/api/v1/identity/passkeys".to_owned(),
            None,
            StatusCode::FORBIDDEN,
        ),
        (
            Method::GET,
            format!("/api/v1/w/{home}/identity/clients"),
            None,
            StatusCode::FORBIDDEN,
        ),
        (
            Method::GET,
            format!("/api/v1/w/{work}/changes"),
            None,
            StatusCode::NOT_FOUND,
        ),
        (
            Method::GET,
            "/admin/v1/system/overview".to_owned(),
            None,
            StatusCode::NOT_FOUND,
        ),
    ] {
        let reply = call(method.clone(), path.clone(), body).await;
        assert_eq!(reply.status, expected, "{method} {path}: {}", reply.body);
    }
    step("outside a workspace: lists Home only; creating, joining, accounts, apps, admin refused");

    // The same, for a bound token that does hold the scopes (no app may be
    // granted `identity:*`, so it is minted here): account-wide and
    // workspace-management routes answer 404, and Work stays out of reach.
    let bound = minted_token(
        &db,
        &issuer,
        &access,
        Some(&home),
        "identity:read identity:write identity:clients connectors:write search:read \
         conversations:read",
    )
    .await;
    let bound = format!("Bearer {bound}");
    let none = uuid::Uuid::now_v7();
    for (method, path, body) in [
        (Method::GET, "/api/v1/identity/ways-in".to_owned(), None),
        (
            Method::DELETE,
            format!("/api/v1/identity/ways-in/{none}"),
            None,
        ),
        (
            Method::POST,
            "/api/v1/identity/links".to_owned(),
            Some(r#"{"purpose":"link","provider":"google"}"#),
        ),
        (Method::GET, "/api/v1/identity/passkeys".to_owned(), None),
        (
            Method::POST,
            "/api/v1/identity/passkeys/registrations".to_owned(),
            None,
        ),
        (
            Method::DELETE,
            format!("/api/v1/identity/passkeys/{none}"),
            None,
        ),
        (Method::GET, "/api/v1/identity/consents".to_owned(), None),
        (
            Method::DELETE,
            format!("/api/v1/identity/consents/{none}"),
            None,
        ),
        (Method::GET, "/api/v1/identity/held-grants".to_owned(), None),
        (
            Method::POST,
            "/api/v1/identity/sign-out-everywhere".to_owned(),
            None,
        ),
        (
            Method::GET,
            format!("/api/v1/w/{home}/identity/clients"),
            None,
        ),
        (
            Method::POST,
            format!("/api/v1/w/{home}/identity/clients"),
            Some(r#"{"name":"Sneaky","type":"confidential","scopes":[]}"#),
        ),
        (
            Method::POST,
            format!("/api/v1/w/{home}/identity/held-grants/{none}/adopt"),
            None,
        ),
        (
            Method::GET,
            format!("/api/v1/w/{work}/search?q=invoice"),
            None,
        ),
        (
            Method::GET,
            format!("/api/v1/w/{work}/conversations/{none}/messages/{none}/attachments/{none}"),
            None,
        ),
    ] {
        let mut headers = vec![("authorization", bound.as_str())];
        if body.is_some() {
            headers.push(("content-type", "application/json"));
        }
        let reply = browser
            .raw(method.clone(), &path, &headers, body.map(ToOwned::to_owned))
            .await;
        assert_eq!(
            reply.status,
            StatusCode::NOT_FOUND,
            "{method} {path}: {}",
            reply.body
        );
    }
    let searched = browser
        .raw(
            Method::GET,
            &format!("/api/v1/w/{home}/search?q=invoice"),
            &[("authorization", bound.as_str())],
            None,
        )
        .await;
    assert_eq!(searched.status, StatusCode::OK, "{}", searched.body);
    step(
        "a bound token holding identity scopes: account and app routes 404, Work 404, Home searchable",
    );

    // Even an administrator's token, once bound to a workspace, reaches no
    // admin route; the same token unbound does.
    let account = {
        use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
        let payload = access.split('.').nth(1).unwrap();
        let claims: Value =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload).unwrap()).unwrap();
        claims["sub"].as_str().unwrap().to_owned()
    };
    campbooks_admin::cli::grant(
        &mut db.acquire().await.unwrap(),
        &account,
        &[campbooks_kernel::admin::Permission::Super],
        None,
        chrono::Utc::now(),
    )
    .await
    .unwrap();
    for (workspace, expected) in [
        (Some(home.as_str()), StatusCode::NOT_FOUND),
        (None, StatusCode::OK),
    ] {
        let token = minted_token(&db, &issuer, &access, workspace, "admin:access").await;
        let reply = browser
            .raw(
                Method::GET,
                "/admin/v1/system/overview",
                &[("authorization", &format!("Bearer {token}"))],
                None,
            )
            .await;
        assert_eq!(reply.status, expected, "{workspace:?}: {}", reply.body);
    }
    step("an administrator's token bound to Home: admin API 404; unbound: 200");

    // A workspace's own server-to-server client.
    let register = serde_json::json!({ "name": "Nightly export", "type": "confidential",
        "scopes": ["conversations:read"] });
    let clients = format!("/bff/api/v1/w/{work}/identity/clients");
    let created = browser
        .send_keyed(Method::POST, &clients, Some(register.clone()), "register-1")
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.body);
    let sync = created.json()["client"]["client_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let secret = created.json()["client_secret"].as_str().unwrap().to_owned();
    step(&format!(
        "Work's admin registers {sync}; its secret is shown once"
    ));
    // The idempotency layer kept that the key was used, never the answer
    // (campbooks-cloud#210): a replay answers 409, without the secret.
    let kept: Vec<(String, Vec<u8>, Option<String>)> = sqlx::query_as(
        "SELECT state, response_body, response_content_type FROM kernel.idempotency
          WHERE operation_id = 'identity.register_client'",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(kept, [("withheld".to_owned(), Vec::new(), None)]);
    let bodies: Vec<Option<Vec<u8>>> =
        sqlx::query_scalar("SELECT response_body FROM kernel.idempotency")
            .fetch_all(&db)
            .await
            .unwrap();
    assert!(
        bodies
            .iter()
            .flatten()
            .all(|body| !String::from_utf8_lossy(body).contains(&secret)),
        "no stored key holds the secret"
    );
    let again = browser
        .send_keyed(Method::POST, &clients, Some(register), "register-1")
        .await;
    assert_eq!(again.status, StatusCode::CONFLICT, "{}", again.body);
    assert_eq!(again.json()["code"], "idempotency_secret_shown_once");
    assert_eq!(again.headers["idempotency-replayed"], "true");
    assert!(!again.body.contains(&secret), "{}", again.body);
    step("the same request again (same key): 409, the secret is not shown twice");
    let token = browser
        .form(
            "/oauth2/token",
            &[
                ("grant_type", "client_credentials"),
                ("client_id", &sync),
                ("client_secret", &secret),
            ],
        )
        .await;
    assert_eq!(token.status, StatusCode::OK, "{}", token.body);
    let bearer = format!("Bearer {}", token.json()["access_token"].as_str().unwrap());
    for (workspace, expected) in [(&work, StatusCode::OK), (&home, StatusCode::NOT_FOUND)] {
        let reply = browser
            .raw(
                Method::GET,
                &format!("/api/v1/w/{workspace}/inboxes"),
                &[("authorization", &bearer)],
                None,
            )
            .await;
        assert_eq!(reply.status, expected, "{}", reply.body);
    }
    step("client_credentials -> Work's inboxes 200, Home's 404");
    // 200, not 403: a bare client sees no inbox, so the composed server's
    // directory (identity's, not the stub) resolved it to the admin who
    // registered it (ADR 0017).
    // Even holding `identity:clients` (no client may register for it; this
    // token is minted by hand), a client acting for an admin manages no
    // clients: the handler takes a person's own token only.
    let manager = campbooks_identity::domain::tokens::client_access_token(
        &issuer.url,
        &issuer.keys.signer(&db).await.unwrap(),
        &sync,
        "identity:clients conversations:read",
        work.parse().unwrap(),
        &format!("{ORIGIN}/api"),
        chrono::Utc::now(),
    )
    .unwrap();
    let listed = browser
        .raw(
            Method::GET,
            &format!("/api/v1/w/{work}/identity/clients"),
            &[("authorization", &format!("Bearer {manager}"))],
            None,
        )
        .await;
    assert_eq!(listed.status, StatusCode::NOT_FOUND, "{}", listed.body);
    step("acting for an admin, the client still may not list the workspace's clients: 404");
    let removed = browser
        .send(
            Method::DELETE,
            &format!("/bff/api/v1/w/{work}/identity/clients/{sync}"),
            None,
        )
        .await;
    assert_eq!(removed.status, StatusCode::NO_CONTENT, "{}", removed.body);
    let after = browser
        .raw(
            Method::GET,
            &format!("/api/v1/w/{work}/inboxes"),
            &[("authorization", &bearer)],
            None,
        )
        .await;
    assert_eq!(after.status, StatusCode::UNAUTHORIZED, "{}", after.body);
    step("the admin removes it: its token -> 401");

    let text = logs.text();
    for secret in [secret.as_str(), access.as_str(), code.as_str()] {
        assert!(!text.contains(secret), "a log line holds a secret");
    }
    step("every log line checked: no secret, token or code");
}
