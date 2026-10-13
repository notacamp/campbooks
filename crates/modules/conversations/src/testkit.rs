//! Test fixtures: the module over a database whose connections run as the
//! runtime role (row-level security applies, as in production), a test
//! identity provider, a stand-in for the workspaces module's membership
//! port, in-memory blobs, and the recorded batches in `fixtures/`.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::{
    Router,
    body::Body,
    http::{Method, Request, header},
};
use campbooks_ai::decide::{AnswerValue, Question};
use campbooks_kernel::{
    AppState, Error,
    auth::{Principal, PrincipalKind},
    blobs::{BlobRef, BlobStore},
    config::BlobsConfig,
    jobs::{Registry, Worker, WorkerStats},
    tenancy::{Access, Membership, MembershipFuture, Role, UserId, WorkspaceId},
    testing::{self, TestIdp, TestResponse},
};
use sqlx::PgPool;

use crate::{
    ConversationsDeps, dev,
    domain::{
        blobs::read_text,
        decide::{BoxFuture, Envelope, Source, Strategy, Verdict},
        questions,
    },
    ingest::MessageChangeBatch,
    model::{InboxId, InboxRole},
};

/// Every scope of this module.
pub const ALL_SCOPES: &str = "conversations:read conversations:write";

/// The membership port, as the workspaces module would answer it: users
/// registered with [`Kit::member`], and clients bound to the workspace.
#[derive(Debug, Default)]
struct TestMembership {
    members: Mutex<HashMap<(String, WorkspaceId), UserId>>,
}

impl Membership for TestMembership {
    fn resolve<'a>(
        &'a self,
        _db: &'a PgPool,
        principal: &'a Principal,
        workspace: WorkspaceId,
    ) -> MembershipFuture<'a> {
        let access = match principal.kind {
            PrincipalKind::Client => Some(Access {
                workspace,
                user: None,
                role: None,
            }),
            PrincipalKind::User => self
                .members
                .lock()
                .ok()
                .and_then(|m| m.get(&(principal.subject.clone(), workspace)).copied())
                .map(|user| Access {
                    workspace,
                    user: Some(user),
                    role: Some(Role::Member),
                }),
        };
        Box::pin(async move { Ok::<_, Error>(access) })
    }
}

/// The module over a fresh database.
pub struct Kit {
    /// The test database as its owner (no row-level security), for
    /// inspecting rows.
    pub db: PgPool,
    /// The state jobs and handlers run with (runtime role).
    pub state: AppState,
    app: Router,
    idp: TestIdp,
    membership: Arc<TestMembership>,
}

impl Kit {
    /// The module sorting with [`HeaderRules`], without AI.
    pub async fn new(db: PgPool) -> Self {
        Self::with_deps(
            db,
            ConversationsDeps::offline().with_rules(Arc::new(HeaderRules)),
        )
        .await
    }

    /// The module composed with `deps`.
    pub async fn with_deps(db: PgPool, deps: ConversationsDeps) -> Self {
        let idp = TestIdp::start().await;
        let pool = testing::app_role_pool(&db).await;
        let blobs = BlobStore::memory(&BlobsConfig::default(), pool.clone());
        let membership = Arc::new(TestMembership::default());
        let state = idp
            .state(pool)
            .with_blobs(blobs)
            .with_membership(membership.clone())
            .with_registry(Registry::build(&[crate::module(deps.clone())]).unwrap());
        let app = testing::app(crate::module(deps), state.clone());
        Self {
            db,
            state,
            app,
            idp,
            membership,
        }
    }

    /// A user `sub`, member of `workspace`, with every scope of the module.
    pub fn member(&self, sub: &str, workspace: WorkspaceId) -> (String, UserId) {
        let user = UserId::new();
        self.membership
            .members
            .lock()
            .unwrap()
            .insert((sub.to_owned(), workspace), user);
        let mut claims = self.idp.claims(ALL_SCOPES);
        claims["sub"] = sub.into();
        (self.idp.token(&claims), user)
    }

    /// A client token bound to `workspace`.
    pub fn client(&self, workspace: WorkspaceId, scopes: &str) -> String {
        let mut claims = self.idp.claims(scopes);
        let object = claims.as_object_mut().unwrap();
        object.remove("sub");
        object.insert("azp".into(), "acme-sync".into());
        object.insert("workspace_id".into(), workspace.to_string().into());
        self.idp.token(&claims)
    }

    /// An inbox of `workspace` for `address`, readable by `members`.
    pub async fn inbox(
        &self,
        workspace: WorkspaceId,
        address: &str,
        members: &[(UserId, InboxRole)],
    ) -> InboxId {
        dev::create_inbox(&self.state, workspace, address, members)
            .await
            .unwrap()
    }

    /// The recorded batch `fixtures/<name>.json`, stored for `inbox`.
    pub async fn batch(
        &self,
        workspace: WorkspaceId,
        inbox: InboxId,
        name: &str,
    ) -> MessageChangeBatch {
        let recorded: serde_json::Value = serde_json::from_str(&fixture(name)).unwrap();
        dev::record(&self.state, workspace, inbox, &recorded)
            .await
            .unwrap()
    }

    /// Apply the recorded batch `name` to `inbox` and run every job it
    /// causes.
    pub async fn ingest(&self, workspace: WorkspaceId, inbox: InboxId, name: &str) -> WorkerStats {
        let batch = self.batch(workspace, inbox, name).await;
        dev::submit(&self.state, workspace, &batch).await.unwrap();
        self.run_jobs().await
    }

    /// A workspace with Sam's inbox holding `fixtures/en.json`: returns the
    /// workspace, the inbox and Sam's token (a member of the inbox).
    pub async fn sams_inbox(&self) -> (WorkspaceId, InboxId, String) {
        let workspace = WorkspaceId::new();
        let (token, sam) = self.member("sam", workspace);
        let inbox = self
            .inbox(workspace, "sam@example.com", &[(sam, InboxRole::Manager)])
            .await;
        self.ingest(workspace, inbox, "en").await;
        (workspace, inbox, token)
    }

    /// The id of the conversation holding message `provider_message_id`.
    pub async fn conversation_of(&self, provider_message_id: &str) -> String {
        sqlx::query_scalar(
            "SELECT conversation_id::text FROM conversations.messages WHERE provider_message_id = $1",
        )
        .bind(provider_message_id)
        .fetch_one(&self.db)
        .await
        .unwrap()
    }

    /// Run every pending job until none is left.
    pub async fn run_jobs(&self) -> WorkerStats {
        let stats = Worker::new(self.state.clone()).run_until_idle().await;
        assert_eq!(stats.failed, 0, "{stats:?}");
        stats
    }

    /// The text of `blob` in `workspace`.
    pub async fn read(&self, workspace: WorkspaceId, blob: &BlobRef) -> String {
        read_text(&self.state.blobs, &workspace.to_string(), blob, 1 << 20)
            .await
            .unwrap()
            .0
    }

    /// `GET uri` with `token`.
    pub async fn get(&self, token: &str, uri: &str) -> TestResponse {
        self.call(Method::GET, token, uri, None).await
    }

    /// `POST uri` with a JSON body (or none).
    pub async fn post(
        &self,
        token: &str,
        uri: &str,
        body: Option<serde_json::Value>,
    ) -> TestResponse {
        self.call(Method::POST, token, uri, body).await
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

    /// Events of type `name` in the log, oldest first, as JSON.
    pub async fn events(&self, name: &str) -> Vec<serde_json::Value> {
        sqlx::query_scalar("SELECT payload FROM kernel.events WHERE name = $1 ORDER BY id")
            .bind(name)
            .fetch_all(&self.db)
            .await
            .unwrap()
    }

    /// `(provider_message_id, conversation_id)` of the inbox's messages.
    pub async fn threads(&self, inbox: InboxId) -> HashMap<String, String> {
        sqlx::query_as::<_, (String, String)>(
            "SELECT provider_message_id, conversation_id::text FROM conversations.messages
              WHERE inbox_id = $1",
        )
        .bind(inbox)
        .fetch_all(&self.db)
        .await
        .unwrap()
        .into_iter()
        .collect()
    }

    /// The stream and stream source of the conversation holding message
    /// `provider_message_id`.
    pub async fn stream_of(&self, provider_message_id: &str) -> (String, String) {
        sqlx::query_as(
            "SELECT c.stream, c.stream_source FROM conversations.conversations c
               JOIN conversations.messages m ON m.conversation_id = c.id
              WHERE m.provider_message_id = $1",
        )
        .bind(provider_message_id)
        .fetch_one(&self.db)
        .await
        .unwrap()
    }
}

/// `fixtures/<name>.json`.
pub fn fixture(name: &str) -> String {
    let path = format!("{}/fixtures/{name}.json", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// A stand-in rules strategy for tests, until campbooks-cloud#59's shared
/// rules land: answers `stream` from a few obvious envelope signals, with a
/// fixed confidence, and abstains otherwise. Not shipped.
pub struct HeaderRules;

impl Strategy for HeaderRules {
    fn name(&self) -> &'static str {
        "test-rules"
    }

    fn source(&self) -> Source {
        Source::Rules
    }

    fn decide<'a>(
        &'a self,
        envelope: &'a Envelope,
        questions: &'a [Question],
    ) -> BoxFuture<'a, Result<Vec<Option<Verdict>>, campbooks_ai::Error>> {
        let stream = if envelope.attachments.iter().any(|(name, _)| {
            let name = name.to_lowercase();
            ["fatura", "factura", "recu", "receipt", "invoice"]
                .iter()
                .any(|word| name.contains(word))
        }) {
            Some(("receipts", "a receipt or invoice attachment"))
        } else if envelope.headers.list_id.is_some() || envelope.headers.list_unsubscribe.is_some()
        {
            Some(("newsletters", "mailing-list headers"))
        } else if envelope.headers.auto_submitted.is_some() {
            Some(("updates", "Auto-Submitted header"))
        } else {
            None
        };
        let verdicts = questions
            .iter()
            .map(|q| {
                let (label, why) = stream.filter(|_| q.id == questions::STREAM)?;
                Some(Verdict {
                    answer: campbooks_ai::decide::Answer {
                        question_id: q.id.clone(),
                        value: AnswerValue::Choice(label.to_owned()),
                        probabilities: None,
                        confidence: Some(0.95),
                        model: "test-rules".to_owned(),
                        model_version: None,
                        latency: Duration::ZERO,
                    },
                    explanation: Some(why.to_owned()),
                })
            })
            .collect();
        Box::pin(async move { Ok(verdicts) })
    }
}
