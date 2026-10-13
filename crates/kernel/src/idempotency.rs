//! Idempotency for every mutating operation (ADR 0013).
//!
//! One middleware sits in front of all module routes. For an operation whose
//! `x-campbooks.mutates` is true (or, with no metadata, any POST, PUT, PATCH
//! or DELETE) and a request carrying `Idempotency-Key`, it:
//!
//! - runs the first request with that key and stores its response;
//! - replays the stored response (with `Idempotency-Replayed: true`) for a
//!   later identical request, without running the handler again;
//! - answers **422** when the key is reused for another operation or another
//!   request (method, path, query or body differ);
//! - answers **409** while the first request with the key is still running.
//!
//! Keys belong to the authenticated caller; requests without a valid token
//! are passed through untouched (the handler answers them as usual). GET and
//! HEAD requests ignore the header. Keys expire after [`RETENTION`]; the
//! kernel's `kernel.prune_idempotency` schedule deletes them. There is no
//! per-operation switch: idempotency is decided by the catalogue alone.
//!
//! Responses are stored when the outcome is deterministic: every status
//! below 500 except 401, 403, 408, 409, 425 and 429. Otherwise the key is
//! released, so a retry runs the operation again.

use std::time::Duration;

use aws_lc_rs::digest;
use axum::{
    body::{Body, Bytes, to_bytes},
    extract::{MatchedPath, Request, State},
    http::{HeaderValue, Method, StatusCode, header, request::Parts},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;

use crate::{
    AppState,
    auth::{AuthenticatedPrincipal, Principal, PrincipalKind, TokenVerifier, bearer_token},
    error::LocalizableProblem,
    i18n::MESSAGES,
    jobs::{Job, JobContext, JobError},
    operations::{Catalogue, OperationInfo},
};

/// The request header carrying the key.
pub const HEADER: &str = "idempotency-key";

/// The response header marking a replayed response.
pub const REPLAYED_HEADER: &str = "idempotency-replayed";

/// How long a key is remembered.
pub const RETENTION: Duration = Duration::from_hours(24);

/// Longest key accepted, in bytes.
pub const MAX_KEY_LEN: usize = 255;

/// Largest request body hashed (and therefore accepted with a key).
pub const MAX_REQUEST_BYTES: usize = 10 * 1024 * 1024;

/// Largest response body stored for replay; larger responses release the key.
pub const MAX_STORED_RESPONSE_BYTES: usize = 1024 * 1024;

/// A pending key older than this belongs to a request that died with its
/// process (requests time out well before); the same request may take it over.
const ABANDONED_AFTER: Duration = Duration::from_mins(2);

/// What the middleware needs: the database, the token verifier and the
/// catalogue telling it which operations mutate.
#[derive(Debug, Clone)]
pub struct Idempotency {
    db: PgPool,
    auth: TokenVerifier,
    catalogue: Arc<Catalogue>,
}

impl Idempotency {
    /// The middleware state for `catalogue`, over `state`'s database and
    /// token verifier. Install it with
    /// `router.route_layer(axum::middleware::from_fn_with_state(it, idempotency::middleware))`.
    pub fn new(state: &AppState, catalogue: Arc<Catalogue>) -> Self {
        Self {
            db: state.db.clone(),
            auth: state.auth.clone(),
            catalogue,
        }
    }

    fn operation(&self, request: &Request) -> Option<&OperationInfo> {
        let path = request.extensions().get::<MatchedPath>()?;
        self.catalogue.route(request.method(), path.as_str())
    }

    /// The caller: authenticated in-process, or by a valid bearer token.
    /// (Takes the parts it needs rather than the request: a body is not
    /// `Sync`, so a borrowed request cannot be held across the verification.)
    async fn principal(
        &self,
        injected: Option<Principal>,
        token: Option<String>,
    ) -> Option<Principal> {
        if injected.is_some() {
            return injected;
        }
        self.auth.verify(&token?).await.ok()
    }
}

/// The middleware (see the module docs).
pub async fn middleware(State(idem): State<Idempotency>, request: Request, next: Next) -> Response {
    if matches!(*request.method(), Method::GET | Method::HEAD) {
        return next.run(request).await;
    }
    let Some(key) = request.headers().get(HEADER).cloned() else {
        return next.run(request).await;
    };
    let Some(operation_id) = idem
        .operation(&request)
        .filter(|op| op.mutates())
        .map(|op| op.id.clone())
    else {
        return next.run(request).await;
    };
    let key = match valid_key(&key) {
        Ok(key) => key,
        Err(problem) => return problem.into_response(),
    };
    let injected = request
        .extensions()
        .get::<AuthenticatedPrincipal>()
        .map(|AuthenticatedPrincipal(principal)| principal.clone());
    let token = bearer_token(request.headers()).ok().map(ToOwned::to_owned);
    let Some(principal) = idem.principal(injected, token).await else {
        // No valid token: nothing to scope the key to. The handler answers.
        return next.run(request).await;
    };

    let (mut parts, body) = request.into_parts();
    let Ok(body) = to_bytes(body, MAX_REQUEST_BYTES).await else {
        return problem(StatusCode::PAYLOAD_TOO_LARGE, "idempotency_body_too_large");
    };
    parts
        .extensions
        .insert(AuthenticatedPrincipal(principal.clone()));
    let owner = owner(&principal);
    let hash = request_hash(&parts, &body);

    match claim(&idem.db, &owner, &key, &operation_id, &hash).await {
        Err(error) => {
            tracing::warn!(%error, "idempotency: cannot claim the key");
            return problem(StatusCode::SERVICE_UNAVAILABLE, "database_unavailable");
        }
        Ok(Claim::Replay(stored)) => return stored.into_response(),
        Ok(Claim::Mismatch) => {
            return problem(StatusCode::UNPROCESSABLE_ENTITY, "idempotency_key_reused");
        }
        Ok(Claim::InFlight) => {
            return problem(StatusCode::CONFLICT, "idempotency_key_in_flight");
        }
        Ok(Claim::Acquired) => {}
    }

    let response = next.run(Request::from_parts(parts, Body::from(body))).await;
    finish(&idem.db, &owner, &key, response).await
}

fn valid_key(value: &HeaderValue) -> Result<String, LocalizableProblem> {
    let key = value.to_str().ok().map(str::trim).unwrap_or_default();
    if key.is_empty() || key.len() > MAX_KEY_LEN || !key.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(LocalizableProblem::new(
            StatusCode::BAD_REQUEST,
            MESSAGES
                .reason("idempotency_key_invalid")
                .arg("max", MAX_KEY_LEN),
        ));
    }
    Ok(key.to_owned())
}

/// A problem raised by this layer, with a kernel reason `code`.
fn problem(status: StatusCode, code: &'static str) -> Response {
    LocalizableProblem::new(status, MESSAGES.reason(code)).into_response()
}

/// Keys are per caller: users by subject, clients by client id.
fn owner(principal: &Principal) -> String {
    match principal.kind {
        PrincipalKind::User => format!("user:{}", principal.subject),
        PrincipalKind::Client => format!("client:{}", principal.subject),
    }
}

fn request_hash(parts: &Parts, body: &Bytes) -> Vec<u8> {
    let mut context = digest::Context::new(&digest::SHA256);
    context.update(parts.method.as_str().as_bytes());
    context.update(b"\0");
    context.update(parts.uri.path().as_bytes());
    context.update(b"\0");
    context.update(parts.uri.query().unwrap_or_default().as_bytes());
    context.update(b"\0");
    context.update(body);
    context.finish().as_ref().to_vec()
}

enum Claim {
    Acquired,
    Replay(Stored),
    Mismatch,
    InFlight,
}

struct Stored {
    status: i16,
    content_type: Option<String>,
    location: Option<String>,
    body: Vec<u8>,
}

impl IntoResponse for Stored {
    fn into_response(self) -> Response {
        let status = u16::try_from(self.status)
            .ok()
            .and_then(|s| StatusCode::from_u16(s).ok())
            .unwrap_or(StatusCode::OK);
        let mut response = (status, self.body).into_response();
        let headers = response.headers_mut();
        headers.remove(header::CONTENT_TYPE);
        for (name, value) in [
            (header::CONTENT_TYPE, self.content_type),
            (header::LOCATION, self.location),
        ] {
            if let Some(value) = value.and_then(|v| HeaderValue::from_str(&v).ok()) {
                headers.insert(name, value);
            }
        }
        headers.insert(REPLAYED_HEADER, HeaderValue::from_static("true"));
        response
    }
}

async fn claim(
    db: &PgPool,
    owner: &str,
    key: &str,
    operation_id: &str,
    hash: &[u8],
) -> Result<Claim, sqlx::Error> {
    let retention = seconds(RETENTION);
    let abandoned = seconds(ABANDONED_AFTER);
    // Two rounds: the row seen as a conflict may expire or be released before
    // it is read back.
    for _ in 0..2 {
        let acquired = sqlx::query_scalar!(
            r#"
            INSERT INTO kernel.idempotency
                (principal, key, operation_id, request_hash, expires_at)
            VALUES ($1, $2, $3, $4, now() + make_interval(secs => $5))
            ON CONFLICT (principal, key) DO UPDATE
               SET operation_id = EXCLUDED.operation_id,
                   request_hash = EXCLUDED.request_hash,
                   state = 'pending',
                   response_status = NULL,
                   response_content_type = NULL,
                   response_location = NULL,
                   response_body = NULL,
                   created_at = now(),
                   expires_at = EXCLUDED.expires_at
             WHERE kernel.idempotency.expires_at <= now()
                OR (kernel.idempotency.state = 'pending'
                    AND kernel.idempotency.created_at < now() - make_interval(secs => $6)
                    AND kernel.idempotency.operation_id = EXCLUDED.operation_id
                    AND kernel.idempotency.request_hash = EXCLUDED.request_hash)
            RETURNING true AS "acquired!"
            "#,
            owner,
            key,
            operation_id,
            hash,
            retention,
            abandoned,
        )
        .fetch_optional(db)
        .await?;
        if acquired.is_some() {
            return Ok(Claim::Acquired);
        }

        let existing = sqlx::query!(
            r#"
            SELECT operation_id, request_hash, state, response_status,
                   response_content_type, response_location, response_body
              FROM kernel.idempotency
             WHERE principal = $1 AND key = $2 AND expires_at > now()
            "#,
            owner,
            key,
        )
        .fetch_optional(db)
        .await?;
        let Some(row) = existing else {
            continue;
        };
        if row.operation_id != operation_id || row.request_hash != hash {
            return Ok(Claim::Mismatch);
        }
        return Ok(
            match (row.state.as_str(), row.response_status, row.response_body) {
                ("completed", Some(status), Some(body)) => Claim::Replay(Stored {
                    status,
                    content_type: row.response_content_type,
                    location: row.response_location,
                    body,
                }),
                _ => Claim::InFlight,
            },
        );
    }
    Ok(Claim::InFlight)
}

/// Whether a response with `status` is the operation's settled outcome.
fn storable(status: StatusCode) -> bool {
    status.as_u16() < 500
        && !matches!(
            status,
            StatusCode::UNAUTHORIZED
                | StatusCode::FORBIDDEN
                | StatusCode::REQUEST_TIMEOUT
                | StatusCode::CONFLICT
                | StatusCode::TOO_EARLY
                | StatusCode::TOO_MANY_REQUESTS
        )
}

/// Store the response for replay, or release the key.
async fn finish(db: &PgPool, owner: &str, key: &str, response: Response) -> Response {
    let status = response.status();
    if !storable(status) {
        release(db, owner, key).await;
        return response;
    }
    let (parts, body) = response.into_parts();
    let Ok(body) = to_bytes(body, usize::MAX).await else {
        release(db, owner, key).await;
        return problem(StatusCode::INTERNAL_SERVER_ERROR, "internal_error");
    };
    if body.len() > MAX_STORED_RESPONSE_BYTES {
        tracing::warn!(
            bytes = body.len(),
            "idempotency: response too large to store; key released"
        );
        release(db, owner, key).await;
        return Response::from_parts(parts, Body::from(body));
    }
    let header = |name| {
        parts
            .headers
            .get(name)
            .and_then(|v: &HeaderValue| v.to_str().ok())
            .map(ToOwned::to_owned)
    };
    let stored = sqlx::query!(
        r#"
        UPDATE kernel.idempotency
           SET state = 'completed', response_status = $3, response_content_type = $4,
               response_location = $5, response_body = $6
         WHERE principal = $1 AND key = $2 AND state = 'pending'
        "#,
        owner,
        key,
        i16::try_from(status.as_u16()).unwrap_or(i16::MAX),
        header(header::CONTENT_TYPE),
        header(header::LOCATION),
        body.as_ref(),
    )
    .execute(db)
    .await;
    if let Err(error) = stored {
        tracing::warn!(%error, "idempotency: cannot store the response");
    }
    Response::from_parts(parts, Body::from(body))
}

async fn release(db: &PgPool, owner: &str, key: &str) {
    let released = sqlx::query!(
        "DELETE FROM kernel.idempotency WHERE principal = $1 AND key = $2 AND state = 'pending'",
        owner,
        key,
    )
    .execute(db)
    .await;
    if let Err(error) = released {
        tracing::warn!(%error, "idempotency: cannot release the key");
    }
}

fn seconds(duration: Duration) -> f64 {
    duration.as_secs_f64()
}

/// Rows deleted per statement, so one prune never holds a long lock.
const PRUNE_BATCH: i64 = 5_000;

/// Delete expired idempotency keys. Scheduled hourly by the kernel.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct PruneIdempotency;

impl Job for PruneIdempotency {
    const NAME: &'static str = "kernel.prune_idempotency";
    const QUEUE: &'static str = "maintenance";
    const MAX_ATTEMPTS: u32 = 3;

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        let mut pruned = 0_u64;
        loop {
            let deleted = sqlx::query!(
                r#"
                DELETE FROM kernel.idempotency
                 WHERE (principal, key) IN (
                     SELECT principal, key FROM kernel.idempotency
                      WHERE expires_at <= now()
                      LIMIT $1)
                "#,
                PRUNE_BATCH,
            )
            .execute(ctx.pool())
            .await?
            .rows_affected();
            pruned += deleted;
            if deleted < PRUNE_BATCH.unsigned_abs() {
                break;
            }
        }
        tracing::info!(pruned, "pruned expired idempotency keys");
        Ok(())
    }
}
