//! `campbooks-ai` — the role-based AI gateway.
//!
//! Every AI call in Campbooks goes through [`Gateway`] by **role**
//! ([`Role::Decide`], [`Role::Embed`], [`Role::Complete`], [`Role::Vision`]),
//! never by vendor. `embed`, `complete` and `vision` point at any provider
//! speaking the OpenAI-compatible HTTP API (`/v1/chat/completions`,
//! `/v1/embeddings`), so a self-hosted vLLM / llama.cpp / Ollama / TEI and a
//! hosted provider serving open weights are interchangeable (ADR 0005).
//!
//! `decide` is typed ([`decide`], ADR 0009): questions in, answers with
//! probabilities out, served by Laya (`laya-serve`) or by an instruct model
//! through an adapter.

#![forbid(unsafe_code)]

pub mod catalog;
mod config;
pub mod decide;
mod error;
mod gateway;
mod telemetry;

pub use config::{AiConfig, Protocol, ProviderConfig, Role};
pub use error::Error;
pub use gateway::{
    ChatRequest, ChatResponse, Content, ContentPart, DEFAULT_CONNECT_TIMEOUT,
    DEFAULT_REQUEST_TIMEOUT, Gateway, ImageUrl, JsonSchemaFormat, Message, MessageRole,
    ResponseFormat, RetryPolicy, Usage,
};
