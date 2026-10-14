//! The assistant end to end: the composed API with the real workspaces and
//! conversations modules, recorded fictional mail, a real Postgres
//! (`#[sqlx::test]`), an in-process identity provider and, for compiling,
//! a stand-in language model.
//!
//! - "Do it and tell me" reaches the conversations module through the
//!   `MemberPreferences` port: the next rule Scout proposes from the
//!   member's corrections in their own inbox starts active.
//! - Compiling reads the member's own context through the conversations
//!   operations as the member: a `who` question's candidates never come
//!   from an inbox they can't read, and nothing from anyone's mail reaches
//!   the model.

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
};
use campbooks_ai::{AiConfig, Gateway, ProviderConfig, Role};
use campbooks_conversations::{dev, model::InboxRole};
use campbooks_kernel::{
    AppState,
    blobs::BlobStore,
    config::BlobsConfig,
    jobs::Worker,
    operations,
    tenancy::{UserId, WorkspaceId},
    testing::{self, TestIdp},
};
use http_body_util::BodyExt as _;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt as _;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

use crate::Composition;

const SCOPES: &str = "workspaces:read workspaces:write conversations:read conversations:write \
                      assistant:read assistant:write";

struct Server {
    app: Router,
    state: AppState,
    idp: TestIdp,
}

impl Server {
    async fn new(db: &PgPool, composition: &Composition) -> Self {
        let idp = TestIdp::start().await;
        let mut config = testing::config("postgres://test@localhost/test");
        config.auth = idp.auth_config();
        let pool = testing::app_role_pool(db).await;
        let state = crate::with_ports(
            AppState::new(pool.clone(), config)
                .with_blobs(BlobStore::memory(&BlobsConfig::default(), pool))
                .with_registry(crate::registry_with(composition).expect("valid registrations")),
        );
        let operations::Composed { routes, state, .. } =
            operations::compose(crate::api_with(composition), state);
        let app = routes
            .fallback(|| async { campbooks_kernel::Error::NotFound })
            .with_state(state.clone());
        Self { app, state, idp }
    }

    fn token(&self, sub: &str) -> String {
        let mut claims = self.idp.claims(SCOPES);
        claims["sub"] = sub.into();
        self.idp.token(&claims)
    }

    async fn call(
        &self,
        token: &str,
        method: &str,
        uri: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut request = Request::builder()
            .method(method)
            .uri(uri)
            .header(header::AUTHORIZATION, format!("Bearer {token}"));
        let body = match body {
            Some(body) => {
                request = request.header(header::CONTENT_TYPE, "application/json");
                Body::from(body.to_string())
            }
            None => Body::empty(),
        };
        let response = self
            .app
            .clone()
            .oneshot(request.body(body).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    async fn ingest(
        &self,
        workspace: WorkspaceId,
        inbox: campbooks_conversations::model::InboxId,
        json: &str,
    ) {
        let recorded: Value = serde_json::from_str(json).unwrap();
        let batch = dev::record(&self.state, workspace, inbox, &recorded)
            .await
            .unwrap();
        dev::submit(&self.state, workspace, &batch).await.unwrap();
        let stats = Worker::new(self.state.clone()).run_until_idle().await;
        assert_eq!(stats.failed, 0, "{stats:?}");
    }

    /// A workspace created by `sub`, and their user id.
    async fn workspace(&self, db: &PgPool, sub: &str) -> (WorkspaceId, UserId) {
        let token = self.token(sub);
        let (status, created) = self
            .call(
                &token,
                "POST",
                "/api/v1/workspaces",
                Some(json!({"name": "Atelier Sousa", "kind": "business"})),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{created}");
        let workspace = created["id"].as_str().unwrap().parse().unwrap();
        (workspace, user(db, sub).await)
    }
}

async fn user(db: &PgPool, sub: &str) -> UserId {
    sqlx::query_scalar::<_, uuid::Uuid>("SELECT id FROM workspaces.users WHERE idp_subject = $1")
        .bind(sub)
        .fetch_one(db)
        .await
        .map(UserId::from_uuid)
        .unwrap()
}

/// `sub`, a new user, made a member of `workspace`.
async fn add_member(db: &PgPool, workspace: WorkspaceId, sub: &str) -> UserId {
    let user = UserId::new();
    sqlx::query("INSERT INTO workspaces.users (id, idp_subject) VALUES ($1, $2)")
        .bind(user.as_uuid())
        .bind(sub)
        .execute(db)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO workspaces.members (workspace_id, user_id, role) VALUES ($1, $2, 'member')",
    )
    .bind(workspace.as_uuid())
    .bind(user.as_uuid())
    .execute(db)
    .await
    .unwrap();
    user
}

const EN: &str = include_str!("../../modules/conversations/fixtures/en.json");

fn suggestion() -> Value {
    json!({
        "conditions": [{ "field": "sender_address", "value": "hello@letters.paperkite.example" }],
        "actions": [{ "action": "move_to_stream", "stream": "newsletters" }],
        "origin": "suggestion",
        "state": "proposed",
        "apply_to_existing": false,
    })
}

#[sqlx::test(migrations = "../../migrations")]
async fn do_it_and_tell_me_starts_scouts_next_proposal(db: PgPool) {
    let server = Server::new(&db, &Composition::default()).await;
    let (workspace, ines_id) = server.workspace(&db, "ines").await;
    let ines = server.token("ines");
    let inbox = dev::create_inbox(
        &server.state,
        workspace,
        "ines@example.com",
        &[(ines_id, InboxRole::Manager)],
    )
    .await
    .unwrap();
    server.ingest(workspace, inbox, EN).await;
    let rules = format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules");

    // Ask first (the default): Scout's proposal waits for a yes.
    let (status, waiting) = server
        .call(&ines, "POST", &rules, Some(suggestion()))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{waiting}");
    assert_eq!(waiting["rule"]["state"], "proposed");
    assert_eq!(waiting["acted_without_asking"], false);
    let (_, _) = server
        .call(
            &ines,
            "DELETE",
            &format!(
                "/api/v1/w/{workspace}/rules/{}",
                waiting["rule"]["id"].as_str().unwrap()
            ),
            None,
        )
        .await;

    // "Do it and tell me": the next one starts, and says so for the Undo.
    let (status, profile) = server
        .call(
            &ines,
            "PUT",
            &format!("/api/v1/w/{workspace}/assistant/profile"),
            Some(json!({ "proactivity": "act_then_tell", "asked": true })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{profile}");
    let (status, started) = server
        .call(&ines, "POST", &rules, Some(suggestion()))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{started}");
    assert_eq!(started["rule"]["state"], "active");
    assert_eq!(started["acted_without_asking"], true);
}

#[sqlx::test(migrations = "../../migrations")]
async fn who_candidates_come_only_from_the_members_own_inboxes(db: PgPool) {
    let model = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": "test-model",
            "choices": [{
                "finish_reason": "stop",
                "message": { "content": json!({
                    "outcome": "rules",
                    "unsupported": "none",
                    "rules": [{
                        "who": "my studio contact",
                        "conditions": [],
                        "actions": [{ "kind": "surface", "value": "always" }],
                    }],
                }).to_string() },
            }],
        })))
        .mount(&model)
        .await;
    let gateway = Gateway::new(AiConfig {
        roles: [(
            Role::Complete,
            ProviderConfig::new(
                format!("{}/v1", model.uri()).parse().unwrap(),
                "test-model",
            ),
        )]
        .into(),
        ..AiConfig::default()
    })
    .unwrap();
    let composition = Composition {
        assistant: campbooks_assistant::AssistantDeps { ai: Some(gateway) },
        ..Composition::default()
    };
    let server = Server::new(&db, &composition).await;
    let (workspace, sam_id) = server.workspace(&db, "sam").await;
    let sams = dev::create_inbox(
        &server.state,
        workspace,
        "sam@example.com",
        &[(sam_id, InboxRole::Manager)],
    )
    .await
    .unwrap();
    server.ingest(workspace, sams, EN).await;
    let ana_id = add_member(&db, workspace, "ana").await;
    dev::create_inbox(
        &server.state,
        workspace,
        "ana@example.com",
        &[(ana_id, InboxRole::Manager)],
    )
    .await
    .unwrap();
    let uri = format!("/api/v1/w/{workspace}/assistant/compile");
    let body = json!({ "text": "anything from my studio contact always matters" });

    // Sam reads the mail from Northwind Studio: Maya is a candidate.
    let (status, sams_answer) = server
        .call(&server.token("sam"), "POST", &uri, Some(body.clone()))
        .await;
    assert_eq!(status, StatusCode::OK, "{sams_answer}");
    let who = &sams_answer["proposals"][0]["who"];
    assert_eq!(who["matched"], true);
    assert_eq!(
        who["candidates"][0]["address"],
        "maya@northwind-studio.example"
    );

    // Ana can't read Sam's inbox: none of its senders is offered.
    let (status, anas_answer) = server
        .call(&server.token("ana"), "POST", &uri, Some(body))
        .await;
    assert_eq!(status, StatusCode::OK, "{anas_answer}");
    assert!(
        !anas_answer.to_string().contains("northwind"),
        "{anas_answer}"
    );
    assert_eq!(
        anas_answer["proposals"][0]["inboxes"][0]["address"],
        "ana@example.com"
    );

    // What reached the model: the sentence and the caller's own data only.
    for request in model.received_requests().await.unwrap() {
        let sent = String::from_utf8_lossy(&request.body).to_string();
        assert!(!sent.contains("northwind"), "{sent}");
        assert!(!sent.contains("Q4 plan"), "{sent}");
    }
}
