//! `conversations.summarize_need`: Scout's summary of the email behind a need
//! (campbooks-cloud#204).
//!
//! When a need opens, the need's refresh queues this job for its message
//! (never inside the decide transaction, never on a request path); the
//! needs backfill queues it for the open needs that have none. The
//! `complete` role reads the message as data (a fixed instruction, the mail
//! only as JSON string values, plain text without quoted history, bounded)
//! and writes one or two plain sentences in every shipped locale, held to a
//! strict JSON schema, in one call. The result is stored once in
//! `conversations.need_summaries` and read back by `conversations.get` in the
//! caller's locale; it is never written again, and opening an item never asks
//! the model.
//!
//! Bounds on cost: one call per message that raises a need, so a burst of
//! mail is bounded by the needs it raises, and each call is background work
//! counted against the workspace's daily AI limit (over it, the job waits
//! and tries again after the limit resets). A call that gives no usable
//! answer is recorded as failed and tried at most once more later. The job
//! runs only while the `complete` role may receive mail on this server.
//! Failures are logged by kind, never with mail content.

mod prompt;

use std::time::Duration;

use campbooks_ai::{Gateway, Role, usage::Scope};
use campbooks_kernel::{
    JobContext, JobError, JobHandler,
    blobs::BlobRef,
    db::BeginFor as _,
    jobs::{EnqueueOptions, Enqueued},
    tenancy::WorkspaceId,
};
use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{Postgres, Transaction};

use self::prompt::{Answer, Email};
use crate::{
    domain::{
        blobs::{Budget, words},
        plain_text::{self, Quotes},
        redact,
    },
    events::NeedSummarised,
    model::{Address, ConversationId, InboxId, MessageId},
};

/// The feature name its model calls are counted under.
const FEATURE: &str = "conversations.summarize_need";
/// Most of a message's own words handed to the model, in bytes.
const BODY_BYTES: usize = 6 * 1024;
/// Most of a text part read, in bytes.
const RAW_TEXT_BYTES: usize = 64 * 1024;
/// Most of an HTML part read, in bytes.
const RAW_HTML_BYTES: usize = 256 * 1024;
/// Longest subject or sender name handed on, in bytes.
const SHORT_BYTES: usize = 300;
/// Calls for one message: the first, and one retry later.
const MAX_CALLS: i16 = 2;
/// How long a job over the daily AI limit waits before it tries again.
const OVER_LIMIT_WAIT: Duration = Duration::from_hours(4);

/// The message the summary is for, if its need is still open and is not a
/// phishing warning: its sender,
/// subject, parts, where it is, and what is stored for it.
const LOAD: &str = "SELECT m.sender, m.subject, m.bodies, m.sent_at, m.conversation_id, \
            m.inbox_id, s.state, s.attempts \
       FROM conversations.messages m \
       JOIN conversations.inboxes i ON i.id = m.inbox_id AND i.detached_at IS NULL \
       JOIN conversations.needs n ON n.conversation_id = m.conversation_id \
        AND n.message_id = m.id AND n.state = 'open' AND n.question <> 'suspicious' \
       LEFT JOIN conversations.need_summaries s ON s.message_id = m.id \
      WHERE m.id = $1 AND m.direction = 'inbound' AND NOT m.in_spam AND NOT m.in_trash";

/// A written summary, or a failure, of a message: nothing changes
/// once it is written.
const STORE: &str = "INSERT INTO conversations.need_summaries \
            (message_id, workspace_id, state, texts, attempts) \
     VALUES ($1, $2, $3, $4, 1) \
     ON CONFLICT (message_id) DO UPDATE \
        SET state = EXCLUDED.state, texts = EXCLUDED.texts, \
            attempts = need_summaries.attempts + 1, written_at = now() \
      WHERE need_summaries.state = 'failed'";

/// A row of [`LOAD`].
#[derive(sqlx::FromRow)]
struct Loaded {
    sender: Value,
    subject: String,
    bodies: Value,
    sent_at: DateTime<Utc>,
    conversation_id: ConversationId,
    inbox_id: InboxId,
    /// `need_summaries.state`, when a row exists.
    state: Option<String>,
    attempts: Option<i16>,
}

/// The message to summarise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SummarizeNeedPayload {
    /// The inbound message a need comes from.
    pub message: MessageId,
}

/// Writes the summary of the email behind a need.
pub struct SummarizeNeed {
    gateway: Option<Gateway>,
}

impl SummarizeNeed {
    /// Summaries written through `gateway`'s `complete` role; `None` (the
    /// role is not configured, or may not receive mail here) does nothing.
    pub fn new(gateway: Option<Gateway>) -> Self {
        Self { gateway }
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

impl JobHandler for SummarizeNeed {
    type Payload = SummarizeNeedPayload;
    const NAME: &'static str = "conversations.summarize_need";
    const QUEUE: &'static str = "ai";
    // Waiting out the daily limit uses attempts: 8 x 4 h covers a reset.
    const MAX_ATTEMPTS: u32 = 8;

    async fn run(
        &self,
        ctx: &mut JobContext,
        payload: SummarizeNeedPayload,
    ) -> Result<(), JobError> {
        let Some(gateway) = &self.gateway else {
            return Ok(());
        };
        let workspace: WorkspaceId = ctx
            .workspace()
            .and_then(|w| w.parse().ok())
            .ok_or_else(|| JobError::fatal("summarize_need enqueued outside a workspace"))?;
        let message = payload.message;
        let Some(loaded) = load(ctx, workspace, message).await? else {
            return Ok(());
        };
        let email = loaded.email;
        let usage = Scope::workspace(ctx.state().db.clone(), workspace, FEATURE);
        let answer = match usage
            .run(gateway.chat(Role::Complete, prompt::request(&email)))
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
                    "the summary call failed"
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
        let texts = answer.map(prompt::kept).filter(|texts| !texts.is_empty());
        if texts.is_none() {
            tracing::warn!(feature = FEATURE, "no usable summary");
        }
        let events = ctx.state().events.clone();
        let tx = ctx.tx().await?;
        let (state, texts) = match texts {
            Some(texts) => (
                "written",
                serde_json::to_value(texts).unwrap_or(Value::Null),
            ),
            None => ("failed", Value::Object(serde_json::Map::new())),
        };
        let stored = sqlx::query(STORE)
            .bind(message)
            .bind(workspace.as_uuid())
            .bind(state)
            .bind(texts)
            .execute(&mut **tx)
            .await?;
        if state == "written" && stored.rows_affected() == 1 {
            events
                .publish(
                    tx,
                    &NeedSummarised {
                        workspace: workspace.to_string(),
                        inbox: loaded.inbox,
                        conversation: loaded.conversation,
                        message,
                    },
                )
                .await?;
        }
        Ok(())
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
            "the summary answer did not parse"
        );
    }
    answer
}

/// Model calls one job may make for a provider that fails to answer, so a
/// flapping provider costs at most this many metered calls per message.
const PROVIDER_ATTEMPTS: u32 = 3;

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

/// What the call is made from.
struct Read {
    email: Email,
    conversation: ConversationId,
    inbox: InboxId,
}

/// The message as the model reads it, when it is worth a call: its need is
/// still open, nothing is stored for it (or one failure that may be tried
/// once more), and it has words.
async fn load(
    ctx: &JobContext,
    workspace: WorkspaceId,
    message: MessageId,
) -> Result<Option<Read>, JobError> {
    let state = ctx.state();
    let mut tx = state.db.begin_for(workspace).await?;
    let row: Option<Loaded> = sqlx::query_as(LOAD)
        .bind(message)
        .fetch_optional(&mut *tx)
        .await?;
    tx.commit().await?;
    let Some(row) = row else {
        return Ok(None);
    };
    match row.state.as_deref() {
        Some("written") => return Ok(None),
        Some(_) if row.attempts.is_none_or(|n| n >= MAX_CALLS) => return Ok(None),
        _ => {}
    }
    let decode =
        |e: serde_json::Error| JobError::fatal(format!("stored message: {}", redact::json(&e)));
    let sender: Address = serde_json::from_value(row.sender).map_err(decode)?;
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
    // Nothing to read: a summary of a subject alone would be a guess.
    if text.trim().is_empty() {
        return Ok(None);
    }
    let from = sender
        .name
        .filter(|name| !name.trim().is_empty())
        .unwrap_or(sender.address);
    Ok(Some(Read {
        email: Email {
            from: cut_bytes(&from, SHORT_BYTES),
            sent_at: row.sent_at.to_rfc3339_opts(SecondsFormat::Secs, true),
            subject: cut_bytes(&row.subject, SHORT_BYTES),
            text,
        },
        conversation: row.conversation_id,
        inbox: row.inbox_id,
    }))
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

/// Queue the summary of `message`: one pending or running job per message.
pub async fn enqueue(
    tx: &mut Transaction<'_, Postgres>,
    workspace: &str,
    message: MessageId,
) -> Result<Enqueued, sqlx::Error> {
    campbooks_kernel::jobs::enqueue_to::<SummarizeNeed>(
        tx,
        &SummarizeNeedPayload { message },
        EnqueueOptions::for_workspace(workspace.to_owned())
            .unique(format!("conversations.summarize_need:{message}"))
            .priority(-10),
    )
    .await
}

#[cfg(test)]
mod tests;
