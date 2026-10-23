//! Gateway behaviour against a mock OpenAI-compatible provider.

// Helpers outside `#[test]` fns are not covered by clippy's test exemptions.
#![allow(clippy::unwrap_used)]

use std::{collections::HashMap, time::Duration};

use campbooks_ai::{
    AiConfig, ChatEvent,
    ChatEvent::{Done, Reasoning, Text},
    ChatRequest, Error, Gateway, Message, ProviderConfig, Role,
};
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
        ..AiConfig::default()
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
async fn provider_errors_surface_the_status_never_the_body() {
    // A provider's error can echo the request: the mail it was given.
    let echo = "Subject: Q4 plan — from maya@northwind-studio.example";
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(400).set_body_string(echo))
        .mount(&server)
        .await;

    let gw = gateway([(Role::Complete, provider(&server, "m", None))]);
    let err = gw
        .chat(Role::Complete, ChatRequest::new(vec![Message::user(echo)]))
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Status { status: 400 }), "{err:?}");
    for shown in [err.to_string(), format!("{err:?}")] {
        assert!(!shown.contains("maya"), "{shown}");
        assert!(!shown.contains("Q4"), "{shown}");
    }
}

#[tokio::test]
async fn an_unreadable_answer_is_described_without_its_text() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"choices": "Dear Maya, the Q4 plan is attached"})),
        )
        .mount(&server)
        .await;
    let gw = gateway([(Role::Complete, provider(&server, "m", None))]);
    let err = gw
        .chat(Role::Complete, ChatRequest::new(vec![Message::user("hi")]))
        .await
        .unwrap_err();
    assert!(matches!(err, Error::InvalidResponse(_)), "{err:?}");
    for shown in [err.to_string(), format!("{err:?}")] {
        assert!(!shown.contains("Maya"), "{shown}");
    }
}

#[tokio::test]
async fn an_off_network_role_gets_no_content_without_opt_in() {
    // Nothing listens there, and nothing is sent: the gateway refuses first.
    let external = ProviderConfig::new("https://api.example.com/v1".parse().unwrap(), "m");
    let gw = gateway([
        (Role::Complete, external.clone()),
        (Role::Embed, external.clone()),
    ]);
    let refused = |result: Result<_, Error>| {
        matches!(
            result,
            Err(Error::ExternalContentRefused { host, .. }) if host == "api.example.com"
        )
    };
    assert!(refused(
        gw.chat(Role::Complete, ChatRequest::new(vec![Message::user("hi")]))
            .await
            .map(|_| ())
    ));
    assert!(refused(gw.embed(&["hi".into()]).await.map(|_| ())));
    let err = gw
        .chat(Role::Complete, ChatRequest::default())
        .await
        .unwrap_err();
    assert!(
        err.to_string()
            .contains("CAMPBOOKS_AI__COMPLETE__EXTERNAL_CONTENT=allow"),
        "{err}"
    );
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
            ..AiConfig::default()
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

#[tokio::test]
async fn chat_and_embed_never_follow_a_redirect_off_the_box() {
    let elsewhere = MockServer::start().await;
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(308)
                .insert_header("location", format!("{}/v1/x", elsewhere.uri())),
        )
        .mount(&server)
        .await;
    let gw = gateway([
        (Role::Complete, provider(&server, "m", None)),
        (Role::Embed, provider(&server, "e", None)),
    ]);
    let chat = gw
        .chat(
            Role::Complete,
            ChatRequest::new(vec![Message::user("Subject: Q4 plan")]),
        )
        .await;
    assert!(
        matches!(chat, Err(Error::Status { status: 308 })),
        "{chat:?}"
    );
    let embed = gw.embed(&["Subject: Q4 plan".into()]).await;
    assert!(
        matches!(embed, Err(Error::Status { status: 308 })),
        "{embed:?}"
    );
    assert!(
        elsewhere.received_requests().await.unwrap().is_empty(),
        "the mail went to the redirect target"
    );
}

// --- streamed chat (campbooks-cloud#288) -----------------------------------

const STREAM: &str = "\
data: {\"model\":\"m-1\",\"choices\":[{\"delta\":{\"reasoning\":\"Thinking.\"}}]}\n\n\
data: {\"choices\":[{\"delta\":{\"content\":\"Hi\"}}]}\n\n\
data: {\"choices\":[{\"delta\":{\"content\":\" Maya.\"},\"finish_reason\":\"stop\"}]}\n\n\
data: {\"choices\":[],\"usage\":{\"prompt_tokens\":12,\"completion_tokens\":5}}\n\n\
data: [DONE]\n\n";

async fn stream_server(body: &str) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body.to_owned(), "text/event-stream"))
        .mount(&server)
        .await;
    server
}

async fn all(gw: &Gateway, request: ChatRequest) -> Result<Vec<Result<ChatEvent, Error>>, Error> {
    use futures_util::StreamExt as _;
    Ok(gw
        .chat_stream(Role::Complete, request)
        .await?
        .collect()
        .await)
}

#[tokio::test]
async fn chat_stream_yields_reasoning_text_and_usage() {
    let server = stream_server(STREAM).await;
    let gw = gateway([(Role::Complete, provider(&server, "m", None))]);
    let events = all(&gw, ChatRequest::new(vec![Message::user("hi")]))
        .await
        .unwrap();
    let events: Vec<ChatEvent> = events.into_iter().map(Result::unwrap).collect();
    assert_eq!(events[0], Reasoning("Thinking.".into()));
    assert_eq!(events[1], Text("Hi".into()));
    assert_eq!(events[2], Text(" Maya.".into()));
    let Done(summary) = &events[3] else {
        panic!("{:?}", events[3]);
    };
    assert_eq!(summary.model, "m-1");
    assert_eq!(summary.finish_kind(), "stop");
    assert_eq!(summary.usage.unwrap().prompt_tokens, 12);
    assert!(summary.ttft.is_some());

    let sent: serde_json::Value =
        serde_json::from_slice(&server.received_requests().await.unwrap()[0].body).unwrap();
    assert_eq!(sent["stream"], true);
    assert_eq!(sent["stream_options"]["include_usage"], true);
    assert!(sent.get("reasoning").is_none(), "no effort set, none sent");
}

#[tokio::test]
async fn a_reasoning_effort_is_sent_when_set() {
    let server = stream_server(STREAM).await;
    let gw = gateway([(
        Role::Complete,
        provider(&server, "m", None).with_reasoning_effort(campbooks_ai::ReasoningEffort::Low),
    )]);
    all(&gw, ChatRequest::new(vec![Message::user("hi")]))
        .await
        .unwrap();
    // And on a plain chat call, which this server answers with a stream it
    // cannot read: only the request matters here.
    let _ = gw
        .chat(Role::Complete, ChatRequest::new(vec![Message::user("hi")]))
        .await;
    for request in server.received_requests().await.unwrap() {
        let sent: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
        assert_eq!(sent["reasoning"], json!({"effort": "low"}));
    }
}

#[tokio::test]
async fn a_refused_stream_fails_before_any_event_without_the_body() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Subject: Q4 plan"))
        .mount(&server)
        .await;
    let gw = gateway([(Role::Complete, provider(&server, "m", None))]);
    let err = gw
        .chat_stream(Role::Complete, ChatRequest::new(vec![Message::user("hi")]))
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Status { status: 500 }), "{err:?}");
    assert!(!err.to_string().contains("Q4 plan"));
}

#[tokio::test]
async fn a_server_that_ignores_stream_is_read_whole() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": "m",
            "choices": [{"message": {"role": "assistant", "content": "<think>Hm.</think>Hi Maya."},
                         "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 3, "completion_tokens": 4}
        })))
        .mount(&server)
        .await;
    let gw = gateway([(Role::Complete, provider(&server, "m", None))]);
    let events: Vec<_> = all(&gw, ChatRequest::new(vec![Message::user("hi")]))
        .await
        .unwrap()
        .into_iter()
        .map(Result::unwrap)
        .collect();
    assert_eq!(events[0], Reasoning("Hm.".into()));
    assert_eq!(events[1], Text("Hi Maya.".into()));
    assert!(matches!(&events[2], Done(s) if s.usage.unwrap().completion_tokens == 4));
}

#[tokio::test]
async fn streaming_needs_the_openai_protocol() {
    let server = stream_server(STREAM).await;
    let gw = gateway([(
        Role::Complete,
        provider(&server, "m", None).with_protocol(campbooks_ai::Protocol::Laya),
    )]);
    let err = gw
        .chat_stream(Role::Complete, ChatRequest::new(vec![Message::user("hi")]))
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Config(_)), "{err:?}");
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn a_request_timeout_bounds_one_call() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(5)))
        .mount(&server)
        .await;
    let gw = gateway([(Role::Complete, provider(&server, "m", None))]);
    let mut request = ChatRequest::new(vec![Message::user("hi")]);
    request.timeout = Some(Duration::from_millis(200));
    let err = gw.chat(Role::Complete, request.clone()).await;
    assert!(
        matches!(err, Err(Error::Transport(ref e)) if e.is_timeout()),
        "{err:?}"
    );
    let err = gw.chat_stream(Role::Complete, request).await;
    assert!(
        matches!(err, Err(Error::Transport(ref e)) if e.is_timeout()),
        "{err:?}"
    );
}

/// What a test subscriber wrote: every log line of every level.
#[derive(Clone, Default)]
struct Captured(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

impl std::io::Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Captured {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

impl Captured {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }
}

fn find_invoices() -> campbooks_ai::ToolDefinition {
    campbooks_ai::ToolDefinition::new(
        "find_conversations",
        "Find conversations.",
        json!({"type": "object", "additionalProperties": false,
               "properties": {"query": {"type": "string"}}}),
    )
}

#[tokio::test]
async fn a_whole_answer_carries_its_tool_calls_and_logs_none_of_them() {
    let captured = Captured::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(captured.clone())
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(body_partial_json(json!({
            "tools": [{"type": "function", "function": {"name": "find_conversations"}}],
            "tool_choice": "auto",
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": "m",
            "choices": [{
                "message": {"role": "assistant", "content": null, "tool_calls": [
                    {"id": "a", "type": "function",
                     "function": {"name": "find_conversations", "arguments": "{\"query\": \"marta@clientes.example\"}"}},
                    {"id": "b", "type": "function",
                     "function": {"name": "what_needs_me", "arguments": "{}"}},
                ]},
                "finish_reason": "tool_calls",
            }],
        })))
        .expect(1)
        .mount(&server)
        .await;
    let gw = gateway([(Role::Complete, provider(&server, "m", None))]);
    let mut request = ChatRequest::new(vec![Message::user("Has Marta written before?")]);
    request.tools = vec![find_invoices()];
    request.tool_choice = Some(campbooks_ai::ToolChoice::Auto);
    let response = gw.chat(Role::Complete, request).await.unwrap();
    assert_eq!(response.content, "");
    assert_eq!(response.finish_kind(), "tool_calls");
    let names: Vec<&str> = response
        .tool_calls
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(names, ["find_conversations", "what_needs_me"]);
    assert_eq!(
        response.tool_calls[0].arguments,
        "{\"query\": \"marta@clientes.example\"}"
    );
    let logged = captured.text();
    assert!(!logged.contains("marta@clientes.example"), "{logged}");
    assert!(!logged.contains("Has Marta"), "{logged}");
}

#[tokio::test]
async fn a_streamed_tool_call_is_yielded_whole_and_never_logged() {
    let captured = Captured::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(captured.clone())
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    let body = "\
data: {\"model\":\"m\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"function\":{\"name\":\"find_people\",\"arguments\":\"{\\\"query\\\": \"}}]}}]}\n\n\
data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"\\\"marta@clientes\"}}]}}]}\n\n\
data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\".example\\\"}\"}}]},\"finish_reason\":\"tool_calls\"}]}\n\n\
data: [DONE]\n\n";
    let server = stream_server(body).await;
    let gw = gateway([(Role::Complete, provider(&server, "m", None))]);
    let mut request = ChatRequest::new(vec![Message::user("Who is Marta?")]);
    request.tools = vec![find_invoices()];
    let events: Vec<ChatEvent> = all(&gw, request)
        .await
        .unwrap()
        .into_iter()
        .map(Result::unwrap)
        .filter(|event| *event != ChatEvent::Alive)
        .collect();
    assert_eq!(events.len(), 2, "{events:?}");
    let ChatEvent::ToolCall(call) = &events[0] else {
        panic!("{events:?}");
    };
    assert_eq!(call.id, "call_1");
    assert_eq!(call.name, "find_people");
    assert_eq!(call.arguments, "{\"query\": \"marta@clientes.example\"}");
    assert!(matches!(&events[1], Done(summary) if summary.finish_kind() == "tool_calls"));
    let logged = captured.text();
    assert!(!logged.contains("marta@clientes"), "{logged}");
    assert!(!logged.contains("Who is Marta"), "{logged}");
}
