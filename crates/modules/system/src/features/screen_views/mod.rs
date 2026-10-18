//! `POST /api/v1/system/screen-views` — the app says which screen it shows,
//! and the server relays it to the operator's Matomo (usage analytics,
//! `CAMPBOOKS_ANALYTICS__*`).
//!
//! The app never talks to Matomo and loads none of its scripts: its pages
//! allow connections to their own origin only and the native app reaches
//! nothing but its bridge. What it sends is the screen's route template
//! (`/w/:workspaceId/inbox`), checked here whatever the app sent: literal
//! segments of lowercase letters, `-` and `_`, parameters `:name`, no digit,
//! query or fragment, so no id, address or title passes.
//!
//! The view joins a bounded in-memory queue ([`ScreenViewRelay`]) and the
//! answer is 204 at once; one background worker per process sends each view
//! to Matomo's HTTP tracking API with a short timeout. A full queue or a
//! failed send drops the view (`campbooks.screen_views`, `outcome`), never
//! retried and never an error for the person. Matomo gets the route
//! template, the request's `User-Agent`, its language when we ship it and a
//! visitor id that changes every UTC day (the first 16 hex characters of an
//! HMAC of the account and the day under the visitor key): never an IP
//! (`cip`), a user id (`uid`), a workspace or custom dimensions, and no
//! `token_auth`. Without the three settings the operation does nothing.

use std::{
    sync::{Arc, OnceLock},
    time::Duration,
};

use axum::{
    Extension, Json,
    extract::State,
    http::{HeaderMap, StatusCode, header},
};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    auth::Unbound,
    authz::{self, Action},
    config::Matomo,
    crypto,
    i18n::Locale,
    redact,
};
use chrono::{NaiveDate, Utc};
use opentelemetry::{KeyValue, metrics::Counter};
use reqwest::Url;
use serde::Deserialize;
use tokio::sync::mpsc;
use utoipa::ToSchema;

use crate::scopes::SystemRead;

/// Longest route template accepted, in bytes.
pub const MAX_ROUTE: usize = 120;

/// Most segments a route template may have.
pub const MAX_SEGMENTS: usize = 8;

/// Longest segment of a route template, in bytes.
pub const MAX_SEGMENT: usize = 32;

/// Screen views one process holds while they wait for the worker.
pub const QUEUE_CAPACITY: usize = 1024;

/// Longest `User-Agent` forwarded, in characters.
const MAX_USER_AGENT_CHARS: usize = 256;

/// How long one send to Matomo may take.
const FORWARD_TIMEOUT: Duration = Duration::from_secs(3);

/// How long connecting to Matomo may take.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);

/// A screen the app shows.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ScreenView {
    /// The screen's route template, never its address: `/`, or segments of
    /// lowercase letters, `-` and `_` and parameters `:name`, with no digit,
    /// query or fragment.
    #[schema(example = "/w/:workspaceId/inbox")]
    pub route: String,
}

/// Report the screen the app shows.
#[utoipa::path(
    post,
    path = "/api/v1/system/screen-views",
    operation_id = "system.report_screen_view",
    tag = "system",
    security(("bearer" = ["system:read"])),
    request_body = ScreenView,
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).access("system.screen_view", Action::Create)))),
    responses(
        (status = NO_CONTENT, description = "Taken: relayed in the background when the instance \
            records usage analytics, otherwise dropped."),
        (status = BAD_REQUEST, description = "`invalid_route`: not a route template.",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "The token lacks `system:read`.", body = Problem,
            content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    Extension(relay): Extension<ScreenViewRelay>,
    Unbound { principal, .. }: Unbound<SystemRead>,
    headers: HeaderMap,
    Json(view): Json<ScreenView>,
) -> Result<StatusCode> {
    authz::require_account(
        &state,
        &principal,
        Action::Create,
        "system.screen_view",
        || Error::Forbidden,
    )?;
    let route = route(&view.route)
        .ok_or_else(|| Error::BadRequest(crate::MESSAGES.reason("invalid_route")))?;
    let Some(matomo) = state.config.analytics.matomo() else {
        return Ok(StatusCode::NO_CONTENT);
    };
    let visitor = visitor_id(
        matomo.visitor_key(),
        &principal.subject,
        Utc::now().date_naive(),
    );
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(|ua| ua.chars().take(MAX_USER_AGENT_CHARS).collect::<String>());
    let lang = headers
        .get(header::ACCEPT_LANGUAGE)
        .and_then(|value| value.to_str().ok())
        .and_then(Locale::from_accept_language);
    let hit = Hit {
        route,
        visitor: &visitor,
        user_agent: user_agent.as_deref(),
        lang,
    };
    if let Some(url) = tracking_url(&matomo, &state.config.public_url, &hit) {
        relay.enqueue(url);
    }
    Ok(StatusCode::NO_CONTENT)
}

/// `value` when it is a route template: `/`, or at most [`MAX_SEGMENTS`]
/// segments, each a literal of lowercase letters, `-` and `_` starting with
/// a letter, or a parameter `:name` of letters starting lowercase; at most
/// [`MAX_ROUTE`] bytes. No digit, so no id or number; no `?`, `#`, space,
/// capital or dot, so no query, fragment, title, address or file passes.
pub(crate) fn route(value: &str) -> Option<&str> {
    if value == "/" {
        return Some(value);
    }
    if value.len() > MAX_ROUTE {
        return None;
    }
    let rest = value.strip_prefix('/')?;
    let mut segments = 0;
    for segment in rest.split('/') {
        segments += 1;
        if segments > MAX_SEGMENTS || !is_segment(segment) {
            return None;
        }
    }
    Some(value)
}

/// A literal (`inbox`, `sign-in`) or a parameter (`:workspaceId`).
fn is_segment(segment: &str) -> bool {
    if segment.len() > MAX_SEGMENT {
        return false;
    }
    match segment.strip_prefix(':') {
        Some(name) => {
            name.starts_with(|c: char| c.is_ascii_lowercase())
                && name.chars().all(|c| c.is_ascii_alphabetic())
        }
        None => {
            segment.starts_with(|c: char| c.is_ascii_lowercase())
                && segment
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || matches!(c, '-' | '_'))
        }
    }
}

/// The visitor id Matomo sees for `account` on `day`: the first 16 hex
/// characters of HMAC-SHA256(`key`, `<account>:<YYYY-MM-DD>`). The same all
/// day, another the next day and for another account; without the key it
/// cannot be linked to the account or across days.
pub(crate) fn visitor_id(key: &[u8], account: &str, day: NaiveDate) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let message = format!("{account}:{}", day.format("%Y-%m-%d"));
    crypto::hmac_sha256(key, message.as_bytes())
        .iter()
        .take(8)
        .flat_map(|byte| {
            [
                char::from(HEX[usize::from(byte >> 4)]),
                char::from(HEX[usize::from(byte & 0x0f)]),
            ]
        })
        .collect()
}

/// What one screen view tells Matomo.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Hit<'a> {
    /// A route template ([`route`]).
    pub route: &'a str,
    /// The day's visitor id ([`visitor_id`]).
    pub visitor: &'a str,
    /// The request's `User-Agent`, capped.
    pub user_agent: Option<&'a str>,
    /// The request's language, when we ship it.
    pub lang: Option<Locale>,
}

/// The tracking request for `hit`: `<matomo>/matomo.php` with `idsite`,
/// `rec`, `apiv`, `url` (`app` and the route), `action_name` (the route),
/// `_id`, `ua`, `lang` and `send_image=0`, and nothing else.
pub(crate) fn tracking_url(matomo: &Matomo<'_>, app: &str, hit: &Hit<'_>) -> Option<Url> {
    let mut url = Url::parse(&format!("{}/matomo.php", matomo.url)).ok()?;
    url.set_query(None);
    url.set_fragment(None);
    {
        let mut query = url.query_pairs_mut();
        query
            .append_pair("idsite", matomo.site_id)
            .append_pair("rec", "1")
            .append_pair("apiv", "1")
            .append_pair(
                "url",
                &format!("{}{}", app.trim_end_matches('/'), hit.route),
            )
            .append_pair("action_name", hit.route)
            .append_pair("_id", hit.visitor)
            .append_pair("send_image", "0");
        if let Some(user_agent) = hit.user_agent {
            query.append_pair("ua", user_agent);
        }
        if let Some(lang) = hit.lang {
            query.append_pair("lang", lang.tag());
        }
    }
    Some(url)
}

/// The queue screen views wait in on their way to Matomo, and the one
/// worker that sends them, started on the first view. Cheap to clone;
/// clones share the queue.
#[derive(Clone)]
pub struct ScreenViewRelay {
    inner: Arc<Relay>,
}

struct Relay {
    capacity: usize,
    sender: OnceLock<mpsc::Sender<Url>>,
}

impl Default for ScreenViewRelay {
    fn default() -> Self {
        Self::with_capacity(QUEUE_CAPACITY)
    }
}

impl std::fmt::Debug for ScreenViewRelay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScreenViewRelay")
            .field("capacity", &self.inner.capacity)
            .finish_non_exhaustive()
    }
}

impl ScreenViewRelay {
    /// A relay holding at most `capacity` views (at least one).
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: Arc::new(Relay {
                capacity: capacity.max(1),
                sender: OnceLock::new(),
            }),
        }
    }

    /// Queue `hit` for the worker (started on first use, on the current
    /// Tokio runtime). Never waits: a full queue drops the view. Whether it
    /// was queued.
    pub(crate) fn enqueue(&self, hit: Url) -> bool {
        let sender = self.inner.sender.get_or_init(|| {
            let (sender, receiver) = mpsc::channel(self.inner.capacity);
            tokio::spawn(forward_all(receiver));
            sender
        });
        let queued = sender.try_send(hit).is_ok();
        if !queued {
            views().add(1, &[KeyValue::new("outcome", "dropped")]);
        }
        queued
    }

    /// A relay whose queue nobody drains: what it would send stays in the
    /// receiver.
    #[cfg(test)]
    pub(crate) fn detached(capacity: usize) -> (Self, mpsc::Receiver<Url>) {
        let relay = Self::with_capacity(capacity);
        let (sender, receiver) = mpsc::channel(relay.inner.capacity);
        let _ = relay.inner.sender.set(sender);
        (relay, receiver)
    }
}

/// Send every queued view to Matomo, one at a time, until the relay is
/// dropped. A failure drops that view; nothing is retried.
async fn forward_all(mut hits: mpsc::Receiver<Url>) {
    let client = reqwest::Client::builder()
        .timeout(FORWARD_TIMEOUT)
        .connect_timeout(CONNECT_TIMEOUT)
        .build()
        .unwrap_or_default();
    while let Some(hit) = hits.recv().await {
        let outcome = match client.get(hit).send().await {
            Ok(response) if response.status().is_success() => "forwarded",
            Ok(response) => {
                tracing::warn!(
                    status = response.status().as_u16(),
                    "Matomo refused a screen view"
                );
                "failed"
            }
            Err(e) => {
                // Without the URL: it holds the visitor id and user agent.
                let e = e.without_url();
                tracing::warn!(error = %redact::error_kind(&e.to_string()), "could not reach Matomo");
                "failed"
            }
        };
        views().add(1, &[KeyValue::new("outcome", outcome)]);
    }
}

/// `campbooks.screen_views`, by outcome (`forwarded`, `failed`, `dropped`).
fn views() -> &'static Counter<u64> {
    static VIEWS: OnceLock<Counter<u64>> = OnceLock::new();
    VIEWS.get_or_init(|| {
        campbooks_kernel::telemetry::meter()
            .u64_counter("campbooks.screen_views")
            .with_description("Screen views relayed to usage analytics, or dropped.")
            .build()
    })
}

#[cfg(test)]
mod tests;
