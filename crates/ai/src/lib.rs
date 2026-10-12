//! `campbooks-ai` — the role-based AI gateway.
//!
//! Every AI call in Campbooks goes through [`Gateway`] by **role**
//! ([`Role::Decide`], [`Role::Embed`], [`Role::Complete`], [`Role::Vision`]),
//! never by vendor. Each role points at any provider speaking the
//! OpenAI-compatible HTTP API (`/v1/chat/completions`, `/v1/embeddings`), so a
//! self-hosted vLLM / llama.cpp / Ollama / TEI and a hosted provider serving
//! open weights are interchangeable. See ADR 0005.

#![forbid(unsafe_code)]

pub mod catalog;
mod config;
mod error;
mod gateway;

pub use config::{AiConfig, ProviderConfig, Role};
pub use error::Error;
pub use gateway::{
    ChatRequest, ChatResponse, Content, ContentPart, DEFAULT_CONNECT_TIMEOUT,
    DEFAULT_REQUEST_TIMEOUT, Gateway, ImageUrl, JsonSchemaFormat, Message, MessageRole,
    ResponseFormat, Usage,
};
