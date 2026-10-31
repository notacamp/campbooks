//! A member's explanation of a thread (campbooks-cloud#407): what is kept,
//! who may be served it, and how it is shown.
//!
//! The explanation is private to the member who asked for it
//! (`conversations.explanations`, one row per member and conversation). Every
//! read re-checks, at that moment, that the member may read the conversation
//! and that every message the explanation was made from is still one of its
//! messages, in an inbox they can see; otherwise the answer is 404 and the row
//! is deleted. Nothing here reads the mail to decide anything: the version of
//! a thread is its newest message and its count, and `stale` compares them.

use std::collections::HashMap;

use campbooks_kernel::{
    Error, Result,
    tenancy::{Access, UserId},
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgConnection;
use utoipa::ToSchema;

use crate::{
    domain::access::visible_inboxes,
    model::{Address, ConversationId, InboxId, MessageId},
};

/// Days an explanation is kept after it was written.
pub const KEEP_DAYS: i32 = 30;
/// Minutes after which a request that never answered is asked again.
pub const WRITING_MINUTES: i32 = 10;
/// Longest "in short", in characters.
pub const MAX_IN_SHORT: usize = 400;
/// Longest point, in characters.
pub const MAX_POINT: usize = 300;
/// Most entries in each list of an explanation.
pub const MAX_ENTRIES: usize = 6;
/// Most messages one point cites.
pub const MAX_REFS: usize = 3;
/// Longest sender name shown on a chip, in characters.
const MAX_SENDER: usize = 120;

/// Where an explanation is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExplanationState {
    /// Asked for; the answer is not stored yet.
    Writing,
    /// Written.
    Ready,
    /// The last request gave no explanation (see [`Failure`]); an earlier one
    /// is kept when there was one.
    Failed,
}

impl ExplanationState {
    /// As stored in `explanations.state`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Writing => "writing",
            Self::Ready => "ready",
            Self::Failed => "failed",
        }
    }

    fn parse(text: &str) -> Self {
        match text {
            "ready" => Self::Ready,
            "failed" => Self::Failed,
            _ => Self::Writing,
        }
    }
}

/// Why a request gave no explanation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExplanationFailure {
    /// The workspace used its whole daily AI limit.
    Limit,
    /// The model gave no usable answer.
    Unusable,
    /// The model could not be reached.
    Unavailable,
}

impl ExplanationFailure {
    /// As stored in `explanations.failure`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Limit => "limit",
            Self::Unusable => "unusable",
            Self::Unavailable => "unavailable",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        match text {
            "limit" => Some(Self::Limit),
            "unusable" => Some(Self::Unusable),
            "unavailable" => Some(Self::Unavailable),
            _ => None,
        }
    }
}

/// What someone in the thread wants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ExplanationWant {
    /// Who wants it, as the thread names them.
    pub who: String,
    /// What they want: one plain line.
    pub what: String,
    /// The messages it comes from (one to three).
    pub refs: Vec<MessageId>,
}

/// Something agreed, or still open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ExplanationPoint {
    /// One plain line.
    pub text: String,
    /// The messages it comes from (one to three).
    pub refs: Vec<MessageId>,
}

/// Something on the member.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ExplanationDuty {
    /// One plain line.
    pub text: String,
    /// The day it is due, as the thread states it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_on: Option<NaiveDate>,
    /// The messages it comes from (one to three).
    pub refs: Vec<MessageId>,
}

/// The explanation itself: plain text only, no Markdown, no links.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Explanation {
    /// The thread in a sentence or two.
    pub in_short: String,
    /// Who wants what.
    pub wants: Vec<ExplanationWant>,
    /// What was agreed.
    pub agreed: Vec<ExplanationPoint>,
    /// What is still open.
    pub open: Vec<ExplanationPoint>,
    /// What is on the member.
    pub on_you: Vec<ExplanationDuty>,
}

impl Explanation {
    /// Every message cited anywhere in it, each once.
    pub fn cited(&self) -> Vec<MessageId> {
        let mut ids: Vec<MessageId> = Vec::new();
        let all = self
            .wants
            .iter()
            .flat_map(|w| &w.refs)
            .chain(self.agreed.iter().flat_map(|p| &p.refs))
            .chain(self.open.iter().flat_map(|p| &p.refs))
            .chain(self.on_you.iter().flat_map(|d| &d.refs));
        for id in all {
            if !ids.contains(id) {
                ids.push(*id);
            }
        }
        ids
    }
}

/// A message an explanation cites, for the chip beside a point.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ExplanationCited {
    /// The message.
    pub id: MessageId,
    /// Who sent it: their name, else their address.
    pub from: String,
    /// When it was sent.
    pub sent_at: DateTime<Utc>,
}

/// The newest message, when the thread has moved on since the explanation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ExplanationNewer {
    /// Who sent it: their name, else their address.
    pub from: String,
    /// When it was sent.
    pub sent_at: DateTime<Utc>,
}

/// The member's explanation of a conversation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "the wire shape: independent facts"
)]
pub struct ExplanationView {
    /// Where it is.
    pub state: ExplanationState,
    /// Why the last request gave no explanation, when it did not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<ExplanationFailure>,
    /// The explanation kept: none while the first is being written, and none
    /// after a first request that failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<Explanation>,
    /// The language it is written in (`en`, `pt-PT`, `es-ES`, `fr`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    /// When it was written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub written_at: Option<DateTime<Utc>>,
    /// How many messages it was made from.
    pub messages: u32,
    /// How many of them were read in full; the earlier ones came from
    /// Scout's notes.
    pub in_full: u32,
    /// The thread has a newer message, or the member reads another language
    /// now. Nothing is explained again on its own.
    pub stale: bool,
    /// The newest message, when the thread has moved on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub newer: Option<ExplanationNewer>,
    /// The messages the explanation cites, for its chips.
    pub refs: Vec<ExplanationCited>,
}

/// A conversation's newest message and how many messages it has (spam and
/// the trash left out): what the explanation is made from.
#[derive(Debug, Clone, PartialEq)]
pub struct Current {
    /// The newest message.
    pub newest: MessageId,
    /// When it was sent.
    pub sent_at: DateTime<Utc>,
    /// Who sent it.
    pub sender: Address,
    /// How many messages.
    pub count: i64,
}

impl Current {
    /// `"<newest id>:<count>"`, stored as `thread_version`.
    pub fn version(&self) -> String {
        format!("{}:{}", self.newest, self.count)
    }
}

/// The newest message of `conversation` and its count; none when it has no
/// message outside spam and the trash.
pub async fn current(
    conn: &mut PgConnection,
    conversation: ConversationId,
) -> Result<Option<Current>> {
    let row: Option<(MessageId, DateTime<Utc>, Value, i64)> = sqlx::query_as(
        "SELECT id, sent_at, sender,
                (SELECT count(*) FROM conversations.messages
                  WHERE conversation_id = $1 AND NOT in_spam AND NOT in_trash)
           FROM conversations.messages
          WHERE conversation_id = $1 AND NOT in_spam AND NOT in_trash
          ORDER BY sent_at DESC, id DESC LIMIT 1",
    )
    .bind(conversation)
    .fetch_optional(&mut *conn)
    .await?;
    Ok(row.map(|(newest, sent_at, sender, count)| Current {
        newest,
        sent_at,
        sender: serde_json::from_value(sender).unwrap_or(Address {
            name: None,
            address: String::new(),
        }),
        count,
    }))
}

/// Whether `conversation` may be explained: not in spam or the trash, its
/// inbox not kept read-only, with a message, and no phishing warning on it
/// (an open need whose question is `suspicious`, or a `suspicious` answer
/// of the decide pass), because an explanation would retell the scam in
/// Scout's voice.
pub async fn explainable(conn: &mut PgConnection, conversation: ConversationId) -> Result<bool> {
    let ok: Option<bool> = sqlx::query_scalar(
        "SELECT NOT c.trashed
            AND i.detached_at IS NULL
            AND EXISTS (SELECT 1 FROM conversations.messages m
                         WHERE m.conversation_id = c.id AND NOT m.in_spam AND NOT m.in_trash)
            AND NOT EXISTS (SELECT 1 FROM conversations.needs n
                             WHERE n.conversation_id = c.id AND n.state = 'open'
                               AND n.question = 'suspicious')
            AND NOT EXISTS (SELECT 1 FROM conversations.decisions d
                             WHERE d.conversation_id = c.id AND d.question = 'suspicious'
                               AND d.value = 'true'::jsonb)
           FROM conversations.conversations c
           JOIN conversations.inboxes i ON i.id = c.inbox_id
          WHERE c.id = $1",
    )
    .bind(conversation)
    .fetch_optional(&mut *conn)
    .await?;
    Ok(ok.unwrap_or(false))
}

/// A stored explanation row.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Row {
    /// `writing`, `ready` or `failed`.
    pub state: String,
    /// Why the last request failed.
    pub failure: Option<String>,
    /// The thread version the last request was accepted for.
    pub asked_version: String,
    /// The language the last request was made in.
    pub asked_locale: String,
    /// The language of the kept explanation.
    pub locale: Option<String>,
    /// The thread version the kept explanation was made from.
    pub thread_version: Option<String>,
    /// Every message the kept explanation was made from.
    pub message_ids: Vec<MessageId>,
    /// How many of them in full.
    pub in_full: i32,
    /// The kept explanation.
    pub body: Option<Value>,
    /// When it was written.
    pub written_at: Option<DateTime<Utc>>,
    /// When the row last changed.
    pub updated_at: DateTime<Utc>,
}

/// The member's row for a conversation (`$1` workspace, `$2` member, `$3`
/// conversation).
pub const SELECT: &str = "SELECT state, failure, asked_version, asked_locale, locale, \
     thread_version, message_ids, in_full, body, written_at, updated_at \
     FROM conversations.explanations \
     WHERE workspace_id = $1 AND member_id = $2 AND conversation_id = $3";

/// [`SELECT`], the row locked.
pub const SELECT_LOCKED: &str = "SELECT state, failure, asked_version, asked_locale, locale, \
     thread_version, message_ids, in_full, body, written_at, updated_at \
     FROM conversations.explanations \
     WHERE workspace_id = $1 AND member_id = $2 AND conversation_id = $3 FOR UPDATE";

/// The newest message id of a stored `thread_version`.
fn newest_of(version: &str) -> &str {
    version.split(':').next().unwrap_or_default()
}

/// The message count of a stored `thread_version`.
fn count_of(version: &str) -> i64 {
    version
        .rsplit(':')
        .next()
        .and_then(|n| n.parse().ok())
        .unwrap_or_default()
}

/// A sender as a chip names them: their name, else their address, capped.
fn shown(sender: &Value) -> String {
    let address: Option<Address> = serde_json::from_value(sender.clone()).ok();
    let name = address.map_or_else(String::new, |a| {
        a.name
            .filter(|name| !name.trim().is_empty())
            .unwrap_or(a.address)
    });
    campbooks_kernel::text::displayable(&name)
        .chars()
        .take(MAX_SENDER)
        .collect::<String>()
        .trim()
        .to_owned()
}

/// `row` as the member sees it now: stale when the thread has a newer
/// message or the member reads another language, with the sender and time of
/// the newest message, and the messages its points cite. The cited messages
/// are read only within `inboxes`.
pub async fn view(
    conn: &mut PgConnection,
    row: &Row,
    now: Option<&Current>,
    language: &str,
    inboxes: &[InboxId],
    conversation: ConversationId,
) -> Result<ExplanationView> {
    let body: Option<Explanation> = row
        .body
        .clone()
        .and_then(|body| serde_json::from_value(body).ok());
    let moved = match (row.thread_version.as_deref(), now) {
        (Some(kept), Some(now)) => {
            now.newest.to_string() != newest_of(kept) || now.count > count_of(kept)
        }
        _ => false,
    };
    let other_language = body.is_some() && row.locale.as_deref() != Some(language);
    let refs = match &body {
        Some(body) => cited(conn, &body.cited(), inboxes, conversation).await?,
        None => Vec::new(),
    };
    Ok(ExplanationView {
        state: ExplanationState::parse(&row.state),
        failure: row.failure.as_deref().and_then(ExplanationFailure::parse),
        locale: body.as_ref().and(row.locale.clone()),
        written_at: body.as_ref().and(row.written_at),
        messages: u32::try_from(row.message_ids.len()).unwrap_or(u32::MAX),
        in_full: u32::try_from(row.in_full).unwrap_or_default(),
        stale: body.is_some() && (moved || other_language),
        newer: now
            .filter(|_| body.is_some() && moved)
            .map(|now| ExplanationNewer {
                from: shown(&serde_json::to_value(&now.sender).unwrap_or(Value::Null)),
                sent_at: now.sent_at,
            }),
        body,
        refs,
    })
}

/// The messages `ids` as chips: their sender and date, only those still in
/// `conversation` and in an inbox in `inboxes`.
async fn cited(
    conn: &mut PgConnection,
    ids: &[MessageId],
    inboxes: &[InboxId],
    conversation: ConversationId,
) -> Result<Vec<ExplanationCited>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows: Vec<(MessageId, Value, DateTime<Utc>)> = sqlx::query_as(
        "SELECT id, sender, sent_at FROM conversations.messages
          WHERE id = ANY($1) AND conversation_id = $2 AND inbox_id = ANY($3)",
    )
    .bind(ids)
    .bind(conversation)
    .bind(inboxes)
    .fetch_all(&mut *conn)
    .await?;
    let mut by_id: HashMap<MessageId, ExplanationCited> = rows
        .into_iter()
        .map(|(id, sender, sent_at)| {
            (
                id,
                ExplanationCited {
                    id,
                    from: shown(&sender),
                    sent_at,
                },
            )
        })
        .collect();
    Ok(ids.iter().filter_map(|id| by_id.remove(id)).collect())
}

/// Whether every message in `ids` is still a message of `conversation`, in
/// an inbox in `inboxes`.
pub async fn messages_still_there(
    conn: &mut PgConnection,
    ids: &[MessageId],
    inboxes: &[InboxId],
    conversation: ConversationId,
) -> Result<bool> {
    if ids.is_empty() {
        return Ok(true);
    }
    let found: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM conversations.messages
          WHERE id = ANY($1) AND conversation_id = $2 AND inbox_id = ANY($3)",
    )
    .bind(ids)
    .bind(conversation)
    .bind(inboxes)
    .fetch_one(&mut *conn)
    .await?;
    let wanted = ids.iter().collect::<std::collections::HashSet<_>>().len();
    Ok(usize::try_from(found).unwrap_or_default() == wanted)
}

/// Forget `member`'s explanation of `conversation`.
pub async fn forget(
    conn: &mut PgConnection,
    access: &Access,
    member: UserId,
    conversation: ConversationId,
) -> Result<u64> {
    Ok(sqlx::query(
        "DELETE FROM conversations.explanations
          WHERE workspace_id = $1 AND member_id = $2 AND conversation_id = $3",
    )
    .bind(access.workspace.as_uuid())
    .bind(member.as_uuid())
    .bind(conversation)
    .execute(&mut *conn)
    .await?
    .rows_affected())
}

/// The member asking and the inboxes they can see, given the answer of the
/// central access check (`allowed`, the handler's own
/// `resources::authorize` of a read of `conversation`). When they may not
/// read the conversation (it is not theirs to see any more), their
/// explanation of it is deleted, and the answer is 404. A caller acting for
/// no member finds nothing. Any other refusal is passed on.
///
/// On `Err(Error::NotFound)` the deletion has been done in `conn`; the caller
/// commits before answering so it sticks.
pub async fn member_sees(
    conn: &mut PgConnection,
    access: &Access,
    conversation: ConversationId,
    allowed: Result<()>,
) -> Result<(UserId, Vec<InboxId>)> {
    let user = access.user.ok_or(Error::NotFound)?;
    let inboxes = match allowed {
        Ok(()) => visible_inboxes(conn, access).await?,
        Err(Error::NotFound) => Vec::new(),
        Err(other) => return Err(other),
    };
    let visible: Option<i32> = sqlx::query_scalar(
        "SELECT 1 FROM conversations.conversations WHERE id = $1 AND inbox_id = ANY($2)",
    )
    .bind(conversation)
    .bind(inboxes.as_slice())
    .fetch_optional(&mut *conn)
    .await?;
    if visible.is_none() {
        forget(conn, access, user, conversation).await?;
        return Err(Error::NotFound);
    }
    Ok((user, inboxes))
}
