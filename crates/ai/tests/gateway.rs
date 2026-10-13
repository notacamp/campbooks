//! Gateway behaviour against a mock OpenAI-compatible provider.

// Helpers outside `#[test]` fns are not covered by clippy's test exemptions.
#![allow(clippy::unwrap_used)]

use std::{collections::HashMap, time::Duration};

use campbooks_ai::{AiConfig, ChatRequest, Error, Gateway, Message, ProviderConfig, Role};
use serde::Deserialize;
use serde_json::json;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_partial_json, header, method, path},
};

#[derive(Deserialize)]
struct Verdict {
    invoice: bool,
}

fn provider(server: &MockServer, model: &str, api_key: Option<&str>) -> ProviderConfig {
    let provider = ProviderConfig::new(format!("{}/v1", server.uri()).parse().unwrap(), model);
    match api_key {
        Some(key) => provider.with_api_key(key),
        None => provider,
    }
}

fn gateway(roles: impl IntoIterator<Item = (Role, ProviderConfig)>) -> Gateway {
    Gateway::new(AiConfig {
        roles: roles.into_iter().collect::<HashMap<_, _>>(),
    })
    .unwrap()
}

#[tokio::test]
async fn chat_sends_model_schema_and_key() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header("authorization", "Bearer sk-test"))
        .and(body_partial_json(json!({
            "model": "qwen3:8b",
            "stream": false,
            "messages": [{"role": "user", "content": "Is this an invoice?"}],
            "response_format": {"type": "json_schema", "json_schema": {"name": "verdict", "strict": true}}
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": "qwen3:8b",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "{\"invoice\": true}"},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 12, "completion_tokens": 5, "total_tokens": 17}
        })))
        .expect(1)
        .mount(&server)
        .await;

    let gw = gateway([(Role::Decide, provider(&server, "qwen3:8b", Some("sk-test")))]);
    let request = ChatRequest::new(vec![Message::user("Is this an invoice?")]).with_json_schema(
        "verdict",
        json!({"type": "object", "properties": {"invoice": {"type": "boolean"}}}),
    );
    let response = gw.chat(Role::Decide, request).await.unwrap();

    assert!(response.json::<Verdict>().unwrap().invoice);
    assert_eq!(response.finish_reason.as_deref(), Some("stop"));
    assert_eq!(response.usage.unwrap().completion_tokens, 5);
}

#[tokio::test]
async fn embed_returns_vectors_in_input_order() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/embeddings"))
        .and(body_partial_json(
            json!({"model": "BAAI/bge-m3", "input": ["a", "b"]}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [
                {"object": "embedding", "index": 1, "embedding": [0.0, 1.0]},
                {"object": "embedding", "index": 0, "embedding": [1.0, 0.0]}
            ],
            "model": "BAAI/bge-m3"
        })))
        .mount(&server)
        .await;

    let gw = gateway([(Role::Embed, provider(&server, "BAAI/bge-m3", None))]);
    let vectors = gw.embed(&["a".into(), "b".into()]).await.unwrap();
    assert_eq!(vectors, vec![vec![1.0, 0.0], vec![0.0, 1.0]]);
}

#[tokio::test]
async fn unconfigured_role_fails_without_a_request() {
    let gw = gateway([]);
    let err = gw
        .chat(Role::Vision, ChatRequest::default())
        .await
        .unwrap_err();
    assert!(matches!(err, Error::RoleNotConfigured(Role::Vision)));
    assert!(matches!(
        gw.embed(&["x".into()]).await,
        Err(Error::RoleNotConfigured(Role::Embed))
    ));
}

#[tokio::test]
async fn provider_errors_surface_status_and_body() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(429).set_body_string("rate limited"))
        .mount(&server)
        .await;

    let gw = gateway([(Role::Complete, provider(&server, "m", None))]);
    match gw
        .chat(Role::Complete, ChatRequest::new(vec![Message::user("hi")]))
        .await
    {
        Err(Error::Status { status, body }) => {
            assert_eq!(status, 429);
            assert_eq!(body, "rate limited");
        }
        other => panic!("expected status error, got {other:?}"),
    }
}

#[tokio::test]
async fn slow_provider_times_out() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(5)))
        .mount(&server)
        .await;

    let gw = Gateway::with_timeouts(
        AiConfig {
            roles: [(Role::Complete, provider(&server, "m", None))].into(),
        },
        Duration::from_secs(1),
        Duration::from_millis(200),
    )
    .unwrap();
    let err = gw
        .chat(Role::Complete, ChatRequest::new(vec![Message::user("hi")]))
        .await;
    assert!(matches!(err, Err(Error::Transport(e)) if e.is_timeout()));
}

#[tokio::test]
async fn malformed_response_is_invalid_response() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"choices": []})))
        .mount(&server)
        .await;

    let gw = gateway([(Role::Decide, provider(&server, "m", None))]);
    let err = gw
        .chat(Role::Decide, ChatRequest::new(vec![Message::user("hi")]))
        .await;
    assert!(matches!(err, Err(Error::InvalidResponse(_))));
}
