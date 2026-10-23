//! `conversations.read_message`: Scout's read of one message
//! (campbooks-cloud#204, #275, #42; it was `summarize_need`).
//!
//! It is the only `complete` call that reads a message's content to
//! understand it, and no module adds another. One call gives everything Scout
//! needs from the message: when a need is open on it, a summary in every
//! shipped locale and the need's two dates (when what it asks lapses, by when
//! to act); and always its **items** (things of the reader's that happen at a
//! time) and **asks** (obligations of the reader), held to a strict JSON
//! schema and vetted one by one ([`prompt::vet`]). The mail reaches the model
//! as data under a fixed instruction, with the reader's time zone, the
//! decide pass's `dated_kind` as a hint, the items already read in the
//! conversation and the tasks already open for it (kernel ports).
//!
//! It runs only for a message the decide pass **flagged**
//! ([`crate::domain::dated`]) that is not a phishing warning, and only when
//! the read was asked for: a need opened on the message (queued in the
//! transaction that stores the need), or a member opened it
//! (`conversations.request_message_read`). Unflagged mail and unopened mail
//! without a need never reach the model. A message the sender's own
//! machine-readable data covered ([`crate::features::read_structured`]) is not
//! read for dates, only for its need's summary.
//!
//! The result is stored once in `conversations.message_reads` (layer
//! `model`), keyed by the message, and published as
//! `conversations.message_read`; a summary also as `need_summarised`. Then the
//! conversation's need is worked out again, so a lapse time or a due date
//! reaches the need, its event and Home. A read made before items and asks
//! were read (`read_at` null) is read once more by the needs backfill: a
//! usable answer replaces its texts, no usable answer keeps them and marks it
//! read, so it is never asked again.
//!
//! Bounds on cost: one call per message that is flagged and asked for, each
//! counted against the workspace's daily AI limit as background work (over
//! it, the job waits and tries again after the limit resets). A call that
//! gives no usable answer is recorded as failed and tried at most once more
//! later. The job runs only while the `complete` role may receive mail on
//! this server. Failures and drops are logged by kind and reason, never with
//! mail content.

pub(crate) mod prompt;
pub(crate) mod store;

use std::time::Duration;

use campbooks_ai::{Gateway, Role, usage::Scope};
use campbooks_kernel::{
    EventBus, JobContext, JobError, JobHandler,
    blobs::{BlobOwner, BlobRef, BlobStore},
    db::BeginFor as _,
    tenancy::{UserId, WorkspaceId},
};
use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{Postgres, Transaction};

pub use self::store::enqueue;
use self::{
    prompt::{Answer, Email, Input, KnownItem, OpenTask, Veto, Vetted},
    store::Trigger,
};
use crate::{
    config::DatedSettings,
    domain::{
        blobs::{Budget, words},
        dated::{self, Flagged},
        needs::{self, NeedSettings},
        plain_text::{self, Quotes},
        read::{ReadAsk, ReadItem},
        redact,
    },
    events::{MESSAGE_RECORD, MessageRead, NeedSummarised, ReadAskFacts, ReadItemFacts},
    model::{Address, ConversationId, InboxId, MessageId},
};

/// The feature name its model calls are counted under.
const FEATURE: &str = "conversations.read_message";
/// Most of a message's own words handed to the model, in bytes.
const BODY_BYTES: usize = 6 * 1024;
/// Most of a text part read, in bytes.
const RAW_TEXT_BYTES: usize = 64 * 1024;
/// Most of an HTML part read, in bytes.
const RAW_HTML_BYTES: usize = 256 * 1024;
/// Longest subject or sender name handed on, in bytes.
const SHORT_BYTES: usize = 300;
/// Calls for one message: the first, and one retry later.
pub(crate) const MAX_CALLS: i16 = 2;
/// How long a job over the daily AI limit waits before it tries again.
const OVER_LIMIT_WAIT: Duration = Duration::from_hours(4);
/// Model calls one job may make for a provider that fails to answer, so a
/// flapping provider costs at most this many metered calls per message.
const PROVIDER_ATTEMPTS: u32 = 3;

/// The message to read, with what is stored for it: its sender, subject,
/// parts, where it is, its model read if any, and the need open on it.
const LOAD: &str = "SELECT m.sender, m.subject, m.bodies, m.sent_at, m.conversation_id, \
            m.inbox_id, r.state, r.attempts, r.read_at, r.triggered_by, \
            n.reason AS need_reason, n.question AS need_question \
       FROM conversations.messages m \
       JOIN conversations.inboxes i ON i.id = m.inbox_id AND i.detached_at IS NULL \
       LEFT JOIN conversations.message_reads r ON r.message_id = m.id AND r.layer = 'model' \
       LEFT JOIN conversations.needs n ON n.conversation_id = m.conversation_id \
        AND n.message_id = m.id AND n.state = 'open' \
      WHERE m.id = $1 AND m.direction = 'inbound' AND NOT m.in_spam AND NOT m.in_trash";

/// A read, or a failure, of a message, with the dates read: nothing changes
/// once it is written, but a read that was only queued or failed once, and a
/// read made before items and asks were read, which a usable answer replaces
/// once (keeping its summary when the new one has none).
const STORE: &str = "INSERT INTO conversations.message_reads \
            (message_id, workspace_id, layer, state, texts, items, asks, attempts, lapses_at, \
             due_on, dates_read_at, read_at, triggered_by) \
     VALUES ($1, $2, 'model', $3, $4, $5, $6, 1, $7, $8, now(), \
             CASE WHEN $3 = 'read' THEN now() END, $9) \
     ON CONFLICT (message_id, layer) DO UPDATE \
        SET state = EXCLUDED.state, \
            texts = CASE WHEN EXCLUDED.texts = '{}' THEN message_reads.texts \
                         ELSE EXCLUDED.texts END, \
            items = EXCLUDED.items, asks = EXCLUDED.asks, \
            attempts = message_reads.attempts + 1, written_at = now(), \
            lapses_at = EXCLUDED.lapses_at, due_on = EXCLUDED.due_on, \
            dates_read_at = now(), read_at = EXCLUDED.read_at, \
            triggered_by = COALESCE(message_reads.triggered_by, EXCLUDED.triggered_by) \
      WHERE message_reads.state IN ('queued', 'failed') \
         OR (message_reads.state = 'read' AND message_reads.read_at IS NULL \
             AND EXCLUDED.state = 'read')";

/// A read made before items and asks were read, read again with no usable
/// answer: it keeps its texts, takes whatever dates were read, and is marked
/// read so it is never asked again.
const MARK_READ: &str = "UPDATE conversations.message_reads \
        SET lapses_at = $2, due_on = $3, dates_read_at = now(), read_at = now(), \
            attempts = attempts + 1 \
      WHERE message_id = $1 AND layer = 'model' AND state = 'read' AND read_at IS NULL";

/// A row of [`LOAD`].
#[derive(sqlx::FromRow)]
struct Loaded {
    sender: Value,
    subject: String,
    bodies: Value,
    sent_at: DateTime<Utc>,
    conversation_id: ConversationId,
    inbox_id: InboxId,
    /// `message_reads.state` of the model read, when a row exists.
    state: Option<String>,
    attempts: Option<i16>,
    /// When the read was made in full, if it was.
    read_at: Option<DateTime<Utc>>,
    /// Why it was asked for.
    triggered_by: Option<String>,
    /// The reason of the need open on the message.
    need_reason: Option<String>,
    /// The question behind that need.
    need_question: Option<String>,
}

/// The message to read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadMessagePayload {
    /// The inbound message to read.
    pub message: MessageId,
}

/// Reads a message: its summary when a need is open, and its items and asks.
pub struct ReadMessage {
    gateway: Option<Gateway>,
    needs: NeedSettings,
}

impl ReadMessage {
    /// Reads made through `gateway`'s `complete` role; `None` (the role is
    /// not configured, or may not receive mail here) does nothing. Needs are
    /// worked out again under `needs` once dates are read.
    pub fn new(gateway: Option<Gateway>, needs: NeedSettings) -> Self {
        Self { gateway, needs }
    }
}

/// Whether `gateway`'s `complete` role may receive mail content here.
pub fn may_read_mail(gateway: &Gateway) -> bool {
    gateway
        .config()
        .provider_for_content(Role::Complete)
        .is_ok()
}

/// `messages.bodies`.
#[derive(Deserialize)]
struct Bodies {
    #[serde(default)]
    text: Option<BlobRef>,
    #[serde(default)]
    html: Option<BlobRef>,
}

impl JobHandler for ReadMessage {
    type Payload = ReadMessagePayload;
    const NAME: &'static str = "conversations.read_message";
    const QUEUE: &'static str = "ai";
    // Waiting out the daily limit uses attempts: 8 x 4 h covers a reset.
    const MAX_ATTEMPTS: u32 = 8;

    async fn run(&self, ctx: &mut JobContext, payload: ReadMessagePayload) -> Result<(), JobError> {
        let Some(gateway) = &self.gateway else {
            return Ok(());
        };
        let workspace: WorkspaceId = ctx
            .workspace()
            .and_then(|w| w.parse().ok())
            .ok_or_else(|| JobError::fatal("read_message enqueued outside a workspace"))?;
        let message = payload.message;
        // What the sender's own data holds is read first and kept, so the
        // model is asked for dates only where the data left some.
        crate::features::read_structured::read(ctx.state(), workspace, message, &self.needs)
            .await?;
        let Some(read) = load(ctx, workspace, message, &self.needs).await? else {
            return Ok(());
        };
        let usage = Scope::workspace(ctx.state().db.clone(), workspace, FEATURE);
        let answer = match usage
            .run(gateway.chat(
                Role::Complete,
                prompt::request_for(&read.input, read.waiting),
            ))
            .await
        {
            Ok(response) => parsed(&response),
            // The workspace used its whole daily limit: wait for it to
            // reset, as background work does.
            Err(campbooks_ai::Error::OverDailyCap { .. }) => {
                return Err(JobError::retry_after(
                    "the workspace reached its daily AI limit",
                    OVER_LIMIT_WAIT,
                ));
            }
            Err(error) => {
                tracing::warn!(
                    feature = FEATURE,
                    error = error_kind(&error),
                    "the read call failed"
                );
                // A provider that cannot be reached is tried again, at most
                // [`PROVIDER_ATTEMPTS`] calls in all (the other attempts are
                // for waiting out the daily limit); then the failure is
                // recorded.
                if ctx.attempt() < PROVIDER_ATTEMPTS && !ctx.is_last_attempt() && retryable(&error)
                {
                    return Err(redact::ai(&error));
                }
                None
            }
        };
        let events = ctx.state().events.clone();
        let blobs = ctx.state().blobs.clone();
        let tx = ctx.tx().await?;
        keep(tx, &events, &blobs, &self.needs, &read, answer).await?;
        Ok(())
    }
}

/// What a call is made from, and where its answer is kept.
struct Read {
    workspace: WorkspaceId,
    message: MessageId,
    conversation: ConversationId,
    inbox: InboxId,
    /// The inbox's personal owner; none for a shared inbox.
    owner: Option<UserId>,
    /// When the message was sent: the need's dates must not come before it.
    sent_at: DateTime<Utc>,
    /// What the model is given.
    input: Input,
    /// A read is stored but was made before items and asks were read.
    rereading: bool,
    /// A need is open on the message.
    need_open: bool,
    /// Its need waits on someone else: `due_on` is when they are expected
    /// to act.
    waiting: bool,
    /// Why the read was asked for.
    trigger: Trigger,
    /// What the vetoes check items and asks against.
    veto: Veto,
}

/// Why a stored message is not read now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Skip {
    /// Its need is a phishing warning: a read would retell the scam.
    Phishing,
    /// The decide pass did not flag it.
    NotFlagged,
    /// It has its read already, or has used both calls.
    Done,
    /// The read was asked for by a need that is no longer open, or by an
    /// open that dated things switched off, or there is nothing left to ask.
    NothingToAsk,
}

/// What a stored message needs of the model, or why it needs nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Plan {
    rereading: bool,
    need_open: bool,
    dates: bool,
    trigger: Trigger,
}

/// Whether `row`'s message is read now: flagged, not a warning, not read
/// already, asked for by something that still stands, and with something left
/// to ask (`covered`: the sender's own data holds its dates, so only the
/// summary of an open need is left).
fn plan(row: &Loaded, flagged: bool, covered: bool, settings: DatedSettings) -> Result<Plan, Skip> {
    if row.need_question.as_deref() == Some("suspicious") {
        return Err(Skip::Phishing);
    }
    if !flagged {
        return Err(Skip::NotFlagged);
    }
    let rereading = match row.state.as_deref() {
        Some("read") if row.read_at.is_some() => return Err(Skip::Done),
        Some("read") => true,
        Some("failed") if row.attempts.is_none_or(|n| n >= MAX_CALLS) => return Err(Skip::Done),
        _ => false,
    };
    let need_open = row.need_reason.is_some();
    let trigger = match row.triggered_by.as_deref() {
        Some("open") => Trigger::Open,
        // A read with no trigger on record was queued with a need.
        _ => Trigger::Need,
    };
    let stands = match trigger {
        Trigger::Need => need_open,
        Trigger::Open => settings.enabled,
    };
    let dates = !covered;
    // A read made before is read again for its need's summary only.
    if !stands || (!dates && !need_open) || (rereading && !need_open) {
        return Err(Skip::NothingToAsk);
    }
    Ok(Plan {
        rereading,
        need_open,
        dates,
        trigger,
    })
}

/// The message as the model reads it, when it is worth a call.
#[allow(clippy::too_many_lines)]
async fn load(
    ctx: &JobContext,
    workspace: WorkspaceId,
    message: MessageId,
    settings: &NeedSettings,
) -> Result<Option<Read>, JobError> {
    let decode =
        |e: serde_json::Error| JobError::fatal(format!("stored message: {}", redact::json(&e)));
    let state = ctx.state();
    let mut tx = state.db.begin_for(workspace).await?;
    let row: Option<Loaded> = sqlx::query_as(LOAD)
        .bind(message)
        .fetch_optional(&mut *tx)
        .await?;
    let Some(row) = row else {
        tx.commit().await?;
        return Ok(None);
    };
    let answers = needs::answers(&mut tx, message).await?;
    let flag = dated::flag(&answers, &settings.dated);
    let structured: Option<(Value, Value)> = sqlx::query_as(
        "SELECT items, asks FROM conversations.message_reads
          WHERE message_id = $1 AND layer = 'structured' AND state = 'read'",
    )
    .bind(message)
    .fetch_optional(&mut *tx)
    .await?;
    let covered = covers(structured.as_ref(), flag.as_ref());
    let plan = match plan(&row, flag.is_some(), covered, settings.dated) {
        Ok(plan) => plan,
        Err(skip) => {
            tx.commit().await?;
            tracing::debug!(feature = FEATURE, skip = ?skip, "no read");
            return Ok(None);
        }
    };
    let known = store::known_items(&mut tx, row.conversation_id).await?;
    let owner = store::owner(&mut tx, row.inbox_id).await?;
    let sender: Address = serde_json::from_value(row.sender.clone()).map_err(decode)?;
    let zone = match owner {
        Some(owner) => {
            state
                .time_zones
                .time_zone(&mut tx, workspace, owner)
                .await?
        }
        None => None,
    };
    let tasks = state
        .open_tasks
        .open_tasks(
            &mut tx,
            workspace,
            *row.inbox_id.as_uuid(),
            *row.conversation_id.as_uuid(),
            &sender.address,
        )
        .await?;
    tx.commit().await?;

    let bodies: Bodies = serde_json::from_value(row.bodies).map_err(decode)?;
    let ws = workspace.to_string();
    // Its text part, else its HTML read as text (campbooks-cloud#218).
    let raw = words(
        &state.blobs,
        &ws,
        (bodies.text.as_ref(), bodies.html.as_ref()),
        Budget {
            text: RAW_TEXT_BYTES,
            html: RAW_HTML_BYTES,
        },
        Quotes::Drop,
    )
    .await;
    let raw = match raw {
        Ok((text, _)) => text,
        // Its blob went with the message (its inbox removed): nothing to do.
        Err(_) if !still_there(ctx, workspace, message).await? => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let text = cut_bytes(&plain_text::own_words(&raw), BODY_BYTES);
    // Nothing to read: a read of a subject alone would be a guess.
    if text.trim().is_empty() {
        return Ok(None);
    }
    let from = sender
        .name
        .filter(|name| !name.trim().is_empty())
        .unwrap_or(sender.address);
    let shown: Vec<KnownItem> = known
        .iter()
        .rev()
        .take(prompt::MAX_KNOWN)
        .rev()
        .map(|item| KnownItem {
            id: item.key.clone(),
            kind: item.kind,
            title: item.title.clone(),
            starts: item.starts.clone(),
        })
        .collect();
    let open_tasks: Vec<OpenTask> = tasks
        .iter()
        .take(campbooks_kernel::tasks::MAX_OPEN_TASKS)
        .map(|task| OpenTask {
            task_ref: task.reference.clone(),
            title: task.title.clone(),
            due: task
                .due_at
                .map(|at| at.to_rfc3339_opts(SecondsFormat::Secs, true))
                .or_else(|| task.due_on.map(|day| day.to_string())),
        })
        .collect();
    let veto = Veto {
        message,
        sent_at: row.sent_at,
        now: Utc::now(),
        words: prompt::normalise(&text),
        known: shown.iter().map(|k| k.id.clone()).collect(),
        tasks: open_tasks.iter().map(|t| t.task_ref.clone()).collect(),
    };
    Ok(Some(Read {
        workspace,
        message,
        conversation: row.conversation_id,
        inbox: row.inbox_id,
        owner,
        sent_at: row.sent_at,
        input: Input {
            email: Email {
                from: cut_bytes(&from, SHORT_BYTES),
                sent_at: row.sent_at.to_rfc3339_opts(SecondsFormat::Secs, true),
                subject: cut_bytes(&row.subject, SHORT_BYTES),
                text,
            },
            time_zone: zone
                .unwrap_or_else(|| campbooks_kernel::schedule::DEFAULT_TIME_ZONE.to_owned()),
            need: row.need_reason.clone().filter(|_| plan.need_open),
            dates: plan.dates,
            dated_kind: flag.and_then(|flagged| flagged.kind),
            known_items: shown,
            open_tasks,
        },
        rereading: plan.rereading,
        need_open: plan.need_open,
        waiting: matches!(row.need_reason.as_deref(), Some("waiting" | "nudge")),
        trigger: plan.trigger,
        veto,
    }))
}

/// Whether the sender's own data (`structured`: its items and asks, when it
/// has a stored read) holds the dates of a message the pass flagged
/// (`flag`): the pass named a kind among the items found, a deadline with a
/// payment found, or none (a deadline without one, or no kind at all, counts
/// as covered once something was found).
pub(crate) fn covers(structured: Option<&(Value, Value)>, flag: Option<&Flagged>) -> bool {
    let Some((items, asks)) = structured else {
        return false;
    };
    let kind = flag.and_then(|flagged| flagged.kind.as_deref());
    match kind {
        None | Some("none") => true,
        Some("deadline") => asks.as_array().is_some_and(|asks| !asks.is_empty()),
        Some(kind) => items.as_array().is_some_and(|items| {
            items
                .iter()
                .any(|item| item.get("kind").and_then(Value::as_str) == Some(kind))
        }),
    }
}

/// Keep what the call gave for `read` in `tx`: the read or a failure, with the
/// dates, items and asks; then work its need out again (the dates may change
/// it) and say what was written.
async fn keep(
    tx: &mut Transaction<'_, Postgres>,
    events: &EventBus,
    blobs: &BlobStore,
    settings: &NeedSettings,
    read: &Read,
    answer: Option<Answer>,
) -> Result<(), JobError> {
    let (lapses_at, due_on) = answer.as_ref().map_or((None, None), |answer| {
        prompt::read_dates(answer, read.sent_at)
    });
    let Vetted {
        mut items,
        mut asks,
        dropped,
    } = answer
        .as_ref()
        .map(|answer| prompt::vet(answer, &read.veto))
        .unwrap_or_default();
    for (what, reason) in &dropped {
        tracing::info!(
            feature = FEATURE,
            what,
            reason = reason.as_str(),
            "dropped from a read"
        );
    }
    // The sender's own data holds its dates: a model's would only repeat them.
    if !read.input.dates {
        items.clear();
        asks.clear();
    }
    let texts = answer
        .as_ref()
        .map(prompt::kept)
        .filter(|texts| !texts.is_empty());
    // Usable: it parsed, and either said something or had nothing to say
    // (no need open to summarise).
    let usable = answer.is_some()
        && (texts.is_some() || !items.is_empty() || !asks.is_empty() || !read.need_open);
    if !usable {
        tracing::warn!(feature = FEATURE, "no usable read");
    }
    let state = if usable { "read" } else { "failed" };
    let summary_ready = texts.is_some();
    let stored = sqlx::query(STORE)
        .bind(read.message)
        .bind(read.workspace.as_uuid())
        .bind(state)
        .bind(serde_json::to_value(texts.unwrap_or_default()).unwrap_or(Value::Null))
        .bind(serde_json::to_value(&items).unwrap_or(Value::Null))
        .bind(serde_json::to_value(&asks).unwrap_or(Value::Null))
        .bind(lapses_at)
        .bind(due_on)
        .bind(read.trigger.as_str())
        .execute(&mut **tx)
        .await?
        .rows_affected();
    // Read again with nothing usable: the old summary stays, read.
    let marked = if !usable && read.rereading {
        sqlx::query(MARK_READ)
            .bind(read.message)
            .bind(lapses_at)
            .bind(due_on)
            .execute(&mut **tx)
            .await?
            .rows_affected()
    } else {
        0
    };
    let workspace = read.workspace.to_string();
    // Its dates may change the need (a lapse time, a due date).
    if stored + marked > 0 {
        needs::refresh(tx, events, &workspace, read.conversation, settings).await?;
    }
    if usable && stored == 1 {
        if !items.is_empty() || !asks.is_empty() || summary_ready {
            let texts = put_texts(blobs, tx, &workspace, read.message, &items, &asks).await?;
            events
                .publish(
                    tx,
                    &MessageRead {
                        workspace: workspace.clone(),
                        message: read.message,
                        conversation: read.conversation,
                        inbox: read.inbox,
                        owner: read.owner,
                        layer: crate::model::ReadLayer::Model,
                        items: items.iter().map(item_facts).collect(),
                        asks: asks.iter().map(ask_facts).collect(),
                        summary_ready,
                        texts,
                    },
                )
                .await?;
        }
        if summary_ready {
            events
                .publish(
                    tx,
                    &NeedSummarised {
                        workspace,
                        inbox: read.inbox,
                        conversation: read.conversation,
                        message: read.message,
                    },
                )
                .await?;
        }
    }
    Ok(())
}

/// The titles, places and calendar `UID`s of `items`, and the title and amount
/// of `asks`, as a blob that the
/// message holds: what `message_read` points at instead of carrying it. `None`
/// when no item has any.
pub(crate) async fn put_texts(
    blobs: &BlobStore,
    tx: &mut Transaction<'_, Postgres>,
    workspace: &str,
    message: MessageId,
    items: &[ReadItem],
    asks: &[ReadAsk],
) -> Result<Option<BlobRef>, JobError> {
    let entries: Vec<Value> = items
        .iter()
        .filter(|item| {
            !item.title.is_empty()
                || item.place.is_some()
                || item.uid.is_some()
                || item.quote.is_some()
        })
        .map(|item| {
            serde_json::json!({
                "key": item.key,
                "title": item.title,
                "place": item.place,
                "ical_uid": item.uid,
                "quote": item.quote,
            })
        })
        .collect();
    let ask_entries: Vec<Value> = asks
        .iter()
        .filter(|ask| !ask.title.is_empty() || ask.amount.is_some())
        .map(|ask| {
            serde_json::json!({
                "key": ask.key,
                "title": ask.title,
                "amount": ask.amount.as_ref().map(|a| serde_json::json!({
                    "minor": a.minor,
                    "currency": a.currency,
                })),
            })
        })
        .collect();
    if entries.is_empty() && ask_entries.is_empty() {
        return Ok(None);
    }
    let mut body = serde_json::json!({ "items": entries });
    if !ask_entries.is_empty() {
        body["asks"] = Value::Array(ask_entries);
    }
    let bytes = serde_json::to_vec(&body)
        .map_err(|_| JobError::fatal("the texts of a read do not encode"))?;
    let blob = blobs
        .put(
            workspace,
            bytes,
            "application/vnd.campbooks.read-texts+json; version=1",
        )
        .await?;
    blobs
        .attach(
            tx,
            workspace,
            &blob,
            &BlobOwner::new(MESSAGE_RECORD, message),
        )
        .await?;
    Ok(Some(blob))
}

/// An item as `message_read` carries it: no title, place or quote.
pub(crate) fn item_facts(item: &ReadItem) -> ReadItemFacts {
    ReadItemFacts {
        key: item.key.clone(),
        kind: item.kind,
        starts: item.starts.clone(),
        ends: item.ends.clone(),
        all_day: item.all_day,
        time_zone: item.time_zone.clone(),
        updates: item.updates.clone(),
        cancels: item.cancels.clone(),
    }
}

/// An ask as `message_read` carries it: no title, quote or amount.
pub(crate) fn ask_facts(ask: &ReadAsk) -> ReadAskFacts {
    ReadAskFacts {
        key: ask.key.clone(),
        verb: ask.verb,
        due: ask.due.clone(),
        has_amount: ask.amount.is_some(),
        same_as: ask.same_as.clone(),
    }
}

/// The model's answer, or `None` with a warning saying why it did not
/// parse: sizes and the stop reason only, never what the model wrote.
fn parsed(response: &campbooks_ai::ChatResponse) -> Option<Answer> {
    let answer = response.json::<Answer>().ok();
    if answer.is_none() {
        let finish = response.finish_kind();
        let reason = if response.content.trim().is_empty() {
            "empty"
        } else if finish == "length" {
            "truncated"
        } else {
            "not_json"
        };
        tracing::warn!(
            feature = FEATURE,
            reason,
            finish_reason = finish,
            completion_tokens = response.usage.map(|u| u.completion_tokens),
            content_chars = response.content.chars().count(),
            "the read answer did not parse"
        );
    }
    answer
}

/// A failure that may pass: the provider could not be reached or refused
/// for now, or the call could not be counted. A malformed answer is not
/// tried again: the same mail would likely get the same answer.
fn retryable(error: &campbooks_ai::Error) -> bool {
    matches!(
        error,
        campbooks_ai::Error::Transport(_)
            | campbooks_ai::Error::Status { .. }
            | campbooks_ai::Error::Metering
    )
}

/// What kind of failure, for the log: never the provider's words.
fn error_kind(error: &campbooks_ai::Error) -> &'static str {
    match error {
        campbooks_ai::Error::RoleNotConfigured(_) => "role_not_configured",
        campbooks_ai::Error::ExternalContentRefused { .. } => "external_content_refused",
        campbooks_ai::Error::Transport(_) => "transport",
        campbooks_ai::Error::Status { .. } => "status",
        campbooks_ai::Error::InvalidResponse(_) => "invalid_response",
        campbooks_ai::Error::InvalidQuestion(_) | campbooks_ai::Error::Config(_) => "config",
        campbooks_ai::Error::AnswersNotTrusted => "answers_not_trusted",
        campbooks_ai::Error::OverDailyCap { .. } => "over_daily_cap",
        campbooks_ai::Error::Unattributed => "unattributed",
        campbooks_ai::Error::Metering => "metering",
    }
}

/// Whether `message` is still stored.
async fn still_there(
    ctx: &JobContext,
    workspace: WorkspaceId,
    message: MessageId,
) -> Result<bool, JobError> {
    let mut tx = ctx.state().db.begin_for(workspace).await?;
    let present: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM conversations.messages WHERE id = $1)")
            .bind(message)
            .fetch_one(&mut *tx)
            .await?;
    tx.commit().await?;
    Ok(present)
}

/// `text` cut to at most `max` bytes, at a character boundary.
fn cut_bytes(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

#[cfg(test)]
mod tests;
