//! Test fixtures: the module over a database whose connections run as the
//! runtime role (row-level security applies), a test identity provider, a
//! stand-in for the membership port, and a stand-in for another module's
//! records (a task each member owns) to mention.

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
    AppState, Error, RecordRef,
    auth::{Principal, PrincipalKind},
    authz::{
        Catalogue, Facts, FactsFuture, Owner, OwnerFact, ResourceFacts, ResourceType, Resources,
        Tenancy,
    },
    blobs::BlobStore,
    changes::Visibilities,
    config::BlobsConfig,
    db::BeginFor as _,
    jobs::{Registry, Worker, WorkerStats},
    tenancy::{Access, Membership, MembershipFuture, Role, RoleFuture, UserId, WorkspaceId},
    testing::{self, TestIdp, TestResponse},
};
use sqlx::{PgConnection, PgPool};

/// Every scope of this module.
pub const ALL_SCOPES: &str = "pages:read pages:write";

#[derive(Debug, Default)]
struct TestMembership {
    members: Mutex<HashMap<(String, WorkspaceId), (UserId, Role)>>,
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
                .map(|(user, role)| Access {
                    workspace,
                    user: Some(user),
                    role: Some(role),
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
        let held = self.members.lock().ok().and_then(|m| {
            m.iter()
                .find(|((_, w), (u, _))| *w == workspace && *u == user)
                .map(|(_, (_, role))| *role)
        });
        Box::pin(async move { Ok::<_, Error>(held) })
    }
}

/// A task, as the tasks module would declare it: its member's alone.
static TASK_TYPES: [ResourceType; 1] = [ResourceType {
    owner: Owner::Member,
    owner_actions: &[campbooks_kernel::authz::Action::Read],
    ..ResourceType::new("tasks.task", "A test task.", Tenancy::Workspace)
}];

#[derive(Debug, Default)]
struct TaskFacts {
    owners: Mutex<HashMap<String, UserId>>,
}

impl ResourceFacts for TaskFacts {
    fn facts<'a>(
        &'a self,
        _conn: &'a mut PgConnection,
        _access: &'a Access,
        resource: &'a RecordRef,
    ) -> FactsFuture<'a> {
        let owner = self
            .owners
            .lock()
            .ok()
            .and_then(|owners| owners.get(&resource.id).copied());
        let found = (resource.record_type == "tasks.task")
            .then_some(owner)
            .flatten()
            .map(|owner| Facts::owned_by(OwnerFact::User(owner)));
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
    tasks: Arc<TaskFacts>,
}

impl Kit {
    /// The module over `db`.
    pub async fn new(db: PgPool) -> Self {
        let idp = TestIdp::start().await;
        let pool = testing::app_role_pool(&db).await;
        let membership = Arc::new(TestMembership::default());
        let tasks = Arc::new(TaskFacts::default());
        let module = crate::module(crate::Deps::default());
        let catalogue = Catalogue::new([
            &module.resources,
            &Resources::new(&TASK_TYPES, tasks.clone()),
        ])
        .unwrap();
        let blobs = BlobStore::memory(&BlobsConfig::default(), pool.clone());
        let state = idp
            .state(pool)
            .with_blobs(blobs)
            .with_membership(membership.clone())
            .with_visibility(Arc::new(Visibilities::new(vec![crate::visibility()])))
            .with_search_sources(campbooks_kernel::search::SearchSources::new(vec![
                crate::search_source(),
            ]))
            .with_registry(Registry::build(&[module]).unwrap())
            .with_resources(catalogue);
        let app = testing::app(crate::module(crate::Deps::default()), state.clone());
        Self {
            db,
            state,
            app,
            idp,
            membership,
            tasks,
        }
    }

    /// The same kit with pages switched off.
    #[must_use]
    pub fn switched_off(mut self) -> Self {
        self.app = testing::app(
            crate::module(crate::Deps { enabled: false }),
            self.state.clone(),
        );
        self
    }

    /// A user `sub`, member of `workspace`, with every scope of the module.
    pub fn member(&self, sub: &str, workspace: WorkspaceId) -> (String, UserId) {
        self.member_as(sub, workspace, Role::Member)
    }

    /// A user `sub`, owner of `workspace`.
    pub fn owner(&self, sub: &str, workspace: WorkspaceId) -> (String, UserId) {
        self.member_as(sub, workspace, Role::Owner)
    }

    fn member_as(&self, sub: &str, workspace: WorkspaceId, role: Role) -> (String, UserId) {
        let user = UserId::new();
        self.membership
            .members
            .lock()
            .unwrap()
            .insert((sub.to_owned(), workspace), (user, role));
        let mut claims = self.idp.claims(ALL_SCOPES);
        claims["sub"] = sub.into();
        (self.idp.token(&claims), user)
    }

    /// `sub` is no longer a member of `workspace`.
    pub fn remove_member(&self, sub: &str, workspace: WorkspaceId) {
        self.membership
            .members
            .lock()
            .unwrap()
            .remove(&(sub.to_owned(), workspace));
    }

    /// A token for the already-registered member `sub` with only `scopes`.
    pub fn token_with(&self, sub: &str, scopes: &str) -> String {
        let mut claims = self.idp.claims(scopes);
        claims["sub"] = sub.into();
        self.idp.token(&claims)
    }

    /// A client token bound to `workspace`, acting for nobody.
    pub fn client(&self, workspace: WorkspaceId, scopes: &str) -> String {
        let mut claims = self.idp.claims(scopes);
        let object = claims.as_object_mut().unwrap();
        object.remove("sub");
        object.insert("azp".into(), "acme-sync".into());
        object.insert("workspace_id".into(), workspace.to_string().into());
        self.idp.token(&claims)
    }

    /// A task that `owner` alone may read, to mention.
    pub fn task_of(&self, owner: UserId) -> String {
        let id = sqlx::types::Uuid::now_v7().to_string();
        self.tasks.owners.lock().unwrap().insert(id.clone(), owner);
        id
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

    /// A workspace's transaction on the runtime role.
    pub async fn begin(
        &self,
        workspace: WorkspaceId,
    ) -> sqlx::Transaction<'static, sqlx::Postgres> {
        self.state.db.begin_for(workspace).await.unwrap()
    }

    /// How many pages the workspace holds.
    pub async fn page_count(&self, workspace: WorkspaceId) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM pages.pages WHERE workspace_id = $1")
            .bind(workspace.as_uuid())
            .fetch_one(&self.db)
            .await
            .unwrap()
    }

    /// The path of the module's pages.
    pub fn pages(workspace: WorkspaceId) -> String {
        format!("/api/v1/w/{workspace}/pages")
    }

    /// Make a page as `token` and return its JSON.
    pub async fn add(
        &self,
        token: &str,
        workspace: WorkspaceId,
        body: serde_json::Value,
    ) -> serde_json::Value {
        let made = self.post(token, &Self::pages(workspace), Some(body)).await;
        assert_eq!(
            made.status,
            axum::http::StatusCode::CREATED,
            "{:?}",
            made.json
        );
        made.json
    }

    /// `POST …/sync`: send `update` (if any) and what the client holds.
    pub async fn sync(
        &self,
        token: &str,
        workspace: WorkspaceId,
        page: &str,
        client: &yrs::Doc,
        update: Option<&[u8]>,
    ) -> TestResponse {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        let vector = docs::vector(client);
        let mut body = serde_json::json!({"state_vector": STANDARD.encode(vector)});
        if let Some(update) = update {
            body["update"] = STANDARD.encode(update).into();
        }
        self.post(
            token,
            &format!("{}/{page}/sync", Self::pages(workspace)),
            Some(body),
        )
        .await
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

    /// `PUT uri` with a JSON body.
    pub async fn put(&self, token: &str, uri: &str, body: serde_json::Value) -> TestResponse {
        self.call(Method::PUT, token, uri, Some(body)).await
    }

    /// `DELETE uri`.
    pub async fn delete(&self, token: &str, uri: &str) -> TestResponse {
        self.call(Method::DELETE, token, uri, None).await
    }

    /// `POST uri` with raw bytes, as an upload.
    pub async fn upload(
        &self,
        token: &str,
        uri: &str,
        content_type: &str,
        bytes: Vec<u8>,
    ) -> TestResponse {
        let request = Request::builder()
            .method(Method::POST)
            .uri(uri)
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, content_type)
            .body(Body::from(bytes))
            .unwrap();
        testing::send(&self.app, request).await
    }

    /// `GET uri` with `token`, the raw response (status, headers, bytes).
    pub async fn download(
        &self,
        token: &str,
        uri: &str,
    ) -> (axum::http::StatusCode, axum::http::HeaderMap, Vec<u8>) {
        use http_body_util::BodyExt as _;
        use tower::ServiceExt as _;
        let request = Request::builder()
            .method(Method::GET)
            .uri(uri)
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap();
        let response = self.app.clone().oneshot(request).await.unwrap();
        let (parts, body) = response.into_parts();
        let bytes = body.collect().await.unwrap().to_bytes();
        (parts.status, parts.headers, bytes.to_vec())
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

/// Yjs documents as clients make them, for tests.
pub mod docs {
    use std::sync::Arc;

    use yrs::{
        Any, Doc, ReadTxn, Transact, TransactionMut, Xml, XmlElementPrelim, XmlFragment,
        XmlFragmentRef, XmlTextPrelim,
        updates::{decoder::Decode, encoder::Encode},
    };

    /// A text value.
    pub fn s(text: &str) -> Any {
        Any::String(Arc::from(text))
    }

    /// A client's document, with its own client id.
    pub fn client(id: u64) -> Doc {
        Doc::with_client_id(id)
    }

    /// Make `change` to the client's fragment in one transaction, and get
    /// the update it made.
    pub fn edit(
        doc: &Doc,
        change: impl FnOnce(&mut TransactionMut<'_>, &XmlFragmentRef),
    ) -> Vec<u8> {
        let fragment = doc.get_or_insert_xml_fragment("default");
        let mut txn = doc.transact_mut();
        change(&mut txn, &fragment);
        txn.encode_update_v1()
    }

    /// A paragraph with `text`, as an update.
    pub fn paragraph(doc: &Doc, text: &str) -> Vec<u8> {
        edit(doc, |txn, fragment| {
            let line = fragment.push_back(txn, XmlElementPrelim::empty("paragraph"));
            line.push_back(txn, XmlTextPrelim::new(text));
        })
    }

    /// An element `tag` with `attributes` (and `text` inside), as an update.
    pub fn element(
        doc: &Doc,
        tag: &str,
        attributes: &[(&str, Any)],
        text: Option<&str>,
    ) -> Vec<u8> {
        edit(doc, |txn, fragment| {
            let node = fragment.push_back(txn, XmlElementPrelim::empty(tag));
            for (key, value) in attributes {
                node.insert_attribute(txn, *key, value.clone());
            }
            if let Some(text) = text {
                node.push_back(txn, XmlTextPrelim::new(text));
            }
        })
    }

    /// The client's state vector (v1).
    pub fn vector(doc: &Doc) -> Vec<u8> {
        doc.transact().state_vector().encode_v1()
    }

    /// Apply a server's `update` to the client's document.
    pub fn apply(doc: &Doc, update: &[u8]) {
        let update = yrs::Update::decode_v1(update).unwrap();
        doc.transact_mut().apply_update(update).unwrap();
    }

    /// The text of the client's paragraphs, in order.
    pub fn text(doc: &Doc) -> String {
        crate::domain::document::inspect(doc).unwrap().text
    }
}
