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
//! Before any call to the provider, the conversations module is asked
//! again, through the kernel's outbound-requests port, whether the send
//! stands (campbooks-cloud#226): its draft is still being sent as this
//! send, from this inbox, by an author who may still send from it. The
//! document sent is the one that module stored, never one an event names,
//! and this module's row must name that same one. A send that no longer
//! stands is settled as failed (`unavailable`, or `unknown_outcome` after an
//! attempt that may have gone) without calling the provider.
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
    google::{MessageMeta, MessageRaw},
    mail::{self, Fetched},
    provider::{HTTP_TIMEOUT, ProviderError},
    zoho,
};

mod via_zoho;

/// The blob owner type of a send (its outgoing document).
pub const SEND_OWNER: &str = "connectors.send";

/// The largest message sent, in bytes once encoded (Gmail's limit).
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
    provider_server: Option<String>,
    mailbox_key: Option<String>,
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
        target: Target,
        give_up: bool,
    ) -> Result<(), JobError> {
        if give_up && !target.unclear {
            // No attempt may have gone out: nothing was sent.
            return finish_failed(ctx, workspace, send, &target, "unavailable").await;
        }
        let provider = target
            .provider
            .as_deref()
            .map(Provider::parse)
            .transpose()?;
        let configured = match provider {
            Some(Provider::Google) => self.deps.google.is_some(),
            Some(Provider::Zoho) => self.deps.zoho.is_some(),
            None => false,
        };
        let (Some(stream), Some(connection), Some(provider), true) =
            (target.stream, target.connection, provider, configured)
        else {
            return finish_failed(ctx, workspace, send, &target, "not_connected").await;
        };
        if target.health.as_deref() == Some(Health::NeedsReauth.as_str())
            || target.paused == Some(true)
        {
            return finish_failed(ctx, workspace, send, &target, "needs_reauth").await;
        }
        let now = self.deps.clock.now();
        // A runner that lost its lease may still be in its call to the
        // provider: never call again before that call has surely ended.
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

        // The document recorded here came from that module too
        // (`queue_send`): one that differs was not written by it. A send
        // that does not stand is never sent; an unclear one is still looked
        // up (a read), so one that went out is recorded as sent.
        let stands = standing_document(ctx, workspace, send, target.inbox)
            .await?
            .is_some_and(|stored| stored == target.document);
        if !stands && !target.unclear {
            return not_standing(ctx, workspace, send, &target).await;
        }
        let Some(token) = self
            .token(ctx, workspace, send, &target, connection)
            .await?
        else {
            return Ok(());
        };
        // The recorded document: the port's whenever the send stands.
        let document = read_document(ctx, workspace, &target.document).await?;
        let attempt = Attempt {
            workspace,
            send,
            stream,
            token: &token,
            document: &document,
            give_up,
            stands,
            now,
        };
        match provider {
            Provider::Google => self.attempt_gmail(ctx, target, attempt).await,
            Provider::Zoho => self.attempt_zoho(ctx, target, attempt).await,
        }
    }

    /// An attempt through Gmail.
    async fn attempt_gmail(
        &self,
        ctx: &mut JobContext,
        mut target: Target,
        attempt: Attempt<'_>,
    ) -> Result<(), JobError> {
        let Attempt {
            workspace,
            send,
            stream,
            token,
            document,
            give_up,
            stands,
            now,
        } = attempt;
        let Some(google) = self.deps.google.clone() else {
            return finish_failed(ctx, workspace, send, &target, "not_connected").await;
        };
        let Some(raw) = message_bytes(ctx, workspace, document).await? else {
            return finish_failed(ctx, workspace, send, &target, "too_large").await;
        };
        let gmail = google.gmail(token);

        let rate = self.deps.config.google.units_per_second;
        if let Err(wait) =
            schedule::take_budget(ctx.pool(), Provider::Google, UNITS_PER_SEND, rate).await?
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
                    let sent = gmail_sent(&meta, raw, now);
                    return self
                        .finish_sent(ctx, workspace, send, &target, stream, sent, now)
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
        if give_up || !stands {
            return unsent(ctx, workspace, send, &target, stands).await;
        }

        // From here until Gmail answers, the message may be out.
        record_attempt(ctx.pool(), workspace, send, target.attempted_at, now).await?;
        target.unclear = true;
        match gmail.send(&raw, document.thread_key.as_deref()).await {
            Ok(answer) => {
                let meta = MessageMeta {
                    id: answer.id,
                    thread_id: answer.thread_id,
                    label_ids: answer.label_ids,
                    internal_date: None,
                };
                let sent = gmail_sent(&meta, raw, now);
                self.finish_sent(ctx, workspace, send, &target, stream, sent, now)
                    .await
            }
            Err(ProviderError::Unavailable(reason)) => {
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

/// What one attempt works with.
#[derive(Debug, Clone, Copy)]
struct Attempt<'a> {
    workspace: WorkspaceId,
    send: SendId,
    stream: StreamId,
    token: &'a str,
    document: &'a OutgoingMessage,
    give_up: bool,
    /// The conversations module still confirms the send: without it, an
    /// unclear attempt is only looked up, never sent again.
    stands: bool,
    now: DateTime<Utc>,
}

/// A message the provider accepted, as the send records it: its id, and
/// the message to index and announce when it could be had.
#[derive(Debug)]
struct Sent {
    id: Option<String>,
    thread_key: Option<String>,
    fetched: Option<Fetched>,
}

/// A message Gmail has (`meta`), sent as `raw`.
fn gmail_sent(meta: &MessageMeta, raw: Vec<u8>, now: DateTime<Utc>) -> Sent {
    let fetched_bytes = u64::try_from(raw.len()).unwrap_or(u64::MAX);
    let meta = MessageMeta {
        internal_date: meta
            .internal_date
            .clone()
            .or_else(|| Some(now.timestamp_millis().to_string())),
        ..meta.clone()
    };
    Sent {
        id: Some(meta.id.clone()),
        thread_key: Some(meta.thread_id.clone()),
        fetched: Some(gmail::fetched(
            &meta.id.clone(),
            MessageRaw {
                meta,
                raw,
                fetched_bytes,
            },
            &HashMap::new(),
            now,
        )),
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
        err: &ProviderError,
    ) -> Result<(), JobError> {
        match err {
            ProviderError::RateLimited { retry_after } => Err(JobError::retry_after(
                "rate limited by the provider",
                retry_after.unwrap_or(Duration::from_secs(30)),
            )),
            ProviderError::Unauthorized => {
                if let Some(connection) = target.connection {
                    let mut tx = ctx.pool().begin_for(workspace).await?;
                    credentials::forget_access_token(&mut tx, connection).await?;
                    tx.commit().await?;
                }
                Err(JobError::retry("the access token was refused"))
            }
            ProviderError::InvalidGrant => {
                finish_failed(ctx, workspace, send, target, "needs_reauth").await
            }
            ProviderError::Unavailable(_)
            | ProviderError::NotFound
            | ProviderError::Unexpected(_) => {
                tracing::warn!(%workspace, %send, error = %err, "the provider refused the message");
                finish_failed(ctx, workspace, send, target, "rejected").await
            }
        }
    }

    /// The provider has the message: index and announce it (when it could
    /// be had), mark the send sent.
    #[allow(
        clippy::too_many_arguments,
        reason = "three call sites; the values are distinct"
    )]
    async fn finish_sent(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        send: SendId,
        target: &Target,
        stream: StreamId,
        accepted: Sent,
        now: DateTime<Utc>,
    ) -> Result<(), JobError> {
        let state = ctx.state().clone();
        let ws = workspace.to_string();
        let (token, generation) = cursor_of(target)?;
        // Blobs are stored before the transaction that references them.
        let stored = match accepted.fetched {
            Some(fetched) => {
                let (message, op) = mail::store_message(&state.blobs, &ws, fetched, generation)
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
                Some((op, state.blobs.put(&ws, bytes, BATCH_CONTENT_TYPE).await?))
            }
            None => None,
        };

        let tx = ctx.tx().await?;
        // Settled already (another runner, or the check): announce it once.
        let first = sqlx::query_scalar!(
            r#"UPDATE connectors.sends
                  SET state = 'sent', provider_message_id = $2, thread_key = $3, updated_at = now()
                WHERE id = $1 AND state <> 'sent'
            RETURNING id"#,
            send as SendId,
            accepted.id,
            accepted.thread_key,
        )
        .fetch_optional(&mut **tx)
        .await?;
        if first.is_none() {
            return Ok(());
        }
        if let Some((op, batch)) = stored {
            emit::apply_index(tx, &state, workspace, stream, vec![op]).await?;
            emit::store_batch(
                tx,
                &state,
                workspace,
                stream,
                target.inbox,
                &batch,
                &token,
                1,
                now,
            )
            .await?;
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
                    inbox_id: target.inbox,
                    send_id: send,
                    outcome: SendOutcome::Sent,
                    provider_message_id: accepted.id,
                    reason: None,
                },
            )
            .await?;
        // Pick up the provider's own view of it soon.
        schedule::set_due(tx, workspace, stream, now).await?;
        tracing::info!(%workspace, %send, "message sent");
        Ok(())
    }
}

/// The stream's cursor as an event carries it, and its pass generation.
fn cursor_of(target: &Target) -> Result<(String, i32), JobError> {
    let unreadable =
        |e: serde_json::Error| JobError::fatal(format!("a stream has an unreadable cursor: {e}"));
    let Some(cursor) = target.cursor.clone() else {
        return Ok((String::new(), 0));
    };
    if target.provider.as_deref() == Some(Provider::Zoho.as_str()) {
        let cursor: zoho::read::Cursor = serde_json::from_value(cursor).map_err(unreadable)?;
        return Ok((cursor.token(), cursor.generation));
    }
    let cursor: gmail::Cursor = serde_json::from_value(cursor).map_err(unreadable)?;
    Ok((cursor.token(), cursor.generation))
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
                  st.mailbox_key AS "mailbox_key?",
                  c.id AS "connection?: ConnectionId", c.provider AS "provider?",
                  c.provider_server,
                  c.granted_by AS "granted_by?: UserId", c.health AS "health?"
             FROM connectors.sends s
             LEFT JOIN connectors.streams st ON st.id = s.stream_id AND st.inbox_id = s.inbox_id
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
        provider_server: row.provider_server,
        mailbox_key: row.mailbox_key,
        granted_by: row.granted_by,
        health: row.health,
        paused: row.paused,
        cursor: row.cursor,
    }))
}

/// The document of send `send` as the conversations module stored it,
/// while the send stands and is sent from `inbox` (the outbound-requests
/// port, in a short transaction of its own); `None` otherwise.
async fn standing_document(
    ctx: &JobContext,
    workspace: WorkspaceId,
    send: SendId,
    inbox: InboxId,
) -> Result<Option<BlobRef>, JobError> {
    let state = ctx.state().clone();
    let mut tx = ctx.pool().begin_for(workspace).await?;
    let stored = state
        .outbound
        .stored_send(&state, &mut tx, workspace, send.into())
        .await?;
    tx.commit().await?;
    Ok(stored
        .filter(|stored| stored.inbox == uuid::Uuid::from(inbox))
        .map(|stored| stored.message))
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

/// Claim an attempt before the call, in a transaction of its own, so a
/// later run knows the provider may have the message. The claim holds only
/// while the send is pending and no other runner recorded an attempt since
/// `previous` was read: of two jobs of one send, one calls the provider and
/// the other waits and finds the outcome.
async fn record_attempt(
    pool: &sqlx::PgPool,
    workspace: WorkspaceId,
    send: SendId,
    previous: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> Result<(), JobError> {
    let mut tx = pool.begin_for(workspace).await?;
    let claimed = sqlx::query_scalar!(
        r#"UPDATE connectors.sends
              SET attempts = attempts + 1, attempted_at = $2, unclear = true, updated_at = now()
            WHERE id = $1 AND state = 'pending' AND attempted_at IS NOT DISTINCT FROM $3
        RETURNING id"#,
        send as SendId,
        now,
        previous,
    )
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    if claimed.is_none() {
        return Err(JobError::retry_after(
            "another attempt at this send was claimed first",
            UNCLEAR_WAIT,
        ));
    }
    Ok(())
}

/// Settle a send proven unsent without calling the provider: given up
/// (`unavailable`), or no longer standing.
async fn unsent(
    ctx: &mut JobContext,
    workspace: WorkspaceId,
    send: SendId,
    target: &Target,
    stands: bool,
) -> Result<(), JobError> {
    if stands {
        return finish_failed(ctx, workspace, send, target, "unavailable").await;
    }
    not_standing(ctx, workspace, send, target).await
}

/// The send no longer stands (the conversations module does not confirm
/// it): settle it without calling the provider.
async fn not_standing(
    ctx: &mut JobContext,
    workspace: WorkspaceId,
    send: SendId,
    target: &Target,
) -> Result<(), JobError> {
    tracing::warn!(
        %workspace,
        %send,
        "a recorded send no longer stands; not sending"
    );
    finish_failed(ctx, workspace, send, target, "unavailable").await
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
