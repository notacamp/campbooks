//! A browser, a fake Google and the issuer, wired together for tests.
//!
//! The fake Google is the kernel's [`TestIdp`]: it approves an authorization
//! request as if the person had signed in, redeems codes with PKCE, and signs
//! ID tokens with keys it publishes. The grant-intake port is a fake that
//! records what it was handed, and every user is a member of every
//! workspace.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, PoisonError},
};

use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Method, Request, StatusCode, header},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use campbooks_kernel::{
    AppState,
    auth::{Principal, TokenVerifier},
    clock::SystemClock,
    connections::{AdoptFuture, GrantIntake, ProviderGrant},
    crypto::{SecretKey, pkce_challenge, random_token},
    tenancy::{Access, Membership, MembershipFuture, Role, UserId, WorkspaceId},
    testing::{self, TestIdp, WEB_CLIENT_ID as FAKE_GOOGLE_CLIENT, WEB_CLIENT_SECRET as FAKE_GOOGLE_SECRET},
};
use figment::{Figment, providers::Serialized};
use http_body_util::BodyExt as _;
use secrecy::{ExposeSecret as _, SecretString};
use sqlx::PgPool;
use tower::ServiceExt as _;
use url::Url;

use crate::{
    deps::{ClientSecret, Deps, WEB_CLIENT_SECRET_PURPOSE},
    domain::{clients::WEB_CLIENT_ID, keys::LocalKeys},
    upstream::google::{Google, GoogleEndpoints},
};

/// The instance under test.
pub const ORIGIN: &str = "http://localhost:8080";
/// The scope the fake intake asks for.
pub const MAIL_SCOPE: &str = "https://mail.example.com/modify";

/// Records the grants it is handed.
#[derive(Debug, Default)]
pub struct FakeIntake {
    adopted: Mutex<Vec<(WorkspaceId, String, String, Option<String>, Vec<String>)>>,
}

impl FakeIntake {
    /// `(workspace, subject, address, refresh token, scopes)` of each grant.
    pub fn adopted(&self) -> Vec<(WorkspaceId, String, String, Option<String>, Vec<String>)> {
        self.adopted
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl GrantIntake for FakeIntake {
    fn mailbox_scopes(&self, provider: &str) -> Vec<String> {
        if provider == "google" {
            vec![MAIL_SCOPE.to_owned()]
        } else {
            Vec::new()
        }
    }

    fn adopt<'a>(&'a self, _state: &'a AppState, grant: ProviderGrant) -> AdoptFuture<'a> {
        self.adopted
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((
                grant.workspace,
                grant.subject,
                grant.address,
                grant
                    .refresh_token
                    .as_ref()
                    .map(|t| t.expose_secret().to_owned()),
                grant.scopes,
            ));
        Box::pin(async { Ok("connection-1".to_owned()) })
    }
}

/// Everyone belongs everywhere, as an owner.
#[derive(Debug)]
struct Everyone;

impl Membership for Everyone {
    fn resolve<'a>(
        &'a self,
        _db: &'a PgPool,
        _principal: &'a Principal,
        workspace: WorkspaceId,
    ) -> MembershipFuture<'a> {
        Box::pin(async move {
            Ok(Some(Access {
                workspace,
                user: Some(UserId::new()),
                role: Some(Role::Owner),
            }))
        })
    }
}

/// A response, read to the end.
#[derive(Debug)]
pub struct Response {
    /// Status.
    pub status: StatusCode,
    /// Headers.
    pub headers: HeaderMap,
    /// Body as JSON (`Null` when not JSON).
    pub json: serde_json::Value,
}

impl Response {
    /// The `Location` header.
    pub fn location(&self) -> String {
        self.headers
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_else(|| panic!("no Location: {self:?}"))
            .to_owned()
    }

    /// The query parameters of the `Location` header.
    pub fn location_query(&self) -> HashMap<String, String> {
        query(&self.location())
    }
}

/// The query parameters of `url` (absolute or a path).
pub fn query(url: &str) -> HashMap<String, String> {
    Url::parse(url)
        .or_else(|_| Url::parse(&format!("{ORIGIN}{url}")))
        .unwrap_or_else(|e| panic!("{url}: {e}"))
        .query_pairs()
        .into_owned()
        .collect()
}

/// The world.
pub struct Kit {
    /// The issuer's protocol endpoints and the module's API.
    pub app: Router,
    /// The fake Google.
    pub google: TestIdp,
    /// The state (database, ports).
    pub state: AppState,
    /// What the grant-intake port was handed.
    pub intake: Arc<FakeIntake>,
    /// The browser's cookies.
    cookies: Mutex<HashMap<String, String>>,
}

impl Kit {
    /// A fresh world over the `#[sqlx::test]` database `db`.
    pub async fn new(db: PgPool) -> Self {
        let google = TestIdp::start().await;
        let pool = testing::app_role_pool(&db).await;
        let config = testing::config("postgres://test@localhost/test");
        let mut deps = Deps::from_figment(
            &Figment::from(Serialized::defaults(serde_json::json!({}))),
            &config,
            Some(SecretKey::from_bytes([9; 32])),
            &["system:read", "identity:read", "identity:write"],
            Arc::new(SystemClock),
        )
        .unwrap_or_else(|e| panic!("deps: {e}"));
        let issuer = deps.issuer.as_mut().and_then(Arc::get_mut).unwrap_or_else(|| panic!("issuer"));
        let base = google.issuer();
        issuer.google = Some(
            Google::new(
                FAKE_GOOGLE_CLIENT.to_owned(),
                ClientSecret::Inline(SecretString::from(FAKE_GOOGLE_SECRET)),
                ORIGIN,
                GoogleEndpoints {
                    issuer: base.to_owned(),
                    authorize: Url::parse(&format!("{base}oidc/authorize")).unwrap_or_else(|e| panic!("{e}")),
                    token: Url::parse(&format!("{base}oidc/token")).unwrap_or_else(|e| panic!("{e}")),
                    revoke: Url::parse(&format!("{base}oidc/token/revoke")).unwrap_or_else(|e| panic!("{e}")),
                    jwks: Url::parse(&google.jwks_url()).unwrap_or_else(|e| panic!("{e}")),
                },
            )
            .unwrap_or_else(|e| panic!("google: {e}")),
        );
        let intake = Arc::new(FakeIntake::default());
        let issuer = deps.issuer.clone().unwrap_or_else(|| panic!("issuer"));
        let state = AppState::new(pool.clone(), config.clone())
            .with_membership(Arc::new(Everyone))
            .with_grant_intake(intake.clone())
            .with_auth(TokenVerifier::in_process(
                &config.auth,
                &issuer.url,
                Arc::new(LocalKeys {
                    db: pool,
                    keys: Arc::clone(&issuer.keys),
                }),
            ));
        let api = testing::app(crate::module(deps.clone()), state.clone());
        let app = crate::protocol::router(state.clone(), deps.clone()).merge(api);
        Self {
            app,
            google,
            state,
            intake,
            cookies: Mutex::default(),
        }
    }

    /// The web client's secret, as the BFF derives it.
    pub fn web_secret() -> String {
        campbooks_kernel::crypto::derive_secret(
            &SecretKey::from_bytes([9; 32]),
            WEB_CLIENT_SECRET_PURPOSE,
        )
        .expose_secret()
        .to_owned()
    }

    /// Forget every cookie (another browser).
    pub fn new_browser(&self) {
        self.cookies.lock().unwrap_or_else(PoisonError::into_inner).clear();
    }

    /// Send `method uri` from the browser, with `headers` and a form or JSON
    /// `body`, keeping the cookies it sets.
    pub async fn request(
        &self,
        method: Method,
        uri: &str,
        headers: &[(&str, &str)],
        body: Option<String>,
    ) -> Response {
        let path = uri.strip_prefix(ORIGIN).unwrap_or(uri);
        let mut request = Request::builder().method(method).uri(path);
        let cookie = self
            .cookies
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("; ");
        if !cookie.is_empty() {
            request = request.header(header::COOKIE, cookie);
        }
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let request = request
            .body(body.map_or_else(Body::empty, Body::from))
            .unwrap_or_else(|e| panic!("request: {e}"));
        let response = match self.app.clone().oneshot(request).await {
            Ok(response) => response,
            Err(never) => match never {},
        };
        let status = response.status();
        let headers = response.headers().clone();
        for set in headers.get_all(header::SET_COOKIE) {
            let set = set.to_str().unwrap_or_default();
            let pair = set.split(';').next().unwrap_or_default();
            if let Some((name, value)) = pair.split_once('=') {
                let mut jar = self.cookies.lock().unwrap_or_else(PoisonError::into_inner);
                if value.is_empty() || set.contains("Max-Age=0") {
                    jar.remove(name);
                } else {
                    jar.insert(name.to_owned(), value.to_owned());
                }
            }
        }
        let bytes = response
            .into_body()
            .collect()
            .await
            .map(http_body_util::Collected::to_bytes)
            .unwrap_or_default();
        Response {
            status,
            headers,
            json: serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
        }
    }

    /// `GET uri`.
    pub async fn get(&self, uri: &str) -> Response {
        self.request(Method::GET, uri, &[], None).await
    }

    /// A POST of `form` to the token or revocation endpoint.
    pub async fn post_form(&self, path: &str, form: &[(&str, &str)]) -> Response {
        let body = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(form.iter().copied())
            .finish();
        self.request(
            Method::POST,
            path,
            &[("content-type", "application/x-www-form-urlencoded")],
            Some(body),
        )
        .await
    }

    /// The URL a client sends the browser to, with a fresh PKCE pair: the
    /// URL and the verifier.
    pub fn authorize_url(client_id: &str, redirect_uri: &str, scope: &str) -> (String, String) {
        let verifier = random_token();
        let mut url = Url::parse(&format!("{ORIGIN}/oauth2/authorize")).unwrap_or_else(|e| panic!("{e}"));
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", client_id)
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("scope", scope)
            .append_pair("state", "client-state")
            .append_pair("nonce", "client-nonce")
            .append_pair("code_challenge", &pkce_challenge(&verifier))
            .append_pair("code_challenge_method", "S256");
        (url.into(), verifier)
    }

    /// The web client's authorization request.
    pub fn web_authorize() -> (String, String) {
        Self::authorize_url(
            WEB_CLIENT_ID,
            &format!("{ORIGIN}/bff/callback"),
            "openid profile email offline_access identity:read identity:write",
        )
    }

    /// Run the provider round trip for the sign-in screen's `request` (or
    /// none) as the fake Google's current user: start, approve, callback.
    /// The callback's response.
    pub async fn round_trip(&self, request: Option<&str>) -> Response {
        let start = match request {
            Some(request) => format!("/auth/start/google?request={request}"),
            None => "/auth/start/google".to_owned(),
        };
        let to_google = self.get(&start).await;
        assert_eq!(to_google.status, StatusCode::SEE_OTHER, "{to_google:?}");
        self.via_google(&to_google.location()).await
    }

    /// Approve the authorization request `url` at the fake Google and follow
    /// its redirect back to the callback.
    pub async fn via_google(&self, url: &str) -> Response {
        let callback = self.google.authorize(url);
        self.get(&callback).await
    }

    /// The full web sign-up as a new person: authorize, round trip, create.
    /// The code the web client gets and the PKCE verifier.
    pub async fn sign_up(&self) -> (String, String) {
        let (url, verifier) = Self::web_authorize();
        let to_sign_in = self.get(&url).await;
        let request = to_sign_in.location_query()["request"].clone();
        let to_new = self.round_trip(Some(&request)).await;
        assert!(to_new.location().starts_with("/sign-in/new"), "{to_new:?}");
        let created = self.create_account().await;
        assert_eq!(created.status, StatusCode::CREATED, "{created:?}");
        let redirect = created.json["redirect"].as_str().unwrap_or_default().to_owned();
        (query(&redirect)["code"].clone(), verifier)
    }

    /// `POST /auth/new-account` as the sign-in screen sends it.
    pub async fn create_account(&self) -> Response {
        self.request(
            Method::POST,
            "/auth/new-account",
            &[("x-campbooks-csrf", "1"), ("sec-fetch-site", "same-origin")],
            None,
        )
        .await
    }

    /// Redeem `code` as the web client.
    pub async fn redeem(&self, code: &str, verifier: &str) -> Response {
        let secret = Self::web_secret();
        self.post_form(
            "/oauth2/token",
            &[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", &format!("{ORIGIN}/bff/callback")),
                ("code_verifier", verifier),
                ("client_id", WEB_CLIENT_ID),
                ("client_secret", &secret),
            ],
        )
        .await
    }

    /// Refresh as the web client.
    pub async fn refresh(&self, refresh_token: &str) -> Response {
        let secret = Self::web_secret();
        self.post_form(
            "/oauth2/token",
            &[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
                ("client_id", WEB_CLIENT_ID),
                ("client_secret", &secret),
            ],
        )
        .await
    }

    /// Call the API with `access_token`.
    pub async fn api(&self, method: Method, path: &str, access_token: &str) -> Response {
        self.request(
            method,
            path,
            &[("authorization", &format!("Bearer {access_token}"))],
            None,
        )
        .await
    }

    /// The decoded header and claims of a JWT (no verification).
    pub fn decode(token: &str) -> (serde_json::Value, serde_json::Value) {
        let mut parts = token.split('.');
        let mut part = || {
            let bytes = URL_SAFE_NO_PAD
                .decode(parts.next().unwrap_or_default())
                .unwrap_or_else(|e| panic!("JWT part: {e}"));
            serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("JWT JSON: {e}"))
        };
        (part(), part())
    }
}
