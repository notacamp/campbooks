//! Test fixtures: the module over a database whose connections run as the
//! runtime role (row-level security applies), a test identity provider,
//! stand-ins for the membership port and for the conversations module's
//! access facts (a conversation lives in an inbox that some members see).

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
    AppState, Error, RecordRef,
    auth::{Principal, PrincipalKind},
    authz::{
        Catalogue, Facts, FactsFuture, Owner, OwnerFact, ResourceFacts, ResourceType, Resources,
        Tenancy,
    },
    changes::{Visibilities, Visibility, VisibilityFuture},
    db::BeginFor as _,
    jobs::{Registry, Worker, WorkerStats},
    tenancy::{Access, Membership, MembershipFuture, Role, RoleFuture, UserId, WorkspaceId},
    testing::{self, TestIdp, TestResponse},
};
use sqlx::{PgConnection, PgPool};

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

/// The module over a fresh database.
pub struct Kit {
    /// The test database as its owner (no row-level security).
    pub db: PgPool,
    /// The state jobs and handlers run with (runtime role).
    pub state: AppState,
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
        let state = idp
            .state(pool)
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
            app,
            idp,
            membership,
            inboxes,
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
