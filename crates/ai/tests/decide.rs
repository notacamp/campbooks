//! The `decide` role against mock providers: `laya-serve` (`POST /v1/systemone`),
//! Jev through the decisions endpoint of a router (the same protocol at another
//! path) and an OpenAI-compatible instruct model.

// Helpers outside `#[test]` fns are not covered by clippy's test exemptions.
#![allow(clippy::unwrap_used)]

use std::time::Duration;

use campbooks_ai::{
    AiConfig, Error, Gateway, Protocol, ProviderConfig, RetryPolicy, Role,
    decide::{AnswerValue, Calibration, DecisionContext, DecisionState, Question},
};
use serde_json::{Value, json};
use wiremock::{
    Mock, MockServer, Request, ResponseTemplate,
    matchers::{body_json, body_partial_json, header, method, path},
};

const CTX: DecisionContext<'static> = DecisionContext::new("test.triage");

fn questions() -> Vec<Question> {
    vec![
        Question::choice(
            "category",
            "What kind of email is this?",
            [
                ("invoice", "A bill or invoice asking for payment"),
                ("newsletter", "Marketing or newsletter"),
                ("personal", "A personal message"),
            ],
        ),
        Question::score(
            "urgency",
            "How urgent is a reply?",
            ["no reply needed", "reply this week", "reply today"],
        ),
        Question::noul(
            "asks_user",
            "The sender asks the recipient to do something.",
        ),
    ]
}

fn state() -> DecisionState {
    DecisionState::from("Subject: October invoice\n\nInvoice 2026-118, EUR 450, due 31 October.")
}

fn fast_retry() -> RetryPolicy {
    RetryPolicy {
        attempts: 3,
        base_delay: Duration::from_millis(1),
        max_delay: Duration::from_millis(5),
    }
}

fn gateway(provider: ProviderConfig) -> Gateway {
    Gateway::new(AiConfig {
        roles: [(Role::Decide, provider)].into(),
        ..AiConfig::default()
    })
    .unwrap()
    .with_retry(fast_retry())
}

fn laya(server: &MockServer) -> ProviderConfig {
    ProviderConfig::new(
        format!("{}/v1", server.uri()).parse().unwrap(),
        "convaiinnovations/laya-multilingual",
    )
    .with_protocol(Protocol::Laya)
    .with_api_key("laya-key")
}

/// A response in exactly the shape laya-serve 0.3.21 returns.
fn laya_response() -> Value {
    json!({
        "model": "laya-rl-agent",
        "answers": {
            "category": {
                "type": "choice", "choice": "invoice",
                "probabilities": {"invoice": 0.9, "newsletter": 0.08, "personal": 0.02},
                "confidence": 0.62, "answer_confidence": 0.9,
                "action": {"act_probability": 1.0}
            },
            "urgency": {
                "type": "score", "score": 1.1,
                "legend": {"0": "no reply needed", "1": "reply this week", "2": "reply today"},
                "probabilities": {"0": 0.1, "1": 0.7, "2": 0.2},
                "confidence": 0.2, "answer_confidence": 0.7,
                "action": {"act_probability": 1.0}
            },
            "asks_user": {
                "type": "noul", "noul": 0.9, "confidence": 0.9, "answer_confidence": 0.9,
                "action": {"act_probability": 1.0}
            }
        },
        "usage": {"input_tokens": 120, "output_tokens": 0},
        "routing": {
            "model": "multilingual", "repo": "convaiinnovations/laya/multilingual",
            "reason": "explicit model='convaiinnovations/laya-multilingual'",
            "detection": null, "workflow": null
        }
    })
}

/// A response in the shape Jev returns through its router: the answers as
/// Laya's, plus fields the gateway does not read.
fn jev_response() -> Value {
    json!({
        "model": "typesafe/jev-1.13-20261014",
        "answers": {
            "category": {
                "type": "choice", "choice": "invoice",
                "probabilities": {"invoice": 0.97, "newsletter": 0.02, "personal": 0.01},
                "confidence": 1
            },
            "urgency": {
                "type": "score", "score": 0.01,
                "legend": {"0": "no reply needed", "1": "reply this week", "2": "reply today"},
                "probabilities": {"0": 0.99, "1": 0.01, "2": 0.0},
                "confidence": 0.98
            },
            "asks_user": {"type": "noul", "noul": 0.25}
        },
        "usage": {"input_tokens": 438, "output_tokens": 79, "cost": 1.8e-05},
        "id": "gen-dec-1",
        "provider": "TypeSafe"
    })
}

#[tokio::test]
async fn jev_is_reached_at_the_configured_path_with_the_laya_body() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/alpha/decisions"))
        .and(header("authorization", "Bearer router-key"))
        .and(body_partial_json(json!({
            "model": "typesafe/jev-1.13",
            "questions": {"asks_user": {"type": "noul"}}
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(jev_response()))
        .expect(1)
        .mount(&server)
        .await;
    let provider = ProviderConfig::new(
        format!("{}/api/alpha", server.uri()).parse().unwrap(),
        "typesafe/jev-1.13",
    )
    .with_protocol(Protocol::Laya)
    .with_path("decisions")
    .unwrap()
    .with_api_key("router-key");

    let answers = gateway(provider)
        .decide(&CTX, &state(), &questions())
        .await
        .unwrap();

    let category = answers.get("category").unwrap();
    assert_eq!(category.choice(), Some("invoice"));
    assert_eq!(category.model, "typesafe/jev-1.13-20261014");
    let asks = answers.get("asks_user").unwrap();
    assert_eq!(asks.noul(), Some(false));
    assert!((asks.confidence.unwrap() - 0.75).abs() < 1e-9);
    assert!(answers.get("urgency").unwrap().score().unwrap() < 0.1);
}

#[tokio::test]
async fn laya_request_matches_laya_serve_and_answers_are_typed() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .and(header("authorization", "Bearer laya-key"))
        .and(body_json(json!({
            "model": "convaiinnovations/laya-multilingual",
            "state": "Subject: October invoice\n\nInvoice 2026-118, EUR 450, due 31 October.",
            "questions": {
                "category": {
                    "type": "choice",
                    "instructions": "What kind of email is this?",
                    "criteria": {
                        "invoice": "A bill or invoice asking for payment",
                        "newsletter": "Marketing or newsletter",
                        "personal": "A personal message"
                    }
                },
                "urgency": {
                    "type": "score",
                    "instructions": "How urgent is a reply?",
                    "criteria": ["no reply needed", "reply this week", "reply today"]
                },
                "asks_user": {
                    "type": "noul",
                    "instructions": "The sender asks the recipient to do something."
                }
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(laya_response()))
        .expect(1)
        .mount(&server)
        .await;

    let answers = gateway(laya(&server))
        .decide(&CTX, &state(), &questions())
        .await
        .unwrap();

    assert_eq!(answers.len(), 3);
    let ids: Vec<_> = answers.iter().map(|a| a.question_id.as_str()).collect();
    assert_eq!(ids, ["category", "urgency", "asks_user"]);

    let category = answers.get("category").unwrap();
    assert_eq!(category.choice(), Some("invoice"));
    assert_eq!(
        category.probabilities.as_deref(),
        Some(&[0.9, 0.08, 0.02][..])
    );
    assert_eq!(category.confidence, Some(0.9));
    assert_eq!(category.model, "convaiinnovations/laya/multilingual");
    assert_eq!(category.model_version, None);
    assert_eq!(
        category.accept_if(0.85),
        Some(&AnswerValue::Choice("invoice".into()))
    );
    assert_eq!(category.accept_if(0.95), None);

    let urgency = answers.get("urgency").unwrap();
    // Expected level, recomputed from the probabilities.
    assert!((urgency.score().unwrap() - 1.1).abs() < 1e-9);
    assert_eq!(urgency.confidence, Some(0.7));

    let asks = answers.get("asks_user").unwrap();
    assert_eq!(asks.noul(), Some(true));
    assert!((asks.probability_true().unwrap() - 0.9).abs() < 1e-9);
    assert!((asks.confidence.unwrap() - 0.9).abs() < 1e-9);
}

#[tokio::test]
async fn laya_json_state_is_sent_as_json() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/systemone"))
        .and(body_partial_json(
            json!({"state": {"subject": "Hi", "sender_kind": "person"}}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "answers": {"asks_user": {"type": "noul", "noul": 0.2}}
        })))
        .expect(1)
        .mount(&server)
        .await;

    let state = DecisionState::from(json!({"subject": "Hi", "sender_kind": "person"}));
    let answers = gateway(laya(&server))
        .decide(&CTX, &state, &questions()[2..])
        .await
        .unwrap();
    let asks = answers.get("asks_user").unwrap();
    assert_eq!(asks.noul(), Some(false));
    // No routing block: the configured model names the answer.
    assert_eq!(asks.model, "convaiinnovations/laya-multilingual");
}

#[tokio::test]
async fn calibration_tempers_provider_probabilities() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/systemone"))
        .respond_with(ResponseTemplate::new(200).set_body_json(laya_response()))
        .mount(&server)
        .await;

    let calibration = Calibration::from_pairs([("noul", 2.0), ("choice_3", 1.0)]).unwrap();
    let answers = gateway(laya(&server).with_calibration(calibration))
        .decide(&CTX, &state(), &questions())
        .await
        .unwrap();

    // 0.9 at T = 2: sqrt(0.9) / (sqrt(0.9) + sqrt(0.1)) = 0.75.
    let asks = answers.get("asks_user").unwrap();
    assert!((asks.probability_true().unwrap() - 0.75).abs() < 1e-9);
    assert!((asks.confidence.unwrap() - 0.75).abs() < 1e-9);
    // choice_3 = 1.0 leaves the three-option choice alone.
    assert_eq!(answers.get("category").unwrap().confidence, Some(0.9));
}

#[tokio::test]
async fn laya_busy_or_restarting_is_retried() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/systemone"))
        .respond_with(
            ResponseTemplate::new(503)
                .insert_header("retry-after", "0")
                .set_body_json(json!({"detail": "server busy, try again later"})),
        )
        .up_to_n_times(1)
        .with_priority(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/v1/systemone"))
        .respond_with(ResponseTemplate::new(502))
        .up_to_n_times(1)
        .with_priority(2)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/v1/systemone"))
        .respond_with(ResponseTemplate::new(200).set_body_json(laya_response()))
        .expect(1)
        .mount(&server)
        .await;

    let answers = gateway(laya(&server))
        .decide(&CTX, &state(), &questions())
        .await
        .unwrap();
    assert_eq!(answers.len(), 3);
}

#[tokio::test]
async fn persistent_server_errors_give_up_after_the_last_attempt() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/systemone"))
        .respond_with(
            ResponseTemplate::new(500).set_body_json(json!({"detail": "inference failed"})),
        )
        .expect(3)
        .mount(&server)
        .await;

    let err = gateway(laya(&server))
        .decide(&CTX, &state(), &questions())
        .await
        .unwrap_err();
    assert!(
        matches!(&err, Error::Status { status: 500 }) && !err.to_string().contains("inference"),
        "{err:?}"
    );
}

#[tokio::test]
async fn client_errors_are_not_retried() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/systemone"))
        .respond_with(
            ResponseTemplate::new(401)
                .set_body_json(json!({"detail": "invalid or missing bearer token"})),
        )
        .expect(1)
        .mount(&server)
        .await;

    let err = gateway(laya(&server))
        .decide(&CTX, &state(), &questions())
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Status { status: 401, .. }), "{err:?}");
}

#[tokio::test]
async fn unreachable_provider_is_retried_then_reported() {
    // Bind and drop a listener so the port is closed.
    let addr = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let provider = ProviderConfig::new(format!("http://{addr}/v1").parse().unwrap(), "m")
        .with_protocol(Protocol::Laya);
    let started = std::time::Instant::now();
    let err = gateway(provider)
        .with_retry(RetryPolicy {
            attempts: 3,
            base_delay: Duration::from_millis(50),
            max_delay: Duration::from_millis(50),
        })
        .decide(&CTX, &state(), &questions())
        .await
        .unwrap_err();
    assert!(
        matches!(&err, Error::Transport(e) if e.is_connect()),
        "{err:?}"
    );
    assert!(
        started.elapsed() >= Duration::from_millis(100),
        "two retries waited"
    );
}

#[tokio::test]
async fn slow_provider_times_out_without_retry() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/systemone"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(laya_response())
                .set_delay(Duration::from_secs(5)),
        )
        .expect(1)
        .mount(&server)
        .await;

    let gw = Gateway::with_timeouts(
        AiConfig {
            roles: [(Role::Decide, laya(&server))].into(),
            ..AiConfig::default()
        },
        Duration::from_secs(1),
        Duration::from_millis(200),
    )
    .unwrap()
    .with_retry(fast_retry());
    let err = gw.decide(&CTX, &state(), &questions()).await.unwrap_err();
    assert!(
        matches!(&err, Error::Transport(e) if e.is_timeout()),
        "{err:?}"
    );
}

#[tokio::test]
async fn malformed_answers_are_invalid_responses() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/systemone"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "answers": {"category": {"choice": "spam", "probabilities": {}}}
        })))
        .mount(&server)
        .await;

    let err = gateway(laya(&server))
        .decide(&CTX, &state(), &questions()[..1])
        .await
        .unwrap_err();
    assert!(matches!(err, Error::InvalidResponse(_)), "{err:?}");
}

#[tokio::test]
async fn bad_questions_fail_before_any_request() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    let gw = gateway(laya(&server));

    let too_many = Question::choice(
        "bank_category",
        "Which category?",
        (0..30).map(|i| (format!("c{i}"), String::new())),
    );
    let err = gw.decide(&CTX, &state(), &[too_many]).await.unwrap_err();
    assert!(
        matches!(&err, Error::InvalidQuestion(m) if m.contains("coarse")),
        "{err}"
    );

    // No questions: nothing to ask.
    assert!(gw.decide(&CTX, &state(), &[]).await.unwrap().is_empty());
}

#[tokio::test]
async fn unconfigured_decide_role() {
    let gw = Gateway::new(AiConfig::default()).unwrap();
    let err = gw.decide(&CTX, &state(), &questions()).await.unwrap_err();
    assert!(matches!(err, Error::RoleNotConfigured(Role::Decide)));
}

// --- OpenAI-compatible instruct model -------------------------------------------

fn instruct(server: &MockServer) -> ProviderConfig {
    ProviderConfig::new(format!("{}/v1", server.uri()).parse().unwrap(), "qwen3:8b")
        .with_api_key("sk-test")
}

/// A logprobs entry for one generated token.
fn token(text: &str, p: f64, alternatives: &[(&str, f64)]) -> Value {
    json!({
        "token": text,
        "logprob": p.ln(),
        "bytes": text.as_bytes(),
        "top_logprobs": alternatives
            .iter()
            .map(|(t, p)| json!({"token": t, "logprob": p.ln(), "bytes": t.as_bytes()}))
            .collect::<Vec<_>>(),
    })
}

fn chat_response(content: &str, logprobs: Option<Vec<Value>>) -> Value {
    let mut choice = json!({
        "index": 0,
        "message": {"role": "assistant", "content": content},
        "finish_reason": "stop"
    });
    if let Some(tokens) = logprobs {
        choice["logprobs"] = json!({"content": tokens});
    }
    json!({
        "model": "qwen3:8b",
        "system_fingerprint": "fp_test",
        "choices": [choice]
    })
}

#[tokio::test]
async fn instruct_model_answers_typed_questions_with_logprob_probabilities() {
    let server = MockServer::start().await;
    let tokens = vec![
        token("{\"", 1.0, &[]),
        token("category", 1.0, &[]),
        token("\":\"", 1.0, &[]),
        token("inv", 0.8, &[("news", 0.15), ("pers", 0.05)]),
        token("oice", 1.0, &[]),
        token("\",\"", 1.0, &[]),
        token("urgency", 1.0, &[]),
        token("\":", 1.0, &[]),
        token("1", 0.6, &[("2", 0.3), ("0", 0.1)]),
        token(",\"", 1.0, &[]),
        token("asks_user", 1.0, &[]),
        token("\":", 1.0, &[]),
        token("true", 0.9, &[("false", 0.1)]),
        token("}", 1.0, &[]),
    ];
    let content = r#"{"category":"invoice","urgency":1,"asks_user":true}"#;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header("authorization", "Bearer sk-test"))
        .and(body_partial_json(json!({
            "model": "qwen3:8b",
            "stream": false,
            "temperature": 0.0,
            "max_tokens": 4096,
            "logprobs": true,
            "top_logprobs": 20,
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": "decision",
                    "strict": true,
                    "schema": {
                        "type": "object",
                        "properties": {
                            "category": {"type": "string", "enum": ["invoice", "newsletter", "personal"]},
                            "urgency": {"type": "integer", "enum": [0, 1, 2]},
                            "asks_user": {"type": "boolean"}
                        },
                        "required": ["category", "urgency", "asks_user"],
                        "additionalProperties": false
                    }
                }
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(chat_response(content, Some(tokens))))
        .expect(1)
        .mount(&server)
        .await;

    let answers = gateway(instruct(&server))
        .decide(&CTX, &state(), &questions())
        .await
        .unwrap();

    let category = answers.get("category").unwrap();
    assert_eq!(category.choice(), Some("invoice"));
    let p = category.probabilities.as_ref().unwrap();
    assert!((p[0] - 0.8).abs() < 1e-9 && (p[1] - 0.15).abs() < 1e-9 && (p[2] - 0.05).abs() < 1e-9);
    assert!((category.confidence.unwrap() - 0.8).abs() < 1e-9);
    assert_eq!(category.model, "qwen3:8b");
    assert_eq!(category.model_version.as_deref(), Some("fp_test"));

    let urgency = answers.get("urgency").unwrap();
    // Expected level: 0·0.1 + 1·0.6 + 2·0.3 = 1.2.
    assert!((urgency.score().unwrap() - 1.2).abs() < 1e-9);

    let asks = answers.get("asks_user").unwrap();
    assert_eq!(asks.noul(), Some(true));
    assert!((asks.probability_true().unwrap() - 0.9).abs() < 1e-9);
}

#[tokio::test]
async fn instruct_model_without_logprobs_has_no_confidence() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(chat_response(
            r#"{"category": "newsletter", "urgency": 0, "asks_user": false}"#,
            None,
        )))
        .mount(&server)
        .await;

    let answers = gateway(instruct(&server))
        .decide(&CTX, &state(), &questions())
        .await
        .unwrap();
    for answer in &answers {
        assert_eq!(answer.probabilities, None, "{}", answer.question_id);
        assert_eq!(answer.confidence, None);
        assert_eq!(
            answer.accept_if(0.0),
            None,
            "unknown confidence never passes"
        );
    }
    assert_eq!(
        answers.get("category").unwrap().choice(),
        Some("newsletter")
    );
    assert_eq!(answers.get("urgency").unwrap().score(), Some(0.0));
    assert_eq!(answers.get("asks_user").unwrap().noul(), Some(false));
}

#[tokio::test]
async fn logprobs_can_be_turned_off_for_servers_that_reject_them() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/chat/completions"))
        .and(|request: &Request| {
            let body: Value = serde_json::from_slice(&request.body).unwrap();
            body.get("logprobs").is_none() && body.get("top_logprobs").is_none()
        })
        .respond_with(
            ResponseTemplate::new(200).set_body_json(chat_response(r#"{"asks_user": true}"#, None)),
        )
        .expect(1)
        .mount(&server)
        .await;

    let mut provider = instruct(&server);
    provider.logprobs = false;
    let answers = gateway(provider)
        .decide(&CTX, &state(), &questions()[2..])
        .await
        .unwrap();
    assert_eq!(answers.get("asks_user").unwrap().noul(), Some(true));
}

#[tokio::test]
async fn instruct_answers_outside_the_schema_are_invalid() {
    for content in [
        "Sure! The category is invoice.",
        r#"{"category": "spam", "urgency": 0, "asks_user": true}"#,
        r#"{"category": "invoice", "asks_user": true}"#,
    ] {
        let server = MockServer::start().await;
        Mock::given(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(chat_response(content, None)))
            .mount(&server)
            .await;
        let err = gateway(instruct(&server))
            .decide(&CTX, &state(), &questions())
            .await
            .unwrap_err();
        assert!(
            matches!(err, Error::InvalidResponse(_)),
            "{content}: {err:?}"
        );
    }
}

/// An endpoint on the instance's network that answers 307 to another host.
async fn redirecting_to(elsewhere: &MockServer) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(307)
                .insert_header("location", format!("{}/v1/systemone", elsewhere.uri())),
        )
        .mount(&server)
        .await;
    server
}

#[tokio::test]
async fn decide_never_follows_a_redirect_off_the_box() {
    for protocol in [Protocol::Laya, Protocol::OpenAi] {
        let elsewhere = MockServer::start().await;
        let server = redirecting_to(&elsewhere).await;
        let provider = match protocol {
            Protocol::Laya => laya(&server),
            Protocol::OpenAi => instruct(&server),
        };
        let err = gateway(provider)
            .decide(&CTX, &state(), &questions())
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Status { status: 307 }), "{err:?}");
        assert!(
            elsewhere.received_requests().await.unwrap().is_empty(),
            "{protocol:?}: the mail went to the redirect target"
        );
    }
}
