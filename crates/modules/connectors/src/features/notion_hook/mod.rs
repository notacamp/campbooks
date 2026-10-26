//! `POST /hooks/notion` — where Notion posts its webhook events for the
//! pages a connection shares (campbooks-cloud#376).
//!
//! Public and outside the operation catalogue: Notion is the only caller,
//! and nothing else (no automation, no assistant) can reach it by an
//! operation id. It does the least it can:
//!
//! - **Absent** unless Notion is on (`CAMPBOOKS_CONNECTORS__NOTION__ENABLED`)
//!   and the instance secret is set: the path falls through to the
//!   application's fallback otherwise, and pages are polled. It exists
//!   **before** the webhook secret is configured, because Notion posts the
//!   subscription's verification token to it first.
//! - **Rate limited** first: [`PER_PROCESS_SECOND`] posts a second in each
//!   process before the database is asked anything, then the shared
//!   [`PER_ADDRESS`] and [`ALL`] limits (429 `rate_limited`).
//! - **No webhook secret configured**: the only body taken is exactly
//!   `{"verification_token": "<up to 200 printable characters>"}`. It is
//!   kept sealed as the instance's one token (a newer one replaces it; the
//!   operator reads it with `campbooks-server admin notion-webhook-token`
//!   and pastes it into Notion), answered 200, and logged as having arrived,
//!   never with its value. Every other post is 404.
//! - **A webhook secret configured** (`CAMPBOOKS_NOTION__WEBHOOK_SECRET[_FILE]`):
//!   a verification-shaped body is 404 (a token cannot be armed again); an
//!   event at most [`MAX_BODY`] bytes has its `X-Notion-Signature`
//!   (`sha256=` and the hexadecimal HMAC-SHA256 of the **raw body** under the
//!   secret) checked in constant time before the body is read as anything. A
//!   wrong or missing signature is 401 and nothing else happens. For a page
//!   event, the streams linked to that Notion page are found across
//!   workspaces in the maintenance scope (`FOR SELECT` on
//!   `connectors.notion_links` only: ids), and each is made due now in its own
//!   workspace unless it is paused or waiting for the member. 202.
//!
//! It calls no provider and reads no page: the run reads the blocks, so an
//! event can only make a stream read early. Neither the secret, the
//! signature nor the body is ever logged.

#[cfg(test)]
mod tests;

use std::{
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, Instant},
};

use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, FromRef, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
};
use campbooks_kernel::{
    AppState,
    crypto::{hmac_sha256, same_hash},
    db::BeginFor as _,
    rate_limit::{ClientIp, Limit, RateKey},
    tenancy::WorkspaceId,
};
use secrecy::ExposeSecret as _;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    deps::Deps,
    domain::{
        model::{ConnectionId, StreamId},
        notion_webhook, schedule,
    },
    notion::HOOK_PATH,
};

/// The largest event body read.
pub const MAX_BODY: usize = 64 * 1024;

/// The page events that make a page's stream read: its content, title or
/// place changed, or it was trashed or brought back.
const PAGE_EVENTS: [&str; 5] = [
    "page.content_updated",
    "page.properties_updated",
    "page.moved",
    "page.deleted",
    "page.undeleted",
];

/// Posts one address may make.
pub const PER_ADDRESS: Limit =
    Limit::new("connectors.notion_webhook", 1_200, Duration::from_mins(1));

/// Posts the whole instance takes, every process together: generous, a
/// bound on what anonymous posts can cost.
pub const ALL: Limit = Limit::new(
    "connectors.notion_webhook_all",
    30_000,
    Duration::from_mins(1),
);

/// Posts one process takes per second before it asks the database anything
/// (the shared limits above included).
pub const PER_PROCESS_SECOND: u32 = 500;

/// The hook's route over `state` and `deps`, ready to merge into the
/// server's application: none unless this instance serves the webhook.
pub fn router(state: AppState, deps: Deps) -> Router {
    if !serves(&deps) {
        return Router::new();
    }
    Router::new()
        .route(HOOK_PATH, post(receive))
        .layer(DefaultBodyLimit::max(MAX_BODY))
        .with_state(Hook {
            state,
            deps,
            second: Arc::new(Mutex::new((Instant::now(), 0))),
        })
}

/// Whether the hook is served: Notion is on and the instance secret seals
/// the verification token. The webhook secret is not needed (it is set from
/// the token this keeps).
fn serves(deps: &Deps) -> bool {
    deps.config.notion.is_enabled() && deps.notion_cipher.is_some()
}

/// The longest body taken while no webhook secret is configured: a
/// verification post is a few dozen bytes.
const MAX_VERIFICATION_BODY: usize = 1024;

/// The token a verification post carries: a body that is exactly
/// `{"verification_token": "<text>"}` and nothing else.
fn verification_token(body: &Value) -> Option<&str> {
    let object = body.as_object()?;
    if object.len() != 1 {
        return None;
    }
    object.get("verification_token")?.as_str()
}

/// What the hook reaches.
#[derive(Clone)]
struct Hook {
    state: AppState,
    deps: Deps,
    /// This process's count of posts in the current second.
    second: Arc<Mutex<(Instant, u32)>>,
}

impl FromRef<Hook> for AppState {
    fn from_ref(hook: &Hook) -> Self {
        hook.state.clone()
    }
}

impl Hook {
    /// Count one post in this process; whether it is within
    /// [`PER_PROCESS_SECOND`].
    fn within_second(&self) -> bool {
        let mut second = self.second.lock().unwrap_or_else(PoisonError::into_inner);
        let now = Instant::now();
        if now.duration_since(second.0) >= Duration::from_secs(1) {
            *second = (now, 0);
        }
        second.1 = second.1.saturating_add(1);
        second.1 <= PER_PROCESS_SECOND
    }

    /// The shared limits: per address, then the instance's (a refusal is
    /// `rate_limited`, 429).
    async fn within_limits(&self, ip: ClientIp) -> Result<(), campbooks_kernel::Error> {
        let limits = &self.state.rate_limits;
        for (limit, key) in [
            (&PER_ADDRESS, RateKey::ip(ip)),
            (&ALL, RateKey::text("instance")),
        ] {
            limits.check(&self.state.db, limit, &key).await?;
        }
        Ok(())
    }
}

impl Hook {
    /// A post while no webhook secret is configured: only a verification
    /// token is taken, kept sealed (it replaces the one before). Anything
    /// else is 404.
    async fn keep_verification(&self, body: &[u8]) -> Response {
        if body.len() > MAX_VERIFICATION_BODY {
            return StatusCode::NOT_FOUND.into_response();
        }
        let Ok(posted) = serde_json::from_slice::<Value>(body) else {
            return StatusCode::NOT_FOUND.into_response();
        };
        let Some(token) = verification_token(&posted).filter(|t| notion_webhook::acceptable(t))
        else {
            return StatusCode::NOT_FOUND.into_response();
        };
        let Some(cipher) = self.deps.notion_cipher.as_ref() else {
            return StatusCode::NOT_FOUND.into_response();
        };
        let kept = async {
            let mut tx = campbooks_kernel::db::begin_maintenance(&self.state.db).await?;
            notion_webhook::store(&mut tx, cipher, token, self.deps.clock.now()).await?;
            tx.commit().await?;
            Ok::<(), campbooks_kernel::Error>(())
        }
        .await;
        match kept {
            Ok(()) => {
                tracing::info!("notion webhook verification token received");
                StatusCode::OK.into_response()
            }
            Err(err) => {
                tracing::warn!(
                    error = %campbooks_kernel::redact::error(&err),
                    "could not keep a Notion webhook verification token"
                );
                StatusCode::SERVICE_UNAVAILABLE.into_response()
            }
        }
    }
}

/// The bytes `hex` spells, when it is whole hexadecimal.
fn decode_hex(hex: &str) -> Option<Vec<u8>> {
    if !hex.len().is_multiple_of(2) || !hex.is_ascii() {
        return None;
    }
    (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(hex.get(at..at + 2)?, 16).ok())
        .collect()
}

/// Whether `signature` (`sha256=<hex>`) is the HMAC-SHA256 of `body` under
/// `secret`, compared in constant time.
pub fn signed(secret: &[u8], body: &[u8], signature: &str) -> bool {
    let Some(given) = signature
        .trim()
        .strip_prefix("sha256=")
        .and_then(decode_hex)
    else {
        return false;
    };
    same_hash(&hmac_sha256(secret, body), &given)
}

async fn receive(
    State(hook): State<Hook>,
    ip: ClientIp,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !serves(&hook.deps) {
        return StatusCode::NOT_FOUND.into_response();
    }
    // Over this process's own bound, nothing reaches the database.
    if !hook.within_second() {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }
    if let Err(refused) = hook.within_limits(ip).await {
        return refused.into_response();
    }
    let Some(secret) = hook.deps.notion_webhook.as_ref() else {
        return hook.keep_verification(&body).await;
    };
    let Ok(secret) = secret.read("CAMPBOOKS_NOTION__WEBHOOK_SECRET_FILE") else {
        tracing::warn!("the Notion webhook secret could not be read");
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let signature = headers
        .get("x-notion-signature")
        .and_then(|value| value.to_str().ok());
    let authentic = signature
        .is_some_and(|signature| signed(secret.expose_secret().as_bytes(), &body, signature));
    let Ok(posted) = serde_json::from_slice::<Value>(&body) else {
        return if authentic {
            StatusCode::BAD_REQUEST.into_response()
        } else {
            StatusCode::UNAUTHORIZED.into_response()
        };
    };
    // With the secret set, a verification post is none of ours: a token
    // cannot be armed again.
    if verification_token(&posted).is_some() {
        return StatusCode::NOT_FOUND.into_response();
    }
    if !authentic {
        tracing::info!("a Notion webhook post with a wrong or missing signature was refused");
        return StatusCode::UNAUTHORIZED.into_response();
    }
    // The subscription is made for one API version, and an event of another
    // shape is not read (answered 200: Notion would only post it again).
    if posted.get("api_version").and_then(Value::as_str) != Some(crate::notion::VERSION) {
        tracing::info!("a Notion webhook event of another API version was ignored");
        return StatusCode::OK.into_response();
    }
    let page = posted
        .get("type")
        .and_then(Value::as_str)
        .filter(|kind| PAGE_EVENTS.contains(kind))
        .and_then(|_| posted.pointer("/entity/id"))
        .and_then(Value::as_str)
        .and_then(|id| Uuid::parse_str(id).ok());
    // An event type this hook does not act on is answered 200 so Notion does
    // not retry it, and marks nothing due.
    let Some(page) = page else {
        return StatusCode::OK.into_response();
    };
    if let Err(err) = wake(&hook, page).await {
        tracing::warn!(
            error = %campbooks_kernel::redact::error(&err),
            "could not take a Notion webhook event"
        );
        // Notion posts it again later.
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    StatusCode::ACCEPTED.into_response()
}

/// A stream as the maintenance scope reads it.
type Found = (Uuid, Uuid, Uuid);

/// Make the streams linked to Notion page `page` read now: found across
/// workspaces (ids only), woken in their own.
async fn wake(hook: &Hook, page: Uuid) -> Result<(), campbooks_kernel::Error> {
    let found: Vec<Found> = {
        let mut tx = campbooks_kernel::db::begin_maintenance(&hook.state.db).await?;
        let found: Vec<Found> = sqlx::query_as(
            "SELECT workspace_id, connection_id, stream_id
               FROM connectors.notion_links WHERE notion_page_id = $1",
        )
        .bind(page)
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        found
    };
    let now = hook.deps.clock.now();
    for (workspace, connection, stream) in found {
        let (workspace, connection, stream) = (
            WorkspaceId::from_uuid(workspace),
            ConnectionId::from_uuid(connection),
            StreamId::from_uuid(stream),
        );
        let mut tx = hook.state.db.begin_for(workspace).await?;
        let standing: Option<(bool, String)> = sqlx::query_as(
            "SELECT s.paused, c.health
               FROM connectors.streams s JOIN connectors.connections c ON c.id = s.connection_id
              WHERE s.id = $1 AND s.connection_id = $2",
        )
        .bind(stream)
        .bind(connection)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some((false, health)) = standing
            && health != "needs_reauth"
        {
            // Notion rounds edit times to the minute: a run woken by an
            // event reads the page whatever its time says.
            sqlx::query(
                "UPDATE connectors.streams
                    SET cursor = jsonb_set(cursor, '{force}', 'true'::jsonb)
                  WHERE id = $1",
            )
            .bind(stream)
            .execute(&mut *tx)
            .await?;
            schedule::run_now(&mut tx, workspace, stream, now).await?;
        }
        tx.commit().await?;
    }
    Ok(())
}
