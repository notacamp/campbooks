//! `conversations.read_structured`: the dates a sender structured for
//! machines (ADR 0020, campbooks-cloud#42), read with no model at all.
//!
//! After the decide pass has answered a new inbound message (the pass queues
//! this job, so its answer about phishing is on record), this job reads the
//! message's `text/calendar` parts with an iCalendar parser and its HTML for
//! schema.org markup (JSON-LD and microdata): an invitation becomes a
//! `meeting` or an `appointment`, a flight, train, bus or rental car
//! reservation `travel`, a lodging reservation `stay`, an event reservation
//! `show`, a restaurant booking `reservation`, a parcel `delivery`, and an
//! invoice with a payment due date an ask to `pay`
//! ([`crate::domain::structured`]). Only the typed fields are read, never the
//! prose around them.
//!
//! **Only for a sender whose domain passed DMARC aligned**, as the facts read
//! the receiving provider's `Authentication-Results`
//! (`campbooks_ai::decide::signals::facts::sender_authenticated`), and never
//! for mail the decide pass called suspicious, because markup is trivially
//! forged. Otherwise the message goes on as if it had none. Only mail sent
//! within the decide window is read, so importing an old mailbox costs
//! nothing; an HTML part is parsed only when the sanitiser's own work budget
//! allows.
//!
//! What it finds is stored as the message's read of layer `structured`
//! (`conversations.message_reads`) and published as
//! `conversations.message_read`; an event that cancels or updates one read
//! from an earlier message of the conversation (same `UID`) says so. A
//! message these items cover is not read by the model for dates
//! (`conversations.read_message`), only for its need's summary. Job payloads
//! hold the message's id; failures are logged by kind.

use campbooks_ai::decide::signals::facts::sender_authenticated;
use campbooks_kernel::{
    AppState, EventBus, JobContext, JobError, JobHandler,
    blobs::BlobRef,
    db::BeginFor as _,
    jobs::{EnqueueOptions, Enqueued},
    tenancy::WorkspaceId,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{Postgres, Transaction};

use crate::{
    domain::{
        blobs::read_text,
        needs::{self, NeedSettings},
        read::ReadItem,
        redact,
        structured::{self, Known},
    },
    features::read_message::{ask_facts, item_facts, store},
    ingest::Headers,
    model::{Address, ConversationId, InboxId, MessageId, ReadLayer},
};

/// A row of [`LOAD`].
#[derive(sqlx::FromRow)]
struct Loaded {
    sender: Value,
    headers: Value,
    bodies: Value,
    conversation_id: ConversationId,
    inbox_id: InboxId,
    sent_at: DateTime<Utc>,
    /// It has a structured read already.
    read: bool,
    /// The decide pass accepted that it is suspicious.
    suspicious: bool,
}

/// The message, if it can be read: inbound, in the inbox, not in spam or the
/// trash, with what is stored for it.
const LOAD: &str = "SELECT m.sender, m.headers, m.bodies, m.conversation_id, m.inbox_id, m.sent_at, \
            EXISTS (SELECT 1 FROM conversations.message_reads r \
                     WHERE r.message_id = m.id AND r.layer = 'structured') AS read, \
            EXISTS (SELECT 1 FROM conversations.decisions d \
                     WHERE d.message_id = m.id AND d.question = 'suspicious' AND d.acted \
                       AND d.value = 'true'::jsonb) AS suspicious \
       FROM conversations.messages m \
       JOIN conversations.inboxes i ON i.id = m.inbox_id AND i.detached_at IS NULL \
      WHERE m.id = $1 AND m.direction = 'inbound' AND NOT m.in_spam AND NOT m.in_trash";

/// The calendar parts of a message: the media types a calendar travels as.
const CALENDARS: &str = "SELECT blob FROM conversations.attachments \
      WHERE message_id = $1 \
        AND (lower(content_type) LIKE 'text/calendar%' \
             OR lower(content_type) LIKE 'application/ics%') \
      ORDER BY position LIMIT 3";

/// The message to read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadStructuredPayload {
    /// The inbound message to read.
    pub message: MessageId,
}

/// Reads the dates a sender structured for machines.
pub struct ReadStructured {
    needs: NeedSettings,
}

impl ReadStructured {
    /// The structured read, working a conversation's need out again under
    /// `needs` once it stores something.
    pub fn new(needs: NeedSettings) -> Self {
        Self { needs }
    }
}

/// `messages.bodies`.
#[derive(Deserialize)]
struct Bodies {
    #[serde(default)]
    html: Option<BlobRef>,
}

/// What a job read of a message before it parses.
struct Parts {
    calendars: Vec<String>,
    html: Option<String>,
}

impl JobHandler for ReadStructured {
    type Payload = ReadStructuredPayload;
    const NAME: &'static str = "conversations.read_structured";
    const QUEUE: &'static str = "default";
    const MAX_ATTEMPTS: u32 = 5;

    async fn run(
        &self,
        ctx: &mut JobContext,
        payload: ReadStructuredPayload,
    ) -> Result<(), JobError> {
        let workspace: WorkspaceId = ctx
            .workspace()
            .and_then(|w| w.parse().ok())
            .ok_or_else(|| JobError::fatal("read_structured enqueued outside a workspace"))?;
        read(ctx.state(), workspace, payload.message, &self.needs).await
    }
}

/// Read `message`'s structured data now, once: nothing when it has been read,
/// the sender is not believed, or the data names nothing. The model's read
/// (`conversations.read_message`) calls this first, so what the sender's own
/// data holds is on record before the model is asked for dates.
pub async fn read(
    state: &AppState,
    workspace: WorkspaceId,
    message: MessageId,
    needs: &NeedSettings,
) -> Result<(), JobError> {
    let Some((loaded, parts)) = load(state, workspace, message).await? else {
        return Ok(());
    };
    // Parsing is CPU work with attacker-chosen input: off the async workers,
    // and a panic in a parser costs this read, not the worker.
    let found = tokio::task::spawn_blocking(move || {
        structured::read(&parts.calendars, parts.html.as_deref())
    })
    .await;
    let Ok(found) = found else {
        tracing::warn!(feature = "conversations.read_structured", "a parser failed");
        return Ok(());
    };
    if found.items.is_empty() && found.asks.is_empty() {
        return Ok(());
    }
    let mut tx = state.db.begin_for(workspace).await?;
    keep(
        &mut tx,
        &state.events,
        needs,
        workspace,
        message,
        &loaded,
        found,
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// The message, its sender checked, and its calendar parts and HTML read as
/// text. `None` when there is nothing to read or nothing may be.
async fn load(
    state: &AppState,
    workspace: WorkspaceId,
    message: MessageId,
) -> Result<Option<(Loaded, Parts)>, JobError> {
    let mut tx = state.db.begin_for(workspace).await?;
    let row: Option<Loaded> = sqlx::query_as(LOAD)
        .bind(message)
        .fetch_optional(&mut *tx)
        .await?;
    let Some(row) = row.filter(|row| !row.read && !row.suspicious) else {
        tx.commit().await?;
        return Ok(None);
    };
    let sender: Address = serde_json::from_value(row.sender.clone()).map_err(decode)?;
    let headers: Headers = serde_json::from_value(row.headers.clone()).map_err(decode)?;
    // Markup is trivially forged: only a sender whose domain passed DMARC
    // aligned is believed.
    if !sender_authenticated(
        &sender.address.to_lowercase(),
        headers.authentication_results.as_deref(),
    ) {
        tx.commit().await?;
        return Ok(None);
    }
    let blobs: Vec<Value> = sqlx::query_scalar(CALENDARS)
        .bind(message)
        .fetch_all(&mut *tx)
        .await?;
    tx.commit().await?;
    let bodies: Bodies = serde_json::from_value(row.bodies.clone()).map_err(decode)?;
    if blobs.is_empty() && bodies.html.is_none() {
        return Ok(None);
    }
    let ws = workspace.to_string();
    let mut calendars = Vec::new();
    for blob in blobs {
        let blob: BlobRef = serde_json::from_value(blob).map_err(decode)?;
        match read_text(&state.blobs, &ws, &blob, structured::MAX_CALENDAR_BYTES).await {
            Ok((text, _)) => calendars.push(text),
            // Its blob went with the message (its inbox removed): nothing to do.
            Err(_) if !still_there(state, workspace, message).await? => return Ok(None),
            Err(error) => return Err(error.into()),
        }
    }
    let html = match &bodies.html {
        Some(blob) => match read_text(&state.blobs, &ws, blob, structured::MAX_HTML_BYTES).await {
            Ok((text, _)) => Some(text),
            Err(_) if !still_there(state, workspace, message).await? => return Ok(None),
            Err(error) => return Err(error.into()),
        },
        None => None,
    };
    Ok(Some((row, Parts { calendars, html })))
}

fn decode(e: serde_json::Error) -> JobError {
    JobError::fatal(format!("stored message: {}", redact::json(&e)))
}

/// Whether `message` is still stored.
async fn still_there(
    state: &AppState,
    workspace: WorkspaceId,
    message: MessageId,
) -> Result<bool, JobError> {
    let mut tx = state.db.begin_for(workspace).await?;
    let present: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM conversations.messages WHERE id = $1)")
            .bind(message)
            .fetch_one(&mut *tx)
            .await?;
    tx.commit().await?;
    Ok(present)
}

/// Store the read of what the data named, once, and say so.
async fn keep(
    tx: &mut Transaction<'_, Postgres>,
    events: &EventBus,
    settings: &NeedSettings,
    workspace: WorkspaceId,
    message: MessageId,
    loaded: &Loaded,
    found: structured::Found,
) -> Result<(), JobError> {
    let known: Vec<Known> = store::known_items(&mut **tx, loaded.conversation_id)
        .await?
        .iter()
        .map(known)
        .collect();
    let (items, asks) = structured::resolve(found, &known, message, loaded.sent_at, Utc::now());
    if items.is_empty() && asks.is_empty() {
        return Ok(());
    }
    let stored = sqlx::query(
        "INSERT INTO conversations.message_reads
             (message_id, workspace_id, layer, state, texts, items, asks, attempts, dates_read_at,
              read_at)
         VALUES ($1, $2, 'structured', 'read', '{}', $3, $4, 1, now(), now())
         ON CONFLICT (message_id, layer) DO NOTHING",
    )
    .bind(message)
    .bind(workspace.as_uuid())
    .bind(serde_json::to_value(&items).unwrap_or(Value::Null))
    .bind(serde_json::to_value(&asks).unwrap_or(Value::Null))
    .execute(&mut **tx)
    .await?
    .rows_affected();
    if stored == 0 {
        return Ok(());
    }
    let owner = store::owner(&mut **tx, loaded.inbox_id).await?;
    let ws = workspace.to_string();
    events
        .publish(
            tx,
            &crate::events::MessageRead {
                workspace: ws.clone(),
                message,
                conversation: loaded.conversation_id,
                inbox: loaded.inbox_id,
                owner,
                layer: ReadLayer::Structured,
                items: items.iter().map(item_facts).collect(),
                asks: asks.iter().map(ask_facts).collect(),
                summary_ready: false,
            },
        )
        .await?;
    // A payment due may be the date of the conversation's Pay need.
    needs::refresh(tx, events, &ws, loaded.conversation_id, settings).await?;
    Ok(())
}

/// An item already read, as the structured data can tell it again.
fn known(item: &ReadItem) -> Known {
    Known {
        key: item.key.clone(),
        kind: item.kind,
        title: item.title.clone(),
        starts: item.starts.clone(),
        uid: item.uid.clone(),
        sequence: item.sequence,
        cancelled: false,
    }
}

/// Queue the structured read of `message`: one pending job per message. The
/// decide pass queues it once it has answered (or when it will not), so the
/// answer about phishing is on record. Ids only.
pub async fn enqueue(
    tx: &mut Transaction<'_, Postgres>,
    workspace: &str,
    message: MessageId,
) -> Result<Enqueued, sqlx::Error> {
    campbooks_kernel::jobs::enqueue_to::<ReadStructured>(
        tx,
        &ReadStructuredPayload { message },
        EnqueueOptions::for_workspace(workspace.to_owned())
            .unique(format!("conversations.read_structured:{message}")),
    )
    .await
}

#[cfg(test)]
mod tests;
