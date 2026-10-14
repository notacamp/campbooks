//! Compiling over HTTP: what reaches the model, what comes back, and what
//! happens when there is no model the sentence may go to.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::{Value, json};
use sqlx::PgPool;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

use crate::{
    AssistantDeps,
    testkit::{Kit, external_gateway, gateway},
};

/// A chat completion whose content is `answer`.
fn completion(answer: &Value) -> Value {
    json!({
        "model": "test-model",
        "choices": [{
            "message": { "role": "assistant", "content": answer.to_string() },
            "finish_reason": "stop",
        }],
    })
}

async fn kit_with_model(db: PgPool, server: &MockServer) -> Kit {
    let kit = Kit::with_deps(
        db,
        AssistantDeps {
            ai: Some(gateway(&server.uri())),
        },
    )
    .await;
    {
        let mut mail = kit.mail.lock().unwrap();
        mail.inboxes = vec![
            json!({ "id": "inbox-1", "address": "ines@ateliersousa.example", "role": "manager", "shared": false }),
            json!({ "id": "inbox-2", "address": "info@ateliersousa.example", "role": "reader", "shared": true }),
        ];
        mail.labels = vec![json!({ "id": "label-a", "inbox": "inbox-1", "name": "Clients" })];
        mail.senders = vec![
            json!({ "address": "rita@silva-contas.example", "name": "Rita Gomes", "received": 41, "replied": 23 }),
            json!({ "address": "duarte@pessoa-obras.example", "name": "Duarte Pessoa", "received": 28, "replied": 17 }),
        ];
    }
    kit
}

async fn answer_with(server: &MockServer, answer: &Value) {
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(completion(answer)))
        .mount(server)
        .await;
}

fn uri(workspace: WorkspaceId) -> String {
    format!("/api/v1/w/{workspace}/assistant/compile")
}

#[sqlx::test(migrations = "../../../migrations")]
async fn anything_from_my_accountant(db: PgPool) {
    let server = MockServer::start().await;
    answer_with(
        &server,
        &json!({
            "outcome": "rules",
            "unsupported": "none",
            "rules": [{
                "who": "my accountant",
                "conditions": [],
                "actions": [{ "kind": "surface", "value": "always" }],
            }],
        }),
    )
    .await;
    let kit = kit_with_model(db, &server).await;
    let workspace = WorkspaceId::new();
    let (ines, _) = kit.member("ines", workspace);
    // Rita's domain says "contas": the phrase's word matches nobody, so the
    // people replied to most are offered instead.
    kit.mail.lock().unwrap().senders[0]["name"] = json!("Rita Gomes · Silva Contabilidade");

    let response = kit
        .post(
            &ines,
            &uri(workspace),
            json!({ "text": "anything from my accountant always matters" }),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.json);
    assert_eq!(response.json["outcome"], "rules");
    assert_eq!(
        response.json["text"],
        "anything from my accountant always matters"
    );
    let proposal = &response.json["proposals"][0];
    assert_eq!(
        proposal["actions"],
        json!([{ "action": "surface", "value": "always" }])
    );
    assert_eq!(proposal["conditions"], json!([]));
    assert_eq!(proposal["who"]["phrase"], "my accountant");
    let candidates = proposal["who"]["candidates"].as_array().unwrap();
    assert_eq!(candidates[0]["address"], "rita@silva-contas.example");
    assert_eq!(
        proposal["inboxes"],
        json!([{ "id": "inbox-1", "address": "ines@ateliersousa.example", "shared": false }]),
        "only inboxes the member may teach in"
    );
    // The stop word went; only "accountant" was looked up.
    assert_eq!(kit.mail.lock().unwrap().asked[0], "accountant");

    // What the model was given: the sentence and the member's own data, as
    // one JSON object after a fixed instruction, a strict schema, no tools.
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let body: Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert!(body.get("tools").is_none());
    assert_eq!(body["response_format"]["type"], "json_schema");
    assert_eq!(body["response_format"]["json_schema"]["strict"], true);
    assert_eq!(body["messages"][0]["role"], "system");
    let input: Value =
        serde_json::from_str(body["messages"][1]["content"].as_str().unwrap()).unwrap();
    assert_eq!(
        input,
        json!({
            "sentence": "anything from my accountant always matters",
            "my_addresses": ["ines@ateliersousa.example", "info@ateliersousa.example"],
            "streams": ["primary", "updates", "receipts", "newsletters"],
            "labels": ["Clients"],
        })
    );
    let sent = String::from_utf8_lossy(&requests[0].body).to_string();
    assert!(
        !sent.contains("rita@silva-contas.example"),
        "no sender reaches the model"
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_invented_address_is_unclear(db: PgPool) {
    let server = MockServer::start().await;
    answer_with(
        &server,
        &json!({
            "outcome": "rules",
            "unsupported": "none",
            "rules": [{
                "who": "",
                "conditions": [{ "field": "sender_address", "value": "boss@corp.example" }],
                "actions": [{ "kind": "archive", "value": "" }],
            }],
        }),
    )
    .await;
    let kit = kit_with_model(db, &server).await;
    let workspace = WorkspaceId::new();
    let (ines, _) = kit.member("ines", workspace);

    let response = kit
        .post(
            &ines,
            &uri(workspace),
            json!({ "text": "archive what my boss sends" }),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.json["outcome"], "unclear");
    assert_eq!(response.json["proposals"], json!([]));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn forwarding_is_unsupported(db: PgPool) {
    let server = MockServer::start().await;
    answer_with(
        &server,
        &json!({
            "outcome": "unsupported",
            "unsupported": "sends_mail",
            "rules": [],
        }),
    )
    .await;
    let kit = kit_with_model(db, &server).await;
    let workspace = WorkspaceId::new();
    let (ines, _) = kit.member("ines", workspace);

    let response = kit
        .post(
            &ines,
            &uri(workspace),
            json!({ "text": "forward invoices to my accountant" }),
        )
        .await;
    assert_eq!(response.json["outcome"], "unsupported");
    assert_eq!(response.json["unsupported"], "sends_mail");
    assert_eq!(response.json["proposals"], json!([]));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_malformed_answer_is_unclear(db: PgPool) {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": "test-model",
            "choices": [{ "message": { "content": "Sure! Here are your rules." }, "finish_reason": "stop" }],
        })))
        .mount(&server)
        .await;
    let kit = kit_with_model(db, &server).await;
    let workspace = WorkspaceId::new();
    let (ines, _) = kit.member("ines", workspace);

    let response = kit
        .post(
            &ines,
            &uri(workspace),
            json!({ "text": "newsletters never need me" }),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.json["outcome"], "unclear");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_provider_error_never_reaches_the_member(db: PgPool) {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(500)
                .set_body_string("upstream echo: anything from my accountant always matters"),
        )
        .mount(&server)
        .await;
    let kit = kit_with_model(db, &server).await;
    let workspace = WorkspaceId::new();
    let (ines, _) = kit.member("ines", workspace);

    let response = kit
        .post(
            &ines,
            &uri(workspace),
            json!({ "text": "anything from my accountant always matters" }),
        )
        .await;
    assert_eq!(response.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(response.json["code"], "compile_failed");
    assert!(!response.json.to_string().contains("upstream echo"));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn no_model_means_the_builder(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ines, _) = kit.member("ines", workspace);

    let response = kit
        .post(
            &ines,
            &uri(workspace),
            json!({ "text": "newsletters never need me" }),
        )
        .await;
    assert_eq!(response.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(response.json["code"], "ai_role_not_configured");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_external_model_without_the_opt_in_gets_nothing(db: PgPool) {
    let kit = Kit::with_deps(
        db,
        AssistantDeps {
            ai: Some(external_gateway()),
        },
    )
    .await;
    let workspace = WorkspaceId::new();
    let (ines, _) = kit.member("ines", workspace);

    let response = kit
        .post(
            &ines,
            &uri(workspace),
            json!({ "text": "newsletters never need me" }),
        )
        .await;
    assert_eq!(response.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(response.json["code"], "ai_external_content_refused");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn refused_sentences_never_reach_the_model(db: PgPool) {
    let server = MockServer::start().await;
    let kit = kit_with_model(db, &server).await;
    let workspace = WorkspaceId::new();
    let (ines, _) = kit.member("ines", workspace);

    for (text, code) in [
        ("   ", "instruction_empty"),
        (
            "From: Ana <ana@example.com>\nSubject: Invoice",
            "instruction_pasted_mail",
        ),
        ("archive https://example.com/news", "instruction_has_link"),
    ] {
        let response = kit
            .post(&ines, &uri(workspace), json!({ "text": text }))
            .await;
        assert_eq!(response.status, StatusCode::BAD_REQUEST, "{text}");
        assert_eq!(response.json["code"], code);
    }
    let long = kit
        .post(&ines, &uri(workspace), json!({ "text": "a".repeat(301) }))
        .await;
    assert_eq!(long.json["code"], "instruction_too_long");
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn twenty_an_hour(db: PgPool) {
    let server = MockServer::start().await;
    answer_with(
        &server,
        &json!({ "outcome": "unclear", "unsupported": "none", "rules": [] }),
    )
    .await;
    let kit = kit_with_model(db, &server).await;
    let workspace = WorkspaceId::new();
    let (ines, _) = kit.member("ines", workspace);

    for _ in 0..20 {
        let ok = kit
            .post(
                &ines,
                &uri(workspace),
                json!({ "text": "newsletters never need me" }),
            )
            .await;
        assert_eq!(ok.status, StatusCode::OK);
    }
    let limited = kit
        .post(
            &ines,
            &uri(workspace),
            json!({ "text": "newsletters never need me" }),
        )
        .await;
    assert_eq!(limited.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(server.received_requests().await.unwrap().len(), 20);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn members_only(db: PgPool) {
    let server = MockServer::start().await;
    let kit = kit_with_model(db, &server).await;
    let workspace = WorkspaceId::new();
    let client = kit.client(workspace, "assistant:write");
    let (_, _) = kit.member("ines", workspace);
    let read_only = kit.token("ines", "assistant:read");
    let body = json!({ "text": "newsletters never need me" });

    let response = kit.post(&client, &uri(workspace), body.clone()).await;
    assert_eq!(response.status, StatusCode::FORBIDDEN);
    assert_eq!(response.json["code"], "profile_needs_member");
    assert_eq!(
        kit.post(&read_only, &uri(workspace), body.clone())
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        kit.post("", &uri(workspace), body.clone()).await.status,
        StatusCode::UNAUTHORIZED
    );
    let (stranger, _) = kit.member("leo", WorkspaceId::new());
    assert_eq!(
        kit.post(&stranger, &uri(workspace), body).await.status,
        StatusCode::NOT_FOUND
    );
    assert!(server.received_requests().await.unwrap().is_empty());
}
