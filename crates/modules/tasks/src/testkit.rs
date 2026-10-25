//! Test fixtures: the module over a database whose connections run as the
//! runtime role (row-level security applies), a test identity provider,
//! stand-ins for the membership port, for the conversations module's
//! access facts (a conversation lives in an inbox that some members see) and
//! for the schedule module's `Holds` port (blocks kept in memory).

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use axum::{
    Router,
    body::Body,
    http::{Method, Request, header},
};
use campbooks_kernel::{
    AppState, DomainEvent, Error, RecordRef,
    auth::{Principal, PrincipalKind, Scope},
    authz::{
        Catalogue, Facts, FactsFuture, Owner, OwnerFact, ResourceFacts, ResourceType, Resources,
        Tenancy,
    },
    blobs::BlobStore,
    changes::{Visibilities, Visibility, VisibilityFuture},
    config::BlobsConfig,
    db::BeginFor as _,
    jobs::{Registry, Worker, WorkerStats},
    operations,
    schedule::{Held, HoldFuture, HoldRequest, Holds, ReleaseFuture},
    tenancy::{Access, Membership, MembershipFuture, Role, RoleFuture, UserId, WorkspaceId},
    testing::{self, TestIdp, TestResponse},
};
use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgPool, Postgres, Transaction, types::Uuid};

/// Every scope of this module.
pub const ALL_SCOPES: &str = "tasks:read tasks:write";

/// The inbox every test conversation lives in.
pub const INBOX: &str = "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c22";

/// A conversation in [`INBOX`].
pub const CONVERSATION: &str = "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c11";

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
        _conn: &'a mut PgConnection,
        workspace: WorkspaceId,
        user: UserId,
    ) -> RoleFuture<'a> {
        let held = self
            .members
            .lock()
            .is_ok_and(|m| m.iter().any(|((_, w), u)| *w == workspace && *u == user));
        Box::pin(async move { Ok::<_, Error>(held.then_some(Role::Member)) })
    }
}

/// Inboxes and who may see them, as the conversations module would answer.
#[derive(Debug, Default)]
struct TestInboxes {
    members: Mutex<HashSet<(String, UserId)>>,
}

impl Visibility for TestInboxes {
    fn container_types(&self) -> &'static [&'static str] {
        &["conversations.inbox"]
    }

    fn container_visible<'a>(
        &'a self,
        _db: &'a PgPool,
        access: &'a Access,
        container: &'a RecordRef,
    ) -> VisibilityFuture<'a> {
        let visible = container.record_type == "conversations.inbox"
            && access.user.is_some_and(|user| {
                self.members
                    .lock()
                    .is_ok_and(|m| m.contains(&(container.id.clone(), user)))
            });
        Box::pin(async move { Ok(visible) })
    }
}

/// A conversation, as its module would declare it: read by whoever may see
/// its inbox.
static CONVERSATION_TYPES: [ResourceType; 1] = [ResourceType {
    container: Some("conversations.inbox"),
    grants: &[campbooks_kernel::authz::Grant::new(
        campbooks_kernel::authz::Grantee::ContainerViewers,
        &[campbooks_kernel::authz::Action::Read],
    )],
    owner: Owner::Nobody,
    ..ResourceType::new(
        "conversations.conversation",
        "A test conversation.",
        Tenancy::Workspace,
    )
}];

#[derive(Debug)]
struct ConversationFacts;

impl ResourceFacts for ConversationFacts {
    fn facts<'a>(
        &'a self,
        _conn: &'a mut PgConnection,
        _access: &'a Access,
        resource: &'a RecordRef,
    ) -> FactsFuture<'a> {
        let found = (resource.record_type == "conversations.conversation").then(|| {
            Facts::owned_by(OwnerFact::Nobody)
                .within(RecordRef::new("conversations.inbox", INBOX), None)
        });
        Box::pin(async move { Ok(found) })
    }
}

/// A block the stand-in holds port made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// Its event id.
    pub event: Uuid,
    /// Whose calendar.
    pub member: UserId,
    /// The calendar (`None`: the member's Campbooks calendar).
    pub calendar: Option<Uuid>,
    /// The task it is for.
    pub task: RecordRef,
    /// What it is called.
    pub title: String,
    /// When.
    pub starts: DateTime<Utc>,
    /// Until.
    pub ends: DateTime<Utc>,
    /// Whether it was taken off.
    pub released: bool,
}

/// The calendar id the stand-in answers for a member's Campbooks calendar.
pub const CAMPBOOKS_CALENDAR: Uuid = Uuid::from_u128(0x0192_f0c4_7d4e_7b39_a7f1_3a8f_0e2b_9e01);

/// A calendar of another member: holding there is not found.
pub const SOMEONE_ELSES_CALENDAR: Uuid = Uuid::from_u128(0x0192_f0c4_7d4e_7b39_a7f1_3a8f_0e2b_9e02);

/// The schedule module's `Holds` port, in memory.
#[derive(Debug, Default)]
pub struct TestHolds {
    blocks: Mutex<Vec<Block>>,
}

impl TestHolds {
    /// Every block made, oldest first.
    pub fn blocks(&self) -> Vec<Block> {
        self.blocks.lock().unwrap().clone()
    }
}

impl Holds for TestHolds {
    fn hold<'a>(
        &'a self,
        _state: &'a AppState,
        _tx: &'a mut Transaction<'static, Postgres>,
        _workspace: WorkspaceId,
        request: HoldRequest<'a>,
    ) -> HoldFuture<'a> {
        let answer = if request.calendar == Some(SOMEONE_ELSES_CALENDAR) {
            Err(Error::NotFound)
        } else {
            let event = Uuid::now_v7();
            self.blocks.lock().unwrap().push(Block {
                event,
                member: request.member,
                calendar: request.calendar,
                task: request.task.clone(),
                title: request.title.to_owned(),
                starts: request.starts,
                ends: request.ends,
                released: false,
            });
            Ok(Held {
                event,
                calendar: request.calendar.unwrap_or(CAMPBOOKS_CALENDAR),
                at_provider: request.calendar.is_some(),
            })
        };
        Box::pin(async move { answer })
    }

    fn release<'a>(
        &'a self,
        _state: &'a AppState,
        _tx: &'a mut Transaction<'static, Postgres>,
        _workspace: WorkspaceId,
        member: UserId,
        event: Uuid,
    ) -> ReleaseFuture<'a> {
        for block in self.blocks.lock().unwrap().iter_mut() {
            if block.event == event && block.member == member {
                block.released = true;
            }
        }
        Box::pin(async { Ok(()) })
    }
}

/// The module over a fresh database.
pub struct Kit {
    /// The test database as its owner (no row-level security).
    pub db: PgPool,
    /// The state jobs and handlers run with (runtime role).
    pub state: AppState,
    /// The blocks held on members' calendars.
    pub holds: Arc<TestHolds>,
    app: Router,
    idp: TestIdp,
    membership: Arc<TestMembership>,
    inboxes: Arc<TestInboxes>,
}

impl Kit {
    /// The module over `db`.
    pub async fn new(db: PgPool) -> Self {
        let idp = TestIdp::start().await;
        let pool = testing::app_role_pool(&db).await;
        let membership = Arc::new(TestMembership::default());
        let inboxes = Arc::new(TestInboxes::default());
        let module = crate::module();
        let catalogue = Catalogue::new([
            &module.resources,
            &Resources::new(&CONVERSATION_TYPES, Arc::new(ConversationFacts)),
        ])
        .unwrap();
        let blobs = BlobStore::memory(&BlobsConfig::default(), pool.clone());
        let holds = Arc::new(TestHolds::default());
        let state = idp
            .state(pool)
            .with_blobs(blobs)
            .with_holds(holds.clone())
            .with_membership(membership.clone())
            .with_visibility(Arc::new(Visibilities::new(vec![
                inboxes.clone(),
                crate::visibility(),
            ])))
            .with_registry(Registry::build(&[module]).unwrap())
            .with_resources(catalogue);
        let app = testing::app(crate::module(), state.clone());
        Self {
            db,
            state,
            holds,
            app,
            idp,
            membership,
            inboxes,
        }
    }

    /// The same kit, writing tracked events to the outbox (ADR 0020), read
    /// back with [`testing::tracked_events`].
    #[must_use]
    pub fn tracking(mut self) -> Self {
        self.state = testing::tracking_outbox(self.state);
        self.app = testing::app(crate::module(), self.state.clone());
        self
    }

    /// Invoke `operation` with `input` (path parameters and body) as Scout,
    /// for the already-registered member `sub`.
    pub async fn as_assistant(
        &self,
        sub: &str,
        workspace: WorkspaceId,
        (operation, input): (&str, serde_json::Value),
    ) -> Result<operations::Output, operations::InvokeError> {
        let composed = operations::compose(crate::module().router, self.state.clone());
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
            sign_in: None,
        };
        let invocation =
            operations::Invocation::new(operations::Caller::Assistant, principal, operation, input)
                .in_workspace(workspace.to_string())
                .idempotency_key(sqlx::types::Uuid::now_v7().to_string());
        operations::invoke(&composed.state, invocation).await
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

    /// A token for the already-registered member `sub` with only `scopes`.
    pub fn token_with(&self, sub: &str, scopes: &str) -> String {
        let mut claims = self.idp.claims(scopes);
        claims["sub"] = sub.into();
        self.idp.token(&claims)
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

    /// Let `user` see inbox `inbox` (and so its conversations).
    pub fn let_see(&self, inbox: &str, user: UserId) {
        self.inboxes
            .members
            .lock()
            .unwrap()
            .insert((inbox.to_owned(), user));
    }

    /// Publish `event` in `workspace` (as its module would) and run the jobs
    /// it causes.
    pub async fn publish<E: DomainEvent>(&self, workspace: WorkspaceId, event: &E) -> WorkerStats {
        let mut tx = self.state.db.begin_for(workspace).await.unwrap();
        self.state.events.publish(&mut tx, event).await.unwrap();
        tx.commit().await.unwrap();
        self.run_jobs().await
    }

    /// Run every pending job until none is left.
    pub async fn run_jobs(&self) -> WorkerStats {
        let stats = Worker::new(self.state.clone()).run_until_idle().await;
        assert_eq!(stats.failed, 0, "{stats:?}");
        stats
    }

    /// Events of type `name` in the log, oldest first, as JSON.
    pub async fn events(&self, name: &str) -> Vec<serde_json::Value> {
        sqlx::query_scalar("SELECT payload FROM kernel.events WHERE name = $1 ORDER BY id")
            .bind(name)
            .fetch_all(&self.db)
            .await
            .unwrap()
    }

    /// How many tasks the workspace holds.
    pub async fn task_count(&self, workspace: WorkspaceId) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM tasks.tasks WHERE workspace_id = $1")
            .bind(workspace.as_uuid())
            .fetch_one(&self.db)
            .await
            .unwrap()
    }

    /// Make a task as `token` and return its JSON.
    pub async fn add(
        &self,
        token: &str,
        workspace: WorkspaceId,
        body: serde_json::Value,
    ) -> serde_json::Value {
        let made = self
            .post(token, &format!("/api/v1/w/{workspace}/tasks"), Some(body))
            .await;
        assert_eq!(
            made.status,
            axum::http::StatusCode::CREATED,
            "{:?}",
            made.json
        );
        made.json
    }

    /// A workspace's transaction on the runtime role.
    pub async fn begin(
        &self,
        workspace: WorkspaceId,
    ) -> sqlx::Transaction<'static, sqlx::Postgres> {
        self.state.db.begin_for(workspace).await.unwrap()
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

    /// `PATCH uri` with a JSON body.
    pub async fn patch(&self, token: &str, uri: &str, body: serde_json::Value) -> TestResponse {
        self.call(Method::PATCH, token, uri, Some(body)).await
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
}
