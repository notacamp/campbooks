//! Test fixtures: the module over a database whose connections run as the
//! runtime role (row-level security applies, as in production), a test
//! identity provider, a stand-in for the workspaces module's membership
//! port, in-memory blobs, the recorded batches in `fixtures/`, and a
//! scripted model ([`Scripted`]) answering for the fixtures' senders.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Instant,
};

use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Method, Request, StatusCode, header},
};
use campbooks_ai::{
    Error as AiError,
    decide::{
        Answer, AnswerValue, Question, QuestionKind,
        check::stamp,
        signals::email::{Envelope, triage},
        strategy::{BoxFuture, Decided, Decisions, Reason, Strategy},
    },
};
use campbooks_kernel::{
    AppState, Error,
    auth::{Principal, PrincipalKind, Scope},
    blobs::{BlobRef, BlobStore},
    config::BlobsConfig,
    jobs::{Registry, Worker, WorkerStats},
    operations,
    tenancy::{Access, Membership, MembershipFuture, Role, UserId, WorkspaceId},
    testing::{self, TestIdp, TestResponse},
};
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt as _;

use crate::{
    ConversationsDeps, dev,
    domain::{
        blobs::read_text,
        decide::Source,
        policy::{Policy, Switch},
    },
    ingest::MessageChangeBatch,
    model::{InboxId, InboxRole},
};

/// Every scope of this module.
pub const ALL_SCOPES: &str = "conversations:read conversations:write conversations:send";

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

    fn role_held<'a>(
        &'a self,
        _conn: &'a mut sqlx::PgConnection,
        workspace: WorkspaceId,
        user: UserId,
    ) -> campbooks_kernel::tenancy::RoleFuture<'a> {
        let member = self
            .members
            .lock()
            .ok()
            .is_some_and(|m| m.iter().any(|((_, w), u)| *w == workspace && *u == user));
        Box::pin(async move { Ok(member.then_some(Role::Member)) })
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
    deps: ConversationsDeps,
}

impl Kit {
    /// The module under [`sorting_deps`], without AI.
    pub async fn new(db: PgPool) -> Self {
        Self::with_deps(db, sorting_deps()).await
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
        let app = testing::app(crate::module(deps.clone()), state.clone());
        Self {
            db,
            state,
            app,
            idp,
            membership,
            deps,
        }
    }

    /// The same kit, reading members' preferences through `preferences`.
    #[must_use]
    pub fn with_preferences(
        mut self,
        preferences: campbooks_kernel::preferences::MemberPreferencesPort,
    ) -> Self {
        self.state = self.state.with_preferences(preferences);
        self.app = testing::app(crate::module(self.deps.clone()), self.state.clone());
        self
    }

    /// The same kit with no binding key (an instance with no secret): sends
    /// are neither scheduled nor released (campbooks-cloud#227).
    #[must_use]
    pub fn keyless(mut self) -> Self {
        self.state = self
            .state
            .with_binding(campbooks_kernel::binding::Binding::disabled());
        self.app = testing::app(crate::module(self.deps.clone()), self.state.clone());
        self
    }

    /// The same kit, asking `clients` whom each client token acts for.
    #[must_use]
    pub fn with_clients(mut self, clients: campbooks_kernel::tenancy::ClientDirectoryPort) -> Self {
        self.state = self.state.with_client_directory(clients);
        self.app = testing::app(crate::module(self.deps.clone()), self.state.clone());
        self
    }

    /// Apply the recorded batch `recorded` (the fixtures' format) to `inbox`
    /// and run every job it causes.
    pub async fn ingest_value(
        &self,
        workspace: WorkspaceId,
        inbox: InboxId,
        recorded: &serde_json::Value,
    ) -> WorkerStats {
        let batch = dev::record(&self.state, workspace, inbox, recorded)
            .await
            .unwrap();
        dev::submit(&self.state, workspace, &batch).await.unwrap();
        self.run_jobs().await
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

    /// A token for `sub`, already a member, whose claims name them `name`.
    pub fn token_named(&self, sub: &str, name: &str) -> String {
        let mut claims = self.idp.claims(ALL_SCOPES);
        claims["sub"] = sub.into();
        claims["name"] = name.into();
        self.idp.token(&claims)
    }

    /// `sub` leaves `workspace` (their inbox memberships stay until
    /// `workspaces.member_left` is handled).
    pub fn leave(&self, sub: &str, workspace: WorkspaceId) {
        self.membership
            .members
            .lock()
            .unwrap()
            .remove(&(sub.to_owned(), workspace));
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

    /// `GET uri` with `token`, asking for `language` (`Accept-Language`).
    pub async fn get_in(&self, token: &str, uri: &str, language: &str) -> TestResponse {
        let request = Request::get(uri)
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::ACCEPT_LANGUAGE, language)
            .body(Body::empty())
            .unwrap();
        testing::send(&self.app, request).await
    }

    /// `GET uri` with `token`, whole: status, headers and body bytes.
    pub async fn get_raw(&self, token: &str, uri: &str) -> (StatusCode, HeaderMap, Vec<u8>) {
        let request = Request::get(uri)
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap();
        let response = self.app.clone().oneshot(request).await.unwrap();
        let (parts, body) = response.into_parts();
        let bytes = axum::body::to_bytes(body, 64 << 20).await.unwrap();
        (parts.status, parts.headers, bytes.to_vec())
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

    /// `PATCH uri` with a JSON body.
    pub async fn patch(&self, token: &str, uri: &str, body: serde_json::Value) -> TestResponse {
        self.call(Method::PATCH, token, uri, Some(body)).await
    }

    /// Run `operation` by id as the assistant acting for member `sub` in
    /// `workspace` ([`operations::invoke`]): the same routes, scope and
    /// membership checks as their HTTP request, with an idempotency key, and
    /// marked confirmed (the user said yes to the call) when `confirmed`.
    pub async fn as_assistant(
        &self,
        sub: &str,
        workspace: WorkspaceId,
        (operation, input): (&str, Value),
        confirmed: bool,
    ) -> Result<operations::Output, operations::InvokeError> {
        let module = crate::module(self.deps.clone());
        let resources = campbooks_kernel::authz::Catalogue::new([&module.resources]).unwrap();
        let state =
            operations::compose(module.router, self.state.clone().with_resources(resources)).state;
        let principal = Principal {
            subject: sub.to_owned(),
            kind: PrincipalKind::User,
            client_id: None,
            scopes: ALL_SCOPES.split(' ').map(Scope::new).collect(),
            workspace: None,
            email: Some(format!("{sub}@example.com")),
            email_verified: true,
            name: None,
            expires_at: None,
            issued_at: None,
            auth_time: None,
        };
        let mut invocation =
            operations::Invocation::new(operations::Caller::Assistant, principal, operation, input)
                .in_workspace(workspace.to_string())
                .idempotency_key(sqlx::types::Uuid::now_v7().to_string());
        if confirmed {
            invocation = invocation.confirmed();
        }
        operations::invoke(&state, invocation).await
    }

    /// The id of message `provider_message_id`.
    pub async fn message_of(&self, provider_message_id: &str) -> String {
        sqlx::query_scalar(
            "SELECT id::text FROM conversations.messages WHERE provider_message_id = $1",
        )
        .bind(provider_message_id)
        .fetch_one(&self.db)
        .await
        .unwrap()
    }

    /// Add user `user` to `inbox` with `role`.
    pub async fn add_to_inbox(&self, inbox: InboxId, user: UserId, role: InboxRole) {
        sqlx::query(
            "INSERT INTO conversations.inbox_members (workspace_id, inbox_id, user_id, role)
             SELECT workspace_id, id, $2, $3 FROM conversations.inboxes WHERE id = $1",
        )
        .bind(inbox)
        .bind(user.as_uuid())
        .bind(role.as_str())
        .execute(&self.db)
        .await
        .unwrap();
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

/// The shipped facts as rules and the [`Scripted`] model in place of the
/// `decide` role, under the shipped facts' switch and a test policy for the
/// model: every question acts from 0.9. Needs are raised and the model is
/// asked whatever the fixtures' dates (`needs_window_days`,
/// `decide_backfill_days`), so tests do not drift with the calendar.
pub fn sorting_deps() -> ConversationsDeps {
    let mut policy = Policy::baseline();
    for question in [
        triage::STREAM,
        triage::NEEDS_REPLY,
        triage::ASKS_ACTION,
        triage::HAS_DEADLINE,
        triage::URGENCY,
        triage::SUSPICIOUS,
    ] {
        policy = policy.with(question, Source::Model, Switch::ActAbove(0.9));
    }
    let mut deps = ConversationsDeps::offline()
        .with_policy(policy)
        .with_model(Arc::new(Scripted::fixtures()));
    deps.config.needs_window_days = 36_500;
    deps.config.decide_backfill_days = 36_500;
    deps
}

/// What a scripted strategy answers about one sender's mail: per question,
/// the answer and its confidence.
pub type Script = Vec<(&'static str, AnswerValue, f64)>;

/// A strategy answering from a script keyed by the sender's address: the
/// model a test stands in for (named `scripted`, its answers stamped as
/// given under a passed answers check), or a test's rules. A sender not in
/// the script, or a question not in its line, is abstained on. A sender who
/// writes twice in the fixtures has a line per message too
/// ([`Scripted::followup`]), found by the message's exact subject: the
/// fixture's identity, a canned answer, never a rule.
pub struct Scripted {
    name: &'static str,
    stamped: bool,
    by_sender: HashMap<String, Script>,
    by_message: HashMap<(String, String), Script>,
}

impl Scripted {
    /// A model answering `by_sender`.
    pub fn model<S: Into<String>>(by_sender: impl IntoIterator<Item = (S, Script)>) -> Self {
        Self {
            name: "scripted",
            stamped: true,
            by_sender: by_sender
                .into_iter()
                .map(|(sender, script)| (sender.into(), script))
                .collect(),
            by_message: HashMap::new(),
        }
    }

    /// The same script, answering `script` for `sender`'s message
    /// `subject`.
    #[must_use]
    pub fn followup(mut self, sender: &str, subject: &str, script: Script) -> Self {
        self.by_message
            .insert((sender.to_owned(), subject.to_owned()), script);
        self
    }

    /// Rules (unstamped) answering `by_sender`.
    pub fn rules<S: Into<String>>(by_sender: impl IntoIterator<Item = (S, Script)>) -> Self {
        Self {
            name: "test-rules",
            stamped: false,
            ..Self::model(by_sender)
        }
    }

    /// The model as a good one reads the fixtures' mail, sure of every
    /// answer (0.97): a person's question needs a reply, a request is an
    /// action, a bill from a service is in Receipts and asks for a payment
    /// still owed (so does a supplier's invoice sent by hand), a receipt
    /// confirms one made, a look-alike asking for a password is phishing,
    /// lists and notifications ask nothing.
    pub fn fixtures() -> Self {
        // Pedro writes again about the lease, asking nothing new.
        Self::model(fixture_answers()).followup(
            "pedro.alves@example.com",
            "Re: Lease for the studio",
            script("primary", false, false, false, 0.0, false),
        )
    }
}

/// `answers` for one sender: stream, needs reply, asks action, has a
/// deadline, urgency level, suspicious, and money "neither"
/// ([`money`] says otherwise); all at 0.97.
// One flag per yes/no question, in the questions' order: a table row.
#[allow(clippy::fn_params_excessive_bools)]
pub fn script(
    stream: &str,
    reply: bool,
    action: bool,
    deadline: bool,
    urgency: f64,
    suspicious: bool,
) -> Script {
    let at = 0.97;
    vec![
        (triage::STREAM, AnswerValue::Choice(stream.to_owned()), at),
        (triage::NEEDS_REPLY, AnswerValue::Noul(reply), at),
        (triage::ASKS_ACTION, AnswerValue::Noul(action), at),
        (triage::HAS_DEADLINE, AnswerValue::Noul(deadline), at),
        (triage::URGENCY, AnswerValue::Score(urgency), at),
        (triage::SUSPICIOUS, AnswerValue::Noul(suspicious), at),
        (
            triage::MONEY,
            AnswerValue::Choice(triage::NEITHER.to_owned()),
            at,
        ),
    ]
}

/// `script` with `label` (`owed`, `paid`) as its money answer.
pub fn money(mut script: Script, label: &str) -> Script {
    for (id, value, _) in &mut script {
        if *id == triage::MONEY {
            *value = AnswerValue::Choice(label.to_owned());
        }
    }
    script
}

fn fixture_answers() -> Vec<(&'static str, Script)> {
    vec![
        // en: a colleague's review with a deadline, a newsletter, a sign-in
        // alert.
        (
            "maya@northwind-studio.example",
            script("primary", true, true, true, 1.0, false),
        ),
        (
            "hello@letters.paperkite.example",
            script("newsletters", false, false, false, 0.0, false),
        ),
        (
            "no-reply@accounts.cloudvault.example",
            script("updates", false, false, false, 0.0, false),
        ),
        // pt-PT: a water bill, a friend's dinner question, a promotion.
        (
            "faturacao@aguas-do-vale.example",
            money(script("receipts", false, true, true, 1.0, false), "owed"),
        ),
        (
            "rui.tavares@correio.example",
            script("primary", true, false, false, 1.0, false),
        ),
        (
            "novidades@alfarroba.example",
            script("newsletters", false, false, false, 0.0, false),
        ),
        // es-ES: an invoice a supplier sent by hand, a list digest.
        (
            "administracion@harinas-del-sur.example",
            money(script("primary", false, true, true, 1.0, false), "owed"),
        ),
        (
            "boletin@gremio-panaderos.example",
            script("newsletters", false, false, false, 0.0, false),
        ),
        // fr: a shipping update, a receipt, a neighbour's leak.
        (
            "suivi@colis-rapide.example",
            script("updates", false, false, false, 0.0, false),
        ),
        (
            "caisse@boulangerie-halles.example",
            money(script("receipts", false, false, false, 0.0, false), "paid"),
        ),
        (
            "hugo.bernard@courriel.example",
            script("primary", true, true, false, 2.0, false),
        ),
        // needs: a question, a lease to sign, a bill, phishing, lunch.
        (
            "marta@casa-lume.example",
            script("primary", true, false, true, 1.0, false),
        ),
        (
            "pedro.alves@example.com",
            script("primary", false, true, true, 1.0, false),
        ),
        (
            "billing@acme-hosting.example",
            money(script("receipts", false, true, true, 1.0, false), "owed"),
        ),
        (
            "it-support@at3lier-sol.example",
            script("updates", false, true, true, 2.0, true),
        ),
        (
            "jonas@weber-studio.example",
            script("primary", true, false, false, 1.0, false),
        ),
    ]
}

impl Strategy for Scripted {
    fn name(&self) -> &str {
        self.name
    }

    fn decide<'a>(
        &'a self,
        envelope: &'a Envelope,
        questions: &'a [Question],
    ) -> BoxFuture<'a, Result<Decisions, AiError>> {
        Box::pin(async move {
            let started = Instant::now();
            let sender = envelope.from.address.to_lowercase();
            let script = self
                .by_message
                .get(&(sender.clone(), envelope.subject.clone()))
                .or_else(|| self.by_sender.get(&sender));
            let answers = questions
                .iter()
                .map(|q| {
                    let (_, value, confidence) =
                        script?.iter().find(|(id, ..)| *id == q.id)?.clone();
                    Some(Decided {
                        answer: Answer {
                            question_id: q.id.clone(),
                            probabilities: Some(probabilities(q, &value, confidence)),
                            value,
                            confidence: Some(confidence),
                            model: self.name.to_owned(),
                            model_version: self.stamped.then(|| stamp(None)),
                            latency: started.elapsed(),
                        },
                        reasons: vec![Reason::new(
                            if self.stamped {
                                "model.decide"
                            } else {
                                "test.scripted"
                            },
                            self.name,
                        )],
                    })
                })
                .collect();
            Ok(Decisions {
                answers,
                remote_calls: u32::from(self.stamped),
            })
        })
    }
}

/// A model that answers only once it is let in ([`Gated::open`]); before
/// that it is a model that has not passed its answers check, so the pass
/// stores nothing.
pub struct Gated {
    open: std::sync::atomic::AtomicBool,
    model: Scripted,
}

impl Gated {
    /// `model`, not let in yet.
    pub fn closed(model: Scripted) -> Arc<Self> {
        Arc::new(Self {
            open: false.into(),
            model,
        })
    }

    /// Let it answer from now on.
    pub fn open(&self) {
        self.open.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

impl Strategy for Gated {
    fn name(&self) -> &str {
        self.model.name()
    }

    fn decide<'a>(
        &'a self,
        envelope: &'a Envelope,
        questions: &'a [Question],
    ) -> BoxFuture<'a, Result<Decisions, AiError>> {
        if self.open.load(std::sync::atomic::Ordering::SeqCst) {
            self.model.decide(envelope, questions)
        } else {
            Box::pin(async { Err(AiError::AnswersNotTrusted) })
        }
    }
}

/// Probabilities in question order putting `confidence` on `value`.
fn probabilities(question: &Question, value: &AnswerValue, confidence: f64) -> Vec<f64> {
    let n = question.option_count();
    #[allow(clippy::cast_precision_loss)] // a handful of options
    let rest = (1.0 - confidence) / (n - 1) as f64;
    let index = match (value, &question.kind) {
        (AnswerValue::Choice(label), QuestionKind::Choice { options, .. }) => options
            .iter()
            .position(|o| &o.label == label)
            .unwrap_or_default(),
        (AnswerValue::Noul(holds), _) => usize::from(*holds),
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // 0..3
        (AnswerValue::Score(level), _) => level.round() as usize,
        _ => 0,
    };
    (0..n)
        .map(|i| if i == index { confidence } else { rest })
        .collect()
}

/// `n` notifications from `from` (`<prefix>-0` … ids, one conversation each),
/// subjects `[acme/web] Pull request #<i> merged`, on `list` when given, sent
/// a minute apart from 2026-10-01: a recorded batch in the fixtures' format.
pub fn notifications(prefix: &str, n: usize, from: &str, list: Option<&str>) -> Value {
    let base = chrono::DateTime::parse_from_rfc3339("2026-10-01T08:00:00Z").unwrap();
    let changes: Vec<Value> = (0..n)
        .map(|i| {
            let mut headers = serde_json::json!({"auto_submitted": "auto-generated"});
            if let Some(list) = list {
                headers["list_id"] = list.into();
            }
            let minutes = i64::try_from(i).unwrap();
            serde_json::json!({
                "op": "upsert",
                "message": {
                    "provider_message_id": format!("{prefix}-{i}"),
                    "thread_key": format!("thr-{prefix}-{i}"),
                    "from": {"name": "GitHub", "address": from},
                    "to": [{"name": "Sam Rivers", "address": "sam@example.com"}],
                    "subject": format!("[acme/web] Pull request #{} merged", 4000 + i),
                    "sent_at": (base + chrono::TimeDelta::minutes(minutes)).to_rfc3339(),
                    "direction": "inbound",
                    "headers": headers,
                    "text": "Merged into main.",
                    "mailbox": {"in_inbox": true},
                    "read": false, "starred": false
                }
            })
        })
        .collect();
    serde_json::json!({"changes": changes})
}

/// `fixtures/<name>.json`.
pub fn fixture(name: &str) -> String {
    let path = format!("{}/fixtures/{name}.json", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// A compiled contract from `contracts/`.
pub fn contract(file: &str) -> (boon::Schemas, boon::SchemaIndex) {
    let path = format!("{}/../../../contracts/{file}", env!("CARGO_MANIFEST_DIR"));
    let schema: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let mut schemas = boon::Schemas::new();
    let mut compiler = boon::Compiler::new();
    compiler.enable_format_assertions();
    compiler.add_resource(file, schema).unwrap();
    let index = compiler.compile(file, &mut schemas).unwrap();
    (schemas, index)
}
