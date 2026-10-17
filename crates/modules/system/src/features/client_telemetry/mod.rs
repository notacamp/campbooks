//! `POST /api/v1/system/client-telemetry` — the app reports its own errors
//! and timings, and the server forwards them to the operator's Grafana Faro
//! receiver (`CAMPBOOKS_TELEMETRY__CLIENT_COLLECTOR_URL`).
//!
//! The app never talks to the collector itself: its pages allow connections
//! to their own origin only, the native app reaches nothing but its bridge,
//! and the collector's key stays on the server. The server is also where the
//! report is scrubbed, whatever the app sent: each item is rebuilt from the
//! fields below (anything else is never read), every text becomes an error
//! kind or must look like an id ([`redact`]), numbers must be finite, and
//! the lists are capped. Without a collector the operation answers 404, and
//! the app stops sending.

use std::{collections::BTreeMap, sync::OnceLock, time::Duration};

use axum::{Json, extract::State, http::StatusCode};
use campbooks_kernel::{AppState, Error, Operation, Problem, Result, Risk, auth::Unbound, redact};
use chrono::{DateTime, Utc};
use opentelemetry::{KeyValue, metrics::Counter};
use secrecy::ExposeSecret as _;
use serde::Deserialize;
use serde_json::{Value, json};
use utoipa::ToSchema;

use crate::scopes::SystemRead;

/// Most exceptions, measurements or events one report forwards.
pub const MAX_ITEMS: usize = 20;

/// Most stack frames an exception keeps.
pub const MAX_FRAMES: usize = 30;

/// How long a forward to the collector may take.
const FORWARD_TIMEOUT: Duration = Duration::from_secs(5);

/// Web vitals and timings the app may report (`measurements[].values` keys).
const MEASUREMENT_VALUES: &[&str] = &["cls", "fcp", "fid", "inp", "lcp", "ttfb", "delta"];

/// A batch of the app's own telemetry.
#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct ClientReport {
    /// A random id of the app's session (not the sign-in session): joins a
    /// session's reports.
    #[schema(example = "k3Jx8vQ2mN")]
    pub session_id: String,
    /// Where the app is built for.
    pub platform: Option<ClientPlatform>,
    /// The app's version.
    #[schema(example = "0.42.0")]
    pub app_version: Option<String>,
    /// The route template of the page shown (`/w/$workspaceId/inbox`),
    /// never its address.
    #[schema(example = "/w/$workspaceId/inbox")]
    pub view: Option<String>,
    /// The browser or web view.
    pub browser: Option<ClientBrowser>,
    /// Errors the app did not handle.
    #[serde(default)]
    pub exceptions: Vec<ClientException>,
    /// Timings (web vitals).
    #[serde(default)]
    pub measurements: Vec<ClientMeasurement>,
    /// Named moments (`session_start`, `view_changed`).
    #[serde(default)]
    pub events: Vec<ClientEvent>,
}

/// Where the app runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ClientPlatform {
    /// In a browser, served by this server.
    Web,
    /// The desktop or mobile app.
    Native,
}

impl ClientPlatform {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Web => "web",
            Self::Native => "native",
        }
    }
}

/// The browser or web view the app runs in.
#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct ClientBrowser {
    /// `Chrome`, `Safari`, `WebKit`.
    pub name: Option<String>,
    /// Its version.
    pub version: Option<String>,
    /// The operating system.
    pub os: Option<String>,
    /// Whether the device is a phone or a tablet.
    pub mobile: Option<bool>,
}

/// An error the app did not handle.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ClientException {
    /// The error's class (`TypeError`).
    #[schema(example = "TypeError")]
    pub kind: String,
    /// Its message: forwarded as an error kind only (its head, with
    /// addresses, quotes, links and tokens masked).
    #[schema(example = "Cannot read properties of undefined")]
    pub message: String,
    /// When it happened.
    pub timestamp: DateTime<Utc>,
    /// Where, innermost first.
    #[serde(default)]
    pub frames: Vec<ClientFrame>,
}

/// One frame of an exception's stack.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ClientFrame {
    /// The script's URL: forwarded as its file name only.
    #[schema(example = "https://app.example.com/assets/index-3f2c1e0.js")]
    pub file: Option<String>,
    /// The function's name.
    pub function: Option<String>,
    /// Line, from 1.
    pub line: Option<u32>,
    /// Column, from 1.
    pub column: Option<u32>,
}

/// A timing.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ClientMeasurement {
    /// What was measured (`web-vitals`).
    #[schema(example = "web-vitals")]
    pub kind: String,
    /// Its values by name (`lcp`, `inp`, `cls`, `fcp`, `ttfb`, `fid`,
    /// `delta`); others are dropped.
    pub values: BTreeMap<String, f64>,
    /// When it was taken.
    pub timestamp: DateTime<Utc>,
}

/// A named moment.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ClientEvent {
    /// Its name.
    #[schema(example = "session_start")]
    pub name: String,
    /// When it happened.
    pub timestamp: DateTime<Utc>,
}

/// Report the app's own errors and timings.
#[utoipa::path(
    post,
    path = "/api/v1/system/client-telemetry",
    operation_id = "system.report_client_telemetry",
    tag = "system",
    security(("bearer" = ["system:read"])),
    request_body = ClientReport,
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).access("system.client_report", campbooks_kernel::authz::Action::Create)))),
    responses(
        (status = ACCEPTED, description = "Taken: what passes the scrubber is forwarded to the \
            collector."),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "The token lacks `system:read`.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "This instance collects no client telemetry: stop \
            sending.", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    Unbound { principal, .. }: Unbound<SystemRead>,
    Json(report): Json<ClientReport>,
) -> Result<StatusCode> {
    campbooks_kernel::authz::require_account(
        &state,
        &principal,
        campbooks_kernel::authz::Action::Create,
        "system.client_report",
        || Error::Forbidden,
    )?;
    let telemetry = &state.config.telemetry;
    let Some(url) = telemetry.client_collector_url().map(str::to_owned) else {
        return Err(Error::NotFound);
    };
    let key = telemetry
        .client_collector_key
        .as_ref()
        .map(|key| key.expose_secret().to_owned());
    let (payload, counts) = faro_payload(&report, telemetry.environment());
    tokio::spawn(async move {
        let forwarded = forward(&url, key.as_deref(), &payload).await;
        let outcome = if forwarded { "forwarded" } else { "dropped" };
        for (kind, kept, dropped) in counts {
            items().add(
                kept,
                &[
                    KeyValue::new("kind", kind),
                    KeyValue::new("outcome", outcome),
                ],
            );
            items().add(
                dropped,
                &[
                    KeyValue::new("kind", kind),
                    KeyValue::new("outcome", "dropped"),
                ],
            );
        }
    });
    Ok(StatusCode::ACCEPTED)
}

/// `campbooks.client_telemetry.items`, by kind and outcome.
fn items() -> &'static Counter<u64> {
    static ITEMS: OnceLock<Counter<u64>> = OnceLock::new();
    ITEMS.get_or_init(|| {
        campbooks_kernel::telemetry::meter()
            .u64_counter("campbooks.client_telemetry.items")
            .with_description("Items of the app's own telemetry, forwarded or dropped.")
            .build()
    })
}

/// POST `payload` to the collector; whether it took it. Never logs the key
/// or the payload.
async fn forward(url: &str, key: Option<&str>, payload: &Value) -> bool {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    let client = CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(FORWARD_TIMEOUT)
            .build()
            .unwrap_or_default()
    });
    let mut request = client.post(url).json(payload);
    if let Some(key) = key {
        request = request.header("x-api-key", key);
    }
    match request.send().await {
        Ok(response) if response.status().is_success() => true,
        Ok(response) => {
            tracing::warn!(
                status = response.status().as_u16(),
                "the client collector refused a report"
            );
            false
        }
        Err(e) => {
            tracing::warn!(error = %redact::error_kind(&e.to_string()), "could not reach the client collector");
            false
        }
    }
}

/// Text that names something (a browser, a version, an OS): short letters,
/// digits, spaces and `._-`; anything else is dropped.
fn name(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    (!value.is_empty()
        && value.len() <= 40
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '.' | '_' | '-')))
    .then(|| value.to_owned())
}

/// A route template (`/w/$workspaceId/inbox`), never a path holding ids:
/// each segment is a `$parameter` or lowercase letters, `-` and `_` (no
/// digits, so no id or number passes).
fn view(value: Option<&str>) -> Option<String> {
    let value = value?;
    let segment = |segment: &str| match segment.strip_prefix('$') {
        Some(parameter) => {
            !parameter.is_empty() && parameter.chars().all(|c| c.is_ascii_alphanumeric())
        }
        None => segment
            .chars()
            .all(|c| c.is_ascii_lowercase() || matches!(c, '-' | '_')),
    };
    (value.starts_with('/') && value.len() <= 200 && value.split('/').all(segment))
        .then(|| value.to_owned())
}

/// The file name of a script's URL (`index-3f2c1e0.js`): no host, path or
/// query.
fn file_name(value: Option<&str>) -> Option<String> {
    let value = value?;
    let path = value.split(['?', '#']).next().unwrap_or_default();
    let file = path.rsplit('/').next().unwrap_or_default();
    redact::looks_like_an_id(file).then(|| file.to_owned())
}

/// One exception, scrubbed, in Faro's shape.
fn exception(exception: &ClientException) -> Value {
    let frames: Vec<Value> = exception
        .frames
        .iter()
        .take(MAX_FRAMES)
        .map(|frame| {
            json!({
                "filename": file_name(frame.file.as_deref()).unwrap_or_default(),
                "function": frame
                    .function
                    .as_deref()
                    .filter(|f| redact::looks_like_an_id(f))
                    .unwrap_or_default(),
                "lineno": frame.line,
                "colno": frame.column,
            })
        })
        .collect();
    json!({
        "type": redact::error_kind(&exception.kind),
        "value": redact::error_kind(&exception.message),
        "timestamp": exception.timestamp.to_rfc3339(),
        "stacktrace": { "frames": frames },
    })
}

/// One measurement in Faro's shape, when its kind looks like a name and a
/// known value is left.
fn measurement(measurement: &ClientMeasurement) -> Option<Value> {
    if !redact::looks_like_an_id(&measurement.kind) {
        return None;
    }
    let values: BTreeMap<&str, f64> = measurement
        .values
        .iter()
        .filter(|(k, v)| MEASUREMENT_VALUES.contains(&k.as_str()) && v.is_finite())
        .map(|(k, v)| (k.as_str(), *v))
        .collect();
    (!values.is_empty()).then(|| {
        json!({
            "type": measurement.kind,
            "values": values,
            "timestamp": measurement.timestamp.to_rfc3339(),
        })
    })
}

/// The Faro payload of `report` after scrubbing, and per kind how many
/// items were kept and dropped.
fn faro_payload(
    report: &ClientReport,
    environment: &str,
) -> (Value, Vec<(&'static str, u64, u64)>) {
    let session = if redact::looks_like_an_id(&report.session_id) {
        report.session_id.clone()
    } else {
        "invalid".to_owned()
    };
    let exceptions: Vec<Value> = report
        .exceptions
        .iter()
        .take(MAX_ITEMS)
        .map(exception)
        .collect();
    let measurements: Vec<Value> = report
        .measurements
        .iter()
        .take(MAX_ITEMS)
        .filter_map(measurement)
        .collect();
    let events: Vec<Value> = report
        .events
        .iter()
        .take(MAX_ITEMS)
        .filter(|e| redact::looks_like_an_id(&e.name))
        .map(|e| json!({ "name": e.name, "domain": "campbooks", "timestamp": e.timestamp.to_rfc3339() }))
        .collect();
    let browser = report.browser.clone().unwrap_or_default();
    let view = view(report.view.as_deref());
    let counts = vec![
        (
            "exception",
            exceptions.len() as u64,
            (report.exceptions.len() - exceptions.len()) as u64,
        ),
        (
            "measurement",
            measurements.len() as u64,
            (report.measurements.len() - measurements.len()) as u64,
        ),
        (
            "event",
            events.len() as u64,
            (report.events.len() - events.len()) as u64,
        ),
    ];
    let payload = json!({
        "meta": {
            "sdk": { "name": "campbooks-server", "version": env!("CARGO_PKG_VERSION") },
            "app": {
                "name": "campbooks",
                "version": name(report.app_version.as_deref()),
                "environment": environment,
                "bundleId": report.platform.map(ClientPlatform::as_str),
            },
            "session": { "id": session },
            "view": { "name": view.clone().unwrap_or_else(|| "unknown".to_owned()) },
            "page": { "url": view.unwrap_or_default() },
            "browser": {
                "name": name(browser.name.as_deref()),
                "version": name(browser.version.as_deref()),
                "os": name(browser.os.as_deref()),
                "mobile": browser.mobile,
            },
        },
        "exceptions": exceptions,
        "measurements": measurements,
        "events": events,
        "logs": [],
    });
    (payload, counts)
}

#[cfg(test)]
mod tests;
