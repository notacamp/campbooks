//! Test fixtures: the module over a database whose connections run as the
//! runtime role (row-level security applies), a test identity provider, a
//! stand-in for the workspaces module's membership port, and stand-ins for
//! the conversations operations compile invokes (the same ids, paths and
//! scopes), composed the way the server composes modules.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use axum::{
    Extension, Json, Router,
    body::Body,
    http::{Method, Request, header},
};
use campbooks_ai::{AiConfig, ExternalContent, Gateway, ProviderConfig, Role};
use campbooks_kernel::{
    AppState, Error, Operation,
    auth::{Principal, PrincipalKind, ScopeName},
    extract,
    jobs::{Registry, Worker, WorkerStats},
    operations::{Composed, compose},
    tenancy::{
        Access, Membership, MembershipFuture, Role as WorkspaceRole, UserId, Workspace, WorkspaceId,
    },
    testing::{self, TestIdp, TestResponse},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use utoipa_axum::routes;

use crate::AssistantDeps;

/// Every scope a member's token carries here.
pub const ALL_SCOPES: &str =
    "assistant:read assistant:write conversations:read search:read attention:read";

#[derive(Debug, Default)]
struct TestMembership {
    members: Mutex<HashMap<(String, WorkspaceId), (UserId, WorkspaceRole)>>,
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
}

/// What the stand-in conversations operations answer, and what they were
/// asked.
#[derive(Debug, Default)]
pub struct Mail {
    /// `conversations.list_inboxes`'s `inboxes`.
    pub inboxes: Vec<Value>,
    /// `conversations.list_labels`'s `labels`.
    pub labels: Vec<Value>,
    /// `conversations.list_senders`'s `senders`, filtered by `q` on the
    /// address and name.
    pub senders: Vec<Value>,
    /// Every `q` asked of `list_senders`, in order (`""` for none).
    pub asked: Vec<String>,
    /// `conversations.get`'s answers, by conversation id; any other id is
    /// 404.
    pub conversations: HashMap<String, Value>,
    /// `conversations.get_history`'s answers, by conversation id.
    pub histories: HashMap<String, Value>,
    /// `conversations.list`'s `conversations` (filtered by `ids` when asked).
    pub list: Vec<Value>,
    /// `search.query`'s `hits`.
    pub hits: Vec<Value>,
    /// `attention.list`'s `items`.
    pub items: Vec<Value>,
    /// `conversations.list_streams`'s `streams`.
    pub streams: Vec<Value>,
    /// Every stand-in operation called, in order.
    pub called: Vec<String>,
}

struct ConversationsRead;

impl ScopeName for ConversationsRead {
    const NAME: &'static str = "conversations:read";
    const DESCRIPTION: &'static str = "Read conversations (stand-in).";
}

struct SearchRead;

impl ScopeName for SearchRead {
    const NAME: &'static str = "search:read";
    const DESCRIPTION: &'static str = "Search (stand-in).";
}

struct AttentionRead;

impl ScopeName for AttentionRead {
    const NAME: &'static str = "attention:read";
    const DESCRIPTION: &'static str = "What needs you (stand-in).";
}

type Shared = Arc<Mutex<Mail>>;

#[utoipa::path(get, path = "/api/v1/w/{workspace_id}/conversations/{conversation_id}",
    operation_id = "conversations.get",
    params(("workspace_id" = String, Path), ("conversation_id" = String, Path)),
    security(("bearer" = ["conversations:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant().records(["conversations.conversation"])))),
    responses((status = OK, body = Value)))]
async fn get_conversation(
    _: Workspace<ConversationsRead>,
    Extension(mail): Extension<Shared>,
    extract::Path(path): extract::Path<(String, String)>,
) -> Result<Json<Value>, Error> {
    let mut mail = mail.lock().unwrap();
    mail.called.push("conversations.get".into());
    mail.conversations
        .get(&path.1)
        .cloned()
        .map(Json)
        .ok_or(Error::NotFound)
}

#[utoipa::path(get, path = "/api/v1/w/{workspace_id}/conversations/{conversation_id}/history",
    operation_id = "conversations.get_history",
    params(("workspace_id" = String, Path), ("conversation_id" = String, Path)),
    security(("bearer" = ["conversations:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant().records(["conversations.conversation"])))),
    responses((status = OK, body = Value)))]
async fn get_history(
    _: Workspace<ConversationsRead>,
    Extension(mail): Extension<Shared>,
    extract::Path(path): extract::Path<(String, String)>,
) -> Result<Json<Value>, Error> {
    let mut mail = mail.lock().unwrap();
    mail.called.push("conversations.get_history".into());
    if !mail.conversations.contains_key(&path.1) {
        return Err(Error::NotFound);
    }
    Ok(Json(mail.histories.get(&path.1).cloned().unwrap_or_else(
        || json!({ "events": [], "truncated": false }),
    )))
}

#[derive(Deserialize)]
struct ListQuery {
    ids: Option<String>,
}

#[utoipa::path(get, path = "/api/v1/w/{workspace_id}/conversations",
    operation_id = "conversations.list",
    params(("workspace_id" = String, Path), ("ids" = Option<String>, Query),
        ("stream" = Option<String>, Query), ("view" = Option<String>, Query),
        ("limit" = Option<u32>, Query)),
    security(("bearer" = ["conversations:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant().records(["conversations.conversation"])))),
    responses((status = OK, body = Value)))]
async fn list_conversations(
    _: Workspace<ConversationsRead>,
    Extension(mail): Extension<Shared>,
    extract::Query(query): extract::Query<ListQuery>,
) -> Json<Value> {
    let mut mail = mail.lock().unwrap();
    mail.called.push("conversations.list".into());
    let listed: Vec<Value> = match query.ids {
        Some(ids) => {
            let ids: Vec<&str> = ids.split(',').collect();
            mail.list
                .iter()
                .filter(|c| ids.iter().any(|id| c["id"] == *id))
                .cloned()
                .collect()
        }
        None => mail.list.clone(),
    };
    Json(json!({ "conversations": listed }))
}

#[utoipa::path(get, path = "/api/v1/w/{workspace_id}/streams",
    operation_id = "conversations.list_streams",
    params(("workspace_id" = String, Path)),
    security(("bearer" = ["conversations:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant().records(["conversations.inbox"])))),
    responses((status = OK, body = Value)))]
async fn list_streams(
    _: Workspace<ConversationsRead>,
    Extension(mail): Extension<Shared>,
) -> Json<Value> {
    let mut mail = mail.lock().unwrap();
    mail.called.push("conversations.list_streams".into());
    Json(json!({ "streams": mail.streams }))
}

#[utoipa::path(get, path = "/api/v1/w/{workspace_id}/search",
    operation_id = "search.query",
    params(("workspace_id" = String, Path), ("q" = Option<String>, Query),
        ("limit" = Option<u32>, Query), ("sort" = Option<String>, Query)),
    security(("bearer" = ["search:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant().records(["search.result"])))),
    responses((status = OK, body = Value)))]
async fn search(_: Workspace<SearchRead>, Extension(mail): Extension<Shared>) -> Json<Value> {
    let mut mail = mail.lock().unwrap();
    mail.called.push("search.query".into());
    Json(json!({ "hits": mail.hits, "indexing": false, "query": {} }))
}

#[utoipa::path(get, path = "/api/v1/w/{workspace_id}/attention",
    operation_id = "attention.list",
    params(("workspace_id" = String, Path), ("state" = Option<String>, Query),
        ("utc_offset" = Option<i32>, Query)),
    security(("bearer" = ["attention:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant().records(["attention.item"])))),
    responses((status = OK, body = Value)))]
async fn attention(_: Workspace<AttentionRead>, Extension(mail): Extension<Shared>) -> Json<Value> {
    let mut mail = mail.lock().unwrap();
    mail.called.push("attention.list".into());
    Json(json!({ "items": mail.items, "counts": [] }))
}

#[utoipa::path(get, path = "/api/v1/w/{workspace_id}/inboxes",
    operation_id = "conversations.list_inboxes",
    params(("workspace_id" = String, Path)),
    security(("bearer" = ["conversations:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant().records(["conversations.inbox"])))),
    responses((status = OK, body = Value)))]
async fn list_inboxes(
    _: Workspace<ConversationsRead>,
    Extension(mail): Extension<Shared>,
) -> Json<Value> {
    Json(json!({ "inboxes": mail.lock().unwrap().inboxes }))
}

#[utoipa::path(get, path = "/api/v1/w/{workspace_id}/labels",
    operation_id = "conversations.list_labels",
    params(("workspace_id" = String, Path)),
    security(("bearer" = ["conversations:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant().records(["conversations.inbox"])))),
    responses((status = OK, body = Value)))]
async fn list_labels(
    _: Workspace<ConversationsRead>,
    Extension(mail): Extension<Shared>,
) -> Json<Value> {
    Json(json!({ "labels": mail.lock().unwrap().labels }))
}

#[derive(Deserialize)]
struct SendersQuery {
    q: Option<String>,
}

#[utoipa::path(get, path = "/api/v1/w/{workspace_id}/senders",
    operation_id = "conversations.list_senders",
    params(("workspace_id" = String, Path), ("q" = Option<String>, Query), ("limit" = Option<u32>, Query)),
    security(("bearer" = ["conversations:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant().records(["conversations.inbox"])))),
    responses((status = OK, body = Value)))]
async fn list_senders(
    _: Workspace<ConversationsRead>,
    Extension(mail): Extension<Shared>,
    extract::Query(query): extract::Query<SendersQuery>,
) -> Json<Value> {
    let mut mail = mail.lock().unwrap();
    let q = query.q.unwrap_or_default().to_lowercase();
    mail.asked.push(q.clone());
    let senders: Vec<Value> = mail
        .senders
        .iter()
        .filter(|s| q.is_empty() || s.to_string().to_lowercase().contains(&q))
        .cloned()
        .collect();
    Json(json!({ "senders": senders }))
}

/// The module over a fresh database.
pub struct Kit {
    /// The test database as its owner (no row-level security).
    pub db: PgPool,
    /// The state jobs run with (runtime role).
    pub state: AppState,
    /// What the stand-in conversations operations answer.
    pub mail: Shared,
    app: Router,
    idp: TestIdp,
    membership: Arc<TestMembership>,
}

impl Kit {
    /// The module, with no AI, over `db`.
    pub async fn new(db: PgPool) -> Self {
        Self::with_deps(db, AssistantDeps::default()).await
    }

    /// The module composed with `deps` over `db`.
    pub async fn with_deps(db: PgPool, deps: AssistantDeps) -> Self {
        let idp = TestIdp::start().await;
        let pool = testing::app_role_pool(&db).await;
        let membership = Arc::new(TestMembership::default());
        let mail: Shared = Arc::default();
        let module = crate::module(deps);
        let registry = Registry::build(std::slice::from_ref(&module)).unwrap();
        let api = module
            .router
            .routes(routes!(list_inboxes))
            .routes(routes!(list_labels))
            .routes(routes!(list_senders))
            .routes(routes!(get_conversation))
            .routes(routes!(get_history))
            .routes(routes!(list_conversations))
            .routes(routes!(list_streams))
            .routes(routes!(search))
            .routes(routes!(attention))
            .layer(Extension(mail.clone()));
        let state = idp
            .state(pool)
            .with_membership(membership.clone())
            .with_registry(registry);
        let Composed { routes, state, .. } = compose(api, state);
        let app = routes.with_state(state.clone());
        Self {
            db,
            state,
            mail,
            app,
            idp,
            membership,
        }
    }

    /// A user `sub` of `workspace` with `role` and every scope here.
    pub fn member_as(
        &self,
        sub: &str,
        workspace: WorkspaceId,
        role: WorkspaceRole,
    ) -> (String, UserId) {
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

    /// A plain member.
    pub fn member(&self, sub: &str, workspace: WorkspaceId) -> (String, UserId) {
        self.member_as(sub, workspace, WorkspaceRole::Member)
    }

    /// A token for `sub` (already a member) with only `scopes`.
    pub fn token(&self, sub: &str, scopes: &str) -> String {
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

    /// `POST uri` with a JSON body.
    pub async fn post(&self, token: &str, uri: &str, body: Value) -> TestResponse {
        self.call(Method::POST, token, uri, Some(body)).await
    }

    /// `DELETE uri`.
    pub async fn delete(&self, token: &str, uri: &str) -> TestResponse {
        self.call(Method::DELETE, token, uri, None).await
    }

    /// `POST uri` with a JSON body, answered as it comes: its status, and
    /// its body (a stream, for Server-Sent Events).
    pub async fn post_streamed(
        &self,
        token: &str,
        uri: &str,
        body: Value,
    ) -> axum::response::Response {
        use tower::ServiceExt as _;
        let request = Request::builder()
            .method(Method::POST)
            .uri(uri)
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        match self.app.clone().oneshot(request).await {
            Ok(response) => response,
            Err(never) => match never {},
        }
    }

    /// `PUT uri` with a JSON body.
    pub async fn put(&self, token: &str, uri: &str, body: Value) -> TestResponse {
        self.call(Method::PUT, token, uri, Some(body)).await
    }

    async fn call(
        &self,
        method: Method,
        token: &str,
        uri: &str,
        body: Option<Value>,
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

/// The events of a streamed answer, as (event, data).
pub async fn frames(response: axum::response::Response) -> Vec<(String, Value)> {
    use http_body_util::BodyExt as _;
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec())
        .unwrap()
        .split("\n\n")
        .filter(|block| !block.trim().is_empty() && !block.starts_with(':'))
        .map(|block| {
            let mut name = String::new();
            let mut data = String::new();
            for line in block.lines() {
                if let Some(value) = line.strip_prefix("event: ") {
                    value.clone_into(&mut name);
                } else if let Some(value) = line.strip_prefix("data: ") {
                    data.push_str(value);
                }
            }
            (name, serde_json::from_str(&data).unwrap_or(Value::Null))
        })
        .collect()
}

/// A gateway whose `complete` role is the model server at `base_url` (on
/// this machine, so it may read content).
pub fn gateway(base_url: &str) -> Gateway {
    let provider = ProviderConfig::new(format!("{base_url}/v1").parse().unwrap(), "test-model");
    Gateway::new(AiConfig {
        roles: [(Role::Complete, provider)].into(),
        ..AiConfig::default()
    })
    .unwrap()
}

/// A gateway whose `complete` role is off this instance's network, without
/// the content opt-in.
pub fn external_gateway() -> Gateway {
    let provider = ProviderConfig::new("https://ai.example.com/v1".parse().unwrap(), "test-model")
        .with_external_content(ExternalContent::Deny);
    Gateway::new(AiConfig {
        roles: [(Role::Complete, provider)].into(),
        ..AiConfig::default()
    })
    .unwrap()
}
