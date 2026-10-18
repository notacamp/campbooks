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
//! - never runs it again when the first request stopped without an answer
//!   (its process died after it may have committed): once that key has
//!   been pending for longer than any request runs, a retry gets **409**
//!   `idempotency_outcome_unknown` until the key expires (campbooks-cloud#63).
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
//!
//! **A replay is authorized like the request it repeats.** Before a stored
//! response is served, the caller must still hold the operation's scopes
//! (else 403, as [`Scoped`](crate::auth::Scoped) answers), still belong to
//! the workspace of a `{workspace_id}` route (a client token still bound to
//! it), and still see every container the first run's handler checked
//! ([`checked_container`]); otherwise the answer is the same 404 the handler
//! would give. A member removed from a workspace or an inbox never gets a
//! stored response back.
//!
//! **A secret is never stored.** An operation declared
//! `Operation::secret_once` (a new app's client secret, an invitation's
//! link, a single-use ticket) shows its secret in its successful answer and
//! nowhere else, ever: for any answer below 400 the layer keeps only that
//! the key was used (`withheld`: the status, a success's `Location`, no
//! body), and a replay answers
//! **409** `idempotency_secret_shown_once` instead of running the operation
//! again or showing the secret twice.
//!
//! **Keys are their caller's** (campbooks-cloud#210), and a person's keys
//! through one app are that app's alone (campbooks-cloud#213). Every
//! statement on `kernel.idempotency` runs in a transaction naming its
//! caller
//! ([`begin_as_idempotency_caller`](crate::db::begin_as_idempotency_caller)),
//! whose row-level security admits that caller's keys only; a workspace
//! transaction sees and writes none. The prune reads and deletes expired
//! keys in the kernel scope, and nothing else.

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
use sqlx::{PgConnection, PgPool};
use std::sync::Arc;

use crate::{
    AppState,
    auth::{AuthenticatedPrincipal, Principal, PrincipalKind, TokenVerifier, bearer_token},
    changes::VisibilityPort,
    error::LocalizableProblem,
    i18n::MESSAGES,
    jobs::{Job, JobContext, JobError},
    operations::{Catalogue, OperationInfo},
    records::RecordRef,
    tenancy::{ClientDirectoryPort, MembershipPort, WORKSPACE_PATH_PARAM, WorkspaceId},
};

tokio::task_local! {
    /// The containers the running request's handler checked access to.
    static CHECKED: std::sync::Mutex<Vec<RecordRef>>;
}

/// Note that the running request's handler checked the caller's access to
/// `container` (an inbox) before acting inside it. The idempotency layer
/// keeps the list with a stored response and serves a replay only to a
/// caller who may still see each one. A no-op outside a request.
pub fn checked_container(container: RecordRef) {
    let _ = CHECKED.try_with(|checked| {
        if let Ok(mut checked) = checked.lock()
            && !checked.contains(&container)
        {
            checked.push(container);
        }
    });
}

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
/// process (requests time out well before). It is never taken over: a retry
/// learns that its outcome is unknown (campbooks-cloud#63).
const ABANDONED_AFTER: Duration = Duration::from_mins(2);

/// What the middleware needs: the database, the token verifier and the
/// catalogue telling it which operations mutate.
#[derive(Debug, Clone)]
pub struct Idempotency {
    db: PgPool,
    auth: TokenVerifier,
    catalogue: Arc<Catalogue>,
    membership: MembershipPort,
    clients: ClientDirectoryPort,
    visibility: VisibilityPort,
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
            membership: Arc::clone(&state.membership),
            clients: Arc::clone(&state.clients),
            visibility: Arc::clone(&state.visibility),
        }
    }

    /// Whether `principal` may still have the stored response of
    /// `operation`, a request to `parts`, whose handler checked
    /// `containers`: `None` if so, else the answer to give instead.
    async fn replay_refusal(
        &self,
        operation: &OperationInfo,
        principal: &Principal,
        parts: &Parts,
        containers: &[RecordRef],
    ) -> Option<Response> {
        if !operation
            .scopes
            .iter()
            .all(|scope| principal.has_scope(scope))
        {
            return Some(crate::Error::Forbidden.into_response());
        }
        let not_found = || Some(crate::Error::NotFound.into_response());
        let Some(workspace) = workspace_of(operation, parts.uri.path()) else {
            // Not a workspace route: nothing else to resolve unless the
            // handler checked containers (it cannot have without one).
            return if containers.is_empty() {
                None
            } else {
                not_found()
            };
        };
        let Ok(workspace) = workspace.parse::<WorkspaceId>() else {
            return not_found();
        };
        let access = match crate::tenancy::resolve_access(
            self.membership.as_ref(),
            self.clients.as_ref(),
            &self.db,
            principal,
            workspace,
        )
        .await
        {
            Ok(Some(access)) => access,
            Ok(None) => return not_found(),
            Err(error) => return Some(error.into_response()),
        };
        for container in containers {
            match self
                .visibility
                .container_visible(&self.db, &access, container)
                .await
            {
                Ok(true) => {}
                Ok(false) => return not_found(),
                Err(error) => return Some(error.into_response()),
            }
        }
        None
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
    let Some(operation) = idem.operation(&request).filter(|op| op.mutates()).cloned() else {
        return next.run(request).await;
    };
    let operation_id = operation.id.clone();
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
        Ok(Claim::Replay(stored)) => {
            return match idem
                .replay_refusal(&operation, &principal, &parts, &stored.containers)
                .await
            {
                Some(refusal) => refusal,
                None => stored.into_response(),
            };
        }
        Ok(Claim::Withheld(containers)) => {
            return match idem
                .replay_refusal(&operation, &principal, &parts, &containers)
                .await
            {
                Some(refusal) => refusal,
                None => secret_shown_once(),
            };
        }
        Ok(Claim::Mismatch) => {
            return problem(StatusCode::UNPROCESSABLE_ENTITY, "idempotency_key_reused");
        }
        Ok(Claim::InFlight) => {
            return problem(StatusCode::CONFLICT, "idempotency_key_in_flight");
        }
        Ok(Claim::Unknown) => {
            return problem(StatusCode::CONFLICT, "idempotency_outcome_unknown");
        }
        Ok(Claim::Acquired) => {}
    }

    let (response, checked) = CHECKED
        .scope(std::sync::Mutex::new(Vec::new()), async {
            let response = next.run(Request::from_parts(parts, Body::from(body))).await;
            let checked = CHECKED
                .try_with(|checked| checked.lock().map(|c| c.clone()).unwrap_or_default())
                .unwrap_or_default();
            (response, checked)
        })
        .await;
    let keep = if operation.secret_once() {
        Keep::WithheldOnSuccess
    } else {
        Keep::Body
    };
    finish(&idem.db, &owner, &key, response, &checked, keep).await
}

/// The answer to a replay of an operation whose first answer showed a
/// secret: 409, marked as a replay (the operation did not run again).
fn secret_shown_once() -> Response {
    let mut response = problem(StatusCode::CONFLICT, "idempotency_secret_shown_once");
    response
        .headers_mut()
        .insert(REPLAYED_HEADER, HeaderValue::from_static("true"));
    response
}

/// The `{workspace_id}` segment of `path`, a request to `operation`'s route.
fn workspace_of<'a>(operation: &OperationInfo, path: &'a str) -> Option<&'a str> {
    // Templates and paths line up segment by segment, from `/api/` on (a
    // router nested under `/bff` prefixes the path).
    let template: Vec<&str> = operation.path.split('/').collect();
    let path = path.find("/api/").map_or(path, |at| &path[at..]);
    let segments: Vec<&str> = path.split('/').collect();
    if template.len() != segments.len() {
        return None;
    }
    let wanted = format!("{{{WORKSPACE_PATH_PARAM}}}");
    template
        .iter()
        .zip(segments)
        .find_map(|(t, s)| (*t == wanted).then_some(s))
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

/// Keys are per caller: a person through one app (subject and the token's
/// client), so the web app and every app acting for the same person keep
/// separate keys (campbooks-cloud#213, item 3); clients by client id. The
/// subject is length-prefixed, so no subject and client pair can spell
/// another's.
fn owner(principal: &Principal) -> String {
    match principal.kind {
        PrincipalKind::User => format!(
            "user:{}:{}|azp:{}",
            principal.subject.len(),
            principal.subject,
            principal.client_id.as_deref().unwrap_or_default()
        ),
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
    /// The first run answered with a secret, which was not kept; the
    /// containers its handler checked.
    Withheld(Vec<RecordRef>),
    Mismatch,
    InFlight,
    /// The first request with the key stopped without an answer (its
    /// process died): it may have run, and it is never run again.
    Unknown,
}

struct Stored {
    status: i16,
    content_type: Option<String>,
    location: Option<String>,
    body: Vec<u8>,
    containers: Vec<RecordRef>,
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
    let mut tx = crate::db::begin_as_idempotency_caller(db, owner).await?;
    let claim = claim_in(&mut tx, owner, key, operation_id, hash).await?;
    tx.commit().await?;
    Ok(claim)
}

/// What [`claim_in`] reads back of a key it could not take.
#[derive(sqlx::FromRow)]
struct Existing {
    operation_id: String,
    request_hash: Vec<u8>,
    state: String,
    response_status: Option<i16>,
    response_content_type: Option<String>,
    response_location: Option<String>,
    response_body: Option<Vec<u8>>,
    containers: serde_json::Value,
    /// Pending for longer than any request runs: its process stopped.
    abandoned: bool,
}

/// [`claim`] in the caller's transaction.
///
/// A key is taken only when it is new or expired. A pending key is never
/// taken over (campbooks-cloud#63): its request may have committed its
/// effects before its process stopped and before its answer was stored, so
/// running it again could do them twice. Once it has been pending for
/// longer than any request runs ([`ABANDONED_AFTER`]) a retry learns that
/// its outcome is unknown ([`Claim::Unknown`]) until the key expires.
async fn claim_in(
    conn: &mut PgConnection,
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
        let acquired: Option<bool> = sqlx::query_scalar(
            "INSERT INTO kernel.idempotency
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
             RETURNING true",
        )
        .bind(owner)
        .bind(key)
        .bind(operation_id)
        .bind(hash)
        .bind(retention)
        .fetch_optional(&mut *conn)
        .await?;
        if acquired.is_some() {
            return Ok(Claim::Acquired);
        }

        let existing: Option<Existing> = sqlx::query_as(
            "SELECT operation_id, request_hash, state, response_status, response_content_type,
                    response_location, response_body, containers,
                    created_at < now() - make_interval(secs => $3) AS abandoned
               FROM kernel.idempotency
              WHERE principal = $1 AND key = $2 AND expires_at > now()",
        )
        .bind(owner)
        .bind(key)
        .bind(abandoned)
        .fetch_optional(&mut *conn)
        .await?;
        let Some(row) = existing else {
            continue;
        };
        if row.operation_id != operation_id || row.request_hash != hash {
            return Ok(Claim::Mismatch);
        }
        // Unreadable means "checked something": refuse the replay rather
        // than skip the check.
        let containers = || {
            serde_json::from_value(row.containers.clone())
                .unwrap_or_else(|_| vec![RecordRef::new("kernel.unreadable", "unreadable")])
        };
        return Ok(
            match (row.state.as_str(), row.response_status, &row.response_body) {
                ("completed", Some(status), Some(body)) => Claim::Replay(Stored {
                    status,
                    content_type: row.response_content_type.clone(),
                    location: row.response_location.clone(),
                    body: body.clone(),
                    containers: containers(),
                }),
                ("withheld", ..) => Claim::Withheld(containers()),
                ("pending", ..) if row.abandoned => Claim::Unknown,
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

/// What [`finish`] keeps of a settled response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Keep {
    /// The whole response, for replay.
    Body,
    /// For an answer below 400 (a success, or a redirect whose `Location`
    /// may carry the secret), only that the key was used (it showed a
    /// secret: `Operation::secret_once`); an error answer whole.
    WithheldOnSuccess,
}

/// Store the response for replay (or, for a secret, only that the key was
/// used), or release the key.
async fn finish(
    db: &PgPool,
    owner: &str,
    key: &str,
    response: Response,
    containers: &[RecordRef],
    keep: Keep,
) -> Response {
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
    let stored = if keep == Keep::WithheldOnSuccess && status.as_u16() < 400 {
        // A redirect's `Location` may be the secret itself: not kept.
        let location = header(header::LOCATION).filter(|_| status.is_success());
        withhold(db, owner, key, status, location, containers).await
    } else {
        store(db, owner, key, status, &parts, &body, containers).await
    };
    if let Err(error) = stored {
        tracing::warn!(%error, "idempotency: cannot store the response");
    }
    Response::from_parts(parts, Body::from(body))
}

/// Keep `body` (and its headers) as the key's response, in the caller's
/// transaction.
async fn store(
    db: &PgPool,
    owner: &str,
    key: &str,
    status: StatusCode,
    parts: &axum::http::response::Parts,
    body: &Bytes,
    containers: &[RecordRef],
) -> Result<(), sqlx::Error> {
    let header = |name| {
        parts
            .headers
            .get(name)
            .and_then(|v: &HeaderValue| v.to_str().ok())
            .map(ToOwned::to_owned)
    };
    let mut tx = crate::db::begin_as_idempotency_caller(db, owner).await?;
    sqlx::query!(
        r#"
        UPDATE kernel.idempotency
           SET state = 'completed', response_status = $3, response_content_type = $4,
               response_location = $5, response_body = $6, containers = $7
         WHERE principal = $1 AND key = $2 AND state = 'pending'
        "#,
        owner,
        key,
        i16::try_from(status.as_u16()).unwrap_or(i16::MAX),
        header(header::CONTENT_TYPE),
        header(header::LOCATION),
        body.as_ref(),
        serde_json::to_value(containers).unwrap_or_default(),
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await
}

/// Keep only that the key was used (its status and `Location`, an empty
/// body), in the caller's transaction: the response showed a secret, which
/// is never stored. The table refuses a `withheld` row with a body.
async fn withhold(
    db: &PgPool,
    owner: &str,
    key: &str,
    status: StatusCode,
    location: Option<String>,
    containers: &[RecordRef],
) -> Result<(), sqlx::Error> {
    let mut tx = crate::db::begin_as_idempotency_caller(db, owner).await?;
    sqlx::query(
        "UPDATE kernel.idempotency
            SET state = 'withheld', response_status = $3, response_content_type = NULL,
                response_location = $4, response_body = ''::bytea, containers = $5
          WHERE principal = $1 AND key = $2 AND state = 'pending'",
    )
    .bind(owner)
    .bind(key)
    .bind(i16::try_from(status.as_u16()).unwrap_or(i16::MAX))
    .bind(location)
    .bind(serde_json::to_value(containers).unwrap_or_default())
    .execute(&mut *tx)
    .await?;
    tx.commit().await
}

async fn release(db: &PgPool, owner: &str, key: &str) {
    if let Err(error) = release_key(db, owner, key).await {
        tracing::warn!(%error, "idempotency: cannot release the key");
    }
}

/// Delete the caller's pending `key`, in the caller's transaction.
async fn release_key(db: &PgPool, owner: &str, key: &str) -> Result<(), sqlx::Error> {
    let mut tx = crate::db::begin_as_idempotency_caller(db, owner).await?;
    sqlx::query!(
        "DELETE FROM kernel.idempotency WHERE principal = $1 AND key = $2 AND state = 'pending'",
        owner,
        key,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await
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
            // The kernel scope, one batch per transaction: it reads and
            // deletes expired keys only (row-level security), anyone's.
            let mut tx = crate::db::begin_kernel(ctx.pool()).await?;
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
            .execute(&mut *tx)
            .await?
            .rows_affected();
            tx.commit().await?;
            pruned += deleted;
            if deleted < PRUNE_BATCH.unsigned_abs() {
                break;
            }
        }
        tracing::info!(pruned, "pruned expired idempotency keys");
        Ok(())
    }
}

#[cfg(test)]
mod owner_tests {
    use super::{Principal, PrincipalKind, owner};

    fn principal(subject: &str, kind: PrincipalKind, client: Option<&str>) -> Principal {
        Principal {
            subject: subject.to_owned(),
            kind,
            client_id: client.map(ToOwned::to_owned),
            scopes: std::collections::BTreeSet::new(),
            workspace: None,
            email: None,
            email_verified: false,
            name: None,
            expires_at: None,
            issued_at: None,
            auth_time: None,
            sign_in: None,
        }
    }

    /// One person through two apps has two key spaces (campbooks-cloud#213,
    /// item 3), and no subject and client pair spells another's.
    #[test]
    fn a_persons_keys_are_per_app() {
        let web = owner(&principal(
            "ana",
            PrincipalKind::User,
            Some("campbooks-web"),
        ));
        let app = owner(&principal("ana", PrincipalKind::User, Some("mail-helper")));
        assert_ne!(web, app);
        assert_eq!(web, "user:3:ana|azp:campbooks-web");
        assert_eq!(
            owner(&principal("ana", PrincipalKind::User, None)),
            "user:3:ana|azp:"
        );
        assert_ne!(
            owner(&principal("a|azp:b", PrincipalKind::User, None)),
            owner(&principal("a", PrincipalKind::User, Some("b")))
        );
        assert_eq!(
            owner(&principal(
                "ana-agent",
                PrincipalKind::Client,
                Some("ana-agent")
            )),
            "client:ana-agent"
        );
    }
}
