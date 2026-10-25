//! Test fixtures: the module over a database whose connections run as the
//! runtime role (row-level security applies), a test identity provider, a
//! stand-in for the membership port, blobs in memory, and a way to publish
//! the other modules' events this module consumes.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use axum::{
    Router,
    body::Body,
    http::{Method, Request, header},
};
use campbooks_kernel::{
    AppState, DomainEvent, Error,
    auth::{Principal, PrincipalKind},
    authz::Catalogue,
    blobs::{BlobRef, BlobStore},
    config::BlobsConfig,
    db::BeginFor as _,
    jobs::{Registry, Worker, WorkerStats},
    tenancy::{Access, Membership, MembershipFuture, Role, UserId, WorkspaceId},
    testing::{self, TestIdp, TestResponse},
};
use sqlx::PgPool;

/// Every scope of this module.
pub const ALL_SCOPES: &str = "schedule:read schedule:write";

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
    /// The test database as its owner (no row-level security).
    pub db: PgPool,
    /// The state jobs and handlers run with (runtime role).
    pub state: AppState,
    app: Router,
    idp: TestIdp,
    membership: Arc<TestMembership>,
}

impl Kit {
    /// The module over `db`.
    pub async fn new(db: PgPool) -> Self {
        let idp = TestIdp::start().await;
        let pool = testing::app_role_pool(&db).await;
        let membership = Arc::new(TestMembership::default());
        let module = crate::module();
        let catalogue = Catalogue::new([&module.resources]).unwrap();
        let blobs = BlobStore::memory(&BlobsConfig::default(), pool.clone());
        let state = idp
            .state(pool)
            .with_blobs(blobs)
            .with_membership(membership.clone())
            .with_registry(Registry::build(&[module]).unwrap())
            .with_resources(catalogue);
        let app = testing::app(crate::module(), state.clone());
        Self {
            db,
            state,
            app,
            idp,
            membership,
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

    /// Store `document` as a JSON blob of `workspace`.
    pub async fn blob(&self, workspace: WorkspaceId, document: &serde_json::Value) -> BlobRef {
        self.state
            .blobs
            .put(
                &workspace.to_string(),
                serde_json::to_vec(document).unwrap(),
                "application/json",
            )
            .await
            .unwrap()
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
}
