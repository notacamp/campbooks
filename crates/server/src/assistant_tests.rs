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
//! - Scout acting for a member (`operations::invoke` as the assistant) only
//!   proposes rules: it never says yes to one, even with the member's
//!   content token, and the member's own yes still keeps it
//!   (campbooks-cloud#242). It may write a draft, but never send it nor take
//!   back a send, even with the member's content token and as a call marked
//!   confirmed: only the member's own Send and Undo do.

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
                      conversations:send assistant:read assistant:write";

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
                // Rules are bound to the member who saves them
                // (campbooks-cloud#242): the instance's key.
                .with_binding(testing::binding())
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
    let (status, waiting) = server.call(&ines, "POST", &rules, Some(suggestion())).await;
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
    let (status, started) = server.call(&ines, "POST", &rules, Some(suggestion())).await;
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
            ProviderConfig::new(format!("{}/v1", model.uri()).parse().unwrap(), "test-model"),
        )]
        .into(),
        ..AiConfig::default()
    })
    .unwrap();
    let composition = Composition {
        assistant: campbooks_assistant::AssistantDeps {
            ai: Some(gateway),
            ..campbooks_assistant::AssistantDeps::default()
        },
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

/// `sub` as the assistant acts for them: their scopes, no token.
fn on_behalf_of(sub: &str) -> campbooks_kernel::auth::Principal {
    campbooks_kernel::auth::Principal {
        subject: sub.to_owned(),
        kind: campbooks_kernel::auth::PrincipalKind::User,
        client_id: None,
        scopes: SCOPES
            .split_whitespace()
            .map(campbooks_kernel::auth::Scope::new)
            .collect(),
        workspace: None,
        email: Some(format!("{sub}@example.com")),
        email_verified: true,
        name: None,
        expires_at: None,
        issued_at: None,
        auth_time: None,
        sign_in: None,
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn scout_proposes_rules_but_only_the_member_keeps_them(db: PgPool) {
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
    let scout = |operation: &str, input: Value| {
        operations::Invocation::new(
            operations::Caller::Assistant,
            on_behalf_of("ines"),
            operation,
            input,
        )
        .in_workspace(workspace.to_string())
        .idempotency_key(uuid::Uuid::now_v7().to_string())
    };

    // Scout saves a rule, asking for it to act: it is only proposed.
    let mut rule = suggestion();
    rule["state"] = json!("active");
    rule["inbox_id"] = json!(inbox.to_string());
    let created = operations::invoke(&server.state, scout("conversations.create_rule", rule))
        .await
        .unwrap();
    assert_eq!(
        created.body["rule"]["state"], "proposed",
        "{}",
        created.body
    );
    let id = created.body["rule"]["id"].as_str().unwrap().to_owned();
    // It reads the proposal, with Ines's token for it, and says yes: never
    // run, even marked as confirmed.
    let read = operations::invoke(
        &server.state,
        scout("conversations.get_rule", json!({"rule_id": id})),
    )
    .await
    .unwrap();
    let token = read.body["content_token"].clone();
    assert!(token.is_string(), "{}", read.body);
    let yes = json!({"rule_id": id, "apply_to_existing": false, "content_token": token});
    for confirmed in [false, true] {
        let mut call = scout("conversations.confirm_rule", yes.clone());
        if confirmed {
            call = call.confirmed();
        }
        let refused = operations::invoke(&server.state, call).await;
        assert!(
            matches!(refused, Err(operations::InvokeError::NotAllowed { .. })),
            "{refused:?}"
        );
    }
    let uri = format!("/api/v1/w/{workspace}/rules/{id}");
    let (_, still) = server.call(&ines, "GET", &uri, None).await;
    assert_eq!(still["state"], "proposed");

    // Ines's own Keep, with the token her screen got: kept, and hers.
    let (_, shown) = server.call(&ines, "GET", &uri, None).await;
    let (status, kept) = server
        .call(
            &ines,
            "POST",
            &format!("{uri}/confirm"),
            Some(json!({"apply_to_existing": false, "content_token": shown["content_token"]})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{kept}");
    assert_eq!(kept["state"], "active");
    assert_eq!(kept["taught_by"], ines_id.to_string());
}

/// Draft `id`'s state and the MAC binding its author's request to send.
async fn held(db: &PgPool, id: uuid::Uuid) -> (String, Option<Vec<u8>>) {
    sqlx::query_as("SELECT state, request_mac FROM conversations.drafts WHERE id = $1")
        .bind(id)
        .fetch_one(db)
        .await
        .unwrap()
}

/// How many sends were asked of the connector.
async fn sends_requested(db: &PgPool) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM kernel.events WHERE name = 'conversations.send_requested'",
    )
    .fetch_one(db)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn scout_writes_drafts_but_only_the_member_sends_or_takes_one_back(db: PgPool) {
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
    let scout = |operation: &str, input: Value, confirmed: bool| {
        let call = operations::Invocation::new(
            operations::Caller::Assistant,
            on_behalf_of("ines"),
            operation,
            input,
        )
        .in_workspace(workspace.to_string())
        .idempotency_key(uuid::Uuid::now_v7().to_string());
        if confirmed { call.confirmed() } else { call }
    };

    // Scout writes a message for Ines (mail may have asked it to) and reads
    // it back: the content token it gets is Ines's.
    let written = operations::invoke(
        &server.state,
        scout(
            "conversations.create_draft",
            json!({"kind": "new", "inbox_id": inbox.to_string(),
                   "to": [{"name": "Maya Okafor", "address": "maya@northwind-studio.example"}],
                   "subject": "Thursday", "body": "Thursday works."}),
            false,
        ),
    )
    .await
    .unwrap();
    let id = written.body["id"].as_str().unwrap().to_owned();
    let read = operations::invoke(
        &server.state,
        scout("conversations.get_draft", json!({"draft_id": id}), false),
    )
    .await
    .unwrap();
    assert!(read.body["content_token"].is_string(), "{}", read.body);
    let input = json!({"draft_id": id, "version": read.body["version"],
                       "content_token": read.body["content_token"]});
    // Sending it, as a call the user confirmed or not: refused before the
    // handler runs.
    for confirmed in [false, true] {
        let refused = operations::invoke(
            &server.state,
            scout("conversations.send_draft", input.clone(), confirmed),
        )
        .await;
        assert!(
            matches!(refused, Err(operations::InvokeError::NotAllowed { .. })),
            "{refused:?}"
        );
    }
    // Nothing scheduled, signed or asked of the connector.
    let draft: uuid::Uuid = id.parse().unwrap();
    assert_eq!(held(&db, draft).await, ("draft".to_owned(), None));
    let stats = Worker::new(server.state.clone()).run_until_idle().await;
    assert_eq!(stats.failed, 0, "{stats:?}");
    assert_eq!(sends_requested(&db).await, 0);

    // Ines's own Send, with the token her screen got: on its way.
    let uri = format!("/api/v1/w/{workspace}/drafts/{id}");
    let (_, shown) = server.call(&ines, "GET", &uri, None).await;
    let (status, sent) = server
        .call(
            &ines,
            "POST",
            &format!("{uri}/send"),
            Some(json!({"version": shown["version"], "content_token": shown["content_token"]})),
        )
        .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{sent}");
    assert_eq!(sent["state"], "scheduled");
    // Scout can't take it back; Ines's Undo can.
    for confirmed in [false, true] {
        let refused = operations::invoke(
            &server.state,
            scout(
                "conversations.cancel_send",
                json!({"draft_id": id}),
                confirmed,
            ),
        )
        .await;
        assert!(
            matches!(refused, Err(operations::InvokeError::NotAllowed { .. })),
            "{refused:?}"
        );
    }
    assert_eq!(held(&db, draft).await.0, "scheduled");
    let (status, undone) = server
        .call(&ines, "POST", &format!("{uri}/cancel_send"), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{undone}");
    assert_eq!(undone["state"], "draft");
    assert_eq!(sends_requested(&db).await, 0);
}

/// A stand-in model answering each call in turn: `turns` whole answers, the
/// last one repeated.
async fn model_answering(turns: Vec<Value>) -> MockServer {
    let model = MockServer::start().await;
    let last = turns.len().saturating_sub(1);
    for (n, turn) in turns.into_iter().enumerate() {
        let mock = Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(turn));
        let mock = if n == last {
            mock
        } else {
            mock.up_to_n_times(1)
        };
        mock.mount(&model).await;
    }
    model
}

fn tool_call(name: &str, arguments: &Value) -> Value {
    json!({
        "model": "test-model",
        "choices": [{"finish_reason": "tool_calls", "message": {"content": null, "tool_calls": [
            {"id": "call_1", "type": "function",
             "function": {"name": name, "arguments": arguments.to_string()}}]}}],
    })
}

fn said(text: &str) -> Value {
    json!({
        "model": "test-model",
        "choices": [{"finish_reason": "stop", "message": {"content": text}}],
    })
}

/// Scout's chat with the real conversations module (campbooks-cloud#305):
/// what the chat is about and every tool are read as the member, so a
/// conversation of an inbox they can't read is neither a chat's subject nor
/// a tool's result.
#[sqlx::test(migrations = "../../migrations")]
async fn scouts_chat_reads_only_what_the_member_can(db: PgPool) {
    let find = tool_call("find_conversations", &json!({ "stream": "primary" }));
    let model = model_answering(vec![
        find.clone(),
        said("Maya asked you to review the Q4 plan."),
        find,
        said("Nothing came in."),
    ])
    .await;
    let gateway = Gateway::new(AiConfig {
        roles: [(
            Role::Complete,
            ProviderConfig::new(format!("{}/v1", model.uri()).parse().unwrap(), "test-model"),
        )]
        .into(),
        ..AiConfig::default()
    })
    .unwrap();
    let composition = Composition {
        assistant: campbooks_assistant::AssistantDeps {
            ai: Some(gateway),
            ..campbooks_assistant::AssistantDeps::default()
        },
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
    let maya: String = sqlx::query_scalar(
        "SELECT conversation_id::text FROM conversations.messages WHERE provider_message_id = 'en-1'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    let chats = format!("/api/v1/w/{workspace}/assistant/chats");
    let about = json!({ "about": { "type": "conversations.conversation", "id": maya } });

    // Ana can't read Sam's inbox: no chat about its conversation.
    let (status, _) = server
        .call(&server.token("ana"), "POST", &chats, Some(about.clone()))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Sam's chat about Maya's email: the conversation and its history are
    // read for the model, and the tool finds his conversations.
    let (status, chat) = server
        .call(&server.token("sam"), "POST", &chats, Some(about))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{chat}");
    let id = chat["id"].as_str().unwrap();
    let (status, answer) = server
        .call(
            &server.token("sam"),
            "POST",
            &format!("{chats}/{id}/questions"),
            Some(json!({ "text": "What does Maya want?" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["text"], "Maya asked you to review the Q4 plan.");
    let step = &answer["steps"][0];
    assert_eq!(step["tool"], "find_conversations");
    assert!(step["count"].as_u64().unwrap() >= 1, "{step}");
    let requests = model.received_requests().await.unwrap();
    let first: Value = serde_json::from_slice(&requests[0].body).unwrap();
    let context: Value =
        serde_json::from_str(first["messages"][1]["content"].as_str().unwrap()).unwrap();
    assert_eq!(context["about"]["id"], maya.as_str());
    assert!(
        context["about"]["history"]["events"]
            .as_array()
            .is_some_and(|events| !events.is_empty()),
        "{context}"
    );
    let second = String::from_utf8_lossy(&requests[1].body).to_string();
    assert!(second.contains("northwind-studio.example"), "Sam's results");

    // Ana's own chat finds nothing of Sam's.
    let (status, chat) = server
        .call(&server.token("ana"), "POST", &chats, Some(json!({})))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{chat}");
    let id = chat["id"].as_str().unwrap();
    let (status, answer) = server
        .call(
            &server.token("ana"),
            "POST",
            &format!("{chats}/{id}/questions"),
            Some(json!({ "text": "What came in?" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["steps"][0]["count"], 0, "{answer}");
    let requests = model.received_requests().await.unwrap();
    for request in &requests[2..] {
        let sent = String::from_utf8_lossy(&request.body).to_string();
        assert!(!sent.contains("northwind"), "{sent}");
        assert!(!sent.contains("Q4 plan"), "{sent}");
    }
}

/// Scout proposes with the real conversations module (campbooks-cloud#305,
/// phase 2): the proposal names the request the catalogue serves, nothing
/// changes when it is made, and only the member's own request, sent as the
/// proposal says with its id as the key, archives the conversation; its
/// undo puts it back.
#[sqlx::test(migrations = "../../migrations")]
#[allow(clippy::too_many_lines)]
async fn scout_proposes_and_only_the_members_own_request_changes_it(db: PgPool) {
    let model = MockServer::start().await;
    let gateway = Gateway::new(AiConfig {
        roles: [(
            Role::Complete,
            ProviderConfig::new(format!("{}/v1", model.uri()).parse().unwrap(), "test-model"),
        )]
        .into(),
        ..AiConfig::default()
    })
    .unwrap();
    let composition = Composition {
        assistant: campbooks_assistant::AssistantDeps {
            ai: Some(gateway),
            ..campbooks_assistant::AssistantDeps::default()
        },
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
    let maya: String = sqlx::query_scalar(
        "SELECT conversation_id::text FROM conversations.messages WHERE provider_message_id = 'en-1'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    // The model proposes archiving Maya's email, then says so.
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tool_call(
            "propose_change",
            &json!({ "action": "archive", "conversation_id": maya }),
        )))
        .up_to_n_times(1)
        .mount(&model)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(said("I proposed archiving it: confirm on the card.")),
        )
        .mount(&model)
        .await;
    let sam = server.token("sam");
    let root = format!("/api/v1/w/{workspace}");
    let chats = format!("{root}/assistant/chats");
    let (status, chat) = server
        .call(
            &sam,
            "POST",
            &chats,
            Some(json!({ "about": { "type": "conversations.conversation", "id": maya } })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{chat}");
    let id = chat["id"].as_str().unwrap();
    let (status, answer) = server
        .call(
            &sam,
            "POST",
            &format!("{chats}/{id}/questions"),
            Some(json!({ "text": "Can you get this out of my inbox?" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    let proposal = &answer["proposals"][0];
    assert_eq!(
        proposal["request"]["path"],
        format!("conversations/{maya}/archive")
    );
    assert_eq!(
        proposal["undo"]["path"],
        format!("conversations/{maya}/unarchive")
    );
    assert_eq!(proposal["facts"]["stream"], "primary");
    let archived = || async {
        let (status, detail) = server
            .call(&sam, "GET", &format!("{root}/conversations/{maya}"), None)
            .await;
        assert_eq!(status, StatusCode::OK, "{detail}");
        detail["conversation"]["archived"] == true
    };
    assert!(!archived().await, "proposing changed nothing");

    // Sam's own request, as the card sends it.
    let send = |request: &Value, key: String| {
        let method = request["method"].as_str().unwrap().to_owned();
        let uri = format!("{root}/{}", request["path"].as_str().unwrap());
        let token = sam.clone();
        let app = server.app.clone();
        async move {
            let response = app
                .oneshot(
                    Request::builder()
                        .method(method.as_str())
                        .uri(uri)
                        .header(header::AUTHORIZATION, format!("Bearer {token}"))
                        .header("idempotency-key", key)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            response.status()
        }
    };
    let key = proposal["id"].as_str().unwrap().to_owned();
    assert_eq!(
        send(&proposal["request"], key.clone()).await,
        StatusCode::OK
    );
    assert!(archived().await);
    // A second press replays, and acts once.
    assert_eq!(
        send(&proposal["request"], key.clone()).await,
        StatusCode::OK
    );
    let (status, settled) = server
        .call(
            &sam,
            "POST",
            &format!("{chats}/{id}/proposals/{key}"),
            Some(json!({ "state": "done" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{settled}");
    assert_eq!(settled["state"], "done");
    assert_eq!(
        send(&proposal["undo"], format!("{key}:undo")).await,
        StatusCode::OK
    );
    assert!(!archived().await);
}
