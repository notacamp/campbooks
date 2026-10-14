//! Spans and metrics around every provider call.
//!
//! Attributes follow the OpenTelemetry `gen_ai` conventions where they
//! exist. They name the operation, the role, the provider's address and the
//! model, and count tokens. They never carry prompts, messages, embedding
//! inputs, decision states or answers.

use std::{fmt, time::Instant};

use campbooks_kernel::telemetry as kernel;
use opentelemetry::{
    KeyValue,
    metrics::{Histogram, Meter},
};
use tracing::Span;

use crate::{Error, ProviderConfig, Role, Usage};

/// Token-count buckets (powers of 4), as the conventions suggest.
const TOKEN_BUCKETS: [f64; 12] = [
    1.0,
    4.0,
    16.0,
    64.0,
    256.0,
    1_024.0,
    4_096.0,
    16_384.0,
    65_536.0,
    262_144.0,
    1_048_576.0,
    4_194_304.0,
];

/// The gateway's instruments.
#[derive(Clone)]
pub(crate) struct Instruments {
    duration: Histogram<f64>,
    tokens: Histogram<u64>,
}

impl fmt::Debug for Instruments {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Instruments").finish_non_exhaustive()
    }
}

impl Instruments {
    /// Instruments from the installed meter provider.
    pub(crate) fn new() -> Self {
        Self::from_meter(&kernel::meter())
    }

    fn from_meter(meter: &Meter) -> Self {
        Self {
            duration: kernel::duration_histogram(
                meter,
                "gen_ai.client.operation.duration",
                "Duration of AI provider calls, by operation, role, provider and model.",
            ),
            tokens: meter
                .u64_histogram("gen_ai.client.token.usage")
                .with_unit("{token}")
                .with_description("Tokens per AI provider call, input and output.")
                .with_boundaries(TOKEN_BUCKETS.to_vec())
                .build(),
        }
    }
}

/// One provider call being observed: its span, its start, its attributes.
pub(crate) struct Call<'a> {
    instruments: &'a Instruments,
    span: Span,
    started: Instant,
    attributes: Vec<KeyValue>,
}

impl<'a> Call<'a> {
    /// Start observing `operation` (`chat`, `embeddings`, `decide`) for
    /// `role` against `provider`.
    pub(crate) fn start(
        instruments: &'a Instruments,
        operation: &'static str,
        role: Role,
        provider: &ProviderConfig,
    ) -> Self {
        let address = address(provider);
        let span = tracing::info_span!(
            "ai",
            otel.name = %format_args!("{operation} {}", provider.model),
            otel.kind = "client",
            otel.status_code = tracing::field::Empty,
            gen_ai.operation.name = operation,
            campbooks.ai.role = role.as_str(),
            gen_ai.request.model = %provider.model,
            gen_ai.response.model = tracing::field::Empty,
            gen_ai.usage.input_tokens = tracing::field::Empty,
            gen_ai.usage.output_tokens = tracing::field::Empty,
            server.address = %address,
            error.type = tracing::field::Empty,
        );
        Self {
            instruments,
            span,
            started: Instant::now(),
            attributes: vec![
                KeyValue::new("gen_ai.operation.name", operation),
                KeyValue::new("campbooks.ai.role", role.as_str()),
                KeyValue::new("gen_ai.request.model", provider.model.clone()),
                KeyValue::new("server.address", address),
            ],
        }
    }

    /// The call's span, to instrument the request with.
    pub(crate) fn span(&self) -> Span {
        self.span.clone()
    }

    /// Record how the call ended: the response model and token usage on
    /// success, the error type on failure.
    pub(crate) fn finish<T>(
        mut self,
        result: &Result<T, Error>,
        details: impl FnOnce(&T) -> (Option<&str>, Option<Usage>),
    ) {
        let elapsed = self.started.elapsed().as_secs_f64();
        match result {
            Ok(value) => {
                let (model, usage) = details(value);
                if let Some(model) = model.filter(|m| !m.is_empty()) {
                    self.span.record("gen_ai.response.model", model);
                }
                if let Some(usage) = usage {
                    self.span
                        .record("gen_ai.usage.input_tokens", usage.prompt_tokens);
                    self.span
                        .record("gen_ai.usage.output_tokens", usage.completion_tokens);
                    for (kind, count) in [
                        ("input", usage.prompt_tokens),
                        ("output", usage.completion_tokens),
                    ] {
                        let mut attributes = self.attributes.clone();
                        attributes.push(KeyValue::new("gen_ai.token.type", kind));
                        self.instruments
                            .tokens
                            .record(u64::from(count), &attributes);
                    }
                }
            }
            Err(error) => {
                let kind = error_type(error);
                self.span.record("otel.status_code", "error");
                self.span.record("error.type", kind);
                self.attributes.push(KeyValue::new("error.type", kind));
            }
        }
        self.instruments.duration.record(elapsed, &self.attributes);
    }
}

/// `host[:port]` of the provider: low-cardinality, never a path or a key.
fn address(provider: &ProviderConfig) -> String {
    let url = &provider.base_url;
    let host = url.host_str().unwrap_or_default();
    match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_owned(),
    }
}

/// A fixed word for the failure, never its message (a provider's error body
/// can echo the prompt).
fn error_type(error: &Error) -> &'static str {
    match error {
        Error::RoleNotConfigured(_) | Error::Config(_) => "config",
        Error::InvalidQuestion(_) => "invalid_question",
        Error::Transport(e) => kernel::http_error_type(e),
        Error::Status { status, .. } if *status >= 500 => "status_5xx",
        Error::Status { .. } => "status_4xx",
        Error::InvalidResponse(_) => "invalid_response",
        Error::ExternalContentRefused { .. } => "external_content_refused",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_types_are_fixed_words() {
        assert_eq!(error_type(&Error::Config("x".into())), "config");
        assert_eq!(error_type(&Error::Status { status: 503 }), "status_5xx");
        assert_eq!(error_type(&Error::Status { status: 400 }), "status_4xx");
        assert_eq!(
            error_type(&Error::InvalidResponse("x".into())),
            "invalid_response"
        );
    }

    #[test]
    fn address_is_host_and_explicit_port() {
        let provider = |url: &str| ProviderConfig::new(url.parse().unwrap(), "m");
        assert_eq!(address(&provider("http://ollama:11434/v1")), "ollama:11434");
        assert_eq!(
            address(&provider("https://api.example.com/v1/x")),
            "api.example.com"
        );
    }
}
