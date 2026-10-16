//! A browser, fake providers and the issuer, wired together for tests.
//!
//! Each fake provider is the kernel's [`TestIdp`]: it approves an
//! authorization request as if the person had signed in, redeems codes with
//! PKCE, and signs ID tokens with keys it publishes. The fake Microsoft
//! issues ID tokens for tenant [`TENANT`] (its `iss` and `tid`); the fake
//! Zoho has two data centres, `.com` and `.eu`. The grant-intake port is a
//! fake that records what it was handed and knows one reconnectable
//! connection, and every user is a member of every workspace.

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
    AppState, Error,
    auth::{Principal, ScopeInfo, TokenVerifier},
    clock::SystemClock,
    config::SignupPolicy,
    connections::{AdoptFuture, GrantIntake, ProviderGrant, ReconnectTarget, TargetFuture},
    crypto::{SecretKey, pkce_challenge, random_token},
    signup::{Invitations, InvitedFuture},
    tenancy::{Access, Membership, MembershipFuture, Role, UserId, WorkspaceId},
    testing::{
        self, TestIdp, TestUser, WEB_CLIENT_ID as FAKE_GOOGLE_CLIENT,
        WEB_CLIENT_SECRET as FAKE_GOOGLE_SECRET,
    },
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
    upstream::{
        Provider,
        client::{
            Endpoints, IssuerRule, MICROSOFT_ISSUER_PREFIX, MICROSOFT_ISSUER_SUFFIX, Server,
            Upstream,
        },
        redirect_uri,
    },
};

/// The API's scope catalogue in these tests.
const CATALOGUE: [ScopeInfo; 6] = [
    ScopeInfo {
        name: "system:read",
        description: "See the instance's health.",
    },
    ScopeInfo {
        name: "identity:read",
        description: "See your account.",
    },
    ScopeInfo {
        name: "identity:write",
        description: "Change your account.",
    },
    ScopeInfo {
        name: "identity:clients",
        description: "Manage a workspace's apps.",
    },
    ScopeInfo {
        name: "connectors:write",
        description: "Connect mailboxes.",
    },
    ScopeInfo {
        name: "conversations:read",
        description: "Read your conversations.",
    },
];

/// The instance under test.
pub const ORIGIN: &str = "http://localhost:8080";
/// The scope the fake intake asks for.
pub const MAIL_SCOPE: &str = "https://mail.example.com/modify";
/// The fake Microsoft's tenant.
pub const TENANT: &str = "2b5a2c1e-6f1d-4c8e-9a3b-7d4e5f6a7b8c";
/// The connection the fake intake lets its grantor reconnect: its grant
/// stopped working, and it is the fake Google's default account's.
pub const RECONNECTABLE: &str = "connection-1";
/// A connection whose grant still works, of the same account.
pub const HEALTHY: &str = "connection-healthy";
/// That connection's address.
pub const RECONNECTABLE_ADDRESS: &str = "test-user@example.com";

/// What the fake intake was handed: workspace, subject, address, refresh
/// token, scopes, the connection extended.
pub type Handed = (
    WorkspaceId,
    String,
    String,
    Option<String>,
    Vec<String>,
    Option<String>,
);

/// Records the grants it is handed.
#[derive(Debug, Default)]
pub struct FakeIntake {
    adopted: Mutex<Vec<Handed>>,
    /// The issuer of each grant handed over, in order.
    issuers: Mutex<Vec<String>>,
    /// Whether the instance syncs Zoho mailboxes too.
    zoho_mail: std::sync::atomic::AtomicBool,
    /// Refuse the next grant (as when another member connected it).
    refuse_next: std::sync::atomic::AtomicBool,
}

/// The Zoho mailbox scopes the fake intake asks for, when it syncs Zoho.
pub const ZOHO_MAIL_SCOPES: [&str; 3] = [
    "ZohoMail.accounts.READ",
    "ZohoMail.folders.READ",
    "ZohoMail.messages.ALL",
];

impl FakeIntake {
    /// The issuer of each grant handed over, in order.
    pub fn issuers(&self) -> Vec<String> {
        self.issuers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Refuse the next grant handed over, as the connectors module does when
    /// another member already connected that account here.
    pub fn refuse_next(&self) {
        self.refuse_next
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// Sync Zoho mailboxes too: a Zoho sign-in then asks for mail.
    pub fn sync_zoho(&self) {
        self.zoho_mail
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// `(workspace, subject, address, refresh token, scopes)` of each grant.
    pub fn adopted(&self) -> Vec<Handed> {
        self.adopted
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl GrantIntake for FakeIntake {
    fn mailbox_scopes(&self, provider: &str) -> Vec<String> {
        match provider {
            "google" => vec![MAIL_SCOPE.to_owned()],
            "zoho" if self.zoho_mail.load(std::sync::atomic::Ordering::SeqCst) => {
                ZOHO_MAIL_SCOPES.map(str::to_owned).to_vec()
            }
            _ => Vec::new(),
        }
    }

    fn adopt<'a>(&'a self, _state: &'a AppState, grant: ProviderGrant) -> AdoptFuture<'a> {
        if self
            .refuse_next
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            return Box::pin(async {
                Err(campbooks_kernel::Error::Conflict(
                    campbooks_kernel::i18n::MESSAGES.reason("mailbox_unsupported"),
                ))
            });
        }
        self.issuers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(grant.issuer.clone());
        let mut adopted = self.adopted.lock().unwrap_or_else(PoisonError::into_inner);
        // A reconnect lands on its connection; each new one gets the next id.
        let id = grant.extending.clone().unwrap_or_else(|| {
            let new = adopted.iter().filter(|handed| handed.5.is_none()).count() + 1;
            format!("connection-{new}")
        });
        adopted.push((
            grant.workspace,
            grant.subject,
            grant.address,
            grant
                .refresh_token
                .as_ref()
                .map(|t| t.expose_secret().to_owned()),
            grant.scopes,
            grant.extending,
        ));
        Box::pin(async move { Ok(id) })
    }

    fn reconnect_target<'a>(
        &'a self,
        _state: &'a AppState,
        _subject: &'a str,
        _workspace: WorkspaceId,
        connection: &'a str,
    ) -> TargetFuture<'a> {
        let needs_reauth = match connection {
            RECONNECTABLE => Some(true),
            HEALTHY => Some(false),
            _ => None,
        };
        Box::pin(async move {
            needs_reauth
                .map(|needs_reauth| ReconnectTarget {
                    provider: "google".to_owned(),
                    address: RECONNECTABLE_ADDRESS.to_owned(),
                    account_subject: TestUser::default().subject,
                    needs_reauth,
                })
                .ok_or(Error::NotFound)
        })
    }
}

/// These addresses have a pending invitation.
#[derive(Debug)]
struct Invited(Vec<String>);

impl Invitations for Invited {
    fn pending_for<'a>(&'a self, _db: &'a PgPool, email: &'a str) -> InvitedFuture<'a> {
        let invited = self.0.iter().any(|e| e == email);
        Box::pin(async move { Ok(invited) })
    }
}

/// Two workspaces every user belongs to as an owner, and lists.
pub const HOME: uuid::Uuid = uuid::Uuid::from_u128(0x0199_0000_0000_7000_8000_0000_0000_00a1);
/// The second.
pub const WORK: uuid::Uuid = uuid::Uuid::from_u128(0x0199_0000_0000_7000_8000_0000_0000_00a2);
/// A workspace every user belongs to as a plain member.
pub const MEMBER_ONLY: uuid::Uuid =
    uuid::Uuid::from_u128(0x0199_0000_0000_7000_8000_0000_0000_00a3);
/// A workspace nobody here belongs to.
pub const FOREIGN: uuid::Uuid = uuid::Uuid::from_u128(0x0199_0000_0000_7000_8000_0000_0000_00f0);
/// The workspaces module's id of every user in [`HOME`].
pub const MEMBER_IN_HOME: uuid::Uuid =
    uuid::Uuid::from_u128(0x0199_0000_0000_7000_8000_0000_0000_0b01);

/// Everyone belongs everywhere (as an owner) except [`FOREIGN`], and is a
/// plain member of [`MEMBER_ONLY`]; client tokens reach their own
/// workspace.
#[derive(Debug)]
struct Everyone;

impl Membership for Everyone {
    fn resolve<'a>(
        &'a self,
        _db: &'a PgPool,
        principal: &'a Principal,
        workspace: WorkspaceId,
    ) -> MembershipFuture<'a> {
        Box::pin(async move {
            if *workspace.as_uuid() == FOREIGN {
                return Ok(None);
            }
            if principal.kind == campbooks_kernel::auth::PrincipalKind::Client {
                return Ok(Some(Access {
                    workspace,
                    user: None,
                    role: None,
                }));
            }
            let user = if *workspace.as_uuid() == HOME {
                UserId::from_uuid(MEMBER_IN_HOME)
            } else {
                UserId::new()
            };
            let role = if *workspace.as_uuid() == MEMBER_ONLY {
                Role::Member
            } else {
                Role::Owner
            };
            Ok(Some(Access {
                workspace,
                user: Some(user),
                role: Some(role),
            }))
        })
    }

    fn workspaces_of<'a>(
        &'a self,
        _db: &'a PgPool,
        _subject: &'a str,
    ) -> campbooks_kernel::tenancy::WorkspacesFuture<'a> {
        Box::pin(async {
            Ok([(HOME, "Home", Role::Owner), (WORK, "Work", Role::Owner)]
                .into_iter()
                .map(
                    |(id, name, role)| campbooks_kernel::tenancy::WorkspaceSummary {
                        id: WorkspaceId::from_uuid(id),
                        name: name.to_owned(),
                        role,
                    },
                )
                .collect())
        })
    }
}

/// A served document and its `max-age`.
type Served = (Vec<u8>, Option<std::time::Duration>);

/// Client metadata documents served at their URLs (no network).
#[derive(Debug, Default)]
pub struct FakeDocuments {
    documents: Mutex<HashMap<String, Served>>,
    fetched: Mutex<Vec<String>>,
}

impl FakeDocuments {
    /// Serve `document` at `url`, cacheable for `max_age`.
    pub fn serve(&self, url: &str, document: &serde_json::Value, max_age: Option<u64>) {
        self.documents
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(
                url.to_owned(),
                (
                    document.to_string().into_bytes(),
                    max_age.map(std::time::Duration::from_secs),
                ),
            );
    }

    /// Every URL fetched, in order.
    pub fn fetched(&self) -> Vec<String> {
        self.fetched
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl crate::metadata::Fetcher for FakeDocuments {
    fn fetch<'a>(&'a self, url: &'a Url) -> crate::metadata::FetchFuture<'a> {
        self.fetched
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(url.to_string());
        let found = self
            .documents
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(url.as_str())
            .cloned();
        Box::pin(async move {
            found
                .map(|(body, max_age)| crate::metadata::Fetched { body, max_age })
                .ok_or(crate::metadata::Refused::Unavailable)
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

/// `callback` with Zoho's `accounts-server` parameter (the fake's issuer).
fn with_accounts_server(callback: &str, server: &str) -> String {
    let mut url = Url::parse(callback).unwrap_or_else(|e| panic!("{callback}: {e}"));
    url.query_pairs_mut()
        .append_pair("location", "test")
        .append_pair("accounts-server", server);
    url.into()
}

/// `callback` without its `iss` parameter.
fn without_iss(callback: &str) -> String {
    let mut url = Url::parse(callback).unwrap_or_else(|e| panic!("{callback}: {e}"));
    let kept: Vec<(String, String)> = url
        .query_pairs()
        .into_owned()
        .filter(|(name, _)| name != "iss")
        .collect();
    url.query_pairs_mut().clear().extend_pairs(kept);
    url.into()
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

/// A fake provider's token endpoint and keys.
fn server_of(idp: &TestIdp) -> Server {
    let base = idp.issuer();
    let url = |path: &str| Url::parse(&format!("{base}{path}")).unwrap_or_else(|e| panic!("{e}"));
    Server {
        issuer: base.to_owned(),
        token: url("oidc/token"),
        jwks: Url::parse(&idp.jwks_url()).unwrap_or_else(|e| panic!("{e}")),
    }
}

/// The fake providers as the issuer's upstreams.
fn fakes(
    google: &TestIdp,
    microsoft: &TestIdp,
    zoho: &TestIdp,
    zoho_eu: &TestIdp,
    redirect_path: Option<&str>,
) -> Vec<Upstream> {
    vec![
        fake(
            Provider::Google,
            google,
            IssuerRule::Fixed(server_of(google)),
            redirect_path,
        ),
        fake(
            Provider::Microsoft,
            microsoft,
            IssuerRule::Tenant {
                server: server_of(microsoft),
                prefix: MICROSOFT_ISSUER_PREFIX.to_owned(),
                suffix: MICROSOFT_ISSUER_SUFFIX.to_owned(),
                only: None,
                except: None,
            },
            None,
        ),
        fake(
            Provider::Zoho,
            zoho,
            IssuerRule::Servers(vec![server_of(zoho), server_of(zoho_eu)]),
            None,
        ),
    ]
}

/// The client of `provider`, talking to the fake `idp`, registered with
/// callback path `redirect_path` (the default when `None`).
fn fake(
    provider: Provider,
    idp: &TestIdp,
    issuers: IssuerRule,
    redirect_path: Option<&str>,
) -> Upstream {
    Upstream::new(
        provider,
        FAKE_GOOGLE_CLIENT.to_owned(),
        ClientSecret::Inline(SecretString::from(FAKE_GOOGLE_SECRET)),
        redirect_uri(provider, ORIGIN, redirect_path).unwrap_or_else(|e| panic!("{e}")),
        Endpoints {
            authorize: Url::parse(&format!("{}oidc/authorize", idp.issuer()))
                .unwrap_or_else(|e| panic!("{e}")),
            issuers,
        },
    )
    .unwrap_or_else(|e| panic!("{}: {e}", provider.as_str()))
}

/// The world.
pub struct Kit {
    /// The issuer's protocol endpoints and the module's API.
    pub app: Router,
    /// The fake Google.
    pub google: TestIdp,
    /// The fake Microsoft.
    pub microsoft: TestIdp,
    /// The fake Zoho's `.com` data centre (where every authorization starts).
    pub zoho: TestIdp,
    /// The fake Zoho's `.eu` data centre.
    pub zoho_eu: TestIdp,
    /// The state (database, ports).
    pub state: AppState,
    /// What the grant-intake port was handed.
    pub intake: Arc<FakeIntake>,
    /// The module's dependencies.
    pub deps: Deps,
    /// The metadata documents on the (fake) internet.
    pub documents: Arc<FakeDocuments>,
    /// The mail the instance sent (nothing leaves the process).
    pub outbox: Arc<campbooks_kernel::mail::Outbox>,
    /// The browser's cookies.
    cookies: Mutex<HashMap<String, String>>,
    /// The address requests come from (none: from inside the process).
    peer: Mutex<Option<std::net::SocketAddr>>,
}

impl Kit {
    /// A fresh world over the `#[sqlx::test]` database `db`, open to new
    /// accounts.
    pub async fn new(db: PgPool) -> Self {
        Self::with_signup(db, SignupPolicy::Open, &[]).await
    }

    /// A fresh world whose instance has signup `policy`, where `invited`
    /// addresses have a pending invitation.
    pub async fn with_signup(db: PgPool, policy: SignupPolicy, invited: &[&str]) -> Self {
        Self::build(db, policy, invited, None, true, None).await
    }

    /// Like [`with_signup`](Self::with_signup), with the allow-list
    /// `CAMPBOOKS_SIGNUP__ALLOWED` set to `allowed`.
    pub async fn with_allowed(
        db: PgPool,
        policy: SignupPolicy,
        invited: &[&str],
        allowed: &str,
    ) -> Self {
        Self::build(db, policy, invited, None, true, Some(allowed)).await
    }

    /// A fresh world whose instance left self-registration off (the
    /// default: no dynamic registration, no metadata documents).
    pub async fn without_self_registration(db: PgPool) -> Self {
        Self::build(db, SignupPolicy::Open, &[], None, false, None).await
    }

    /// A fresh world whose Google client was registered with the callback
    /// path `redirect_path` (`CAMPBOOKS_GOOGLE__REDIRECT_PATH`).
    pub async fn with_google_redirect_path(db: PgPool, redirect_path: &str) -> Self {
        Self::build(db, SignupPolicy::Open, &[], Some(redirect_path), true, None).await
    }

    async fn build(
        db: PgPool,
        policy: SignupPolicy,
        invited: &[&str],
        redirect_path: Option<&str>,
        self_registration: bool,
        allowed: Option<&str>,
    ) -> Self {
        let google = TestIdp::start().await;
        let microsoft = TestIdp::start().await;
        let zoho = TestIdp::start().await;
        let zoho_eu = TestIdp::start().await;
        let pool = testing::app_role_pool(&db).await;
        let mut config = testing::config("postgres://test@localhost/test");
        // Plain-http public URL (the tests' localhost): development only.
        config.env = campbooks_kernel::config::Environment::Development;
        config.signup.policy = policy;
        config.signup.allowed = allowed.map(str::to_owned);
        let mut deps = Deps::from_figment(
            &Figment::from(Serialized::defaults(serde_json::json!({
                "oauth": {
                    "dynamic_registration": self_registration,
                    "client_metadata_documents": self_registration,
                }
            }))),
            &config,
            Some(SecretKey::from_bytes([9; 32])),
            &CATALOGUE,
            Arc::new(SystemClock),
        )
        .unwrap_or_else(|e| panic!("deps: {e}"));
        let issuer = deps
            .issuer
            .as_mut()
            .and_then(Arc::get_mut)
            .unwrap_or_else(|| panic!("issuer"));
        microsoft.set_id_token_claims(serde_json::json!({
            "iss": format!("{MICROSOFT_ISSUER_PREFIX}{TENANT}{MICROSOFT_ISSUER_SUFFIX}"),
            "tid": TENANT,
            // Microsoft sends no `email_verified`.
            "email_verified": null,
        }));
        issuer.upstreams = fakes(&google, &microsoft, &zoho, &zoho_eu, redirect_path);
        let documents = Arc::new(FakeDocuments::default());
        issuer.fetcher = documents.clone();
        let intake = Arc::new(FakeIntake::default());
        let outbox = Arc::new(campbooks_kernel::mail::Outbox::default());
        let issuer = deps.issuer.clone().unwrap_or_else(|| panic!("issuer"));
        let mut auth = config.auth.clone();
        auth.audience.clone_from(&issuer.resource);
        let state = AppState::new(pool.clone(), config.clone())
            .with_registry(
                campbooks_kernel::jobs::Registry::build(&[crate::module(deps.clone())])
                    .unwrap_or_else(|e| panic!("registry: {e}")),
            )
            .with_membership(Arc::new(Everyone))
            .with_grant_intake(intake.clone())
            .with_mailer(outbox.clone())
            .with_invitations(Arc::new(Invited(
                invited.iter().map(|e| (*e).to_owned()).collect(),
            )))
            .with_auth(
                TokenVerifier::in_process(
                    &auth,
                    &issuer.url,
                    Arc::new(LocalKeys {
                        db: pool.clone(),
                        keys: Arc::clone(&issuer.keys),
                    }),
                )
                // As the server wires it: suspension and sign out
                // everywhere stop a person's tokens at once.
                .with_revocation(pool, crate::token_revocation()),
            );
        let api = testing::app(crate::module(deps.clone()), state.clone());
        let app = crate::protocol::router(state.clone(), deps.clone()).merge(api);
        Self {
            app,
            google,
            microsoft,
            zoho,
            zoho_eu,
            state,
            intake,
            deps,
            documents,
            outbox,
            cookies: Mutex::default(),
            peer: Mutex::default(),
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

    /// Use the browser of `other` (its cookies), as when two instances
    /// configured differently serve the same database.
    pub fn share_cookies_with(&self, other: &Self) {
        let theirs = other
            .cookies
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        *self.cookies.lock().unwrap_or_else(PoisonError::into_inner) = theirs;
    }

    /// Send the next requests from `peer` (as the server sees a socket
    /// address), or from inside the process (`None`).
    pub fn send_from(&self, peer: Option<&str>) {
        *self.peer.lock().unwrap_or_else(PoisonError::into_inner) =
            peer.map(|p| p.parse().unwrap_or_else(|e| panic!("{p}: {e}")));
    }

    /// An access token like `access` (its account, client, session and
    /// scopes) but bound to `workspace`, as an app allowed there holds one.
    /// Minted directly: no app may be granted `identity:*`, so this is the
    /// only way to see a bound token that has the scope refused for being
    /// bound.
    pub async fn bound(&self, access: &str, workspace: uuid::Uuid) -> String {
        let (_, claims) = Self::decode(access);
        let text = |name: &str| claims[name].as_str().unwrap_or_default().to_owned();
        let account: uuid::Uuid = text("sub").parse().unwrap_or_else(|e| panic!("sub: {e}"));
        let grant = crate::domain::tokens::Grant {
            client_id: text("client_id"),
            account_id: account,
            session_id: text("sid").parse().ok(),
            scope: text("scope"),
            auth_time: chrono::DateTime::from_timestamp(
                claims["auth_time"].as_i64().unwrap_or_default(),
                0,
            )
            .unwrap_or_default(),
            resource: text("aud"),
            workspace: Some(workspace),
        };
        let issuer = self.deps.issuer.clone().unwrap_or_else(|| panic!("issuer"));
        let profile = crate::domain::accounts::profile(&self.state.db, account)
            .await
            .unwrap_or_else(|e| panic!("profile: {e}"))
            .unwrap_or_else(|| panic!("no account"));
        let signer = issuer
            .keys
            .signer(&self.state.db)
            .await
            .unwrap_or_else(|e| panic!("signer: {e}"));
        crate::domain::tokens::Mint {
            issuer: &issuer.url,
            signer: &signer,
            grant: &grant,
            profile: &profile,
            amr: &[],
            now: chrono::Utc::now(),
        }
        .access_token()
        .unwrap_or_else(|e| panic!("minting: {e}"))
    }

    /// Run every job that is ready (event deliveries included).
    pub async fn run_jobs(&self) {
        campbooks_kernel::jobs::Worker::new(self.state.clone())
            .run_until_idle()
            .await;
    }

    /// Forget every cookie (another browser).
    pub fn new_browser(&self) {
        self.cookies
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
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
        let mut request = request
            .body(body.map_or_else(Body::empty, Body::from))
            .unwrap_or_else(|e| panic!("request: {e}"));
        if let Some(peer) = *self.peer.lock().unwrap_or_else(PoisonError::into_inner) {
            request
                .extensions_mut()
                .insert(axum::extract::ConnectInfo(peer));
        }
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
        let mut url =
            Url::parse(&format!("{ORIGIN}/oauth2/authorize")).unwrap_or_else(|e| panic!("{e}"));
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
            "openid profile email offline_access identity:read identity:write identity:clients \
             connectors:write",
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

    /// The fake of `provider`.
    pub fn idp(&self, provider: Provider) -> &TestIdp {
        match provider {
            Provider::Google => &self.google,
            Provider::Microsoft => &self.microsoft,
            Provider::Zoho => &self.zoho,
        }
    }

    /// [`round_trip`](Self::round_trip) with `provider`.
    pub async fn round_trip_with(&self, provider: Provider, request: Option<&str>) -> Response {
        let start = match request {
            Some(request) => format!("/auth/start/{}?request={request}", provider.as_str()),
            None => format!("/auth/start/{}", provider.as_str()),
        };
        let to_provider = self.get(&start).await;
        assert_eq!(to_provider.status, StatusCode::SEE_OTHER, "{to_provider:?}");
        self.via(provider, &to_provider.location()).await
    }

    /// Approve `url` at the fake `provider` and follow its redirect back.
    /// Zoho's callback names its `.com` accounts server.
    pub async fn via(&self, provider: Provider, url: &str) -> Response {
        let callback = self.idp(provider).authorize(url);
        let callback = match provider {
            Provider::Zoho => with_accounts_server(&callback, self.zoho.issuer()),
            // Microsoft sends no RFC 9207 `iss`.
            Provider::Microsoft => without_iss(&callback),
            Provider::Google => callback,
        };
        self.get(&callback).await
    }

    /// Approve `url` (Zoho's global authorization endpoint) as a person of
    /// the `.eu` data centre, and follow the redirect back.
    pub async fn via_zoho_eu(&self, url: &str) -> Response {
        let at_eu = url.replacen(self.zoho.issuer(), self.zoho_eu.issuer(), 1);
        let callback = with_accounts_server(&self.zoho_eu.authorize(&at_eu), self.zoho_eu.issuer());
        self.get(&callback).await
    }

    /// Sign up as a new person through the web client and redeem the code:
    /// the access token (this browser keeps the issuer session).
    pub async fn signed_up(&self) -> String {
        let (code, verifier) = self.sign_up().await;
        let tokens = self.redeem(&code, &verifier).await;
        assert_eq!(tokens.status, StatusCode::OK, "{tokens:?}");
        tokens.json["access_token"]
            .as_str()
            .unwrap_or_default()
            .to_owned()
    }

    /// An access token for the account `access` names whose sign-in is an
    /// hour old (as if the person had signed in long ago).
    pub async fn stale(&self, access: &str) -> String {
        self.signed_in_ago(access, chrono::TimeDelta::hours(1))
            .await
    }

    /// An access token for the account `access` names whose sign-in is
    /// `ago` old.
    pub async fn signed_in_ago(&self, access: &str, ago: chrono::TimeDelta) -> String {
        let (_, claims) = Self::decode(access);
        let account: uuid::Uuid = claims["sub"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| panic!("sub"));
        let stale = chrono::Utc::now() - ago;
        for sql in [
            "UPDATE identity.sessions SET auth_time = $2 WHERE account_id = $1",
            "UPDATE identity.refresh_families SET auth_time = $2 WHERE account_id = $1",
        ] {
            sqlx::query(sql)
                .bind(account)
                .bind(stale)
                .execute(&self.state.db)
                .await
                .unwrap_or_else(|e| panic!("{sql}: {e}"));
        }
        let (url, verifier) = Self::web_authorize();
        let back = self.get(&url).await;
        let tokens = self.redeem(&back.location_query()["code"], &verifier).await;
        tokens.json["access_token"]
            .as_str()
            .unwrap_or_else(|| panic!("no token: {tokens:?}"))
            .to_owned()
    }

    /// Call the API with `access_token` and a JSON `body`.
    pub async fn api_json(
        &self,
        method: Method,
        path: &str,
        access_token: &str,
        body: &serde_json::Value,
    ) -> Response {
        self.request(
            method,
            path,
            &[
                ("authorization", &format!("Bearer {access_token}")),
                ("content-type", "application/json"),
            ],
            Some(body.to_string()),
        )
        .await
    }

    /// `identity.start_link` with `body`.
    pub async fn start_link(&self, access_token: &str, body: &serde_json::Value) -> Response {
        self.api_json(Method::POST, "/api/v1/identity/links", access_token, body)
            .await
    }

    /// The caller's ways in.
    pub async fn ways_in(&self, access_token: &str) -> serde_json::Value {
        let listed = self
            .api(Method::GET, "/api/v1/identity/ways-in", access_token)
            .await;
        assert_eq!(listed.status, StatusCode::OK, "{listed:?}");
        listed.json
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
        assert_eq!(created.status, StatusCode::CREATED, "creating the account");
        let redirect = created.json["redirect"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
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
