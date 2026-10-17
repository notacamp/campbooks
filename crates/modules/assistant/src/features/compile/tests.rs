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
    reply(&answer.to_string(), "stop")
}

/// A chat completion whose content is exactly `content`, stopped for
/// `finish_reason`.
fn reply(content: &str, finish_reason: &str) -> Value {
    json!({
        "model": "test-model",
        "choices": [{
            "message": { "role": "assistant", "content": content },
            "finish_reason": finish_reason,
        }],
        "usage": { "prompt_tokens": 412, "completion_tokens": 4096 },
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
async fn an_answer_that_isnt_json_is_the_models_failure_not_the_members(db: PgPool) {
    let server = MockServer::start().await;
    let kit = kit_with_model(db, &server).await;
    let workspace = WorkspaceId::new();
    let (ines, _) = kit.member("ines", workspace);

    // Prose, nothing at all, and JSON cut off where a reasoning model ran
    // out of tokens: none of them is "unclear", which would blame the
    // sentence. Each is 503 `compile_failed`, without the model's words.
    for (content, finish_reason) in [
        ("Sure! Here are your rules.", "stop"),
        ("", "length"),
        (
            "{\"outcome\": \"rules\", \"unsupported\": \"none\", \"rules\": [{\"who\": \"tax",
            "length",
        ),
    ] {
        server.reset().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(reply(content, finish_reason)))
            .mount(&server)
            .await;
        let response = kit
            .post(
                &ines,
                &uri(workspace),
                json!({ "text": "newsletters never need me" }),
            )
            .await;
        assert_eq!(
            response.status,
            StatusCode::SERVICE_UNAVAILABLE,
            "{content:?}: {}",
            response.json
        );
        assert_eq!(response.json["code"], "compile_failed");
        assert!(!response.json.to_string().contains("Sure!"));
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_answer_in_a_code_fence_is_read(db: PgPool) {
    let server = MockServer::start().await;
    let answer = json!({
        "outcome": "rules",
        "unsupported": "none",
        "rules": [{
            "who": "",
            "conditions": [{ "field": "header_present", "value": "list_id" }],
            "actions": [{ "kind": "surface", "value": "never" }],
        }],
    });
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(reply(&format!("```json\n{answer:#}\n```\n"), "stop")),
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
            json!({ "text": "newsletters never need me" }),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.json);
    assert_eq!(response.json["outcome"], "rules");
    assert_eq!(
        response.json["proposals"][0]["actions"],
        json!([{ "action": "surface", "value": "never" }])
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn two_firms_named_without_an_address_are_two_questions(db: PgPool) {
    // campbooks-cloud#217: the sentence that came back "unclear" in
    // production, answered as it should be.
    let sentence = "anything from my accountants at northtax or atelier conseils always matters";
    let server = MockServer::start().await;
    answer_with(
        &server,
        &json!({
            "outcome": "rules",
            "unsupported": "none",
            "rules": [
                {
                    "who": "northtax",
                    "conditions": [],
                    "actions": [{ "kind": "surface", "value": "always" }],
                },
                {
                    "who": "atelier conseils",
                    "conditions": [],
                    "actions": [{ "kind": "surface", "value": "always" }],
                },
            ],
        }),
    )
    .await;
    let kit = kit_with_model(db, &server).await;
    {
        let mut mail = kit.mail.lock().unwrap();
        mail.senders.push(json!({
            "address": "joana@northtax.example", "name": "Joana Reis · Northtax",
            "received": 12, "replied": 7,
        }));
        mail.senders.push(json!({
            "address": "marie@atelier-conseils.example", "name": "Marie Dubois",
            "received": 9, "replied": 4,
        }));
    }
    let workspace = WorkspaceId::new();
    let (ines, _) = kit.member("ines", workspace);

    let response = kit
        .post(&ines, &uri(workspace), json!({ "text": sentence }))
        .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.json);
    assert_eq!(response.json["outcome"], "rules");
    let proposals = response.json["proposals"].as_array().unwrap();
    assert_eq!(proposals.len(), 2);
    for (proposal, phrase, address) in [
        (&proposals[0], "northtax", "joana@northtax.example"),
        (
            &proposals[1],
            "atelier conseils",
            "marie@atelier-conseils.example",
        ),
    ] {
        assert_eq!(proposal["conditions"], json!([]));
        assert_eq!(
            proposal["actions"],
            json!([{ "action": "surface", "value": "always" }])
        );
        assert_eq!(proposal["who"]["phrase"], phrase);
        assert_eq!(proposal["who"]["matched"], true);
        assert_eq!(proposal["who"]["candidates"][0]["address"], address);
    }
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
    assert_eq!(response.status, StatusCode::CONFLICT);
    assert_eq!(response.json["code"], "compile_unavailable");
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
    assert_eq!(response.status, StatusCode::CONFLICT);
    assert_eq!(response.json["code"], "compile_unavailable");
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

#[sqlx::test(migrations = "../../../migrations")]
async fn senders_are_looked_up_once_per_word_in_a_compile(db: PgPool) {
    let server = MockServer::start().await;
    answer_with(
        &server,
        &json!({
            "outcome": "rules",
            "unsupported": "none",
            "rules": [
                {
                    "who": "my accountant",
                    "conditions": [],
                    "actions": [{ "kind": "surface", "value": "always" }],
                },
                {
                    "who": "the accountant office",
                    "conditions": [],
                    "actions": [{ "kind": "surface", "value": "never" }],
                },
            ],
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
            json!({ "text": "my accountant always matters, the accountant office never" }),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.json);
    let proposals = response.json["proposals"].as_array().unwrap();
    assert_eq!(proposals.len(), 2);
    for proposal in proposals {
        assert!(!proposal["who"]["candidates"].as_array().unwrap().is_empty());
    }
    // "accountant" and the fallback once each, though both questions need them.
    assert_eq!(
        kit.mail.lock().unwrap().asked,
        ["accountant", "", "office"],
        "one list_senders per q in a compile"
    );
}

/// A daily AI limit in memory (campbooks-cloud#152): admits `room` calls,
/// noting whether a person was waiting on each.
#[derive(Debug, Default)]
struct Limit {
    room: std::sync::Mutex<u32>,
    asked: std::sync::Mutex<Vec<(String, bool)>>,
}

impl campbooks_ai::usage::Meter for Limit {
    fn admit<'a>(
        &'a self,
        _db: &'a PgPool,
        call: campbooks_ai::usage::MeteredCall<'a>,
    ) -> campbooks_ai::decide::strategy::BoxFuture<
        'a,
        Result<campbooks_ai::usage::Admission, campbooks_ai::usage::MeterError>,
    > {
        use campbooks_ai::usage::Admission;
        Box::pin(async move {
            self.asked
                .lock()
                .unwrap()
                .push((call.feature.to_owned(), call.interactive));
            let mut room = self.room.lock().unwrap();
            if *room == 0 {
                return Ok(Admission::Refused { cap: 40 });
            }
            *room -= 1;
            Ok(Admission::Admitted {
                day: chrono::NaiveDate::from_ymd_opt(2026, 10, 16).unwrap(),
            })
        })
    }

    fn settle<'a>(
        &'a self,
        _db: &'a PgPool,
        _call: campbooks_ai::usage::MeteredCall<'a>,
        _day: chrono::NaiveDate,
        _outcome: campbooks_ai::usage::Outcome,
    ) -> campbooks_ai::decide::strategy::BoxFuture<'a, Result<(), campbooks_ai::usage::MeterError>>
    {
        Box::pin(async { Ok(()) })
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn compiling_is_a_call_a_person_waits_on_and_stops_at_the_daily_limit(db: PgPool) {
    let server = MockServer::start().await;
    answer_with(
        &server,
        &json!({ "outcome": "unclear", "unsupported": "none", "rules": [] }),
    )
    .await;
    let limit = std::sync::Arc::new(Limit {
        room: std::sync::Mutex::new(1),
        ..Limit::default()
    });
    let kit = Kit::with_deps(
        db,
        AssistantDeps {
            ai: Some(gateway(&server.uri()).with_meter(limit.clone())),
        },
    )
    .await;
    let workspace = WorkspaceId::new();
    let (ines, _) = kit.member("ines", workspace);
    let uri = uri(workspace);
    let ask = || kit.post(&ines, &uri, json!({ "text": "invoices matter" }));

    let first = ask().await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.json);
    let over = ask().await;
    assert_eq!(
        over.status,
        StatusCode::SERVICE_UNAVAILABLE,
        "{}",
        over.json
    );
    assert_eq!(over.json["code"], "ai_daily_limit_reached");
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
    assert_eq!(
        *limit.asked.lock().unwrap(),
        [
            ("assistant.compile".to_owned(), true),
            ("assistant.compile".to_owned(), true)
        ]
    );
}
