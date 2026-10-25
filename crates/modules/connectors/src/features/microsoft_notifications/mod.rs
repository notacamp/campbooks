//! `POST /hooks/microsoft/mail` — where Microsoft Graph posts change
//! notifications for Microsoft mailboxes (campbooks-cloud#316).
//!
//! Public and outside the operation catalogue: Graph is the only caller,
//! and nothing else (no automation, no assistant) can reach it by an
//! operation id. It does the least it can:
//!
//! - **Absent** unless this instance takes Microsoft notifications: the path
//!   falls through to the application's fallback.
//! - **Rate limited** first: [`PER_PROCESS_SECOND`] posts a second in each
//!   process before the database is asked anything, then the shared
//!   [`PER_ADDRESS`] and [`ALL`] limits (429 `rate_limited`).
//! - **The validation handshake**: with `validationToken`, the token back
//!   as `text/plain` (Graph checks the URL when a subscription is made or
//!   renewed), nothing else.
//! - **A notification**: at most [`MAX_BODY`] bytes of JSON, at most
//!   [`MAX_NOTIFICATIONS`] notifications. Their subscriptions are looked up
//!   across workspaces in the maintenance scope (`FOR SELECT` on
//!   `connectors.microsoft_subscriptions` only: ids, the `clientState`'s
//!   keyed hash and where the subscription belongs); each notification's
//!   `clientState` is checked against the hash in constant time; for each
//!   connection kept, in its own workspace, its stream's run is made due now
//!   unless the stream is paused or waiting for the member (a pending run
//!   absorbs it: the debounce). 202 when one was kept, **404** otherwise, the
//!   same for an unknown subscription and a wrong `clientState`.
//!
//! It calls no provider and reads no mail: the run reads the folders'
//! deltas, so a notification can only make one stream read early. Neither
//! the `clientState` nor the body is ever logged.

#[cfg(test)]
mod tests;

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, Instant},
};

use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, FromRef, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use campbooks_kernel::{
    AppState,
    db::BeginFor as _,
    rate_limit::{ClientIp, Limit, RateKey},
    tenancy::WorkspaceId,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    deps::Deps,
    domain::{
        model::{ConnectionId, StreamId},
        schedule,
    },
    microsoft::{NOTIFICATION_PATH, graph::valid_subscription_id, subscriptions},
};

/// The largest notification body read.
pub const MAX_BODY: usize = 64 * 1024;

/// Most notifications one post may carry.
pub const MAX_NOTIFICATIONS: usize = 100;

/// The longest validation token answered, in characters.
pub const MAX_VALIDATION_TOKEN: usize = 1024;

/// Posts one address may make: loose, as Graph posts from many addresses
/// but several from each.
pub const PER_ADDRESS: Limit = Limit::new(
    "connectors.microsoft_notifications",
    1_200,
    Duration::from_mins(1),
);

/// Posts the whole instance takes, every process together: generous, a
/// bound on what anonymous posts can cost.
pub const ALL: Limit = Limit::new(
    "connectors.microsoft_notifications_all",
    30_000,
    Duration::from_mins(1),
);

/// Posts one process takes per second before it asks the database anything
/// (the shared limits above included).
pub const PER_PROCESS_SECOND: u32 = 500;

/// The callback's route over `state` and `deps`, ready to merge into the
/// server's application: none (the path falls through to the
/// application's fallback) unless this instance takes Microsoft
/// notifications.
pub fn router(state: AppState, deps: Deps) -> Router {
    let offered = deps
        .microsoft
        .as_ref()
        .is_some_and(|microsoft| microsoft.notification_url().is_some());
    if !offered {
        return Router::new();
    }
    Router::new()
        .route(NOTIFICATION_PATH, post(receive))
        .layer(DefaultBodyLimit::max(MAX_BODY))
        .with_state(Hook {
            state,
            deps,
            second: Arc::new(Mutex::new((Instant::now(), 0))),
        })
}

/// What the callback reaches.
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

/// The query of a validation handshake.
#[derive(Debug, Deserialize)]
struct Handshake {
    #[serde(default, rename = "validationToken")]
    validation_token: Option<String>,
}

/// A notification post.
#[derive(Debug, Deserialize)]
struct Posted {
    #[serde(default)]
    value: Vec<Notification>,
}

/// One notification, as much of it as is used.
#[derive(Debug, Deserialize)]
struct Notification {
    #[serde(default, rename = "subscriptionId")]
    subscription_id: Option<String>,
    #[serde(default, rename = "clientState")]
    client_state: Option<String>,
    #[serde(default, rename = "lifecycleEvent")]
    lifecycle_event: Option<String>,
}

/// What a kept notification asks of its connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Asked {
    /// A change: read.
    Read,
    /// Graph wants the subscription renewed: read, renewing it.
    Renew,
    /// Graph removed the subscription: read, making a new one.
    Removed,
}

/// A subscription as the maintenance scope reads it.
type Found = (String, Uuid, Uuid, Uuid, Vec<u8>);

async fn receive(
    State(hook): State<Hook>,
    ip: ClientIp,
    Query(handshake): Query<Handshake>,
    body: Bytes,
) -> Response {
    let offered = hook
        .deps
        .microsoft
        .as_ref()
        .is_some_and(|microsoft| microsoft.notification_url().is_some());
    if !offered {
        return StatusCode::NOT_FOUND.into_response();
    }
    // Over this process's own bound, nothing reaches the database.
    if !hook.within_second() {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }
    if let Err(refused) = hook.within_limits(ip).await {
        return refused.into_response();
    }
    if let Some(token) = handshake.validation_token {
        let usable = !token.is_empty()
            && token.chars().count() <= MAX_VALIDATION_TOKEN
            && !token.chars().any(char::is_control);
        if !usable {
            return StatusCode::BAD_REQUEST.into_response();
        }
        return ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], token).into_response();
    }
    let Ok(posted) = serde_json::from_slice::<Posted>(&body) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    if posted.value.len() > MAX_NOTIFICATIONS {
        return StatusCode::PAYLOAD_TOO_LARGE.into_response();
    }
    match accept(&hook, posted.value).await {
        Ok(0) => StatusCode::NOT_FOUND.into_response(),
        Ok(_) => StatusCode::ACCEPTED.into_response(),
        Err(err) => {
            tracing::warn!(
                error = %campbooks_kernel::redact::error(&err),
                "could not take a Microsoft notification"
            );
            // Graph posts it again later.
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
    }
}

/// Keep the `notifications` whose subscription is known and whose
/// `clientState` verifies, and ask for each connection's run. How many
/// connections were kept.
async fn accept(
    hook: &Hook,
    notifications: Vec<Notification>,
) -> Result<usize, campbooks_kernel::Error> {
    let mut ids: Vec<String> = notifications
        .iter()
        .filter_map(|n| n.subscription_id.clone())
        .filter(|id| valid_subscription_id(id))
        .collect();
    ids.sort();
    ids.dedup();
    if ids.is_empty() {
        return Ok(0);
    }
    let found: Vec<Found> = {
        let mut tx = campbooks_kernel::db::begin_maintenance(&hook.state.db).await?;
        let found: Vec<Found> = sqlx::query_as(
            "SELECT subscription_id, workspace_id, connection_id, stream_id, client_state_tag
               FROM connectors.microsoft_subscriptions
              WHERE subscription_id = ANY($1) AND client_state_tag IS NOT NULL",
        )
        .bind(&ids)
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        found
    };
    let mut kept: BTreeMap<ConnectionId, (WorkspaceId, StreamId, Asked)> = BTreeMap::new();
    for notification in notifications {
        let (Some(id), Some(state)) = (notification.subscription_id, notification.client_state)
        else {
            continue;
        };
        let Some((_, workspace, connection, stream, tag)) =
            found.iter().find(|(known, ..)| *known == id)
        else {
            continue;
        };
        let signer = &hook.state.signer;
        if !signer.verify(subscriptions::CLIENT_STATE_CONTEXT, state.as_bytes(), tag) {
            tracing::info!("a Microsoft notification with a wrong clientState was refused");
            continue;
        }
        let asked = match notification.lifecycle_event.as_deref() {
            Some("subscriptionRemoved") => Asked::Removed,
            Some("reauthorizationRequired") => Asked::Renew,
            _ => Asked::Read,
        };
        let entry = kept.entry(ConnectionId::from_uuid(*connection)).or_insert((
            WorkspaceId::from_uuid(*workspace),
            StreamId::from_uuid(*stream),
            asked,
        ));
        entry.2 = entry.2.max(asked);
    }
    let now = hook.deps.clock.now();
    for (connection, (workspace, stream, asked)) in &kept {
        let mut tx = hook.state.db.begin_for(*workspace).await?;
        match asked {
            Asked::Removed => {
                sqlx::query(
                    "DELETE FROM connectors.microsoft_subscriptions WHERE connection_id = $1",
                )
                .bind(connection)
                .execute(&mut *tx)
                .await?;
            }
            Asked::Renew => {
                sqlx::query(
                    "UPDATE connectors.microsoft_subscriptions
                        SET expires_at = LEAST(expires_at, $2)
                      WHERE connection_id = $1 AND expires_at IS NOT NULL",
                )
                .bind(connection)
                .bind(now)
                .execute(&mut *tx)
                .await?;
            }
            Asked::Read => {}
        }
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
            schedule::run_now(&mut tx, *workspace, *stream, now).await?;
        }
        tx.commit().await?;
    }
    Ok(kept.len())
}
