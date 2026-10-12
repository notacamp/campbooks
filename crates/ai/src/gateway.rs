//! The gateway: one client for every role, over the OpenAI-compatible API.

use std::time::Duration;

use reqwest::{Client, RequestBuilder};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use url::Url;

use crate::{AiConfig, Error, ProviderConfig, Role};

/// Default TCP connect timeout.
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Default whole-request timeout. Local CPU inference is slow; keep it generous.
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_mins(2);
/// Longest error body kept in [`Error::Status`].
const MAX_ERROR_BODY: usize = 2048;

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

/// Token accounting, when the provider reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
pub struct Usage {
    /// Prompt tokens.
    #[serde(default)]
    pub prompt_tokens: u32,
    /// Generated tokens.
    #[serde(default)]
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
    /// Parse structured (`decide`) output.
    pub fn json<T: DeserializeOwned>(&self) -> Result<T, Error> {
        serde_json::from_str(&self.content)
            .map_err(|e| Error::InvalidResponse(format!("content is not the expected JSON: {e}")))
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
            .user_agent(concat!("campbooks/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self { config, http })
    }

    /// The configuration in use.
    pub fn config(&self) -> &AiConfig {
        &self.config
    }

    /// Run a chat completion with the provider configured for `role`.
    pub async fn chat(&self, role: Role, request: ChatRequest) -> Result<ChatResponse, Error> {
        let provider = self.config.provider(role)?;
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
        let provider = self.config.provider(Role::Embed)?;
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
        Ok(wire.data.into_iter().map(|e| e.embedding).collect())
    }

    fn post(&self, provider: &ProviderConfig, path: &str) -> Result<RequestBuilder, Error> {
        let url = endpoint(&provider.base_url, path)?;
        let mut request = self.http.post(url);
        if let Some(key) = &provider.api_key {
            request = request.bearer_auth(key.expose_secret());
        }
        Ok(request)
    }
}

/// `base_url` + `path`, treating `base_url` as a directory whether or not it
/// ends with a slash.
fn endpoint(base_url: &Url, path: &str) -> Result<Url, Error> {
    let mut base = base_url.clone();
    if !base.path().ends_with('/') {
        let with_slash = format!("{}/", base.path());
        base.set_path(&with_slash);
    }
    base.join(path)
        .map_err(|e| Error::Config(format!("invalid base_url {base_url}: {e}")))
}

async fn send<T: DeserializeOwned>(request: RequestBuilder) -> Result<T, Error> {
    let response = request.send().await?;
    let status = response.status();
    if !status.is_success() {
        let mut body = response.text().await.unwrap_or_default();
        if body.len() > MAX_ERROR_BODY {
            let cut = (0..=MAX_ERROR_BODY)
                .rev()
                .find(|i| body.is_char_boundary(*i))
                .unwrap_or(0);
            body.truncate(cut);
        }
        return Err(Error::Status {
            status: status.as_u16(),
            body,
        });
    }
    let bytes = response.bytes().await?;
    serde_json::from_slice(&bytes).map_err(|e| Error::InvalidResponse(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

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
