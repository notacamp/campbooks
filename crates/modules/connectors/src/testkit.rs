//! Test fixtures: the module over a database whose connections run as the
//! runtime role (row-level security applies, as in production), a test
//! identity provider, a fake Google (tokens + Gmail, on wiremock) serving
//! recorded fictional mail, a fake consent door (the identity module's front
//! door), a fixed clock, and the contract schemas.
//!
//! No test talks to Google.

use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex, PoisonError},
};

use axum::{
    Router,
    body::Body,
    http::{Method, Request, header},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use campbooks_kernel::clock::Clock as _;
use campbooks_kernel::{
    AppState, Error,
    auth::{Principal, PrincipalKind},
    blobs::{BlobRef, BlobStore},
    clock::FixedClock,
    config::BlobsConfig,
    connections::{ConsentDoor, ConsentFuture, ConsentRequest, ProviderGrant, StartedConsent},
    crypto::{Cipher, SecretKey},
    jobs::{Registry, Worker},
    tenancy::{Access, Membership, MembershipFuture, Role, UserId, WorkspaceId},
    testing::{self, TestIdp, TestResponse},
};
use chrono::{DateTime, TimeDelta, Utc};
use futures_util::StreamExt as _;
use secrecy::SecretString;
use sqlx::PgPool;
use url::Url;
use wiremock::{Mock, MockServer, Respond, ResponseTemplate, matchers::any};

use crate::{
    Deps,
    config::ConnectorsConfig,
    deps::CREDENTIALS_PURPOSE,
    domain::intake::grant_intake,
    google::{Endpoints, GMAIL_SCOPE, Google},
    provider::ClientSecret,
    zoho::{Centres, MAIL_SCOPES as ZOHO_SCOPES, Zoho},
};

pub use crate::testkit_zoho::FakeZoho;

/// Every scope of this module.
const ALL_SCOPES: &str = "connectors:read connectors:write";

/// The fictional Gmail account behind the fake.
pub const ACCOUNT_EMAIL: &str = "sam@example.com";
/// Its Google account id.
pub const ACCOUNT_SUB: &str = "google-sub-1001";

/// The recorded fixtures: fictional mail in en, pt-PT, es-ES and fr.
pub const FIXTURES: [(&str, &[u8]); 4] = [
    (
        "en-invoice",
        include_bytes!("../fixtures/gmail/en-invoice.eml"),
    ),
    (
        "pt-PT-order",
        include_bytes!("../fixtures/gmail/pt-PT-order.eml"),
    ),
    (
        "es-ES-newsletter",
        include_bytes!("../fixtures/gmail/es-ES-newsletter.eml"),
    ),
    (
        "fr-statement",
        include_bytes!("../fixtures/gmail/fr-statement.eml"),
    ),
];

/// A caller: their bearer token.
#[derive(Debug, Clone)]
pub struct Caller {
    pub token: String,
    pub subject: String,
}

impl std::ops::Deref for Caller {
    type Target = str;

    fn deref(&self) -> &str {
        &self.token
    }
}

/// A user's id and memberships.
type Member = (UserId, Vec<(WorkspaceId, Role)>);

/// Who belongs where: the `workspaces` module's job, faked.
#[derive(Debug, Default)]
struct FakeMembership {
    members: Mutex<HashMap<String, Member>>,
    /// Workspaces deleted ([`Kit::delete_workspace`]): no one belongs there.
    deleted: Mutex<Vec<WorkspaceId>>,
}

impl Membership for FakeMembership {
    fn resolve<'a>(
        &'a self,
        _db: &'a PgPool,
        principal: &'a Principal,
        workspace: WorkspaceId,
    ) -> MembershipFuture<'a> {
        let deleted = self
            .deleted
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(&workspace);
        Box::pin(async move {
            if deleted {
                return Ok(None);
            }
            if principal.kind == PrincipalKind::Client {
                let bound = principal.workspace.as_deref() == Some(&workspace.to_string());
                return Ok(bound.then_some(Access {
                    workspace,
                    user: None,
                    role: None,
                }));
            }
            let members = self.members.lock().unwrap_or_else(PoisonError::into_inner);
            Ok(members
                .get(&principal.subject)
                .and_then(|(user, workspaces)| {
                    workspaces
                        .iter()
                        .find(|(w, _)| *w == workspace)
                        .map(|&(_, role)| Access {
                            workspace,
                            user: Some(*user),
                            role: Some(role),
                        })
                }))
        })
    }

    fn role_held<'a>(
        &'a self,
        _conn: &'a mut sqlx::PgConnection,
        workspace: WorkspaceId,
        user: UserId,
    ) -> campbooks_kernel::tenancy::RoleFuture<'a> {
        let deleted = self
            .deleted
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(&workspace);
        let role = self
            .members
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .values()
            .find(|(id, _)| *id == user)
            .and_then(|(_, workspaces)| workspaces.iter().find(|(w, _)| *w == workspace))
            .map(|&(_, role)| role)
            .filter(|_| !deleted);
        Box::pin(async move { Ok(role) })
    }
}

/// The module over a fresh database, with a fake Google.
pub struct Kit {
    /// The test database as its owner (no row-level security), for
    /// inspecting rows.
    pub db: PgPool,
    /// The state handlers and jobs run with.
    pub state: AppState,
    /// The clock the module reads.
    pub clock: Arc<FixedClock>,
    /// The fake Google.
    pub google: FakeGoogle,
    /// The fake Zoho (one data centre).
    pub zoho: FakeZoho,
    /// What the module was composed with.
    pub deps: Deps,
    /// The identity module's front door, faked.
    pub door: Arc<FakeDoor>,
    app: Router,
    idp: TestIdp,
    membership: Arc<FakeMembership>,
}

impl Kit {
    /// The module with Google and Zoho configured against the fakes. The
    /// database's clock is real while the module's is fixed, so Zoho's
    /// per-login budget is set high enough never to wait in a test.
    pub async fn new(db: PgPool) -> Self {
        let mut config = ConnectorsConfig::default();
        config.zoho.requests_per_minute = 1_000_000;
        Self::with_config(db, config).await
    }

    /// The module with `config`.
    pub async fn with_config(db: PgPool, config: ConnectorsConfig) -> Self {
        let google = FakeGoogle::start().await;
        let clock = Arc::new(FixedClock::new(
            DateTime::from_timestamp(Utc::now().timestamp(), 0).unwrap(),
        ));
        let zoho = FakeZoho::start(clock.clone()).await;
        let deps = Deps {
            config: Arc::new(config),
            google: Some(Arc::new(
                Google::new(
                    "client-1.apps.example.com".into(),
                    ClientSecret::Inline("test-client-secret".into()),
                    Endpoints::under(&google.base()),
                )
                .unwrap(),
            )),
            zoho: Some(Arc::new(
                Zoho::new(
                    "zoho-client-1".into(),
                    ClientSecret::Inline("test-zoho-secret".into()),
                    Centres::Fake(zoho.base()),
                )
                .unwrap(),
            )),
            cipher: Some(Arc::new(Cipher::new(
                &SecretKey::from_bytes([3; 32]),
                CREDENTIALS_PURPOSE,
            ))),
            clock: clock.clone(),
        };
        Self::assemble(db, deps, clock, google, zoho).await
    }

    /// The module with no provider configured.
    pub async fn inert(db: PgPool) -> Self {
        let google = FakeGoogle::start().await;
        let clock = Arc::new(FixedClock::new(Utc::now()));
        let zoho = FakeZoho::start(clock.clone()).await;
        let deps = Deps {
            clock: clock.clone(),
            ..Deps::default()
        };
        Self::assemble(db, deps, clock, google, zoho).await
    }

    async fn assemble(
        db: PgPool,
        deps: Deps,
        clock: Arc<FixedClock>,
        google: FakeGoogle,
        zoho: FakeZoho,
    ) -> Self {
        let idp = TestIdp::start().await;
        let pool = testing::app_role_pool(&db).await;
        let blobs = BlobStore::memory(&BlobsConfig::default(), pool.clone());
        let membership = Arc::new(FakeMembership::default());
        let door = Arc::new(FakeDoor::default());
        let state = idp
            .state(pool)
            .with_blobs(blobs)
            .with_membership(membership.clone())
            .with_consent_door(door.clone())
            .with_registry(Registry::build(&[crate::module(deps.clone())]).unwrap());
        let app = testing::app(crate::module(deps.clone()), state.clone());
        Self {
            db,
            state,
            clock,
            google,
            zoho,
            deps,
            door,
            app,
            idp,
            membership,
        }
    }

    /// A user `sub` with every scope.
    pub fn user(&self, sub: &str) -> Caller {
        let mut claims = self.idp.claims(ALL_SCOPES);
        claims["sub"] = sub.into();
        claims["email"] = format!("{sub}@example.com").into();
        claims["email_verified"] = true.into();
        Caller {
            token: self.idp.token(&claims),
            subject: sub.to_owned(),
        }
    }

    /// A user `sub` granted only `scopes`.
    pub fn user_scopes(&self, sub: &str, scopes: &str) -> Caller {
        let mut claims = self.idp.claims(scopes);
        claims["sub"] = sub.into();
        Caller {
            token: self.idp.token(&claims),
            subject: sub.to_owned(),
        }
    }

    /// A client token bound to `workspace`.
    pub fn client(&self, workspace: WorkspaceId) -> Caller {
        let mut claims = self.idp.claims(ALL_SCOPES);
        let object = claims.as_object_mut().unwrap();
        object.remove("sub");
        object.insert("azp".into(), "acme-sync".into());
        object.insert("workspace_id".into(), workspace.to_string().into());
        Caller {
            token: self.idp.token(&claims),
            subject: "acme-sync".to_owned(),
        }
    }

    /// Make `caller` a member of `workspace` with `role`; returns their id.
    pub fn join(&self, caller: &Caller, workspace: WorkspaceId, role: Role) -> UserId {
        let mut members = self
            .membership
            .members
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let entry = members
            .entry(caller.subject.clone())
            .or_insert_with(|| (UserId::new(), Vec::new()));
        entry.1.push((workspace, role));
        entry.0
    }

    /// `caller` is no longer a member of `workspace`.
    pub fn leave(&self, caller: &Caller, workspace: WorkspaceId) {
        if let Some((_, workspaces)) = self
            .membership
            .members
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get_mut(&caller.subject)
        {
            workspaces.retain(|(w, _)| *w != workspace);
        }
    }

    /// `workspace` is deleted: no one belongs there any more.
    pub fn delete_workspace(&self, workspace: WorkspaceId) {
        self.membership
            .deleted
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(workspace);
    }

    /// `GET uri`.
    pub async fn get(&self, token: &str, uri: &str) -> TestResponse {
        self.call(Method::GET, token, uri, None).await
    }

    /// `POST uri` with a JSON body.
    pub async fn post(&self, token: &str, uri: &str, body: serde_json::Value) -> TestResponse {
        self.call(Method::POST, token, uri, Some(body)).await
    }

    /// `DELETE uri`.
    pub async fn delete(&self, token: &str, uri: &str) -> TestResponse {
        self.call(Method::DELETE, token, uri, None).await
    }

    async fn call(
        &self,
        method: Method,
        token: &str,
        uri: &str,
        body: Option<serde_json::Value>,
    ) -> TestResponse {
        let mut request = Request::builder().method(method).uri(uri);
        if !token.is_empty() {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        let request = match body {
            Some(body) => request
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string())),
            None => request.body(Body::empty()),
        };
        testing::send(&self.app, request.unwrap()).await
    }

    /// Start a Google `messages` connection as `caller` in `workspace`:
    /// the response JSON.
    pub async fn start(&self, caller: &Caller, workspace: WorkspaceId) -> TestResponse {
        self.post(
            caller,
            &format!("/api/v1/w/{workspace}/connections/authorizations"),
            serde_json::json!({"provider": "google", "capabilities": ["messages"]}),
        )
        .await
    }

    /// A consent for `account_subject` coming back through the identity
    /// module's front door, handed to the grant-intake port for `caller`
    /// in `workspace` (extending `extending`): the connection id.
    pub async fn adopt(
        &self,
        caller: &Caller,
        workspace: WorkspaceId,
        account_subject: &str,
        extending: Option<String>,
    ) -> Result<String, Error> {
        let (access, refresh) = self.google.grant();
        grant_intake(self.deps.clone())
            .adopt(
                &self.state,
                ProviderGrant {
                    workspace,
                    subject: caller.subject.clone(),
                    provider: "google".to_owned(),
                    account_subject: account_subject.to_owned(),
                    issuer: "https://accounts.google.com".to_owned(),
                    address: "Sam@Example.com".to_owned(),
                    address_verified: self.google.email_verified(),
                    display_name: Some("Sam Rivera".to_owned()),
                    scopes: vec![
                        "openid".to_owned(),
                        "email".to_owned(),
                        GMAIL_SCOPE.to_owned(),
                    ],
                    access_token: SecretString::from(access),
                    refresh_token: Some(SecretString::from(refresh)),
                    expires_in: Some(3599),
                    extending,
                },
            )
            .await
    }

    /// A Zoho consent for login `account_subject` (whose mail accounts the
    /// fake lists), from the data centre `issuer`, handed to the
    /// grant-intake port for `caller` in `workspace`: the connection id.
    pub async fn adopt_zoho(
        &self,
        caller: &Caller,
        workspace: WorkspaceId,
        account_subject: &str,
        issuer: &str,
    ) -> Result<String, Error> {
        let (access, refresh) = self.zoho.grant();
        let mut scopes = vec!["openid".to_owned(), "email".to_owned()];
        scopes.extend(ZOHO_SCOPES.map(str::to_owned));
        grant_intake(self.deps.clone())
            .adopt(
                &self.state,
                ProviderGrant {
                    workspace,
                    subject: caller.subject.clone(),
                    provider: "zoho".to_owned(),
                    account_subject: account_subject.to_owned(),
                    issuer: issuer.to_owned(),
                    address: crate::testkit_zoho::LOGIN.to_owned(),
                    address_verified: true,
                    display_name: Some("Sam Rivera".to_owned()),
                    scopes,
                    access_token: SecretString::from(access),
                    refresh_token: Some(SecretString::from(refresh)),
                    expires_in: Some(3600),
                    extending: None,
                },
            )
            .await
    }

    /// Connect the fake Zoho login as `caller` in `workspace`; the
    /// connection JSON.
    pub async fn connect_zoho(&self, caller: &Caller, workspace: WorkspaceId) -> serde_json::Value {
        let id = self
            .adopt_zoho(caller, workspace, "zoho-zuid-7001", &self.zoho.issuer())
            .await
            .unwrap_or_else(|e| panic!("adopting: {e:?}"));
        let read = self
            .get(caller, &format!("/api/v1/w/{workspace}/connections/{id}"))
            .await;
        assert_eq!(read.status.as_u16(), 200, "{:?}", read.json);
        read.json
    }

    /// Connect Gmail as `caller` in `workspace`, end to end from the
    /// grant-intake port; the connection JSON.
    pub async fn connect(&self, caller: &Caller, workspace: WorkspaceId) -> serde_json::Value {
        let id = self
            .adopt(caller, workspace, ACCOUNT_SUB, None)
            .await
            .unwrap_or_else(|e| panic!("adopting: {e:?}"));
        let read = self
            .get(caller, &format!("/api/v1/w/{workspace}/connections/{id}"))
            .await;
        assert_eq!(read.status.as_u16(), 200, "{:?}", read.json);
        read.json
    }

    /// Run every ready job.
    pub async fn run_jobs(&self) {
        Worker::new(self.state.clone()).run_until_idle().await;
    }

    /// Let `by` pass, run the dispatcher at the new time, then every ready
    /// job.
    pub async fn tick(&self, by: TimeDelta) -> usize {
        self.clock.advance(by);
        let dispatched = crate::domain::schedule::dispatch(&self.state.db, self.clock.now())
            .await
            .unwrap();
        self.run_jobs().await;
        dispatched
    }

    /// Move the clock to the only stream's next run and run it.
    pub async fn next_run(&self) -> usize {
        let due: DateTime<Utc> = serde_json::from_value(self.stream().await["due_at"].clone())
            .expect("the stream is scheduled");
        self.tick((due - self.clock.now()).max(TimeDelta::zero()))
            .await
    }

    /// The names of the logged events of `workspace`, oldest first.
    pub async fn events_of(&self, workspace: WorkspaceId) -> Vec<String> {
        sqlx::query_scalar!(
            "SELECT name FROM kernel.events WHERE workspace_id = $1 ORDER BY id",
            workspace.to_string()
        )
        .fetch_all(&self.db)
        .await
        .unwrap()
    }

    /// The payloads of the logged `name` events of `workspace`, oldest first.
    pub async fn payloads(&self, workspace: WorkspaceId, name: &str) -> Vec<serde_json::Value> {
        sqlx::query_scalar!(
            "SELECT payload FROM kernel.events WHERE workspace_id = $1 AND name = $2 ORDER BY id",
            workspace.to_string(),
            name
        )
        .fetch_all(&self.db)
        .await
        .unwrap()
    }

    /// The batch documents announced in `workspace`, oldest first.
    pub async fn batches(&self, workspace: WorkspaceId) -> Vec<serde_json::Value> {
        let mut documents = Vec::new();
        for event in self
            .payloads(workspace, "connectors.messages_changed")
            .await
        {
            let blob: BlobRef = serde_json::from_value(event["batch"].clone()).unwrap();
            documents.push(serde_json::from_slice(&self.blob(workspace, &blob).await).unwrap());
        }
        documents
    }

    /// The bytes of `blob` in `workspace`.
    pub async fn blob(&self, workspace: WorkspaceId, blob: &BlobRef) -> Vec<u8> {
        let mut stream = self
            .state
            .blobs
            .get(&workspace.to_string(), blob)
            .await
            .unwrap();
        let mut bytes = Vec::new();
        while let Some(chunk) = stream.next().await {
            bytes.extend_from_slice(&chunk.unwrap());
        }
        bytes
    }

    /// The `connectors.streams` row of `workspace`'s only stream, as JSON.
    pub async fn stream(&self) -> serde_json::Value {
        sqlx::query_scalar!(
            r#"SELECT to_jsonb(s) || jsonb_build_object('due_at', d.due_at) AS "row!"
                 FROM connectors.streams s LEFT JOIN connectors.sync_due d ON d.stream_id = s.id"#
        )
        .fetch_one(&self.db)
        .await
        .unwrap()
    }

    /// How many `job` jobs are pending.
    pub async fn pending(&self, job: &str) -> i64 {
        sqlx::query_scalar!(
            r#"SELECT count(*) AS "n!" FROM kernel.jobs WHERE kind = $1 AND state = 'pending'"#,
            job
        )
        .fetch_one(&self.db)
        .await
        .unwrap()
    }
}

/// Validate `value` against `contracts/<file>`.
pub fn assert_contract(file: &str, value: &serde_json::Value) {
    let path = format!("{}/../../../contracts/{file}", env!("CARGO_MANIFEST_DIR"));
    let schema: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let mut schemas = boon::Schemas::new();
    let mut compiler = boon::Compiler::new();
    compiler.add_resource(&path, schema).unwrap();
    let index = compiler.compile(&path, &mut schemas).unwrap();
    if let Err(err) = schemas.validate(value, index) {
        panic!("{file}: {err:#}\n{value:#}");
    }
}

// --- the fake Google ---------------------------------------------------------

/// A message in the fake mailbox.
#[derive(Debug, Clone)]
pub struct FakeMessage {
    pub id: String,
    pub thread_id: String,
    pub labels: Vec<String>,
    pub raw: Vec<u8>,
    pub internal_date: DateTime<Utc>,
}

/// What the fake front door was asked: workspace, subject, provider,
/// connection.
pub type Asked = (WorkspaceId, String, String, Option<String>);

/// The identity module's front door, faked: records what it was asked and
/// answers a front-door URL.
#[derive(Debug, Default)]
pub struct FakeDoor {
    asked: Mutex<Vec<Asked>>,
}

impl FakeDoor {
    /// What each request asked.
    pub fn asked(&self) -> Vec<Asked> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl ConsentDoor for FakeDoor {
    fn start<'a>(&'a self, _state: &'a AppState, request: ConsentRequest) -> ConsentFuture<'a> {
        let url = format!("/auth/start/{}?link=fake-ticket", request.provider);
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((
                request.workspace,
                request.principal.subject,
                request.provider,
                request.connection,
            ));
        Box::pin(async move {
            Ok(StartedConsent {
                url,
                expires_at: Utc::now() + TimeDelta::minutes(2),
            })
        })
    }
}

#[derive(Debug, Default)]
struct Mailbox {
    messages: BTreeMap<String, FakeMessage>,
    /// (history id, record JSON).
    history: Vec<(u64, serde_json::Value)>,
    history_id: u64,
    /// History before this id is forgotten (404).
    history_floor: u64,
    next_id: u64,
    access_tokens: Vec<String>,
    refresh_tokens: Vec<String>,
    revoked: Vec<String>,
    refresh_invalid: bool,
    rate_limit_next: Option<u64>,
    email_unverified: bool,
    unauthorized_next: bool,
    /// What goes wrong with the next send.
    send_fault: Option<SendFault>,
    calls: Vec<String>,
    /// Messages whose labels Gmail refuses to change (400).
    refusing: Vec<String>,
    /// The next this many Gmail calls answer 503.
    unavailable_next: u32,
    /// The next this many `getProfile` calls answer 503.
    profile_unavailable_next: u32,
}

/// What goes wrong with a send.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SendFault {
    /// Answer this status and send nothing.
    Refuse(u16),
    /// Send it, but answer 503, as when the answer is lost.
    LoseAnswer,
    /// Send it, and answer 200 with a body that does not parse.
    GarbleAnswer,
}

/// Google's OAuth and Gmail endpoints, faked over a mutable mailbox.
#[derive(Clone)]
pub struct FakeGoogle {
    server: Arc<MockServer>,
    mailbox: Arc<Mutex<Mailbox>>,
}

struct Responder(Arc<Mutex<Mailbox>>);

impl FakeGoogle {
    async fn start() -> Self {
        let server = MockServer::start().await;
        let mailbox = Arc::new(Mutex::new(Mailbox {
            history_id: 1000,
            next_id: 1,
            ..Mailbox::default()
        }));
        Mock::given(any())
            .respond_with(Responder(mailbox.clone()))
            .mount(&server)
            .await;
        Self {
            server: Arc::new(server),
            mailbox,
        }
    }

    fn base(&self) -> Url {
        Url::parse(&format!("{}/", self.server.uri())).unwrap()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Mailbox> {
        self.mailbox.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// A consent, as the identity module's front door obtains one: fresh
    /// access and refresh tokens Google will accept.
    pub fn grant(&self) -> (String, String) {
        let mut mailbox = self.lock();
        let access = format!("fake-access-{}", mailbox.next_id);
        let refresh = format!("fake-refresh-{}", mailbox.next_id + 1);
        mailbox.next_id += 2;
        mailbox.access_tokens.push(access.clone());
        mailbox.refresh_tokens.push(refresh.clone());
        mailbox.refresh_invalid = false;
        (access, refresh)
    }

    /// Whether the provider verified the account's address.
    fn email_verified(&self) -> bool {
        !self.lock().email_unverified
    }

    /// Put a message in the mailbox (no history record: as if it was there
    /// before the connection).
    pub fn seed(&self, raw: &[u8], labels: &[&str], at: DateTime<Utc>) -> String {
        let mut mailbox = self.lock();
        let id = format!("msg-{:04}", mailbox.next_id);
        mailbox.next_id += 1;
        mailbox.messages.insert(
            id.clone(),
            FakeMessage {
                id: id.clone(),
                thread_id: format!("thread-{id}"),
                labels: labels.iter().map(|&l| l.to_owned()).collect(),
                raw: raw.to_vec(),
                internal_date: at,
            },
        );
        id
    }

    /// A message arrives: in the mailbox and in history.
    pub fn deliver(&self, raw: &[u8], labels: &[&str], at: DateTime<Utc>) -> String {
        let id = self.seed(raw, labels, at);
        let mut mailbox = self.lock();
        let message = mailbox.messages[&id].clone();
        push_history(
            &mut mailbox,
            serde_json::json!({"messagesAdded": [{"message": minimal(&message)}]}),
        );
        id
    }

    /// Labels change at Gmail (in history).
    pub fn relabel(&self, id: &str, add: &[&str], remove: &[&str]) {
        let mut mailbox = self.lock();
        let message = mailbox.messages.get_mut(id).unwrap();
        message.labels.retain(|l| !remove.contains(&l.as_str()));
        for label in add {
            if !message.labels.iter().any(|l| l == label) {
                message.labels.push((*label).to_owned());
            }
        }
        let message = message.clone();
        let mut record = serde_json::Map::new();
        if !add.is_empty() {
            record.insert(
                "labelsAdded".into(),
                serde_json::json!([{"message": minimal(&message), "labelIds": add}]),
            );
        }
        if !remove.is_empty() {
            record.insert(
                "labelsRemoved".into(),
                serde_json::json!([{"message": minimal(&message), "labelIds": remove}]),
            );
        }
        push_history(&mut mailbox, serde_json::Value::Object(record));
    }

    /// A message is deleted at Gmail (in history).
    pub fn delete(&self, id: &str) {
        let mut mailbox = self.lock();
        let message = mailbox.messages.remove(id).unwrap();
        push_history(
            &mut mailbox,
            serde_json::json!({"messagesDeleted": [{"message": minimal(&message)}]}),
        );
    }

    /// A message vanishes without a history record (seen only by a resync).
    pub fn vanish(&self, id: &str) {
        self.lock().messages.remove(id);
    }

    /// Gmail forgets history up to now: the stored history id answers 404.
    pub fn expire_history(&self) {
        let mut mailbox = self.lock();
        mailbox.history_id += 10;
        mailbox.history_floor = mailbox.history_id;
        mailbox.history.clear();
    }

    /// The next Gmail call answers 429 with `Retry-After: seconds`.
    pub fn rate_limit_next(&self, seconds: u64) {
        self.lock().rate_limit_next = Some(seconds);
    }

    /// The next `calls` Gmail calls answer 503.
    pub fn unavailable_next(&self, calls: u32) {
        self.lock().unavailable_next = calls;
    }

    /// The next `getProfile` answers 503.
    pub fn profile_unavailable_next(&self) {
        self.lock().profile_unavailable_next = 1;
    }

    /// Gmail refuses every change to message `id` (400).
    pub fn refuse_changes(&self, id: &str) {
        self.lock().refusing.push(id.to_owned());
    }

    /// The labels of message `id` at Gmail now, sorted.
    pub fn labels_of(&self, id: &str) -> Vec<String> {
        let mut labels = self.lock().messages[id].labels.clone();
        labels.sort();
        labels
    }

    /// Forget the calls seen so far.
    pub fn clear_calls(&self) {
        self.lock().calls.clear();
    }

    /// The Gmail write calls seen (`modify`, `batchModify`), in order.
    pub fn writes(&self) -> Vec<String> {
        self.lock()
            .calls
            .iter()
            .filter(|c| c.starts_with("POST /gmail/"))
            .cloned()
            .collect()
    }

    /// The next Gmail call answers 401.
    pub fn unauthorized_next(&self) {
        self.lock().unauthorized_next = true;
    }

    /// Refreshing answers `invalid_grant` (the member revoked access).
    pub fn revoke_grant(&self) {
        let mut mailbox = self.lock();
        mailbox.refresh_invalid = true;
        mailbox.access_tokens.clear();
    }

    /// Google says the account's address is not verified.
    pub fn unverified_email(&self) {
        self.lock().email_unverified = true;
    }

    /// Tokens revoked through the revocation endpoint.
    pub fn revoked(&self) -> Vec<String> {
        self.lock().revoked.clone()
    }

    /// Calls seen, as `METHOD path?format` (no tokens).
    pub fn calls(&self) -> Vec<String> {
        self.lock().calls.clone()
    }

    /// The next send answers `status` and sends nothing.
    pub fn refuse_next_send(&self, status: u16) {
        self.lock().send_fault = Some(SendFault::Refuse(status));
    }

    /// The next send goes out, but its answer is lost (503).
    pub fn lose_next_send_answer(&self) {
        self.lock().send_fault = Some(SendFault::LoseAnswer);
    }

    /// The next send goes out, with an answer that cannot be read.
    pub fn garble_next_send_answer(&self) {
        self.lock().send_fault = Some(SendFault::GarbleAnswer);
    }

    /// The messages sent, oldest first: their thread and raw bytes.
    pub fn sent(&self) -> Vec<FakeMessage> {
        self.lock()
            .messages
            .values()
            .filter(|m| m.labels.iter().any(|l| l == "SENT") && m.id.starts_with("sent-"))
            .cloned()
            .collect()
    }

    /// How many `messages.send` calls were made.
    pub fn send_calls(&self) -> usize {
        self.lock()
            .calls
            .iter()
            .filter(|c| c.starts_with("POST /upload/gmail/v1/users/me/messages/send"))
            .count()
    }

    /// How many raw downloads of `id` were made.
    pub fn raw_fetches(&self, id: &str) -> usize {
        let needle = format!("GET /gmail/v1/users/me/messages/{id}?format=raw");
        self.lock().calls.iter().filter(|c| **c == needle).count()
    }
}

fn minimal(message: &FakeMessage) -> serde_json::Value {
    serde_json::json!({"id": message.id, "threadId": message.thread_id, "labelIds": message.labels})
}

fn push_history(mailbox: &mut Mailbox, record: serde_json::Value) {
    mailbox.history_id += 1;
    let id = mailbox.history_id;
    let mut record = record;
    record["id"] = id.to_string().into();
    mailbox.history.push((id, record));
}

fn json(status: u16, body: &serde_json::Value) -> ResponseTemplate {
    ResponseTemplate::new(status).set_body_json(body)
}

fn form(body: &[u8]) -> HashMap<String, String> {
    url::form_urlencoded::parse(body).into_owned().collect()
}

impl Respond for Responder {
    fn respond(&self, request: &wiremock::Request) -> ResponseTemplate {
        let mut mailbox = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let path = request.url.path().to_owned();
        let query: HashMap<String, String> = request.url.query_pairs().into_owned().collect();
        let format = query
            .get("format")
            .map(|f| format!("?format={f}"))
            .unwrap_or_default();
        mailbox
            .calls
            .push(format!("{} {path}{format}", request.method));
        match (request.method.as_str(), path.as_str()) {
            ("POST", "/token") => token(&mut mailbox, &form(&request.body)),
            ("POST", "/revoke") => {
                let token = form(&request.body).remove("token").unwrap_or_default();
                mailbox.revoked.push(token);
                ResponseTemplate::new(200)
            }
            ("POST", "/upload/gmail/v1/users/me/messages/send") => {
                if !authorized(&mailbox, request) {
                    return json(401, &serde_json::json!({"error": {"code": 401}}));
                }
                send(&mut mailbox, request)
            }
            (method @ ("GET" | "POST"), gmail) if gmail.starts_with("/gmail/v1/users/me/") => {
                if let Some(seconds) = mailbox.rate_limit_next.take() {
                    return json(
                        429,
                        &serde_json::json!({"error": {"code": 429, "status": "RESOURCE_EXHAUSTED"}}),
                    )
                    .insert_header("retry-after", seconds.to_string().as_str());
                }
                if std::mem::take(&mut mailbox.unauthorized_next) || !authorized(&mailbox, request)
                {
                    return json(401, &serde_json::json!({"error": {"code": 401}}));
                }
                if gmail.ends_with("/profile") && mailbox.profile_unavailable_next > 0 {
                    mailbox.profile_unavailable_next -= 1;
                    return json(503, &serde_json::json!({"error": {"code": 503}}));
                }
                if mailbox.unavailable_next > 0 {
                    mailbox.unavailable_next -= 1;
                    return json(503, &serde_json::json!({"error": {"code": 503}}));
                }
                let path = &gmail["/gmail/v1/users/me/".len()..];
                if method == "POST" {
                    let body: serde_json::Value =
                        serde_json::from_slice(&request.body).unwrap_or_default();
                    return gmail_write(&mut mailbox, path, &body);
                }
                gmail_api(&mailbox, path, &query)
            }
            _ => ResponseTemplate::new(404),
        }
    }
}

fn authorized(mailbox: &Mailbox, request: &wiremock::Request) -> bool {
    request
        .headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .is_some_and(|token| mailbox.access_tokens.iter().any(|t| t == token))
}

fn token(mailbox: &mut Mailbox, form: &HashMap<String, String>) -> ResponseTemplate {
    let invalid = || json(400, &serde_json::json!({"error": "invalid_grant"}));
    if form.get("client_secret").map(String::as_str) != Some("test-client-secret") {
        return json(401, &serde_json::json!({"error": "invalid_client"}));
    }
    let access = format!("fake-access-{}", mailbox.next_id);
    mailbox.next_id += 1;
    match form.get("grant_type").map(String::as_str) {
        Some("refresh_token") => {
            let known = form
                .get("refresh_token")
                .is_some_and(|t| mailbox.refresh_tokens.contains(t));
            if mailbox.refresh_invalid || !known {
                return invalid();
            }
            mailbox.access_tokens.push(access.clone());
            json(
                200,
                &serde_json::json!({"access_token": access, "expires_in": 3599, "token_type": "Bearer"}),
            )
        }
        _ => invalid(),
    }
}

/// `messages.send` (multipart upload): the metadata part's `threadId`, then
/// the RFC 5322 message.
fn send(mailbox: &mut Mailbox, request: &wiremock::Request) -> ResponseTemplate {
    let fault = mailbox.send_fault.take();
    if let Some(SendFault::Refuse(status)) = fault {
        return json(status, &serde_json::json!({"error": {"code": status}}));
    }
    let content_type = request
        .headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let Some(boundary) = content_type.split("boundary=").nth(1) else {
        return json(400, &serde_json::json!({"error": {"code": 400}}));
    };
    let body = String::from_utf8_lossy(&request.body).into_owned();
    let parts: Vec<&str> = body.split(&format!("--{boundary}")).collect();
    let (Some(metadata), Some(message)) = (parts.get(1), parts.get(2)) else {
        return json(400, &serde_json::json!({"error": {"code": 400}}));
    };
    let metadata: serde_json::Value = metadata
        .split_once("\r\n\r\n")
        .and_then(|(_, json)| serde_json::from_str(json.trim()).ok())
        .unwrap_or_default();
    let raw = message
        .split_once("\r\n\r\n")
        .map(|(_, raw)| raw.strip_suffix("\r\n").unwrap_or(raw))
        .unwrap_or_default();
    let id = format!("sent-{:04}", mailbox.next_id);
    mailbox.next_id += 1;
    let thread_id = metadata["threadId"]
        .as_str()
        .map_or_else(|| format!("thread-{id}"), ToOwned::to_owned);
    let message = FakeMessage {
        id: id.clone(),
        thread_id: thread_id.clone(),
        labels: vec!["SENT".to_owned()],
        raw: raw.as_bytes().to_vec(),
        internal_date: Utc::now(),
    };
    push_history(
        mailbox,
        serde_json::json!({"messagesAdded": [{"message": minimal(&message)}]}),
    );
    mailbox.messages.insert(id.clone(), message);
    if fault == Some(SendFault::LoseAnswer) {
        return json(503, &serde_json::json!({"error": {"code": 503}}));
    }
    if fault == Some(SendFault::GarbleAnswer) {
        return ResponseTemplate::new(200).set_body_string("{\"id\": ");
    }
    json(
        200,
        &serde_json::json!({"id": id, "threadId": thread_id, "labelIds": ["SENT"]}),
    )
}

/// Whether `raw` carries `Message-ID: <id>`.
fn has_message_id(raw: &[u8], id: &str) -> bool {
    let needle = format!("message-id: <{}>", id.to_ascii_lowercase());
    String::from_utf8_lossy(raw)
        .to_ascii_lowercase()
        .contains(&needle)
}

fn gmail_api(mailbox: &Mailbox, path: &str, query: &HashMap<String, String>) -> ResponseTemplate {
    match path {
        "profile" => json(
            200,
            &serde_json::json!({"emailAddress": ACCOUNT_EMAIL, "historyId": mailbox.history_id.to_string()}),
        ),
        "labels" => json(
            200,
            &serde_json::json!({"labels": [
                {"id": "INBOX", "name": "INBOX", "type": "system"},
                {"id": "UNREAD", "name": "UNREAD", "type": "system"},
                {"id": "STARRED", "name": "STARRED", "type": "system"},
                {"id": "SENT", "name": "SENT", "type": "system"},
                {"id": "CATEGORY_UPDATES", "name": "CATEGORY_UPDATES", "type": "system"},
                {"id": "CATEGORY_PROMOTIONS", "name": "CATEGORY_PROMOTIONS", "type": "system"},
                {"id": "Label_7", "name": "Clients", "type": "user"},
                {"id": "Label_9", "name": "Receipts", "type": "user"}
            ]}),
        ),
        "messages" => {
            let after = query
                .get("q")
                .and_then(|q| q.strip_prefix("after:"))
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0);
            let max: usize = query
                .get("maxResults")
                .and_then(|m| m.parse().ok())
                .unwrap_or(100);
            let offset: usize = query
                .get("pageToken")
                .and_then(|p| p.parse().ok())
                .unwrap_or(0);
            let wanted = query.get("q").and_then(|q| q.strip_prefix("rfc822msgid:"));
            let mut listed: Vec<&FakeMessage> = mailbox
                .messages
                .values()
                .filter(|m| match wanted {
                    Some(id) => has_message_id(&m.raw, id),
                    None => m.internal_date.timestamp() > after,
                })
                .collect();
            listed.sort_by(|a, b| b.internal_date.cmp(&a.internal_date).then(b.id.cmp(&a.id)));
            let page: Vec<_> = listed
                .iter()
                .skip(offset)
                .take(max)
                .map(|m| serde_json::json!({"id": m.id, "threadId": m.thread_id}))
                .collect();
            let next = (offset + max < listed.len()).then(|| (offset + max).to_string());
            json(
                200,
                &serde_json::json!({"messages": page, "nextPageToken": next, "resultSizeEstimate": listed.len()}),
            )
        }
        "history" => {
            let start: u64 = query
                .get("startHistoryId")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            if start < mailbox.history_floor {
                return json(
                    404,
                    &serde_json::json!({"error": {"code": 404, "status": "NOT_FOUND"}}),
                );
            }
            let records: Vec<_> = mailbox
                .history
                .iter()
                .filter(|(id, _)| *id > start)
                .map(|(_, record)| record.clone())
                .collect();
            json(
                200,
                &serde_json::json!({"history": records, "historyId": mailbox.history_id.to_string()}),
            )
        }
        message if message.starts_with("messages/") => {
            let id = &message["messages/".len()..];
            let Some(found) = mailbox.messages.get(id) else {
                return json(404, &serde_json::json!({"error": {"code": 404}}));
            };
            let mut body = serde_json::json!({
                "id": found.id, "threadId": found.thread_id, "labelIds": found.labels,
                "historyId": mailbox.history_id.to_string(),
                "internalDate": found.internal_date.timestamp_millis().to_string(),
            });
            if query.get("format").map(String::as_str) == Some("raw") {
                // Gmail pads its base64url.
                let mut encoded = URL_SAFE_NO_PAD.encode(&found.raw);
                while encoded.len() % 4 != 0 {
                    encoded.push('=');
                }
                body["raw"] = encoded.into();
            }
            json(200, &body)
        }
        _ => ResponseTemplate::new(404),
    }
}

/// The strings of `value[key]`.
fn strings(value: &serde_json::Value, key: &str) -> Vec<String> {
    value[key]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|v| v.as_str().map(ToOwned::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// Change message `id`'s labels as Gmail does: only real changes are
/// recorded in history.
fn modify_labels(mailbox: &mut Mailbox, id: &str, add: &[String], remove: &[String]) {
    let Some(message) = mailbox.messages.get_mut(id) else {
        return;
    };
    let added: Vec<String> = add
        .iter()
        .filter(|l| !message.labels.contains(l))
        .cloned()
        .collect();
    let removed: Vec<String> = remove
        .iter()
        .filter(|l| message.labels.contains(l) && !add.contains(l))
        .cloned()
        .collect();
    message.labels.extend(added.iter().cloned());
    message.labels.retain(|l| !removed.contains(l));
    let message = message.clone();
    let mut record = serde_json::Map::new();
    if !added.is_empty() {
        record.insert(
            "labelsAdded".into(),
            serde_json::json!([{"message": minimal(&message), "labelIds": added}]),
        );
    }
    if !removed.is_empty() {
        record.insert(
            "labelsRemoved".into(),
            serde_json::json!([{"message": minimal(&message), "labelIds": removed}]),
        );
    }
    if !record.is_empty() {
        push_history(mailbox, serde_json::Value::Object(record));
    }
}

fn gmail_write(mailbox: &mut Mailbox, path: &str, body: &serde_json::Value) -> ResponseTemplate {
    let refused = || {
        json(
            400,
            &serde_json::json!({"error": {"code": 400, "errors": [{"reason": "failedPrecondition"}]}}),
        )
    };
    let add = strings(body, "addLabelIds");
    let remove = strings(body, "removeLabelIds");
    if path == "messages/batchModify" {
        let ids = strings(body, "ids");
        if ids
            .iter()
            .any(|id| !mailbox.messages.contains_key(id) || mailbox.refusing.contains(id))
        {
            return refused();
        }
        for id in &ids {
            modify_labels(mailbox, id, &add, &remove);
        }
        return ResponseTemplate::new(204);
    }
    let Some(id) = path
        .strip_prefix("messages/")
        .and_then(|rest| rest.strip_suffix("/modify"))
    else {
        return ResponseTemplate::new(404);
    };
    if !mailbox.messages.contains_key(id) {
        return json(404, &serde_json::json!({"error": {"code": 404}}));
    }
    if mailbox.refusing.iter().any(|r| r == id) {
        return refused();
    }
    modify_labels(mailbox, id, &add, &remove);
    json(200, &minimal(&mailbox.messages[id]))
}

/// A fictional message number `n` from `example.com`.
pub fn synthetic(n: usize) -> Vec<u8> {
    format!(
        "From: \"Sender {n}\" <sender{n}@example.com>\r\nTo: sam@example.com\r\n\
         Subject: Message {n}\r\nMessage-ID: <m{n}@example.com>\r\n\r\nBody {n}.\r\n"
    )
    .into_bytes()
}
