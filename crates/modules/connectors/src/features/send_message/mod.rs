//! `connectors.send_message` — send one recorded send through Gmail, at
//! most once.
//!
//! Gmail has no idempotency key for `messages.send`, so the job makes its
//! own: every call is preceded by an attempt committed on its own, and an
//! attempt after one that may have reached Gmail first looks the message
//! up by its `Message-ID` (which the job sets, and Gmail keeps). An unclear
//! failure (a 5xx, a timeout, a worker that died mid-call) waits at least
//! [`UNCLEAR_WAIT`] before that lookup, so Gmail's search has caught up. A
//! definite refusal fails the send at once.
//!
//! On success, in the job's transaction: the sent message is indexed and
//! announced in a change batch exactly as sync would (so it shows in its
//! conversation now, and the next sync knows it), the send is marked sent,
//! `connectors.send_completed` is published, and the stream is due at once.

use std::{collections::HashMap, time::Duration};

use campbooks_kernel::{
    JobContext, JobError, JobHandler,
    blobs::{BlobOwner, BlobRef},
    db::BeginFor as _,
    tenancy::{UserId, WorkspaceId},
};
use chrono::{DateTime, TimeDelta, Utc};
use futures_util::StreamExt as _;
use serde::{Deserialize, Serialize};

use crate::{
    contract::{
        BATCH_CONTENT_TYPE, MessageChange, MessageChangeBatch, OUTGOING_CONTENT_TYPE,
        OutgoingMessage,
    },
    deps::Deps,
    domain::{
        credentials::{self, TokenError},
        emit, health,
        model::{ConnectionId, Health, InboxId, Provider, SendId, StreamId},
        schedule,
    },
    events::{SendCompleted, SendOutcome},
    gmail::{self, compose},
    google::{GoogleError, HTTP_TIMEOUT, MessageMeta, MessageRaw},
};

/// The blob owner type of a send (its outgoing document).
pub const SEND_OWNER: &str = "connectors.send";

/// The largest message Gmail sends, in bytes once encoded.
pub const MAX_MESSAGE_BYTES: usize = 35 * 1024 * 1024;

/// How long an unclear attempt waits before the next one looks it up.
pub const UNCLEAR_WAIT: Duration = Duration::from_secs(30);

/// When the check behind a send settles it, if no job for it is alive by
/// then.
pub const GIVE_UP_AFTER: TimeDelta = TimeDelta::minutes(30);

/// Gmail quota units of `messages.send` plus a lookup.
const UNITS_PER_SEND: u32 = 100 + 10;

/// The job's payload: the send's id only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SendMessagePayload {
    /// The send.
    pub send: SendId,
    /// Settle the send rather than send it: the check enqueued behind it,
    /// which runs only once no earlier job for the send is alive.
    #[serde(default)]
    pub give_up: bool,
}

/// Sends messages through the provider.
#[derive(Debug, Clone)]
pub struct SendMessage {
    deps: Deps,
}

impl SendMessage {
    /// The job handler, composed with the module's dependencies.
    pub fn new(deps: Deps) -> Self {
        Self { deps }
    }
}

/// A send and the stream it goes through.
#[derive(Debug)]
struct Target {
    inbox: InboxId,
    document: BlobRef,
    state: String,
    unclear: bool,
    attempted_at: Option<DateTime<Utc>>,
    stream: Option<StreamId>,
    connection: Option<ConnectionId>,
    provider: Option<String>,
    granted_by: Option<UserId>,
    health: Option<String>,
    paused: Option<bool>,
    cursor: Option<serde_json::Value>,
}

impl JobHandler for SendMessage {
    type Payload = SendMessagePayload;
    const NAME: &'static str = "connectors.send_message";
    const QUEUE: &'static str = "sync";
    const MAX_ATTEMPTS: u32 = 8;

    async fn run(&self, ctx: &mut JobContext, payload: SendMessagePayload) -> Result<(), JobError> {
        let workspace: WorkspaceId = ctx
            .workspace()
            .and_then(|w| w.parse().ok())
            .ok_or_else(|| JobError::fatal("send enqueued outside a workspace"))?;
        let send = payload.send;
        let Some(target) = load(ctx, workspace, send).await? else {
            return Ok(());
        };
        if target.state != "pending" {
            return Ok(());
        }
        match self
            .attempt(ctx, workspace, send, target, payload.give_up)
            .await
        {
            Ok(()) => Ok(()),
            // Out of attempts, or stuck for good: settle the send rather
            // than leave its draft "sending".
            Err(err) if ctx.is_last_attempt() || matches!(err, JobError::Fatal(_)) => {
                tracing::warn!(%workspace, %send, error = %err, "giving up on a send");
                settle(ctx, workspace, send).await
            }
            Err(err) => Err(err),
        }
    }
}

impl SendMessage {
    /// One attempt at send `send`. Errors are retried by the queue; the
    /// caller settles the send when no attempt is left.
    async fn attempt(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        send: SendId,
        mut target: Target,
        give_up: bool,
    ) -> Result<(), JobError> {
        if give_up && !target.unclear {
            // No attempt may have gone out: nothing was sent.
            return finish_failed(ctx, workspace, send, &target, "unavailable").await;
        }
        let (Some(stream), Some(connection), Some(google)) =
            (target.stream, target.connection, self.deps.google.clone())
        else {
            return finish_failed(ctx, workspace, send, &target, "not_connected").await;
        };
        if target.health.as_deref() == Some(Health::NeedsReauth.as_str())
            || target.paused == Some(true)
        {
            return finish_failed(ctx, workspace, send, &target, "needs_reauth").await;
        }
        let now = self.deps.clock.now();
        // A runner that lost its lease may still be in its call to Gmail:
        // never call again before that call has surely ended.
        if let Some(at) = target.attempted_at {
            let quiet = TimeDelta::from_std(HTTP_TIMEOUT + UNCLEAR_WAIT).unwrap_or(TimeDelta::MAX);
            let wait = at + quiet - now;
            if wait > TimeDelta::zero() {
                return Err(JobError::retry_after(
                    "waiting for an earlier attempt to end",
                    wait.to_std().unwrap_or(UNCLEAR_WAIT),
                ));
            }
        }

        let Some(token) = self
            .token(ctx, workspace, send, &target, connection)
            .await?
        else {
            return Ok(());
        };
        let document = read_document(ctx, workspace, &target.document).await?;
        let Some(raw) = message_bytes(ctx, workspace, &document).await? else {
            return finish_failed(ctx, workspace, send, &target, "too_large").await;
        };
        let gmail = google.gmail(&token);

        let provider = Provider::parse(target.provider.as_deref().unwrap_or("google"))?;
        let rate = self.deps.config.google.units_per_second;
        if let Err(wait) = schedule::take_budget(ctx.pool(), provider, UNITS_PER_SEND, rate).await?
        {
            return Err(JobError::retry_after(
                "waiting for the provider budget",
                wait,
            ));
        }

        // An earlier attempt may have reached Gmail: look before sending.
        if target.unclear {
            match gmail.find_by_message_id(&document.rfc_message_id).await {
                Ok(Some(meta)) => {
                    tracing::info!(%workspace, %send, "an earlier attempt was sent");
                    let accepted = Accepted::from_meta(meta);
                    return self
                        .finish_sent(ctx, workspace, send, &target, stream, accepted, raw, now)
                        .await;
                }
                Ok(None) => {
                    // Proven not sent.
                    set_unclear(ctx, workspace, send, false).await?;
                    target.unclear = false;
                }
                Err(err) => {
                    return Err(JobError::retry_after(
                        format!("looking up an earlier attempt: {err}"),
                        UNCLEAR_WAIT,
                    ));
                }
            }
        }
        if give_up {
            return finish_failed(ctx, workspace, send, &target, "unavailable").await;
        }

        // From here until Gmail answers, the message may be out.
        record_attempt(ctx, workspace, send, now).await?;
        target.unclear = true;
        match gmail.send(&raw, document.thread_key.as_deref()).await {
            Ok(answer) => {
                let accepted = Accepted {
                    id: answer.id,
                    thread_id: answer.thread_id,
                    label_ids: answer.label_ids,
                    internal_date: None,
                };
                self.finish_sent(ctx, workspace, send, &target, stream, accepted, raw, now)
                    .await
            }
            Err(GoogleError::Unavailable(reason)) => {
                // Gmail may have it: look it up before trying again.
                tracing::warn!(%workspace, %send, %reason, "send ended unclearly");
                Err(JobError::retry_after(
                    "the send ended unclearly; looking it up next time",
                    UNCLEAR_WAIT,
                ))
            }
            Err(err) => {
                // A definite answer: this attempt did not send it.
                set_unclear(ctx, workspace, send, false).await?;
                target.unclear = false;
                self.refused(ctx, workspace, send, &target, &err).await
            }
        }
    }
}

/// A message Gmail has, as the send needs it.
struct Accepted {
    id: String,
    thread_id: String,
    label_ids: Vec<String>,
    internal_date: Option<String>,
}

impl Accepted {
    fn from_meta(meta: MessageMeta) -> Self {
        Self {
            id: meta.id,
            thread_id: meta.thread_id,
            label_ids: meta.label_ids,
            internal_date: meta.internal_date,
        }
    }
}

impl SendMessage {
    /// A valid access token, or `None` once the send was settled without
    /// one (the grant is gone, or the provider could not be reached for the
    /// last time).
    async fn token(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        send: SendId,
        target: &Target,
        connection: ConnectionId,
    ) -> Result<Option<String>, JobError> {
        match credentials::access_token(ctx.pool(), &self.deps, workspace, connection).await {
            Ok(token) => Ok(Some(token)),
            Err(TokenError::NeedsReauth(reason)) => {
                let events = ctx.state().events.clone();
                if let Some(granted_by) = target.granted_by {
                    health::needs_reauth(
                        ctx.tx().await?,
                        &events,
                        &workspace.to_string(),
                        connection,
                        granted_by,
                        reason,
                        self.deps.clock.now(),
                    )
                    .await?;
                }
                finish_failed(ctx, workspace, send, target, "needs_reauth").await?;
                Ok(None)
            }
            Err(TokenError::Provider(err)) => {
                Err(JobError::retry(format!("no access token: {err}")))
            }
            Err(TokenError::Database(err)) => Err(err.into()),
        }
    }

    /// Gmail answered the send with a definite refusal: try again later
    /// (a refused token, a rate limit) or fail the send for good.
    async fn refused(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        send: SendId,
        target: &Target,
        err: &GoogleError,
    ) -> Result<(), JobError> {
        match err {
            GoogleError::RateLimited { retry_after } => Err(JobError::retry_after(
                "rate limited by the provider",
                retry_after.unwrap_or(Duration::from_secs(30)),
            )),
            GoogleError::Unauthorized => {
                if let Some(connection) = target.connection {
                    let mut tx = ctx.pool().begin_for(workspace).await?;
                    credentials::forget_access_token(&mut tx, connection).await?;
                    tx.commit().await?;
                }
                Err(JobError::retry("the access token was refused"))
            }
            GoogleError::InvalidGrant => {
                finish_failed(ctx, workspace, send, target, "needs_reauth").await
            }
            GoogleError::Unavailable(_) | GoogleError::NotFound | GoogleError::Unexpected(_) => {
                tracing::warn!(%workspace, %send, error = %err, "the provider refused the message");
                finish_failed(ctx, workspace, send, target, "rejected").await
            }
        }
    }

    /// Gmail has the message: index it, announce it, mark the send sent.
    #[allow(
        clippy::too_many_arguments,
        reason = "two call sites; the values are distinct"
    )]
    async fn finish_sent(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        send: SendId,
        target: &Target,
        stream: StreamId,
        accepted: Accepted,
        raw: Vec<u8>,
        now: DateTime<Utc>,
    ) -> Result<(), JobError> {
        let state = ctx.state().clone();
        let ws = workspace.to_string();
        let cursor: gmail::Cursor = target
            .cursor
            .clone()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|e| JobError::fatal(format!("stream {stream} has an unreadable cursor: {e}")))?
            .unwrap_or_default();
        let meta = MessageMeta {
            id: accepted.id.clone(),
            thread_id: accepted.thread_id.clone(),
            label_ids: accepted.label_ids,
            internal_date: accepted
                .internal_date
                .or_else(|| Some(now.timestamp_millis().to_string())),
        };
        let fetched_bytes = u64::try_from(raw.len()).unwrap_or(u64::MAX);
        // Blobs are stored before the transaction that references them.
        let (message, op) = gmail::store_message(
            &state.blobs,
            &ws,
            now,
            &accepted.id,
            MessageRaw {
                meta,
                raw,
                fetched_bytes,
            },
            &HashMap::new(),
            cursor.generation,
        )
        .await
        .map_err(|e| JobError::retry(e.to_string()))?;
        let batch = MessageChangeBatch::new(
            target.inbox,
            vec![MessageChange::Upsert {
                message: Box::new(message),
            }],
        );
        let bytes = serde_json::to_vec(&batch)
            .map_err(|e| JobError::fatal(format!("encoding a batch: {e}")))?;
        let batch = state.blobs.put(&ws, bytes, BATCH_CONTENT_TYPE).await?;

        let tx = ctx.tx().await?;
        // Settled already (another runner, or the check): announce it once.
        let first = sqlx::query_scalar!(
            r#"UPDATE connectors.sends
                  SET state = 'sent', provider_message_id = $2, thread_key = $3, updated_at = now()
                WHERE id = $1 AND state <> 'sent'
            RETURNING id"#,
            send as SendId,
            accepted.id,
            accepted.thread_id,
        )
        .fetch_optional(&mut **tx)
        .await?;
        if first.is_none() {
            return Ok(());
        }
        emit::apply_index(tx, &state, workspace, stream, vec![op]).await?;
        emit::store_batch(
            tx,
            &state,
            workspace,
            stream,
            target.inbox,
            &batch,
            &cursor.token(),
            1,
            now,
        )
        .await?;
        state
            .blobs
            .detach_owner(tx, &ws, &BlobOwner::new(SEND_OWNER, send))
            .await?;
        state
            .events
            .publish(
                tx,
                &SendCompleted {
                    workspace: ws,
                    inbox_id: target.inbox,
                    send_id: send,
                    outcome: SendOutcome::Sent,
                    provider_message_id: Some(accepted.id),
                    reason: None,
                },
            )
            .await?;
        // Pick up Gmail's own view of it (labels, thread) soon.
        schedule::set_due(tx, workspace, stream, now).await?;
        tracing::info!(%workspace, %send, "message sent");
        Ok(())
    }
}

/// The send and its stream, read in a short transaction of its own.
async fn load(
    ctx: &JobContext,
    workspace: WorkspaceId,
    send: SendId,
) -> Result<Option<Target>, JobError> {
    let mut tx = ctx.pool().begin_for(workspace).await?;
    let row = sqlx::query!(
        r#"SELECT s.inbox_id AS "inbox: InboxId", s.document, s.state, s.unclear, s.attempted_at,
                  st.id AS "stream?: StreamId", st.paused AS "paused?", st.cursor AS "cursor?",
                  c.id AS "connection?: ConnectionId", c.provider AS "provider?",
                  c.granted_by AS "granted_by?: UserId", c.health AS "health?"
             FROM connectors.sends s
             LEFT JOIN connectors.streams st ON st.id = s.stream_id
             LEFT JOIN connectors.connections c ON c.id = st.connection_id
            WHERE s.id = $1"#,
        send as SendId,
    )
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let document: BlobRef = serde_json::from_value(row.document)
        .map_err(|e| JobError::fatal(format!("send {send} has an unreadable document: {e}")))?;
    Ok(Some(Target {
        inbox: row.inbox,
        document,
        state: row.state,
        unclear: row.unclear,
        attempted_at: row.attempted_at,
        stream: row.stream,
        connection: row.connection,
        provider: row.provider,
        granted_by: row.granted_by,
        health: row.health,
        paused: row.paused,
        cursor: row.cursor,
    }))
}

/// The bytes of `blob`, at most `max` (`None` when larger).
async fn read_blob(
    ctx: &JobContext,
    workspace: WorkspaceId,
    blob: &BlobRef,
    max: usize,
) -> Result<Option<Vec<u8>>, JobError> {
    if usize::try_from(blob.size).unwrap_or(usize::MAX) > max {
        return Ok(None);
    }
    let mut stream = ctx.state().blobs.get(&workspace.to_string(), blob).await?;
    let mut bytes = Vec::with_capacity(usize::try_from(blob.size).unwrap_or(0));
    while let Some(chunk) = stream.next().await {
        bytes.extend_from_slice(&chunk?);
        if bytes.len() > max {
            return Ok(None);
        }
    }
    Ok(Some(bytes))
}

/// The outgoing message document.
async fn read_document(
    ctx: &JobContext,
    workspace: WorkspaceId,
    blob: &BlobRef,
) -> Result<OutgoingMessage, JobError> {
    let bytes = read_blob(ctx, workspace, blob, MAX_MESSAGE_BYTES)
        .await?
        .ok_or_else(|| JobError::fatal("the outgoing message document is too large"))?;
    let document: OutgoingMessage = serde_json::from_slice(&bytes).map_err(|e| {
        JobError::fatal(format!(
            "the outgoing message ({OUTGOING_CONTENT_TYPE}) does not parse at line {} column {}",
            e.line(),
            e.column()
        ))
    })?;
    if document.version != 1 {
        return Err(JobError::fatal(format!(
            "outgoing message version {} is not supported",
            document.version
        )));
    }
    Ok(document)
}

/// The RFC 5322 message, or `None` when it would be too large to send.
async fn message_bytes(
    ctx: &JobContext,
    workspace: WorkspaceId,
    document: &OutgoingMessage,
) -> Result<Option<Vec<u8>>, JobError> {
    let mut attachments = Vec::with_capacity(document.attachments.len());
    let mut total = document.text.len() + document.html.as_ref().map_or(0, String::len);
    for attachment in &document.attachments {
        let Some(bytes) = read_blob(ctx, workspace, &attachment.blob, MAX_MESSAGE_BYTES).await?
        else {
            return Ok(None);
        };
        total += bytes.len();
        attachments.push(bytes);
    }
    // Base64 grows attachments by a third.
    if total.saturating_mul(4) / 3 > MAX_MESSAGE_BYTES {
        return Ok(None);
    }
    let raw = compose::rfc5322(document, attachments);
    Ok((raw.len() <= MAX_MESSAGE_BYTES).then_some(raw))
}

/// Record an attempt before the call, in a transaction of its own, so a
/// later run knows Gmail may have the message.
async fn record_attempt(
    ctx: &JobContext,
    workspace: WorkspaceId,
    send: SendId,
    now: DateTime<Utc>,
) -> Result<(), JobError> {
    let mut tx = ctx.pool().begin_for(workspace).await?;
    sqlx::query!(
        r#"UPDATE connectors.sends
              SET attempts = attempts + 1, attempted_at = $2, unclear = true, updated_at = now()
            WHERE id = $1"#,
        send as SendId,
        now,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Record whether an attempt may have reached Gmail without an answer.
async fn set_unclear(
    ctx: &JobContext,
    workspace: WorkspaceId,
    send: SendId,
    unclear: bool,
) -> Result<(), JobError> {
    let mut tx = ctx.pool().begin_for(workspace).await?;
    sqlx::query!(
        "UPDATE connectors.sends SET unclear = $2, updated_at = now() WHERE id = $1",
        send as SendId,
        unclear,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

/// The send failed for good: say so to the conversations module. After an
/// attempt that may have reached Gmail (and no lookup proving otherwise),
/// whatever the reason, it may have gone: `unknown_outcome`.
async fn finish_failed(
    ctx: &mut JobContext,
    workspace: WorkspaceId,
    send: SendId,
    target: &Target,
    reason: &'static str,
) -> Result<(), JobError> {
    let reason = if target.unclear {
        "unknown_outcome"
    } else {
        reason
    };
    let state = ctx.state().clone();
    let tx = ctx.tx().await?;
    fail(tx, &state, workspace, send, target.inbox, reason).await
}

/// Mark a pending send failed with `reason`, release its document and
/// announce it, in `tx`. Does nothing to a send already settled.
async fn fail(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    state: &campbooks_kernel::AppState,
    workspace: WorkspaceId,
    send: SendId,
    inbox: InboxId,
    reason: &str,
) -> Result<(), JobError> {
    let ws = workspace.to_string();
    let failed = sqlx::query_scalar!(
        r#"UPDATE connectors.sends SET state = 'failed', failure_reason = $2, updated_at = now()
            WHERE id = $1 AND state = 'pending'
        RETURNING id"#,
        send as SendId,
        reason,
    )
    .fetch_optional(&mut **tx)
    .await?;
    if failed.is_none() {
        return Ok(());
    }
    state
        .blobs
        .detach_owner(tx, &ws, &BlobOwner::new(SEND_OWNER, send))
        .await?;
    state
        .events
        .publish(
            tx,
            &SendCompleted {
                workspace: ws,
                inbox_id: inbox,
                send_id: send,
                outcome: SendOutcome::Failed,
                provider_message_id: None,
                reason: Some(reason.to_owned()),
            },
        )
        .await?;
    tracing::info!(%workspace, %send, reason, "send failed");
    Ok(())
}

/// Settle a send no attempt is left for, in a transaction of its own (the
/// job's may have failed): `unknown_outcome` when an attempt may have
/// reached Gmail, else `unavailable` (nothing was sent; the draft is
/// editable again).
async fn settle(ctx: &JobContext, workspace: WorkspaceId, send: SendId) -> Result<(), JobError> {
    let state = ctx.state().clone();
    let mut tx = ctx.pool().begin_for(workspace).await?;
    let Some(row) = sqlx::query!(
        r#"SELECT inbox_id AS "inbox: InboxId", unclear FROM connectors.sends
            WHERE id = $1 AND state = 'pending' FOR UPDATE"#,
        send as SendId,
    )
    .fetch_optional(&mut *tx)
    .await?
    else {
        return Ok(());
    };
    let reason = if row.unclear {
        "unknown_outcome"
    } else {
        "unavailable"
    };
    fail(&mut tx, &state, workspace, send, row.inbox, reason).await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests;
