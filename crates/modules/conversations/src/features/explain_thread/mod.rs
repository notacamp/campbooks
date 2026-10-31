//! `conversations.explain_thread`: Scout's explanation of one thread for the
//! member who asked (campbooks-cloud#407).
//!
//! It runs on queue `ai` as interactive work (a member is waiting), once for
//! each accepted `conversations.explain`, and makes exactly one call to the
//! `complete` role, counted under `conversations.explain` against the
//! workspace's daily AI limit. Nothing queues it but the member's request,
//! and nothing retries it: a call that gives nothing usable is stored as
//! failed and the member may ask again.
//!
//! - It re-reads the member's row and stops unless it is still `writing`; it
//!   re-checks that the member still reads the conversation and that it can
//!   be explained, and otherwise deletes the row.
//! - It loads the thread in date order, drafts and spam left out, as each
//!   sender's own words without quoted history: the newest 20 in full at
//!   most 2 KiB each, every earlier message as one line (Scout's read summary
//!   in English if it has one, else its subject), 32 KiB in all, the oldest
//!   full message demoted first ([`load`]).
//! - The mail reaches the model only as JSON string values under a fixed
//!   instruction that says it is data, with no tools and a strict schema
//!   ([`prompt`]); the answer is vetted ([`prompt::vet`]) and stored in the
//!   member's row, or a failure is (`limit`, `unusable`, `unavailable`),
//!   keeping any earlier explanation.
//! - It publishes `conversations.thread_explained` (ids only; only that
//!   member is told) and tracks the outcome, a bucket of the message count
//!   and of the duration, and whether it replaced an earlier explanation.

pub(crate) mod prompt;

use std::time::Instant;

use campbooks_ai::{Gateway, Role, usage::Scope};
use campbooks_kernel::{
    AppState, JobContext, JobError, JobHandler,
    db::BeginFor as _,
    i18n::Locale,
    tenancy::{UserId, WorkspaceId},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use self::prompt::{Answer, Input, Labels, Line, You};
use crate::{
    domain::{
        explanation::{self, Explanation, ExplanationFailure},
        plain_text,
    },
    events::ThreadExplained,
    features::{
        get_conversation::Bodies,
        read_message::{error_kind, store},
        reply::{cut_bytes, own_words, shown_name},
    },
    model::{Address, ConversationId, InboxId, MessageId},
    tracking,
};

/// The feature name its model call is counted under.
const FEATURE: &str = "conversations.explain";
/// Newest messages read in full.
const FULL_MESSAGES: usize = 20;
/// Most of a message's own words handed on, in bytes.
const FULL_BYTES: usize = 2 * 1024;
/// Most of the thread handed on, in bytes.
const INPUT_BYTES: usize = 32 * 1024;
/// Earlier messages handed on as one line each, at most.
const EARLIER_MESSAGES: usize = 180;
/// Longest note on an earlier message, in bytes.
const NOTE_BYTES: usize = 300;
/// Longest sender name or subject handed on, in bytes.
const SHORT_BYTES: usize = 300;

/// Whose explanation of which conversation to write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplainThreadPayload {
    /// The member who asked.
    pub member: UserId,
    /// The conversation.
    pub conversation: ConversationId,
}

/// Writes a member's explanation of a thread.
pub struct ExplainThread {
    gateway: Option<Gateway>,
}

impl ExplainThread {
    /// Explanations written through `gateway`'s `complete` role; `None` (the
    /// role is not configured, or may not receive mail here) does nothing.
    pub fn new(gateway: Option<Gateway>) -> Self {
        Self { gateway }
    }
}

/// What a call is made from.
struct Work {
    inbox: InboxId,
    /// The thread version the explanation is made from.
    current: explanation::Current,
    /// The language it is asked in.
    locale: Locale,
    /// The model's input.
    input: Input,
    /// Every message that went in, by label.
    labels: Labels,
    /// Their ids in order.
    ids: Vec<MessageId>,
    /// How many of them in full.
    in_full: usize,
    /// An earlier explanation is kept in the row.
    replaces: bool,
}

/// What the request came to.
enum Outcome {
    Ready(Explanation, String),
    Failed(ExplanationFailure),
}

impl JobHandler for ExplainThread {
    type Payload = ExplainThreadPayload;
    const NAME: &'static str = "conversations.explain_thread";
    const QUEUE: &'static str = "ai";
    // One call for one request: a failure is stored, never retried.
    const MAX_ATTEMPTS: u32 = 1;

    async fn run(
        &self,
        ctx: &mut JobContext,
        payload: ExplainThreadPayload,
    ) -> Result<(), JobError> {
        let Some(gateway) = &self.gateway else {
            return Ok(());
        };
        let workspace: WorkspaceId = ctx
            .workspace()
            .and_then(|w| w.parse().ok())
            .ok_or_else(|| JobError::fatal("explain_thread enqueued outside a workspace"))?;
        let started = Instant::now();
        let state = ctx.state().clone();
        let Some(work) = load(&state, workspace, payload).await? else {
            return Ok(());
        };
        let usage = Scope::workspace(state.db.clone(), workspace, FEATURE).interactive();
        let outcome = match usage
            .run(gateway.chat_with_room(Role::Complete, prompt::request_for(&work.input)))
            .await
        {
            Ok(response) => {
                let vetted = response
                    .json::<Answer>()
                    .ok()
                    .and_then(|answer| prompt::vet(&answer, &work.labels));
                if let Some(body) = vetted {
                    Outcome::Ready(body, response.model.clone())
                } else {
                    tracing::warn!(
                        feature = FEATURE,
                        finish_reason = response.finish_kind(),
                        completion_tokens = response.usage.map(|u| u.completion_tokens),
                        content_chars = response.content.chars().count(),
                        "the explanation did not parse"
                    );
                    Outcome::Failed(ExplanationFailure::Unusable)
                }
            }
            // The workspace used its whole daily limit: said, not retried.
            Err(campbooks_ai::Error::OverDailyCap { .. }) => {
                Outcome::Failed(ExplanationFailure::Limit)
            }
            Err(error) => {
                tracing::warn!(
                    feature = FEATURE,
                    error = error_kind(&error),
                    "the explanation call failed"
                );
                Outcome::Failed(ExplanationFailure::Unavailable)
            }
        };
        keep(ctx, workspace, payload, &work, outcome, started).await
    }
}

/// The member's row, the thread, and everything the model is given, when the
/// request still stands: the row is `writing`, the member still reads the
/// conversation, and it can still be explained. Otherwise the row is deleted
/// (the member has no business with it any more) and nothing is asked.
#[allow(clippy::too_many_lines)]
async fn load(
    state: &AppState,
    workspace: WorkspaceId,
    payload: ExplainThreadPayload,
) -> Result<Option<Work>, JobError> {
    let ExplainThreadPayload {
        member,
        conversation,
    } = payload;
    let mut tx = state.db.begin_for(workspace).await?;
    let row: Option<(String, String, String, Option<String>, bool)> = sqlx::query_as(
        "SELECT state, asked_version, asked_locale, reader, body IS NOT NULL
           FROM conversations.explanations
          WHERE workspace_id = $1 AND member_id = $2 AND conversation_id = $3",
    )
    .bind(workspace.as_uuid())
    .bind(member.as_uuid())
    .bind(conversation)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((row_state, _, asked_locale, reader, replaces)) = row else {
        tx.commit().await?;
        return Ok(None);
    };
    if row_state != "writing" {
        tx.commit().await?;
        return Ok(None);
    }
    let forget = "DELETE FROM conversations.explanations
                   WHERE workspace_id = $1 AND member_id = $2 AND conversation_id = $3";
    // The member's access now: a member of the conversation's inbox, or its
    // owner.
    let seen: Option<(InboxId, String)> = sqlx::query_as(
        "SELECT c.inbox_id, c.subject FROM conversations.conversations c
          WHERE c.id = $1 AND c.inbox_id IN (
                SELECT inbox_id FROM conversations.inbox_members
                 WHERE workspace_id = $2 AND user_id = $3
                UNION
                SELECT id FROM conversations.inboxes WHERE workspace_id = $2 AND owner = $3)",
    )
    .bind(conversation)
    .bind(workspace.as_uuid())
    .bind(member.as_uuid())
    .fetch_optional(&mut *tx)
    .await?;
    let current = explanation::current(&mut tx, conversation).await?;
    let explainable = seen.is_some() && explanation::explainable(&mut tx, conversation).await?;
    let (Some((inbox, subject)), Some(current), true) = (seen, current, explainable) else {
        sqlx::query(forget)
            .bind(workspace.as_uuid())
            .bind(member.as_uuid())
            .bind(conversation)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        return Ok(None);
    };
    let address: String =
        sqlx::query_scalar("SELECT address FROM conversations.inboxes WHERE id = $1")
            .bind(inbox)
            .fetch_one(&mut *tx)
            .await?;
    let owner = store::owner(&mut tx, inbox).await?;
    let zone = match owner {
        Some(owner) => {
            state
                .time_zones
                .time_zone(&mut tx, workspace, owner)
                .await?
        }
        None => None,
    };
    let rows: Vec<MessageRow> = sqlx::query_as(
        "SELECT m.id, m.direction, m.sender, m.subject, m.sent_at, m.bodies,
                r.texts ->> 'en'
           FROM conversations.messages m
           LEFT JOIN conversations.message_reads r
             ON r.message_id = m.id AND r.layer = 'model' AND r.state = 'read'
          WHERE m.conversation_id = $1 AND NOT m.in_spam AND NOT m.in_trash
          ORDER BY m.sent_at DESC, m.id DESC LIMIT $2",
    )
    .bind(conversation)
    .bind(i64::try_from(FULL_MESSAGES + EARLIER_MESSAGES).unwrap_or(200))
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;

    let locale = Locale::from_tag(&asked_locale).unwrap_or(Locale::En);
    let ws = workspace.to_string();
    let mut drafts: Vec<Draft> = Vec::with_capacity(rows.len());
    // Newest first, so the budget demotes the oldest last.
    for (position, (id, direction, sender, subject_line, sent_at, bodies, read)) in
        rows.into_iter().enumerate()
    {
        let sender: Address = serde_json::from_value(sender).unwrap_or(Address {
            name: None,
            address: String::new(),
        });
        let note = read
            .map(|text| plain_text::own_words(&text))
            .filter(|text| !text.trim().is_empty())
            .unwrap_or(subject_line);
        let text = if position < FULL_MESSAGES {
            full_text(state, &ws, bodies).await
        } else {
            None
        };
        drafts.push(Draft {
            id,
            from: shown_name(sender),
            sent_at,
            mine: direction == "outbound",
            text,
            note: cut_bytes(&note, NOTE_BYTES),
        });
    }
    drafts.reverse();
    let tz: chrono_tz::Tz = zone
        .as_deref()
        .and_then(|zone| zone.parse().ok())
        .unwrap_or(chrono_tz::UTC);
    let mut input = Input {
        you: You {
            name: reader.filter(|name| !name.trim().is_empty()),
            addresses: vec![address],
            shared: owner.is_none(),
        },
        today: Utc::now().with_timezone(&tz).date_naive().to_string(),
        time_zone: tz.name().to_owned(),
        language: prompt::language(locale),
        subject: cut_bytes(&subject, SHORT_BYTES),
        messages: Vec::new(),
    };
    let ids = fit(&mut input, drafts);
    let labels: Labels = ids
        .iter()
        .enumerate()
        .map(|(n, id)| (format!("m{}", n + 1), *id))
        .collect();
    let in_full = input.messages.iter().filter(|l| l.text.is_some()).count();
    Ok(Some(Work {
        inbox,
        current,
        locale,
        input,
        labels,
        ids,
        in_full,
        replaces,
    }))
}

/// A stored message: id, direction, sender, subject, sent, bodies, and
/// Scout's English summary of it when it has one.
type MessageRow = (
    MessageId,
    String,
    Value,
    String,
    DateTime<Utc>,
    Value,
    Option<String>,
);

/// A message on its way into the input.
struct Draft {
    id: MessageId,
    from: String,
    sent_at: DateTime<Utc>,
    mine: bool,
    /// Its own words, when they are read in full.
    text: Option<String>,
    /// Its summary, else its subject.
    note: String,
}

impl Draft {
    fn line(&self, label: usize) -> Line {
        let full = self.text.is_some();
        Line {
            label: format!("m{label}"),
            from: self.from.clone(),
            sent_at: self
                .sent_at
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            mine: self.mine,
            text: self.text.clone(),
            note: (!full).then(|| self.note.clone()),
        }
    }
}

/// A message's own words within [`FULL_BYTES`]; none when it has none or they
/// cannot be read (it is then handed on as a line).
async fn full_text(state: &AppState, workspace: &str, bodies: Value) -> Option<String> {
    let bodies: Bodies = serde_json::from_value(bodies).ok()?;
    match own_words(state, workspace, &bodies).await {
        Ok(words) => {
            let text = cut_bytes(&words, FULL_BYTES);
            (!text.trim().is_empty()).then_some(text)
        }
        Err(error) => {
            tracing::warn!(
                feature = FEATURE,
                error = error.code(),
                "a message's words could not be read"
            );
            None
        }
    }
}

/// Put `drafts` (oldest first) into `input` within [`INPUT_BYTES`]: the
/// oldest full message is demoted to a line first, then, if the lines alone
/// are too long, the oldest lines are left out. The messages are labelled
/// `m1…mN` in date order after that. Returns their ids in order.
fn fit(input: &mut Input, mut drafts: Vec<Draft>) -> Vec<MessageId> {
    let size = |drafts: &[Draft]| -> usize {
        drafts
            .iter()
            .enumerate()
            .map(|(n, draft)| {
                serde_json::to_string(&draft.line(n + 1)).map_or(0, |line| line.len())
            })
            .sum::<usize>()
            // The commas between lines, and the input around them.
            + drafts.len()
            + serde_json::to_string(&*input).map_or(0, |head| head.len())
    };
    while size(&drafts) > INPUT_BYTES {
        if let Some(oldest_full) = drafts.iter_mut().find(|draft| draft.text.is_some()) {
            oldest_full.text = None;
        } else if drafts.len() > 1 {
            drafts.remove(0);
        } else {
            break;
        }
    }
    input.messages = drafts
        .iter()
        .enumerate()
        .map(|(n, draft)| draft.line(n + 1))
        .collect();
    drafts.iter().map(|draft| draft.id).collect()
}

/// Store what the call came to in the member's row, tell their reader and
/// track it. A row that is no longer `writing` for this request (it was
/// deleted with the member's access, or asked again) is left as it is.
async fn keep(
    ctx: &mut JobContext,
    workspace: WorkspaceId,
    payload: ExplainThreadPayload,
    work: &Work,
    outcome: Outcome,
    started: Instant,
) -> Result<(), JobError> {
    let ExplainThreadPayload {
        member,
        conversation,
    } = payload;
    let events = ctx.state().events.clone();
    let tracker = ctx.state().tracker.clone();
    let tx = ctx.tx().await?;
    let version = work.current.version();
    let stored = match &outcome {
        Outcome::Ready(body, model) => sqlx::query(
            "UPDATE conversations.explanations
                    SET state = 'ready', failure = NULL, locale = $4, thread_version = $5,
                        message_ids = $6, in_full = $7, body = $8, model = $9,
                        written_at = now(), updated_at = now()
                  WHERE workspace_id = $1 AND member_id = $2 AND conversation_id = $3
                    AND state = 'writing' AND asked_locale = $4",
        )
        .bind(workspace.as_uuid())
        .bind(member.as_uuid())
        .bind(conversation)
        .bind(work.locale.tag())
        .bind(&version)
        .bind(work.ids.as_slice())
        .bind(i32::try_from(work.in_full).unwrap_or(i32::MAX))
        .bind(serde_json::to_value(body).unwrap_or(Value::Null))
        .bind(model.chars().take(120).collect::<String>())
        .execute(&mut **tx)
        .await?
        .rows_affected(),
        Outcome::Failed(failure) => sqlx::query(
            "UPDATE conversations.explanations
                    SET state = 'failed', failure = $5, updated_at = now()
                  WHERE workspace_id = $1 AND member_id = $2 AND conversation_id = $3
                    AND state = 'writing' AND asked_locale = $4",
        )
        .bind(workspace.as_uuid())
        .bind(member.as_uuid())
        .bind(conversation)
        .bind(work.locale.tag())
        .bind(failure.as_str())
        .execute(&mut **tx)
        .await?
        .rows_affected(),
    };
    if stored == 0 {
        return Ok(());
    }
    events
        .publish(
            tx,
            &ThreadExplained {
                workspace: workspace.to_string(),
                inbox: work.inbox,
                conversation,
                member,
            },
        )
        .await?;
    tracker
        .track(
            tx,
            &tracking::ExplanationCompleted {
                workspace,
                conversation_id: *conversation.as_uuid(),
                outcome: match &outcome {
                    Outcome::Ready(..) => tracking::ExplainOutcome::Ready,
                    Outcome::Failed(ExplanationFailure::Limit) => tracking::ExplainOutcome::Limit,
                    Outcome::Failed(ExplanationFailure::Unusable) => {
                        tracking::ExplainOutcome::Unusable
                    }
                    Outcome::Failed(ExplanationFailure::Unavailable) => {
                        tracking::ExplainOutcome::Unavailable
                    }
                },
                messages: match work.ids.len() {
                    0..=4 => tracking::ExplainMessages::Few,
                    5..=9 => tracking::ExplainMessages::Some,
                    10..=19 => tracking::ExplainMessages::Many,
                    _ => tracking::ExplainMessages::Long,
                },
                duration: match started.elapsed().as_secs() {
                    0..=4 => tracking::ExplainDuration::Quick,
                    5..=14 => tracking::ExplainDuration::Normal,
                    15..=39 => tracking::ExplainDuration::Slow,
                    _ => tracking::ExplainDuration::VerySlow,
                },
                replaced: work.replaces,
            },
        )
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests;
