//! Error reports to a Sentry-compatible tracker (`GlitchTip`, Sentry), off
//! unless `CAMPBOOKS_TELEMETRY__ERROR_DSN` is set.
//!
//! Campbooks handles people's mail, so a report is built from an allowlist,
//! never from whatever an error happened to carry:
//!
//! - [`ErrorReportLayer`] turns each ERROR event (an internal error, a job
//!   that failed for good, a panic) into one report: its message and its
//!   `error` field as the error's kind, both through
//!   [`redact::error_kind`](campbooks_kernel::redact::error_kind), and the
//!   ids, route template and job kind of the request or job it happened in,
//!   as tags that must look like ids
//!   ([`redact::looks_like_an_id`](campbooks_kernel::redact::looks_like_an_id)). Nothing else of the event or its spans
//!   is read.
//! - [`scrub`], the client's `before_send`, rebuilds every outgoing report
//!   from the same allowlist again: no user, request, cookies, headers,
//!   breadcrumbs, contexts, extra data, server name or module list, and the
//!   same error kinds of every text.
//! - `send_default_pii` is off, no default integration runs (the panic hook
//!   is the server's own, which withholds panic messages), and traces are not
//!   sent.

use std::{borrow::Cow, collections::BTreeMap, sync::Arc, time::Duration};

use campbooks_kernel::{
    config::TelemetryConfig,
    redact::{error_kind, looks_like_an_id},
};
use opentelemetry::{KeyValue, metrics::Counter};
use sentry::{
    ClientOptions,
    protocol::{Event, Exception, Level, Map},
    types::Dsn,
};
use tracing::{Subscriber, field::Field, span};
use tracing_subscriber::{Layer, layer::Context, registry::LookupSpan};

/// Fields of a request's or a job's span a report carries, as tags.
const SPAN_FIELDS: &[&str] = &[
    "request_id",
    "trace_id",
    "workspace.id",
    "principal.kind",
    "http.route",
    "http.request.method",
    "http.response.status_code",
    "error.type",
    "job.id",
    "job.kind",
    "job.queue",
    "job.attempt",
];

/// Fields of the event itself a report carries, as tags.
const EVENT_FIELDS: &[&str] = &["error.type", "job_id", "kind", "operation", "kid", "locale"];

/// Every tag a report may carry: the span and event fields, plus what the
/// server adds.
const TAGS: &[&str] = &[
    "request_id",
    "trace_id",
    "workspace.id",
    "principal.kind",
    "http.route",
    "http.request.method",
    "http.response.status_code",
    "error.type",
    "job.id",
    "job.kind",
    "job.queue",
    "job.attempt",
    "job_id",
    "kind",
    "operation",
    "kid",
    "locale",
    "location",
    "campbooks.role",
    "service.version",
];

/// The panic hook's target (`telemetry::install_panic_hook`).
const PANIC_TARGET: &str = "campbooks_server::panic";

/// The release reports are filed under: the image's commit (`GIT_SHA`), else
/// the crate version.
pub fn release() -> String {
    std::env::var("GIT_SHA")
        .ok()
        .map(|sha| sha.trim().to_owned())
        .filter(|sha| !sha.is_empty())
        .unwrap_or_else(|| concat!("campbooks-server@", env!("CARGO_PKG_VERSION")).to_owned())
}

/// The client options for `config` and `role`, when a DSN is configured:
/// the scrubber, no PII, no integrations, no traces. `reported` counts each
/// report that leaves the process. An invalid DSN is an error naming the
/// setting, never the DSN.
pub fn client_options(
    config: &TelemetryConfig,
    role: &'static str,
    reported: Option<Counter<u64>>,
) -> Result<Option<ClientOptions>, String> {
    let Some(dsn) = config.error_dsn() else {
        return Ok(None);
    };
    let dsn: Dsn = dsn
        .parse()
        .map_err(|_| "CAMPBOOKS_TELEMETRY__ERROR_DSN is not a valid DSN".to_owned())?;
    let before_send: Arc<dyn Fn(Event<'static>) -> Option<Event<'static>> + Send + Sync> =
        Arc::new(move |event| {
            let mut event = scrub(event);
            event
                .tags
                .insert("campbooks.role".to_owned(), role.to_owned());
            event.tags.insert(
                "service.version".to_owned(),
                env!("CARGO_PKG_VERSION").to_owned(),
            );
            if let Some(counter) = &reported {
                counter.add(1, &[KeyValue::new("source", "server")]);
            }
            Some(event)
        });
    let mut options = ClientOptions::default();
    options.dsn = Some(dsn);
    options.release = Some(Cow::Owned(release()));
    options.environment = Some(Cow::Owned(config.environment().to_owned()));
    options.send_default_pii = false;
    options.attach_stacktrace = false;
    options.default_integrations = false;
    options.max_breadcrumbs = 0;
    options.auto_session_tracking = false;
    options.server_name = None;
    options.before_send = Some(before_send);
    options.before_breadcrumb = Some(Arc::new(|_| None));
    options.shutdown_timeout = Duration::from_secs(5);
    Ok(Some(options))
}

/// Start the client for `config`, when a DSN is configured. Dropping the
/// guard flushes what is queued.
pub fn init(
    config: &TelemetryConfig,
    role: &'static str,
    reported: Option<Counter<u64>>,
) -> Result<Option<sentry::ClientInitGuard>, String> {
    Ok(client_options(config, role, reported)?.map(sentry::init))
}

/// The counter of reports sent (`campbooks.errors.reported`).
pub fn reported_counter(meter: &opentelemetry::metrics::Meter) -> Counter<u64> {
    meter
        .u64_counter("campbooks.errors.reported")
        .with_description("Error reports sent to the error tracker.")
        .build()
}

// --- the scrubber -----------------------------------------------------------------

/// `event` rebuilt from the allowlist: its id, level, time, release and
/// environment; its logger, culprit and transaction when they look like
/// names; its tags when the key is allowed and the value looks like an id;
/// its message, its exceptions' types and values and its fingerprint as
/// [`error_kind`]s; nothing else.
pub fn scrub(event: Event<'static>) -> Event<'static> {
    let tags = event
        .tags
        .into_iter()
        .filter(|(key, value)| TAGS.contains(&key.as_str()) && looks_like_an_id(value))
        .collect::<Map<_, _>>();
    let exception = event
        .exception
        .values
        .into_iter()
        .map(|exception| Exception {
            ty: error_kind(&exception.ty),
            value: exception.value.as_deref().map(error_kind),
            module: exception.module.filter(|m| looks_like_an_id(m)),
            ..Exception::default()
        })
        .collect::<Vec<_>>();
    Event {
        event_id: event.event_id,
        level: event.level,
        fingerprint: Cow::Owned(
            event
                .fingerprint
                .iter()
                .map(|part| Cow::Owned(error_kind(part)))
                .collect(),
        ),
        culprit: event.culprit.filter(|c| looks_like_an_id(c)),
        transaction: event.transaction.filter(|t| looks_like_an_id(t)),
        message: event.message.as_deref().map(error_kind),
        logger: event.logger.filter(|l| looks_like_an_id(l)),
        platform: event.platform,
        timestamp: event.timestamp,
        release: event.release,
        environment: event.environment,
        exception: exception.into(),
        tags,
        sdk: event.sdk,
        ..Event::default()
    }
}

// --- the layer --------------------------------------------------------------------

/// The allowlisted fields of a span, kept in its extensions.
#[derive(Default)]
struct ReportFields(BTreeMap<&'static str, String>);

/// Collects `fields`' values, and the event's message.
struct Collect<'a> {
    fields: &'static [&'static str],
    values: &'a mut BTreeMap<&'static str, String>,
    message: Option<&'a mut Option<String>>,
    error: Option<&'a mut Option<String>>,
}

impl Collect<'_> {
    fn put(&mut self, field: &Field, value: String) {
        match field.name() {
            "message" => {
                if let Some(message) = self.message.as_deref_mut() {
                    *message = Some(value);
                }
            }
            "error" => {
                if let Some(error) = self.error.as_deref_mut() {
                    *error = Some(value);
                }
            }
            name => {
                if let Some(name) = self.fields.iter().find(|f| **f == name) {
                    self.values.insert(name, value);
                }
            }
        }
    }
}

impl tracing::field::Visit for Collect<'_> {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.put(field, value.to_owned());
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.put(field, value.to_string());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.put(field, value.to_string());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.put(field, value.to_string());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.put(field, format!("{value:?}"));
    }
}

/// Turns ERROR events into error reports (see the module docs). Installed
/// by `Telemetry::install_subscriber` when a DSN is configured.
#[derive(Debug, Default, Clone, Copy)]
pub struct ErrorReportLayer;

impl<S> Layer<S> for ErrorReportLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else {
            return;
        };
        let mut fields = ReportFields::default();
        attrs.record(&mut Collect {
            fields: SPAN_FIELDS,
            values: &mut fields.0,
            message: None,
            error: None,
        });
        span.extensions_mut().insert(fields);
    }

    fn on_record(&self, id: &span::Id, values: &span::Record<'_>, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else {
            return;
        };
        let mut extensions = span.extensions_mut();
        if let Some(fields) = extensions.get_mut::<ReportFields>() {
            values.record(&mut Collect {
                fields: SPAN_FIELDS,
                values: &mut fields.0,
                message: None,
                error: None,
            });
        }
    }

    fn on_event(&self, event: &tracing::Event<'_>, ctx: Context<'_, S>) {
        if *event.metadata().level() != tracing::Level::ERROR {
            return;
        }
        let mut tags = BTreeMap::new();
        if let Some(scope) = ctx.event_scope(event) {
            // Outermost first, so an inner span's value wins.
            for span in scope.from_root() {
                if let Some(fields) = span.extensions().get::<ReportFields>() {
                    tags.extend(fields.0.iter().map(|(k, v)| (*k, v.clone())));
                }
            }
        }
        let mut message = None;
        let mut error = None;
        event.record(&mut Collect {
            fields: EVENT_FIELDS,
            values: &mut tags,
            message: Some(&mut message),
            error: Some(&mut error),
        });
        sentry::Hub::with_active(|hub| {
            hub.capture_event(report(
                event.metadata(),
                message.as_deref(),
                error.as_deref(),
                tags,
            ));
        });
    }
}

/// The report of an ERROR event: an exception named after the message (or
/// `panic`), valued by the `error` field, filed under the route or job kind
/// it happened in and the place it was logged.
fn report(
    metadata: &tracing::Metadata<'_>,
    message: Option<&str>,
    error: Option<&str>,
    tags: BTreeMap<&'static str, String>,
) -> Event<'static> {
    let panic = metadata.target() == PANIC_TARGET;
    let location = metadata
        .file()
        .map(|file| format!("{}:{}", short_path(file), metadata.line().unwrap_or(0)));
    let (ty, value) = if panic {
        ("panic".to_owned(), message.map(error_kind))
    } else {
        (
            message.map_or_else(|| "error".to_owned(), error_kind),
            error.map(error_kind),
        )
    };
    // The route template (the method is a tag) or the job kind: no spaces,
    // so it passes the scrubber.
    let culprit = tags
        .get("http.route")
        .or_else(|| tags.get("job.kind"))
        .cloned();
    let mut fingerprint = vec![Cow::Owned(ty.clone())];
    fingerprint.extend(culprit.clone().map(Cow::Owned));
    fingerprint.extend(location.clone().map(Cow::Owned));
    let mut tags: Map<String, String> = tags.into_iter().map(|(k, v)| (k.to_owned(), v)).collect();
    if let Some(location) = location {
        tags.insert("location".to_owned(), location);
    }
    Event {
        level: Level::Error,
        logger: Some(metadata.target().to_owned()),
        transaction: culprit.clone(),
        culprit,
        fingerprint: Cow::Owned(fingerprint),
        exception: vec![Exception {
            ty,
            value,
            module: metadata.module_path().map(str::to_owned),
            ..Exception::default()
        }]
        .into(),
        tags,
        ..Event::default()
    }
}

/// `file` from its crate directory on (`crates/kernel/src/error.rs`,
/// `sqlx-core-0.9.0/src/pool.rs`), never a home or build directory.
fn short_path(file: &str) -> &str {
    if let Some(at) = file.find("crates/") {
        return &file[at..];
    }
    // A dependency's file: its last three components.
    let mut end = file.len();
    for _ in 0..3 {
        match file[..end].rfind('/') {
            Some(at) => end = at,
            None => return file,
        }
    }
    &file[end + 1..]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_paths_drop_build_directories() {
        assert_eq!(
            short_path("/build/crates/kernel/src/error.rs"),
            "crates/kernel/src/error.rs"
        );
        assert_eq!(
            short_path("crates/kernel/src/error.rs"),
            "crates/kernel/src/error.rs"
        );
        assert_eq!(
            short_path("/home/alice/.cargo/registry/src/index/sqlx-core-0.9.0/src/pool.rs"),
            "sqlx-core-0.9.0/src/pool.rs"
        );
        assert_eq!(short_path("main.rs"), "main.rs");
    }

    #[test]
    fn no_dsn_no_client() {
        assert!(
            client_options(&TelemetryConfig::default(), "all", None)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn a_blank_dsn_means_off() {
        let config = TelemetryConfig {
            error_dsn: Some("  ".to_owned().into()),
            ..TelemetryConfig::default()
        };
        assert!(client_options(&config, "all", None).unwrap().is_none());
    }

    #[test]
    fn a_bad_dsn_names_the_setting_not_the_value() {
        let config = TelemetryConfig {
            error_dsn: Some("not a dsn with secret-key-123".to_owned().into()),
            ..TelemetryConfig::default()
        };
        let Err(message) = client_options(&config, "all", None) else {
            panic!("an invalid DSN is refused");
        };
        assert!(
            message.contains("CAMPBOOKS_TELEMETRY__ERROR_DSN"),
            "{message}"
        );
        assert!(!message.contains("secret-key-123"), "{message}");
    }
}
