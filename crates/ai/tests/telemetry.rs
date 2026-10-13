//! Every provider call is a client span that carries the trace on and never
//! records the prompt.

// Helpers outside `#[test]` fns are not covered by clippy's test exemptions.
#![allow(clippy::unwrap_used)]

use std::collections::HashMap;

use campbooks_ai::{AiConfig, ChatRequest, Gateway, Message, ProviderConfig, Role};
use opentelemetry::trace::{TraceContextExt as _, TracerProvider as _};
use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider};
use serde_json::json;
use tracing::Instrument as _;
use tracing_opentelemetry::OpenTelemetrySpanExt as _;
use tracing_subscriber::prelude::*;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

const PROMPT: &str = "Is the invoice from alice@example.com paid?";

#[tokio::test]
async fn chat_is_a_client_span_that_propagates_the_trace() {
    let exporter = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_simple_exporter(exporter.clone())
        .build();
    let subscriber = tracing_subscriber::registry()
        .with(tracing_opentelemetry::layer().with_tracer(provider.tracer("test")));
    let _guard = tracing::subscriber::set_default(subscriber);

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": "qwen3:8b-q4",
            "choices": [{"message": {"content": "no"}, "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 12, "completion_tokens": 1}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let gateway = Gateway::new(AiConfig {
        roles: HashMap::from([(
            Role::Complete,
            ProviderConfig::new(format!("{}/v1", server.uri()).parse().unwrap(), "qwen3:8b")
                .with_api_key("sk-secret"),
        )]),
    })
    .unwrap();

    let job = tracing::info_span!("job");
    let trace_id = job.context().span().span_context().trace_id();
    gateway
        .chat(
            Role::Complete,
            ChatRequest::new(vec![Message::user(PROMPT)]),
        )
        .instrument(job)
        .await
        .unwrap();

    let received = server.received_requests().await.unwrap();
    let traceparent = received[0].headers["traceparent"].to_str().unwrap();
    assert!(traceparent.contains(&trace_id.to_string()), "{traceparent}");

    let spans = exporter.get_finished_spans().unwrap();
    let call = spans
        .iter()
        .find(|span| span.name == "chat qwen3:8b")
        .unwrap_or_else(|| panic!("{spans:#?}"));
    let attribute = |key: &str| {
        call.attributes
            .iter()
            .find(|kv| kv.key.as_str() == key)
            .map(|kv| kv.value.to_string())
    };
    assert_eq!(call.span_context.trace_id(), trace_id);
    assert_eq!(attribute("gen_ai.operation.name").as_deref(), Some("chat"));
    assert_eq!(attribute("campbooks.ai.role").as_deref(), Some("complete"));
    assert_eq!(
        attribute("gen_ai.response.model").as_deref(),
        Some("qwen3:8b-q4")
    );
    assert_eq!(
        attribute("gen_ai.usage.input_tokens").as_deref(),
        Some("12")
    );
    assert!(attribute("server.address").is_some());
    let everything = format!("{spans:?}");
    for secret in [PROMPT, "alice@example.com", "sk-secret"] {
        assert!(!everything.contains(secret), "{secret}: {everything}");
    }
}
