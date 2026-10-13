//! Telemetry the kernel and the server share: span field names, W3C trace
//! context helpers and the request and job instruments.
//!
//! Spans are `tracing` spans; the server bridges them to OpenTelemetry and
//! decides whether anything is exported (`CAMPBOOKS_TELEMETRY__*`). Without
//! that bridge (tests, the CLI subcommands) every helper here is a no-op, and
//! without a meter provider every instrument is.
//!
//! **Never put personal data or secrets in a span field, a log field or a
//! metric attribute**: no message content, tokens, cookies, codes or email
//! addresses. Ids are fine on spans, never on metrics (route templates,
//! names and small enums only).

use std::{collections::HashMap, fmt, sync::Arc, time::Duration};

use axum::http::{HeaderMap, HeaderName, HeaderValue};
use opentelemetry::{
    InstrumentationScope, KeyValue, global,
    metrics::{Gauge, Histogram, Meter},
    propagation::{Extractor, Injector, TextMapPropagator as _},
    trace::{SpanContext, TraceContextExt as _},
};
use opentelemetry_sdk::propagation::TraceContextPropagator;
use tracing::Span;
use tracing_opentelemetry::OpenTelemetrySpanExt as _;

/// Instrumentation scope of every Campbooks instrument and tracer.
pub const SCOPE: &str = "campbooks";

/// Span fields set outside the code that creates the span. A span that wants
/// them declares them `tracing::field::Empty`.
pub mod field {
    /// The W3C trace id (32 hex digits), so log lines join their trace.
    pub const TRACE_ID: &str = "trace_id";
    /// The workspace a request or job works in.
    pub const WORKSPACE_ID: &str = "workspace.id";
    /// `user` or `client`, once the caller is authenticated.
    pub const PRINCIPAL_KIND: &str = "principal.kind";
}

/// Histogram buckets for durations in seconds, from 5 ms to 10 min. The
/// SDK's default buckets are made for milliseconds.
pub const DURATION_BUCKETS: [f64; 16] = [
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0, 600.0,
];

/// The Campbooks meter of the installed meter provider (the API's no-op
/// until the server installs one).
pub fn meter() -> Meter {
    global::meter_with_scope(
        InstrumentationScope::builder(SCOPE)
            .with_version(env!("CARGO_PKG_VERSION"))
            .build(),
    )
}

/// A histogram of durations in seconds with [`DURATION_BUCKETS`].
pub fn duration_histogram(
    meter: &Meter,
    name: &'static str,
    description: &'static str,
) -> Histogram<f64> {
    meter
        .f64_histogram(name)
        .with_unit("s")
        .with_description(description)
        .with_boundaries(DURATION_BUCKETS.to_vec())
        .build()
}

// --- trace context ------------------------------------------------------------

/// Make the trace context of `headers` (W3C `traceparent`/`tracestate`), if
/// any, the parent of `span`. Call it before anything else touches the span
/// (it cannot change once the span is started).
pub fn set_parent_from(span: &Span, headers: &HeaderMap) {
    let parent = TraceContextPropagator::new().extract(&HeaderExtractor(headers));
    if parent.span().span_context().is_valid() {
        // Fails only without the OpenTelemetry layer, or once started.
        let _ = span.set_parent(parent);
    }
}

/// Write the current span's trace context into `headers` as W3C
/// `traceparent` (and `tracestate`), for a call to another system.
pub fn inject(headers: &mut HeaderMap) {
    let context = Span::current().context();
    if context.span().span_context().is_valid() {
        TraceContextPropagator::new().inject_context(&context, &mut HeaderInjector(headers));
    }
}

/// The current span's W3C `traceparent`, to store with work that runs later
/// (a job). `None` without a trace.
pub fn current_traceparent() -> Option<String> {
    let context = Span::current().context();
    if !context.span().span_context().is_valid() {
        return None;
    }
    let mut carrier = HashMap::new();
    TraceContextPropagator::new().inject_context(&context, &mut carrier);
    carrier.remove("traceparent")
}

/// Link `span` to the span a stored `traceparent` names (the one that
/// enqueued a job). Ignored when it does not parse. Call it before
/// [`record_trace_id`].
pub fn link_to(span: &Span, traceparent: &str) {
    if let Some(context) = parse_traceparent(traceparent) {
        span.add_link(context);
    }
}

/// The span context a `traceparent` names, if it is valid.
fn parse_traceparent(traceparent: &str) -> Option<SpanContext> {
    let carrier = HashMap::from([("traceparent".to_owned(), traceparent.to_owned())]);
    let context = TraceContextPropagator::new().extract(&carrier);
    let span_context = context.span().span_context().clone();
    span_context.is_valid().then_some(span_context)
}

/// Record `span`'s trace id in its [`field::TRACE_ID`] field. This starts
/// the span: set its parent and links first.
pub fn record_trace_id(span: &Span) {
    let context = span.context();
    let span_context = context.span().span_context().clone();
    if span_context.is_valid() {
        span.record(
            field::TRACE_ID,
            tracing::field::display(span_context.trace_id()),
        );
    }
}

/// The `error.type` of a failed HTTP call: a fixed word, never the error's
/// message (it can hold a URL).
pub fn http_error_type(error: &reqwest::Error) -> &'static str {
    if error.is_timeout() {
        "timeout"
    } else if error.is_connect() {
        "connect"
    } else if let Some(status) = error.status() {
        if status.is_server_error() {
            "status_5xx"
        } else {
            "status_4xx"
        }
    } else if error.is_decode() || error.is_body() {
        "invalid_response"
    } else {
        "request"
    }
}

struct HeaderExtractor<'a>(&'a HeaderMap);

impl Extractor for HeaderExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|value| value.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(HeaderName::as_str).collect()
    }
}

struct HeaderInjector<'a>(&'a mut HeaderMap);

impl Injector for HeaderInjector<'_> {
    fn set(&mut self, key: &str, value: String) {
        if let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(key.as_bytes()),
            HeaderValue::from_str(&value),
        ) {
            self.0.insert(name, value);
        }
    }
}

// --- instruments ------------------------------------------------------------------

/// How a job attempt ended, as the `job.outcome` attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobOutcome {
    /// Done; its transaction committed.
    Succeeded,
    /// Failed; it runs again later.
    Retried,
    /// Failed for good (dead).
    Failed,
    /// Another worker took it over; its work was rolled back.
    Lost,
}

impl JobOutcome {
    /// The attribute value.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Retried => "retried",
            Self::Failed => "failed",
            Self::Lost => "lost",
        }
    }
}

/// The request and job instruments, in [`AppState`](crate::AppState).
/// Cheap to clone. Attributes are route templates, names and small enums,
/// never ids.
#[derive(Clone)]
pub struct Metrics {
    instruments: Arc<Instruments>,
    exported: bool,
}

struct Instruments {
    http_duration: Histogram<f64>,
    job_duration: Histogram<f64>,
    job_wait: Histogram<f64>,
    jobs: Gauge<u64>,
    oldest_ready: Gauge<f64>,
}

impl fmt::Debug for Metrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Metrics")
            .field("exported", &self.exported)
            .finish_non_exhaustive()
    }
}

impl Metrics {
    /// Instruments from `meter`. `exported` says whether anything reads them
    /// (it turns on work done only for metrics, like sampling queue depth).
    pub fn new(meter: &Meter, exported: bool) -> Self {
        Self {
            instruments: Arc::new(Instruments {
                http_duration: duration_histogram(
                    meter,
                    "http.server.request.duration",
                    "Duration of HTTP requests, by route template and status.",
                ),
                job_duration: duration_histogram(
                    meter,
                    "campbooks.job.duration",
                    "Duration of job attempts, by kind, queue and outcome.",
                ),
                job_wait: duration_histogram(
                    meter,
                    "campbooks.job.wait",
                    "Time a ready job waited before a worker claimed it.",
                ),
                jobs: meter
                    .u64_gauge("campbooks.jobs")
                    .with_description("Unfinished and dead jobs, by queue and state.")
                    .build(),
                oldest_ready: meter
                    .f64_gauge("campbooks.jobs.oldest_ready_age")
                    .with_unit("s")
                    .with_description("Age of the oldest ready job, by queue.")
                    .build(),
            }),
            exported,
        }
    }

    /// Whether anything reads the instruments.
    pub fn exported(&self) -> bool {
        self.exported
    }

    /// One HTTP request served. `route` is the matched route template.
    pub fn http_request(&self, method: &str, route: Option<&str>, status: u16, elapsed: Duration) {
        let mut attributes = vec![
            KeyValue::new("http.request.method", method.to_owned()),
            KeyValue::new("http.response.status_code", i64::from(status)),
        ];
        if let Some(route) = route {
            attributes.push(KeyValue::new("http.route", route.to_owned()));
        }
        self.instruments
            .http_duration
            .record(elapsed.as_secs_f64(), &attributes);
    }

    pub(crate) fn job_finished(
        &self,
        kind: &str,
        queue: &str,
        outcome: JobOutcome,
        elapsed: Duration,
    ) {
        self.instruments.job_duration.record(
            elapsed.as_secs_f64(),
            &[
                KeyValue::new("job.kind", kind.to_owned()),
                KeyValue::new("job.queue", queue.to_owned()),
                KeyValue::new("job.outcome", outcome.as_str()),
            ],
        );
    }

    pub(crate) fn job_claimed(&self, kind: &str, queue: &str, waited_seconds: f64) {
        self.instruments.job_wait.record(
            waited_seconds.max(0.0),
            &[
                KeyValue::new("job.kind", kind.to_owned()),
                KeyValue::new("job.queue", queue.to_owned()),
            ],
        );
    }

    pub(crate) fn queue_depth(&self, queue: &str, state: &'static str, count: u64) {
        self.instruments.jobs.record(
            count,
            &[
                KeyValue::new("job.queue", queue.to_owned()),
                KeyValue::new("job.state", state),
            ],
        );
    }

    pub(crate) fn oldest_ready(&self, queue: &str, seconds: f64) {
        self.instruments.oldest_ready.record(
            seconds.max(0.0),
            &[KeyValue::new("job.queue", queue.to_owned())],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_valid_traceparents() {
        let context =
            parse_traceparent("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01").unwrap();
        assert_eq!(
            context.trace_id().to_string(),
            "4bf92f3577b34da6a3ce929d0e0e4736"
        );
        assert!(context.is_sampled());
        assert!(context.is_remote());
        for bad in [
            "",
            "garbage",
            "00-00000000000000000000000000000000-00f067aa0ba902b7-01",
            "00-4bf92f3577b34da6a3ce929d0e0e4736-0000000000000000-01",
        ] {
            assert!(parse_traceparent(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn without_the_bridge_helpers_do_nothing() {
        let span = tracing::info_span!("x", trace_id = tracing::field::Empty);
        let mut headers = HeaderMap::new();
        headers.insert(
            "traceparent",
            HeaderValue::from_static("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"),
        );
        set_parent_from(&span, &headers);
        record_trace_id(&span);
        let _entered = span.enter();
        let mut out = HeaderMap::new();
        inject(&mut out);
        assert!(out.is_empty());
        assert_eq!(current_traceparent(), None);
    }

    #[test]
    fn outcomes_are_fixed_words() {
        let words: Vec<_> = [
            JobOutcome::Succeeded,
            JobOutcome::Retried,
            JobOutcome::Failed,
            JobOutcome::Lost,
        ]
        .map(JobOutcome::as_str)
        .into();
        assert_eq!(words, ["succeeded", "retried", "failed", "lost"]);
    }
}
