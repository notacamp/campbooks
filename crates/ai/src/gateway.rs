//! The gateway: one client for every role. `embed`, `complete` and `vision`
//! speak the OpenAI-compatible API; `decide` speaks Laya's typed protocol or
//! goes through the instruct-model adapter (ADR 0009).

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use reqwest::{Client, RequestBuilder, StatusCode, header::RETRY_AFTER};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use url::Url;

use tracing::Instrument as _;

use crate::{
    AiConfig, Error, Protocol, ProviderConfig, Role,
    decide::{self, Answers, DecisionContext, DecisionState, Question},
    telemetry::{Call, Instruments},
    usage::{self, Meter},
};

/// Default TCP connect timeout.
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Default whole-request timeout. Local CPU inference is slow; keep it generous.
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_mins(2);

/// How `decide` retries a provider that is down, restarting or busy.
///
/// A request is retried after a connection failure or a 5xx answer (Laya
/// answers 503 with `Retry-After` when it is saturated), waiting `base_delay`
/// and doubling each time up to `max_delay`; a `Retry-After` header wins when
/// it asks for longer. Timeouts are not retried: on a slow CPU host a retry
/// only doubles the queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Attempts in total, the first included (1 disables retries).
    pub attempts: u32,
    /// Wait before the first retry.
    pub base_delay: Duration,
    /// Longest wait between attempts.
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            attempts: 3,
            base_delay: Duration::from_millis(250),
            max_delay: Duration::from_secs(4),
        }
    }
}

impl RetryPolicy {
    fn delay(&self, retry: u32, retry_after: Option<Duration>) -> Duration {
        let backoff = self
            .base_delay
            .saturating_mul(2u32.saturating_pow(retry))
            .min(self.max_delay);
        retry_after.map_or(backoff, |after| after.min(self.max_delay).max(backoff))
    }
}

/// Who said a chat message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// Instructions.
    System,
    /// The user's input.
    User,
    /// A previous model reply.
    Assistant,
}

/// Message content: plain text, or parts (text and images, for `vision`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Content {
    /// Plain text.
    Text(String),
    /// Multi-part content.
    Parts(Vec<ContentPart>),
}

/// One part of a multi-part message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    /// A text part.
    Text {
        /// The text.
        text: String,
    },
    /// An image, by URL or `data:` URI.
    ImageUrl {
        /// The image reference.
        image_url: ImageUrl,
    },
}

/// An image reference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageUrl {
    /// `https://…` or `data:image/png;base64,…`.
    pub url: String,
}

/// A chat message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    /// Author.
    pub role: MessageRole,
    /// Content.
    pub content: Content,
}

impl Message {
    /// A system message.
    pub fn system(text: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: Content::Text(text.into()),
        }
    }

    /// A user message.
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: Content::Text(text.into()),
        }
    }
}

/// Constrains the shape of the model's answer.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseFormat {
    /// Any valid JSON object.
    JsonObject,
    /// JSON matching a schema — the normal format for `decide`.
    JsonSchema {
        /// The schema.
        json_schema: JsonSchemaFormat,
    },
}

impl ResponseFormat {
    /// A strict JSON-schema response format.
    pub fn json_schema(name: impl Into<String>, schema: serde_json::Value) -> Self {
        Self::JsonSchema {
            json_schema: JsonSchemaFormat {
                name: name.into(),
                schema,
                strict: true,
            },
        }
    }
}

/// A named JSON schema.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JsonSchemaFormat {
    /// Schema name (`[a-zA-Z0-9_-]`).
    pub name: String,
    /// The JSON Schema.
    pub schema: serde_json::Value,
    /// Ask the provider to enforce the schema exactly.
    pub strict: bool,
}

/// A chat completion request. The model comes from the role's config.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChatRequest {
    /// Conversation so far.
    pub messages: Vec<Message>,
    /// Sampling temperature.
    pub temperature: Option<f32>,
    /// Upper bound on generated tokens.
    pub max_tokens: Option<u32>,
    /// Required answer shape.
    pub response_format: Option<ResponseFormat>,
}

impl ChatRequest {
    /// A request with these messages and provider defaults.
    pub fn new(messages: Vec<Message>) -> Self {
        Self {
            messages,
            ..Self::default()
        }
    }

    /// Require JSON matching `schema`.
    #[must_use]
    pub fn with_json_schema(mut self, name: impl Into<String>, schema: serde_json::Value) -> Self {
        self.response_format = Some(ResponseFormat::json_schema(name, schema));
        self
    }
}

/// Token accounting, when the provider reports it: OpenAI-compatible
/// servers name the counts `prompt_tokens`/`completion_tokens`, laya-serve
/// `input_tokens`/`output_tokens`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
pub struct Usage {
    /// Prompt tokens.
    #[serde(default, alias = "input_tokens")]
    pub prompt_tokens: u32,
    /// Generated tokens.
    #[serde(default, alias = "output_tokens")]
    pub completion_tokens: u32,
}

/// The model's answer.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatResponse {
    /// Generated text (JSON when a JSON response format was requested).
    pub content: String,
    /// Model that answered, as reported by the provider.
    pub model: String,
    /// Why generation stopped (`stop`, `length`, …).
    pub finish_reason: Option<String>,
    /// Token usage.
    pub usage: Option<Usage>,
}

impl ChatResponse {
    /// Why the model stopped, for a log line: one of a few known words,
    /// never whatever string the provider sent.
    #[must_use]
    pub fn finish_kind(&self) -> &'static str {
        match self.finish_reason.as_deref() {
            None => "none",
            Some("stop") => "stop",
            Some("length") => "length",
            Some("content_filter") => "content_filter",
            Some("tool_calls") => "tool_calls",
            Some(_) => "other",
        }
    }

    /// Parse structured output. Some models wrap the JSON in one Markdown
    /// code fence (` ```json … ``` ` or ` ``` … ``` `) even when a JSON
    /// response format was asked for; that one fence and the whitespace
    /// around it are dropped first. Nothing else is repaired:
    /// prose or truncated JSON is still an error.
    pub fn json<T: DeserializeOwned>(&self) -> Result<T, Error> {
        serde_json::from_str(unfenced(&self.content)).map_err(|e| {
            Error::InvalidResponse(format!(
                "content is not the expected JSON: {}",
                campbooks_kernel::redact::json(&e)
            ))
        })
    }
}

/// `content` without the whitespace around it and, when it is wrapped in
/// one Markdown code fence whose info string is empty or `json`, without
/// that fence. Anything else comes back trimmed, as it was.
fn unfenced(content: &str) -> &str {
    let trimmed = content.trim();
    let Some(inner) = trimmed
        .strip_prefix("```")
        .and_then(|rest| rest.strip_suffix("```"))
    else {
        return trimmed;
    };
    // The opening fence's line ends at the first line break.
    let Some((info, body)) = inner.split_once('\n') else {
        return trimmed;
    };
    let info = info.trim();
    if info.is_empty() || info.eq_ignore_ascii_case("json") {
        body.trim()
    } else {
        trimmed
    }
}

#[derive(Serialize)]
struct WireChatRequest<'a> {
    model: &'a str,
    messages: &'a [Message],
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<&'a ResponseFormat>,
    stream: bool,
}

/// A chat request that also asks for token log-probabilities (`decide`).
#[derive(Serialize)]
struct WireDecideChatRequest<'a> {
    #[serde(flatten)]
    chat: WireChatRequest<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    logprobs: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_logprobs: Option<u8>,
}

#[derive(Deserialize)]
struct WireChatResponse {
    #[serde(default)]
    model: String,
    choices: Vec<WireChoice>,
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct WireChoice {
    message: WireAssistantMessage,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct WireAssistantMessage {
    content: Option<String>,
}

#[derive(Serialize)]
struct WireEmbedRequest<'a> {
    model: &'a str,
    input: &'a [String],
}

#[derive(Deserialize)]
struct WireEmbedResponse {
    data: Vec<WireEmbedding>,
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct WireEmbedding {
    index: usize,
    embedding: Vec<f32>,
}

/// Role-based AI client. Cheap to clone.
#[derive(Debug, Clone)]
pub struct Gateway {
    config: AiConfig,
    http: Client,
    retry: RetryPolicy,
    instruments: Instruments,
    /// Recent reachability probes ([`Gateway::health`]).
    pub(crate) probes: crate::health::Probes,
    /// The `decide` role's last answers check
    /// ([`Gateway::answers_check`]).
    pub(crate) checks: crate::decide::check::Checks,
    /// Counts every model call and holds the daily cap ([`usage`]); `None`
    /// for tools and tests, whose calls need no scope.
    meter: Option<Arc<dyn Meter>>,
}

impl Gateway {
    /// A gateway with the default timeouts.
    pub fn new(config: AiConfig) -> Result<Self, Error> {
        Self::with_timeouts(config, DEFAULT_CONNECT_TIMEOUT, DEFAULT_REQUEST_TIMEOUT)
    }

    /// A gateway with explicit timeouts.
    pub fn with_timeouts(
        config: AiConfig,
        connect_timeout: Duration,
        request_timeout: Duration,
    ) -> Result<Self, Error> {
        let http = Client::builder()
            .connect_timeout(connect_timeout)
            .timeout(request_timeout)
            // Never follow a redirect: an endpoint on the instance's network
            // answering 307 to an outside host would otherwise have us POST
            // the mail there, past the external-content check. A 3xx is an
            // `Error::Status` like any other non-success answer.
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("campbooks/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            config,
            http,
            retry: RetryPolicy::default(),
            instruments: Instruments::new(),
            probes: crate::health::Probes::default(),
            checks: crate::decide::check::Checks::default(),
            meter: None,
        })
    }

    /// The same gateway counting every model call with `meter`: each call
    /// must then run in a [`usage::Scope`], and is refused over the cap.
    #[must_use]
    pub fn with_meter(mut self, meter: Arc<dyn Meter>) -> Self {
        self.meter = Some(meter);
        self
    }

    /// Whether every model call is counted (and capped).
    pub fn is_metered(&self) -> bool {
        self.meter.is_some()
    }

    pub(crate) fn meter(&self) -> Option<&Arc<dyn Meter>> {
        self.meter.as_ref()
    }

    /// The same gateway with another retry policy for `decide`.
    #[must_use]
    pub fn with_retry(mut self, retry: RetryPolicy) -> Self {
        self.retry = retry;
        self
    }

    /// The HTTP client every call uses.
    pub(crate) fn http(&self) -> &Client {
        &self.http
    }

    /// The configuration in use.
    pub fn config(&self) -> &AiConfig {
        &self.config
    }

    /// Run a chat completion with the provider configured for `role`. A
    /// provider off the instance's network gets nothing unless the role
    /// opted in ([`Error::ExternalContentRefused`]).
    pub async fn chat(&self, role: Role, request: ChatRequest) -> Result<ChatResponse, Error> {
        let provider = self.config.provider_for_content(role)?;
        let ticket = self.admit(role).await?;
        let call = Call::start(&self.instruments, "chat", role, provider);
        let result = self
            .chat_with(role, provider, request)
            .instrument(call.span())
            .await;
        call.finish(&result, |r| (Some(r.model.as_str()), r.usage));
        usage::settle(ticket, &result, |r| r.usage).await;
        result
    }

    async fn chat_with(
        &self,
        role: Role,
        provider: &ProviderConfig,
        request: ChatRequest,
    ) -> Result<ChatResponse, Error> {
        let body = WireChatRequest {
            model: &provider.model,
            messages: &request.messages,
            temperature: request.temperature,
            max_tokens: request.max_tokens,
            response_format: request.response_format.as_ref(),
            stream: false,
        };
        let request = self.post(provider, "chat/completions")?.json(&body);
        tracing::debug!(%role, model = %provider.model, "ai chat request");
        let wire: WireChatResponse = send(request).await?;
        let choice = wire
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| Error::InvalidResponse("no choices".into()))?;
        Ok(ChatResponse {
            content: choice.message.content.unwrap_or_default(),
            model: wire.model,
            finish_reason: choice.finish_reason,
            usage: wire.usage,
        })
    }

    /// Embed `inputs` with the `embed` role. Returns one vector per input, in order.
    pub async fn embed(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>, Error> {
        if inputs.is_empty() {
            return Ok(Vec::new());
        }
        let provider = self.config.provider_for_content(Role::Embed)?;
        let ticket = self.admit(Role::Embed).await?;
        let call = Call::start(&self.instruments, "embeddings", Role::Embed, provider);
        let result = self
            .embed_with(provider, inputs)
            .instrument(call.span())
            .await;
        call.finish(&result, |(_, usage)| (None, *usage));
        usage::settle(ticket, &result, |(_, usage)| *usage).await;
        result.map(|(vectors, _)| vectors)
    }

    async fn embed_with(
        &self,
        provider: &ProviderConfig,
        inputs: &[String],
    ) -> Result<(Vec<Vec<f32>>, Option<Usage>), Error> {
        let body = WireEmbedRequest {
            model: &provider.model,
            input: inputs,
        };
        let request = self.post(provider, "embeddings")?.json(&body);
        tracing::debug!(model = %provider.model, inputs = inputs.len(), "ai embed request");
        let mut wire: WireEmbedResponse = send(request).await?;
        if wire.data.len() != inputs.len() {
            return Err(Error::InvalidResponse(format!(
                "expected {} embeddings, got {}",
                inputs.len(),
                wire.data.len()
            )));
        }
        wire.data.sort_by_key(|e| e.index);
        Ok((
            wire.data.into_iter().map(|e| e.embedding).collect(),
            wire.usage,
        ))
    }

    /// Answer typed `questions` about `state` with the `decide` role, all in
    /// one call. Answers come back in question order, calibrated with the
    /// role's temperatures. The state is never logged.
    ///
    /// Questions are checked first ([`decide::MAX_OPTIONS`] options at most,
    /// unique ids and labels, …); a bad set fails with
    /// [`Error::InvalidQuestion`] before anything is sent.
    pub async fn decide(
        &self,
        ctx: &DecisionContext<'_>,
        state: &DecisionState,
        questions: &[Question],
    ) -> Result<Answers, Error> {
        let provider = self.config.provider_for_content(Role::Decide)?;
        decide::validate(state, questions)?;
        if questions.is_empty() {
            return Ok(Answers::default());
        }
        let ticket = self.admit(Role::Decide).await?;
        let call = Call::start(&self.instruments, "decide", Role::Decide, provider);
        let result = self
            .decide_with(ctx, provider, state, questions)
            .instrument(call.span())
            .await;
        call.finish(&result, |(_, usage)| (None, *usage));
        usage::settle(ticket, &result, |(_, usage)| *usage).await;
        result.map(|(answers, _)| answers)
    }

    async fn decide_with(
        &self,
        ctx: &DecisionContext<'_>,
        provider: &ProviderConfig,
        state: &DecisionState,
        questions: &[Question],
    ) -> Result<(Answers, Option<Usage>), Error> {
        tracing::debug!(
            purpose = ctx.purpose,
            protocol = provider.protocol.as_str(),
            model = %provider.model,
            questions = questions.len(),
            "ai decide request"
        );
        let started = Instant::now();
        let (raw, model, version, usage) = match provider.protocol {
            Protocol::Laya => {
                let body = decide::laya::Request {
                    model: &provider.model,
                    state,
                    questions,
                };
                let request = self.post(provider, provider.decide_path())?.json(&body);
                let wire: decide::laya::Response = self.send_with_retry(request).await?;
                let model = wire.model(&provider.model);
                let usage = wire.usage;
                (wire.into_raw(questions)?, model, None, usage)
            }
            Protocol::OpenAi => {
                let messages = decide::openai::messages(state, questions);
                let format =
                    ResponseFormat::json_schema("decision", decide::openai::schema(questions));
                let body = WireDecideChatRequest {
                    chat: WireChatRequest {
                        model: &provider.model,
                        messages: &messages,
                        temperature: Some(0.0),
                        // Room for a reasoning model to think before its
                        // JSON: a server default of a few hundred tokens
                        // cuts it off. The answer itself is short.
                        max_tokens: Some(decide::openai::MAX_TOKENS),
                        response_format: Some(&format),
                        stream: false,
                    },
                    logprobs: provider.logprobs.then_some(true),
                    top_logprobs: provider.logprobs.then_some(decide::openai::TOP_LOGPROBS),
                };
                let request = self.post(provider, provider.decide_path())?.json(&body);
                let wire: decide::openai::Response = self.send_with_retry(request).await?;
                let model = if wire.model.is_empty() {
                    provider.model.clone()
                } else {
                    wire.model.clone()
                };
                let version = wire.system_fingerprint.clone();
                let usage = wire.usage;
                (wire.into_raw(questions)?, model, version, usage)
            }
        };
        let latency = started.elapsed();
        tracing::debug!(
            purpose = ctx.purpose,
            model = %model,
            latency_ms = latency.as_millis(),
            "ai decide answered"
        );
        // A question left unanswered (only ever a catalogue one, see the
        // parsers) has no answer here.
        let answers = Answers::new(
            questions
                .iter()
                .zip(raw)
                .filter_map(|(question, raw)| {
                    raw.map(|raw| {
                        decide::finish(
                            question,
                            raw,
                            &provider.calibration,
                            &model,
                            version.as_deref(),
                            latency,
                        )
                    })
                })
                .collect(),
        );
        Ok((answers, usage))
    }

    async fn send_with_retry<T: DeserializeOwned>(
        &self,
        request: RequestBuilder,
    ) -> Result<T, Error> {
        let mut retry = 0;
        loop {
            let attempt = request
                .try_clone()
                .ok_or_else(|| Error::Config("request body cannot be retried".into()))?;
            let retry_after = match attempt.send().await {
                Ok(response) if response.status().is_server_error() => {
                    if retry + 1 >= self.retry.attempts {
                        return read(response).await;
                    }
                    retry_after(&response)
                }
                Ok(response) => return read(response).await,
                Err(err) if err.is_connect() && retry + 1 < self.retry.attempts => None,
                Err(err) => return Err(err.into()),
            };
            let delay = self.retry.delay(retry, retry_after);
            retry += 1;
            tracing::debug!(
                retry,
                delay_ms = delay.as_millis(),
                "ai provider unavailable, retrying"
            );
            tokio::time::sleep(delay).await;
        }
    }

    /// A POST to `provider`, carrying the current trace on (`traceparent`).
    fn post(&self, provider: &ProviderConfig, path: &str) -> Result<RequestBuilder, Error> {
        let url = endpoint(&provider.base_url, path)?;
        let mut trace = reqwest::header::HeaderMap::new();
        campbooks_kernel::telemetry::inject(&mut trace);
        let mut request = self.http.post(url).headers(trace);
        if let Some(key) = &provider.api_key {
            request = request.bearer_auth(key.expose_secret());
        }
        Ok(request)
    }
}

/// `base_url` + `path`, treating `base_url` as a directory whether or not it
/// ends with a slash. The result stays under `base_url` (same scheme, host,
/// port and path prefix), or it is a configuration error: the
/// external-content gate judges the base URL alone.
pub(crate) fn endpoint(base_url: &Url, path: &str) -> Result<Url, Error> {
    let mut base = base_url.clone();
    if !base.path().ends_with('/') {
        let with_slash = format!("{}/", base.path());
        base.set_path(&with_slash);
    }
    let url = base
        .join(path)
        .map_err(|e| Error::Config(format!("invalid base_url {base_url}: {e}")))?;
    let under_base = url.scheme() == base.scheme()
        && url.host() == base.host()
        && url.port_or_known_default() == base.port_or_known_default()
        && url.path().starts_with(base.path());
    if under_base {
        Ok(url)
    } else {
        Err(Error::Config(format!(
            "the path {path:?} leaves the base URL {base_url}"
        )))
    }
}

/// `Retry-After` in seconds, when a 503 carries one.
fn retry_after(response: &reqwest::Response) -> Option<Duration> {
    if response.status() != StatusCode::SERVICE_UNAVAILABLE {
        return None;
    }
    let seconds = response.headers().get(RETRY_AFTER)?.to_str().ok()?;
    seconds.trim().parse::<u64>().ok().map(Duration::from_secs)
}

async fn send<T: DeserializeOwned>(request: RequestBuilder) -> Result<T, Error> {
    read(request.send().await?).await
}

/// The answer as `T`. A non-success status keeps only its code: the body of
/// a provider's error can echo the request (and so the mail in it), so it
/// is read to free the connection and dropped.
async fn read<T: DeserializeOwned>(response: reqwest::Response) -> Result<T, Error> {
    let status = response.status();
    if !status.is_success() {
        drop(response.bytes().await);
        return Err(Error::Status {
            status: status.as_u16(),
        });
    }
    let bytes = response.bytes().await?;
    serde_json::from_slice(&bytes)
        .map_err(|e| Error::InvalidResponse(campbooks_kernel::redact::json(&e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_delay_doubles_up_to_the_cap_and_honours_retry_after() {
        let policy = RetryPolicy {
            attempts: 5,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(1),
        };
        assert_eq!(policy.delay(0, None), Duration::from_millis(100));
        assert_eq!(policy.delay(1, None), Duration::from_millis(200));
        assert_eq!(policy.delay(5, None), Duration::from_secs(1));
        assert_eq!(
            policy.delay(0, Some(Duration::from_millis(500))),
            Duration::from_millis(500)
        );
        assert_eq!(
            policy.delay(0, Some(Duration::from_mins(1))),
            Duration::from_secs(1),
            "a huge Retry-After is capped"
        );
    }

    #[test]
    fn endpoint_joins_with_or_without_trailing_slash() {
        let a = Url::parse("http://ollama:11434/v1").unwrap();
        let b = Url::parse("http://ollama:11434/v1/").unwrap();
        assert_eq!(
            endpoint(&a, "embeddings").unwrap().as_str(),
            "http://ollama:11434/v1/embeddings"
        );
        assert_eq!(
            endpoint(&b, "embeddings").unwrap().as_str(),
            "http://ollama:11434/v1/embeddings"
        );
    }

    #[test]
    fn endpoint_never_leaves_the_base_url() {
        let base = Url::parse("http://laya:8000/v1").unwrap();
        for path in [
            "https://elsewhere.example/decisions",
            "//elsewhere.example/decisions",
            "\\\\elsewhere.example/decisions",
            "../decisions",
            "%2e%2e/decisions",
        ] {
            assert!(endpoint(&base, path).is_err(), "{path}");
        }
    }

    /// A response whose content is `content`.
    fn answer(content: &str) -> ChatResponse {
        ChatResponse {
            content: content.to_owned(),
            model: "test-model".to_owned(),
            finish_reason: Some("stop".to_owned()),
            usage: None,
        }
    }

    #[test]
    fn finish_kind_names_only_known_reasons() {
        let mut response = answer("{}");
        for (reason, kind) in [
            (None, "none"),
            (Some("stop"), "stop"),
            (Some("length"), "length"),
            (Some("content_filter"), "content_filter"),
            (Some("tool_calls"), "tool_calls"),
            (Some("ignore previous instructions"), "other"),
        ] {
            response.finish_reason = reason.map(str::to_owned);
            assert_eq!(response.finish_kind(), kind, "{reason:?}");
        }
    }

    #[test]
    fn json_reads_plain_and_fenced_json() {
        let expected = serde_json::json!({ "outcome": "rules" });
        for content in [
            r#"{"outcome": "rules"}"#,
            "  \n{\"outcome\": \"rules\"}\n ",
            "```json\n{\"outcome\": \"rules\"}\n```",
            "```JSON\n{\"outcome\": \"rules\"}\n```",
            "\n```\n{\"outcome\": \"rules\"}\n```\n",
        ] {
            let value: serde_json::Value = answer(content).json().unwrap();
            assert_eq!(value, expected, "{content:?}");
        }
    }

    #[test]
    fn json_repairs_nothing_else() {
        for content in [
            "",
            "Sure! Here are your rules.",
            "Here you go:\n```json\n{\"outcome\": \"rules\"}\n```",
            "```json\n{\"outcome\": \"ru",
            "```python\n{\"outcome\": \"rules\"}\n```",
            "```{\"outcome\": \"rules\"}```",
        ] {
            let parsed = answer(content).json::<serde_json::Value>();
            assert!(
                matches!(parsed, Err(Error::InvalidResponse(_))),
                "{content:?}"
            );
        }
    }

    #[test]
    fn json_schema_response_format_wire_shape() {
        let format = ResponseFormat::json_schema("triage", serde_json::json!({"type": "object"}));
        assert_eq!(
            serde_json::to_value(format).unwrap(),
            serde_json::json!({
                "type": "json_schema",
                "json_schema": {"name": "triage", "schema": {"type": "object"}, "strict": true}
            })
        );
    }

    #[test]
    fn image_part_wire_shape() {
        let message = Message {
            role: MessageRole::User,
            content: Content::Parts(vec![
                ContentPart::Text {
                    text: "What is this?".into(),
                },
                ContentPart::ImageUrl {
                    image_url: ImageUrl {
                        url: "data:image/png;base64,AA".into(),
                    },
                },
            ]),
        };
        assert_eq!(
            serde_json::to_value(message).unwrap(),
            serde_json::json!({"role": "user", "content": [
                {"type": "text", "text": "What is this?"},
                {"type": "image_url", "image_url": {"url": "data:image/png;base64,AA"}}
            ]})
        );
    }
}
