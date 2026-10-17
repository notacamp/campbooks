//! Logs, traces and metrics: the subscriber, the OpenTelemetry providers and
//! exporters (off unless `CAMPBOOKS_TELEMETRY__*` says otherwise), the
//! request middleware and the operator's `/metrics` listener.
//!
//! See `openspec/changes/observability/design.md`.

use std::{collections::HashMap, io::Write as _, time::Duration};

use axum::{
    Router,
    extract::{MatchedPath, Request, State},
    http::{StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
    routing::get,
};
use campbooks_kernel::{
    config::{LogFormat, TelemetryConfig},
    error::ProblemCode,
    telemetry::{self, Metrics},
};
use opentelemetry::{
    InstrumentationScope, KeyValue, global,
    metrics::{Histogram, Meter, MeterProvider as _},
    trace::TracerProvider as _,
};
use opentelemetry_otlp::{
    MetricExporter, SpanExporter, WithExportConfig as _, WithHttpConfig as _,
};
use opentelemetry_sdk::{
    Resource,
    metrics::{PeriodicReader, SdkMeterProvider},
    trace::{Sampler, SdkTracerProvider, Tracer},
};
use sqlx::PgPool;
use tokio::time::Instant;
use tower_http::request_id::RequestId;
use tracing::{Instrument as _, Level, Subscriber, field::Empty};
use tracing_subscriber::{
    EnvFilter, Layer, Registry,
    filter::Targets,
    fmt::{self, MakeWriter},
    layer::Context,
    prelude::*,
    registry::LookupSpan,
};

/// `service.name` unless `OTEL_SERVICE_NAME` says otherwise.
pub const SERVICE_NAME: &str = "campbooks-server";

/// Log filter when `RUST_LOG` is unset.
pub const DEFAULT_LOG_FILTER: &str = "info,sqlx=warn";

/// sqlx's event for each pool acquire (`acquired_after_secs`), enabled at
/// DEBUG by the server's pool options.
const POOL_ACQUIRE_TARGET: &str = "sqlx::pool::acquire";

/// How long shutdown waits for the exporters to flush.
const FLUSH_TIMEOUT: Duration = Duration::from_secs(5);

/// The OpenTelemetry providers of this process.
pub struct Telemetry {
    tracer_provider: SdkTracerProvider,
    meter_provider: Option<SdkMeterProvider>,
    prometheus: Option<prometheus::Registry>,
    traces_exported: bool,
    error_reports: Option<sentry::ClientInitGuard>,
}

impl std::fmt::Debug for Telemetry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Telemetry")
            .field("traces_exported", &self.traces_exported)
            .field("metrics_exported", &self.meter_provider.is_some())
            .field("prometheus", &self.prometheus.is_some())
            .field("error_reports", &self.error_reports.is_some())
            .finish_non_exhaustive()
    }
}

impl Telemetry {
    /// Providers for `config`, for a process running `role` (`all`, `api`,
    /// `worker`). Installs the meter provider globally when metrics are
    /// exported, so build it before the state and the gateway.
    pub fn new(config: &TelemetryConfig, role: &'static str) -> Result<Self, String> {
        let resource = resource(role);
        let endpoint = config.otlp_endpoint();
        let headers: HashMap<String, String> = config.otlp_headers().into_iter().collect();

        let tracer_provider = SdkTracerProvider::builder().with_resource(resource.clone());
        let tracer_provider = match endpoint {
            Some(endpoint) => {
                let exporter = SpanExporter::builder()
                    .with_http()
                    .with_endpoint(format!("{endpoint}/v1/traces"))
                    .with_headers(headers.clone())
                    .build()
                    .map_err(|e| format!("CAMPBOOKS_TELEMETRY__OTLP_ENDPOINT (traces): {e}"))?;
                tracer_provider
                    .with_sampler(Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(
                        config.sample_ratio,
                    ))))
                    .with_batch_exporter(exporter)
            }
            // Spans still get trace ids (for the logs and outbound calls);
            // none is recorded.
            None => tracer_provider.with_sampler(Sampler::AlwaysOff),
        }
        .build();

        let mut meter_provider = None::<opentelemetry_sdk::metrics::MeterProviderBuilder>;
        if let Some(endpoint) = endpoint {
            let exporter = MetricExporter::builder()
                .with_http()
                .with_endpoint(format!("{endpoint}/v1/metrics"))
                .with_headers(headers)
                .build()
                .map_err(|e| format!("CAMPBOOKS_TELEMETRY__OTLP_ENDPOINT (metrics): {e}"))?;
            meter_provider = Some(
                meter_provider
                    .unwrap_or_else(|| SdkMeterProvider::builder().with_resource(resource.clone()))
                    .with_reader(PeriodicReader::builder(exporter).build()),
            );
        }
        let mut prometheus = None;
        if config.metrics_addr.is_some() {
            let registry = prometheus::Registry::new();
            let exporter = opentelemetry_prometheus::exporter()
                .with_registry(registry.clone())
                .build()
                .map_err(|e| format!("CAMPBOOKS_TELEMETRY__METRICS_ADDR: {e}"))?;
            meter_provider = Some(
                meter_provider
                    .unwrap_or_else(|| SdkMeterProvider::builder().with_resource(resource.clone()))
                    .with_reader(exporter),
            );
            prometheus = Some(registry);
        }
        let meter_provider = meter_provider.map(|builder| {
            let provider = builder.build();
            global::set_meter_provider(provider.clone());
            build_info(&provider.meter_with_scope(scope()), role);
            provider
        });

        let reported = meter_provider.as_ref().map(|provider| {
            crate::error_reports::reported_counter(&provider.meter_with_scope(scope()))
        });
        let error_reports = crate::error_reports::init(config, role, reported)?;

        Ok(Self {
            tracer_provider,
            meter_provider,
            prometheus,
            traces_exported: endpoint.is_some(),
            error_reports,
        })
    }

    /// The tracer spans are bridged to.
    pub fn tracer(&self) -> Tracer {
        self.tracer_provider.tracer_with_scope(scope())
    }

    /// Whether spans leave the process.
    pub fn traces_exported(&self) -> bool {
        self.traces_exported
    }

    /// The Campbooks meter of this process's provider, when metrics are
    /// exported.
    pub fn meter(&self) -> Option<Meter> {
        self.meter_provider
            .as_ref()
            .map(|provider| provider.meter_with_scope(scope()))
    }

    /// The request and job instruments, for the state.
    pub fn metrics(&self) -> Metrics {
        match self.meter() {
            Some(meter) => Metrics::new(&meter, true),
            None => Metrics::new(&telemetry::meter(), false),
        }
    }

    /// The pool-acquire histogram, when metrics are exported.
    pub fn pool_wait(&self) -> Option<Histogram<f64>> {
        self.meter().map(|meter| {
            telemetry::duration_histogram(
                &meter,
                "db.client.connection.wait_time",
                "Time spent waiting for a database connection from the pool.",
            )
        })
    }

    /// Connections of `pool` by state, and its maximum, as observable
    /// gauges (when metrics are exported).
    pub fn observe_pool(&self, pool: &PgPool, max_connections: u32) {
        let Some(meter) = self.meter() else {
            return;
        };
        let pool = pool.clone();
        meter
            .u64_observable_gauge("db.client.connection.count")
            .with_description("Database connections in the pool, by state.")
            .with_callback(move |observer| {
                let idle = u64::try_from(pool.num_idle()).unwrap_or(u64::MAX);
                let used = u64::from(pool.size()).saturating_sub(idle);
                observer.observe(idle, &[KeyValue::new("db.client.connection.state", "idle")]);
                observer.observe(used, &[KeyValue::new("db.client.connection.state", "used")]);
            })
            .build();
        meter
            .u64_observable_gauge("db.client.connection.max")
            .with_description("Most connections the pool opens.")
            .with_callback(move |observer| observer.observe(u64::from(max_connections), &[]))
            .build();
    }

    /// Whether errors are reported to an error tracker.
    pub fn reports_errors(&self) -> bool {
        self.error_reports.is_some()
    }

    /// Install the subscriber: JSON or pretty logs on stdout, spans bridged
    /// to OpenTelemetry, pool waits measured, errors reported when a DSN is
    /// configured.
    pub fn install_subscriber(&self, format: LogFormat) {
        let mut layers = layers(
            format,
            std::io::stdout,
            self.tracer(),
            self.traces_exported,
            self.pool_wait(),
        );
        if self.reports_errors() {
            layers.push(crate::error_reports::ErrorReportLayer.boxed());
        }
        tracing_subscriber::registry().with(layers).init();
    }

    /// The `/metrics` router, when the Prometheus listener is configured.
    pub fn metrics_router(&self) -> Option<Router> {
        self.prometheus.clone().map(metrics_router)
    }

    /// Flush and stop the exporters. Call it outside the async runtime.
    pub fn shutdown(self) {
        // Flushes the queued error reports (up to its shutdown timeout).
        drop(self.error_reports);
        if let Err(e) = self.tracer_provider.shutdown_with_timeout(FLUSH_TIMEOUT) {
            tracing::warn!(error = %e, "could not flush traces");
        }
        if let Some(provider) = self.meter_provider
            && let Err(e) = provider.shutdown_with_timeout(FLUSH_TIMEOUT)
        {
            tracing::warn!(error = %e, "could not flush metrics");
        }
    }
}

fn scope() -> InstrumentationScope {
    InstrumentationScope::builder(telemetry::SCOPE)
        .with_version(env!("CARGO_PKG_VERSION"))
        .build()
}

/// `service.*` and `campbooks.role`, plus whatever `OTEL_RESOURCE_ATTRIBUTES`
/// and `OTEL_SERVICE_NAME` add.
fn resource(role: &'static str) -> Resource {
    let builder = Resource::builder();
    let builder = if std::env::var_os("OTEL_SERVICE_NAME").is_some() {
        builder
    } else {
        builder.with_service_name(SERVICE_NAME)
    };
    builder
        .with_attributes([
            KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
            KeyValue::new("campbooks.role", role),
        ])
        .build()
}

/// `campbooks.build.info` = 1, labelled with the version and role.
fn build_info(meter: &Meter, role: &'static str) {
    meter
        .u64_observable_gauge("campbooks.build.info")
        .with_description("Always 1; its attributes name the running build.")
        .with_callback(move |observer| {
            observer.observe(
                1,
                &[
                    KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
                    KeyValue::new("campbooks.role", role),
                ],
            );
        })
        .build();
}

/// The subscriber's layers, each with its own filter: logs (`RUST_LOG`, else
/// [`DEFAULT_LOG_FILTER`]) to `writer`, spans to `tracer` (the same filter,
/// plus pool-acquire events as span events when traces are exported), and
/// pool waits into `pool_wait`.
pub fn layers<W>(
    format: LogFormat,
    writer: W,
    tracer: Tracer,
    traces_exported: bool,
    pool_wait: Option<Histogram<f64>>,
) -> Vec<Box<dyn Layer<Registry> + Send + Sync>>
where
    W: for<'w> MakeWriter<'w> + Send + Sync + 'static,
{
    let logs = match format {
        LogFormat::Json => fmt::layer()
            .json()
            .flatten_event(true)
            .with_writer(writer)
            .with_filter(log_filter())
            .boxed(),
        LogFormat::Pretty => fmt::layer()
            .pretty()
            .with_writer(writer)
            .with_filter(log_filter())
            .boxed(),
    };
    let mut span_filter = log_filter();
    if traces_exported && let Ok(directive) = format!("{POOL_ACQUIRE_TARGET}=debug").parse() {
        span_filter = span_filter.add_directive(directive);
    }
    let spans = tracing_opentelemetry::layer()
        .with_tracer(tracer)
        .with_filter(span_filter)
        .boxed();
    let mut layers = vec![logs, spans];
    if let Some(histogram) = pool_wait {
        layers.push(
            PoolWaitLayer { histogram }
                .with_filter(Targets::new().with_target(POOL_ACQUIRE_TARGET, Level::DEBUG))
                .boxed(),
        );
    }
    layers
}

fn log_filter() -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_LOG_FILTER))
}

/// Records sqlx's `acquired_after_secs` into the pool-wait histogram.
struct PoolWaitLayer {
    histogram: Histogram<f64>,
}

impl<S> Layer<S> for PoolWaitLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        struct Seconds(Option<f64>);
        impl tracing::field::Visit for Seconds {
            fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
                if field.name() == "acquired_after_secs" {
                    self.0 = Some(value);
                }
            }
            fn record_debug(&mut self, _: &tracing::field::Field, _: &dyn std::fmt::Debug) {}
        }
        let mut seconds = Seconds(None);
        event.record(&mut seconds);
        if let Some(seconds) = seconds.0 {
            self.histogram.record(seconds, &[]);
        }
    }
}

fn metrics_router(registry: prometheus::Registry) -> Router {
    Router::new().route(
        "/metrics",
        get(move || {
            let registry = registry.clone();
            async move {
                let mut body = String::new();
                match prometheus::TextEncoder::new().encode_utf8(&registry.gather(), &mut body) {
                    Ok(()) => {
                        ([(header::CONTENT_TYPE, prometheus::TEXT_FORMAT)], body).into_response()
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "could not render metrics");
                        StatusCode::INTERNAL_SERVER_ERROR.into_response()
                    }
                }
            }
        }),
    )
}

// --- panics -----------------------------------------------------------------------

/// What is reported about a panic: where it happened, never its message. A
/// panic message can quote the data that caused it (a search query, an
/// address), and the default hook prints it to stderr.
pub fn withheld_panic_report(info: &std::panic::PanicHookInfo<'_>) -> String {
    let location = info.location().map_or_else(
        || "an unknown location".to_owned(),
        |l| format!("{}:{}", l.file(), l.line()),
    );
    format!("panicked at {location} (message withheld: it may hold personal data)")
}

/// Replace the default panic hook with one that logs
/// [`withheld_panic_report`] (as an error event, and on stderr when no
/// subscriber is installed). Installed first thing by the binary.
pub fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let report = withheld_panic_report(info);
        if tracing::dispatcher::has_been_set() {
            tracing::error!(target: "campbooks_server::panic", "{report}");
        } else {
            let _ = writeln!(std::io::stderr(), "{report}");
        }
    }));
}

// --- the request middleware -----------------------------------------------------

/// Longest client-sent request id recorded as is.
const MAX_REQUEST_ID_LEN: usize = 128;

/// One server span per request (named after the route template, child of
/// an incoming `traceparent`), one log line when it ends, and the request
/// duration metric. The URI is never recorded: a query string or a path
/// segment can hold personal data or a token.
pub async fn observe(State(metrics): State<Metrics>, request: Request, next: Next) -> Response {
    let method = request.method().as_str().to_owned();
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map(|path| path.as_str().to_owned());
    let request_id = request
        .extensions()
        .get::<RequestId>()
        .and_then(|id| id.header_value().to_str().ok())
        .map(loggable_request_id)
        .unwrap_or_default();
    let name = match &route {
        Some(route) => format!("{method} {route}"),
        None => method.clone(),
    };
    let span = tracing::info_span!(
        "http",
        otel.name = %name,
        otel.kind = "server",
        otel.status_code = Empty,
        http.request.method = %method,
        http.route = route.as_deref(),
        http.response.status_code = Empty,
        error.type = Empty,
        request_id = %request_id,
        trace_id = Empty,
        workspace.id = Empty,
        principal.kind = Empty,
    );
    telemetry::set_parent_from(&span, request.headers());
    telemetry::record_trace_id(&span);

    let started = Instant::now();
    let response = next.run(request).instrument(span.clone()).await;
    let status = response.status();
    let elapsed = started.elapsed();
    // The problem's stable code, never its (translated) text.
    let error = response
        .extensions()
        .get::<ProblemCode>()
        .map(|code| code.0)
        .filter(|_| status.is_client_error() || status.is_server_error());
    span.record("http.response.status_code", status.as_u16());
    if let Some(error) = error {
        span.record("error.type", error);
    }
    if status.is_server_error() {
        span.record("otel.status_code", "error");
    }
    span.in_scope(|| {
        tracing::info!(
            status = status.as_u16(),
            latency_ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
            "request finished"
        );
    });
    metrics.http_request(&method, route.as_deref(), status.as_u16(), error, elapsed);
    response
}

/// A client-sent request id, if it is a plain token (letters, digits,
/// `-_.:`), else `invalid`: it is echoed to the client but only logged when
/// it cannot carry personal data.
fn loggable_request_id(id: &str) -> String {
    let plain = id.len() <= MAX_REQUEST_ID_LEN
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'));
    if plain {
        id.to_owned()
    } else {
        "invalid".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_ids_with_personal_data_are_not_logged() {
        assert_eq!(loggable_request_id("abc-123"), "abc-123");
        assert_eq!(
            loggable_request_id("0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b"),
            "0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b"
        );
        assert_eq!(loggable_request_id("alice@example.com"), "invalid");
        assert_eq!(loggable_request_id(&"a".repeat(200)), "invalid");
    }

    #[test]
    fn nothing_is_exported_by_default() {
        let telemetry = Telemetry::new(&TelemetryConfig::default(), "all").unwrap();
        assert!(!telemetry.traces_exported());
        assert!(telemetry.metrics_router().is_none());
        assert!(telemetry.pool_wait().is_none());
        telemetry.shutdown();
    }
}
