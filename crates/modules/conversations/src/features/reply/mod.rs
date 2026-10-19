//! `POST …/conversations/{conversation_id}/reply/suggestions` and
//! `POST …/conversations/{conversation_id}/reply/draft` — Scout proposes a
//! reply (campbooks-cloud#168).
//!
//! Only on the member's own action (Reply, then a pick). The `complete` role
//! reads the conversation's newest messages up to the one answered, as
//! plain text without quoted history, with the same checks as reading the
//! conversation, and only from someone who may send from its inbox. It
//! proposes text in a strict schema with no tools; nothing is stored, and
//! nothing is sent until the member sends a draft through
//! `conversations.create_draft` and `conversations.send_draft`.
//!
//! A written reply also learns who is answered and how the member writes to
//! them (campbooks-cloud#271): the conversation's length, the person's
//! name, how much mail passed between them, and a few of the same inbox's
//! earlier messages to them, for the register only.

mod prompt;
mod types;

use std::time::{Duration, Instant};

use axum::{Extension, Json, extract::State};
use campbooks_ai::{
    Gateway, Role,
    decide::signals::facts::{dmarc_refused, sender_checks_failed},
    usage::Scope,
};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, RecordRef, Result, Risk,
    authz::Action,
    db::BeginFor as _,
    extract,
    i18n::Locale,
    rate_limit::{Limit, RateKey},
    tenancy::{Workspace, WorkspaceId},
};
use chrono::{DateTime, SecondsFormat, Utc};
use serde::de::DeserializeOwned;
use serde_json::Value;

use self::prompt::{Ask, Author, Context, PastMessage, Person, Them, Thread, ThreadMessage};
use self::types::{ReplySuggestions, ReplyText, SuggestReplies, WriteReply};
use crate::{
    domain::{
        access::visible_inboxes,
        blobs::{Budget, words},
        paths::ConversationPath,
        plain_text::{self, Quotes},
        resources,
    },
    events::CONVERSATION_RECORD,
    features::get_conversation::Bodies,
    model::{Address, ConversationId, InboxId, MessageId},
    scopes::ConversationsSend,
};

/// Model calls per member per hour, suggestions and drafts together.
const LIMIT: Limit = Limit::new("conversations.reply", 100, Duration::from_hours(1));
/// Messages of the conversation the model reads, ending with the one
/// answered.
const THREAD_MESSAGES: i64 = 8;
/// Most of a message's own words handed on, in bytes.
const MESSAGE_BYTES: usize = 4 * 1024;
/// Most of the conversation handed on, in bytes.
const THREAD_BYTES: usize = 16 * 1024;
/// Most bytes of a subject or a sender's name sent to the model: one long
/// header must not grow every call on the conversation.
const NAME_BYTES: usize = 300;
/// Most of a text part read before quoted history is dropped, in bytes.
const RAW_TEXT_BYTES: usize = 64 * 1024;
/// Most of an HTML part read, in bytes.
const RAW_HTML_BYTES: usize = 256 * 1024;
/// Longest intent or change the member may ask for, in characters.
const MAX_ASK_CHARS: usize = 300;
/// Longest draft the member may ask to rewrite, in characters.
const MAX_DRAFT_CHARS: usize = 20_000;
/// The inbox's newest sent messages looked through for mail to the person
/// answered. Served by `messages_outbound_idx`.
const SENT_SCAN: i64 = 500;
/// Earlier messages to the person answered handed on, at most.
const PAST_MESSAGES: usize = 3;
/// Most of one earlier message handed on, in bytes.
const PAST_MESSAGE_BYTES: usize = 1536;
/// Most of all the earlier messages handed on, in bytes.
const PAST_BYTES: usize = 4 * 1024;
/// Each mail count sent to the model stops here.
const COUNT_CAP: i64 = 100;
/// Messages counted in a conversation, at most.
const TOTAL: &str = "SELECT count(*) FROM ( \
         SELECT 1 FROM conversations.messages WHERE conversation_id = $1 LIMIT $2) counted";
/// Their mail to the inbox in the last year, capped at `$3`. Served by
/// `messages_inbound_sender_idx`.
const FROM_THEM: &str = "SELECT count(*) FROM ( \
         SELECT 1 FROM conversations.messages \
         WHERE inbox_id = $1 AND direction = 'inbound' AND sender_address = $2 \
           AND NOT in_spam AND NOT in_trash AND sent_at > now() - interval '365 days' \
         LIMIT $3) recent";
/// The inbox's newest sent messages (`$3` of them), each marked when it went to
/// or copied `$2`, followed by `$rest`: a literal, so the SQL stays static.
macro_rules! sent_to_them {
    ($rest:literal) => {
        concat!(
            "WITH sent AS ( \
                 SELECT id, conversation_id, sent_at, in_spam, in_trash, bodies, \
                        EXISTS (SELECT 1 FROM jsonb_array_elements( \
                            CASE WHEN jsonb_typeof(recipients->'to') = 'array' \
                                 THEN recipients->'to' ELSE '[]'::jsonb END || \
                            CASE WHEN jsonb_typeof(recipients->'cc') = 'array' \
                                 THEN recipients->'cc' ELSE '[]'::jsonb END) r \
                            WHERE lower(r->>'address') = $2::text) AS to_them \
                 FROM conversations.messages \
                 WHERE inbox_id = $1 AND direction = 'outbound' \
                 ORDER BY sent_at DESC, id DESC LIMIT $3) ",
            $rest
        )
    };
}
/// Whether the inbox ever wrote to `$2` (a lower-cased address) in its
/// newest `$3` sent messages, and how many such messages went in the last
/// year (capped at `$4`). The address must equal the `address` of a To or
/// Cc recipient, never a name or any other text of the row.
const WROTE_TO_THEM: &str = sent_to_them!(
    "SELECT coalesce(bool_or(to_them), false), \
            LEAST(count(*) FILTER (WHERE to_them AND NOT in_spam AND NOT in_trash \
                AND sent_at > now() - interval '365 days'), $4) \
     FROM sent"
);
/// The newest of those sent messages from other conversations (`$4`),
/// within the last year, not in the trash; `$5` at most.
const PAST: &str = sent_to_them!(
    "SELECT sent_at, bodies FROM sent \
     WHERE to_them AND conversation_id <> $4 AND NOT in_trash \
       AND sent_at > now() - interval '365 days' \
     ORDER BY sent_at DESC, id DESC LIMIT $5"
);

/// A row of [`THREAD`].
type MessageRow = (MessageId, String, Value, String, DateTime<Utc>, Value);
/// Sent messages read for their words: when, and their bodies.
type PastRows = Vec<(DateTime<Utc>, Value)>;
/// A row of [`ANSWERED`].
type AnsweredRow = (
    MessageId,
    DateTime<Utc>,
    String,
    String,
    Value,
    bool,
    Value,
    bool,
);

/// The AI gateway, when one is composed.
#[derive(Debug, Clone)]
pub struct ReplyWriter {
    ai: Option<Gateway>,
}

impl ReplyWriter {
    /// Writing through `ai`'s `complete` role.
    pub fn new(ai: Option<Gateway>) -> Self {
        Self { ai }
    }

    /// The gateway, if its `complete` role is configured and may receive
    /// mail; otherwise 409 `reply_unavailable` (the member writes the reply
    /// themselves), never the gateway's own words.
    fn gateway(&self) -> Result<&Gateway> {
        self.ai
            .as_ref()
            .filter(|gateway| {
                gateway
                    .config()
                    .provider_for_content(Role::Complete)
                    .is_ok()
            })
            .ok_or_else(|| Error::Conflict(crate::MESSAGES.reason("reply_unavailable")))
    }
}

/// Likely replies to a message, as short intents.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/conversations/{conversation_id}/reply/suggestions",
    operation_id = "conversations.suggest_replies",
    tag = "conversations",
    security(("bearer" = ["conversations:send"])),
    // A command: it saves nothing, but it spends a model call and the
    // member's hourly allowance, so it is never replayed or prefetched.
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).records(["conversations.message"]).access("conversations.inbox", campbooks_kernel::authz::Action::Send)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("conversation_id" = ConversationId, Path, description = "The conversation."),
    ),
    request_body = SuggestReplies,
    responses(
        (status = OK, description = "Likely replies; nothing is saved.", body = ReplySuggestions),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may only read this inbox (`inbox_read_only`).",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such conversation or message, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = TOO_MANY_REQUESTS, description = "Too many replies asked for this hour.", body = Problem,
            content_type = "application/problem+json"),
        (status = CONFLICT, description = "No language model may read mail here (`reply_unavailable`): write the reply yourself.",
            body = Problem, content_type = "application/problem+json"),
        (status = SERVICE_UNAVAILABLE, description = "The language model failed (`reply_failed`), \
            or the workspace used its whole daily AI limit (`ai_daily_limit_reached`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn suggest(
    State(state): State<AppState>,
    Extension(writer): Extension<ReplyWriter>,
    workspace: Workspace<ConversationsSend>,
    locale: Locale,
    extract::Path(path): extract::Path<ConversationPath>,
    extract::Json(body): extract::Json<SuggestReplies>,
) -> Result<Json<ReplySuggestions>> {
    let started = Instant::now();
    let read = prepare(
        &state,
        &writer,
        &workspace,
        path.conversation_id,
        body.message_id,
        false,
    )
    .await?;
    let context = read.context(locale);
    let answer: prompt::Intents = ask(
        &state,
        read.gateway,
        workspace.id,
        "conversations.suggest_replies",
        prompt::suggest(&context),
    )
    .await?;
    let intents = prompt::intents(answer);
    tracing::info!(
        intents = intents.len(),
        messages = context.conversation.messages.len(),
        latency_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        "reply suggestions"
    );
    Ok(Json(ReplySuggestions {
        message_id: read.message,
        intents,
    }))
}

/// Scout's reply for an intent, or your draft rewritten.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/conversations/{conversation_id}/reply/draft",
    operation_id = "conversations.write_reply",
    tag = "conversations",
    security(("bearer" = ["conversations:send"])),
    // A command, like `suggest_replies`: nothing is saved or sent.
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).records(["conversations.message"]).access("conversations.inbox", campbooks_kernel::authz::Action::Send)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("conversation_id" = ConversationId, Path, description = "The conversation."),
    ),
    request_body = WriteReply,
    responses(
        (status = OK, description = "The reply as plain text; nothing is saved or sent.", body = ReplyText),
        (status = BAD_REQUEST, description = "No intent, or an intent, change or draft too long (`reply_ask_invalid`).",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may only read this inbox (`inbox_read_only`).",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such conversation or message, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = TOO_MANY_REQUESTS, description = "Too many replies asked for this hour.", body = Problem,
            content_type = "application/problem+json"),
        (status = CONFLICT, description = "No language model may read mail here (`reply_unavailable`): write the reply yourself.",
            body = Problem, content_type = "application/problem+json"),
        (status = SERVICE_UNAVAILABLE, description = "The language model failed (`reply_failed`), \
            or the workspace used its whole daily AI limit (`ai_daily_limit_reached`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn write(
    State(state): State<AppState>,
    Extension(writer): Extension<ReplyWriter>,
    workspace: Workspace<ConversationsSend>,
    locale: Locale,
    extract::Path(path): extract::Path<ConversationPath>,
    extract::Json(body): extract::Json<WriteReply>,
) -> Result<Json<ReplyText>> {
    let started = Instant::now();
    let intent = body.intent.trim();
    let change = body
        .change
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty());
    let draft = body.draft.as_deref().filter(|d| !d.trim().is_empty());
    let too_long = |text: &str, max: usize| text.chars().count() > max;
    if intent.is_empty()
        || too_long(intent, MAX_ASK_CHARS)
        || change.is_some_and(|c| too_long(c, MAX_ASK_CHARS))
        || draft.is_some_and(|d| too_long(d, MAX_DRAFT_CHARS))
    {
        return Err(Error::BadRequest(
            crate::MESSAGES
                .reason("reply_ask_invalid")
                .arg("max", MAX_ASK_CHARS),
        ));
    }
    let read = prepare(
        &state,
        &writer,
        &workspace,
        path.conversation_id,
        body.message_id,
        true,
    )
    .await?;
    let context = read.context(locale);
    let my_name = workspace
        .principal
        .name
        .as_deref()
        .or(read.inbox_name.as_deref())
        .map(str::trim)
        .filter(|name| !name.is_empty());
    let rewriting = draft.is_some() && (change.is_some() || body.shorter);
    let asked = Ask {
        intent,
        my_name,
        draft: if rewriting { draft } else { None },
        change: if rewriting { change } else { None },
        shorter: rewriting && body.shorter,
    };
    let answer: prompt::Reply = ask(
        &state,
        read.gateway,
        workspace.id,
        "conversations.write_reply",
        prompt::write(&context, &read.person, &asked),
    )
    .await?;
    let text = prompt::reply(&answer.reply)
        .ok_or_else(|| Error::Unavailable(crate::MESSAGES.reason("reply_failed")))?;
    tracing::info!(
        rewrite = rewriting,
        messages = context.conversation.messages.len(),
        latency_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        "reply written"
    );
    Ok(Json(ReplyText {
        message_id: read.message,
        text,
    }))
}

/// What a reply is written from.
struct Read<'a> {
    gateway: &'a Gateway,
    /// The message answered.
    message: MessageId,
    /// The inbox's display name, for the signature when the member's token
    /// carries none.
    inbox_name: Option<String>,
    thread: Thread,
    /// Who is answered and how the member writes to them; empty for
    /// suggestions.
    person: Person,
}

impl Read<'_> {
    fn context(&self, locale: Locale) -> Context {
        Context {
            replying_to: self.thread.messages.len().saturating_sub(1),
            conversation: self.thread.clone(),
            my_language: locale.tag(),
        }
    }
}

/// Check the caller may read the conversation and send from its inbox,
/// that a model may read mail here and the member has allowance left, then
/// read the conversation up to the message answered, and, for a written
/// reply (`with_person`), who is answered.
async fn prepare<'a>(
    state: &AppState,
    writer: &'a ReplyWriter,
    workspace: &Workspace<ConversationsSend>,
    conversation: ConversationId,
    requested: Option<MessageId>,
    with_person: bool,
) -> Result<Read<'a>> {
    let access = &workspace.access;
    let (inbox_id, inbox_name) = {
        let mut tx = state.db.begin_for(access.workspace).await?;
        resources::authorize(
            state,
            &mut tx,
            access,
            Action::Read,
            &RecordRef::new(CONVERSATION_RECORD, &conversation),
        )
        .await?;
        let inboxes = visible_inboxes(&mut tx, access).await?;
        let found: Option<(InboxId, Option<String>)> = sqlx::query_as(INBOX)
            .bind(conversation)
            .bind(inboxes.as_slice())
            .fetch_optional(&mut *tx)
            .await?;
        let (inbox, name) = found.ok_or(Error::NotFound)?;
        // A reader, or an inbox kept read-only: 403 `inbox_read_only`.
        resources::authorize(
            state,
            &mut tx,
            access,
            Action::Send,
            &resources::inbox(inbox),
        )
        .await?;
        tx.commit().await?;
        (inbox, name)
    };
    let gateway = writer.gateway()?;
    let who = access.user.map_or_else(
        || workspace.principal.subject.clone(),
        |user| user.to_string(),
    );
    state
        .rate_limits
        .check(
            &state.db,
            &LIMIT,
            &RateKey::text(&format!("{}:{who}", access.workspace)),
        )
        .await?;

    let mut tx = state.db.begin_for(access.workspace).await?;
    let answered: Option<AnsweredRow> = sqlx::query_as(ANSWERED)
        .bind(conversation)
        .bind(requested)
        .fetch_optional(&mut *tx)
        .await?;
    let (message, sent_at, direction, sender_address, recipients, in_spam, headers, suspicious) =
        answered.ok_or(Error::NotFound)?;
    let answered = Answered {
        distrusted: distrusted(&direction, &sender_address, in_spam, &headers, suspicious),
        direction,
        sender_address,
        recipients,
    };
    let rows: Vec<MessageRow> = sqlx::query_as(THREAD)
        .bind(conversation)
        .bind(sent_at)
        .bind(message)
        .bind(THREAD_MESSAGES)
        .fetch_all(&mut *tx)
        .await?;
    let known = if with_person {
        Some(known_person(&mut tx, inbox_id, conversation, &answered).await?)
    } else {
        None
    };
    tx.commit().await?;

    let ws = access.workspace.to_string();
    let thread = read_thread(state, &ws, rows).await?;
    let person = match known {
        Some(known) => known.read(state, &ws).await?,
        None => Person::default(),
    };
    Ok(Read {
        gateway,
        message,
        inbox_name,
        thread,
        person,
    })
}

/// The thread as the model reads it: newest first within the budget, then
/// oldest first.
async fn read_thread(state: &AppState, ws: &str, rows: Vec<MessageRow>) -> Result<Thread> {
    let subject = rows
        .first()
        .map(|row| cut_bytes(&row.3, NAME_BYTES))
        .unwrap_or_default();
    let mut messages = Vec::with_capacity(rows.len());
    let mut budget = THREAD_BYTES;
    for (_, direction, sender, _, sent, bodies) in rows {
        if budget == 0 {
            break;
        }
        let bodies: Bodies = serde_json::from_value(bodies)
            .map_err(|e| Error::internal(crate::domain::redact::json(&e)))?;
        let sender: Address = serde_json::from_value(sender)
            .map_err(|e| Error::internal(crate::domain::redact::json(&e)))?;
        let text = cut_bytes(
            &own_words(state, ws, &bodies).await?,
            MESSAGE_BYTES.min(budget),
        );
        budget = budget.saturating_sub(text.len());
        messages.push(ThreadMessage {
            from: if direction == "outbound" {
                Author::Me
            } else {
                Author::Them
            },
            name: shown_name(sender),
            sent_at: sent.to_rfc3339_opts(SecondsFormat::Secs, true),
            text,
        });
    }
    messages.reverse();
    Ok(Thread { subject, messages })
}

/// What is read about the person answered, before the bodies are.
struct Known {
    total_messages: usize,
    them: Option<(Them, PastRows)>,
}

impl Known {
    /// The person as the model reads them: the earlier messages reduced to
    /// their own words, within [`PAST_MESSAGE_BYTES`] each and [`PAST_BYTES`]
    /// in all, newest kept first and handed on oldest first.
    async fn read(self, state: &AppState, workspace: &str) -> Result<Person> {
        let Some((them, rows)) = self.them else {
            return Ok(Person {
                total_messages: self.total_messages,
                ..Person::default()
            });
        };
        let mut past = Vec::with_capacity(PAST_MESSAGES);
        let mut budget = PAST_BYTES;
        for (sent, bodies) in rows {
            if past.len() == PAST_MESSAGES || budget == 0 {
                break;
            }
            let bodies: Bodies = serde_json::from_value(bodies)
                .map_err(|e| Error::internal(crate::domain::redact::json(&e)))?;
            let text = cut_bytes(
                &own_words(state, workspace, &bodies).await?,
                PAST_MESSAGE_BYTES.min(budget),
            );
            if text.trim().is_empty() {
                continue;
            }
            budget = budget.saturating_sub(text.len());
            past.push(PastMessage {
                sent_at: sent.to_rfc3339_opts(SecondsFormat::Secs, true),
                text,
            });
        }
        past.reverse();
        Ok(Person {
            total_messages: self.total_messages,
            them: Some(them),
            past,
        })
    }
}

/// The message answered, as far as the person is concerned.
struct Answered {
    direction: String,
    sender_address: String,
    recipients: Value,
    /// In spam, flagged suspicious, or failing its sender's checks: it is
    /// no reason to open the member's earlier mail to that address.
    distrusted: bool,
}

/// Whether the message answered earns no person context. Facts only, as
/// everywhere else (ADR 0018): spam, a `suspicious` answer or open Review
/// need, and, for mail received, the receiving provider's results saying the
/// sender failed its checks or is impersonated.
fn distrusted(
    direction: &str,
    sender: &str,
    in_spam: bool,
    headers: &Value,
    suspicious: bool,
) -> bool {
    if in_spam || suspicious {
        return true;
    }
    let results = headers
        .get("authentication_results")
        .and_then(Value::as_str);
    direction == "inbound" && (sender_checks_failed(sender, results) || dmarc_refused(results))
}

/// Whether `address` is plain enough to look up: one `@` with something on
/// both sides, and no quote, backslash, whitespace or control character.
fn plain_address(address: &str) -> bool {
    let mut parts = address.split('@');
    let (Some(local), Some(domain), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    !local.is_empty()
        && !domain.is_empty()
        && !address
            .chars()
            .any(|c| c == '"' || c == '\\' || c.is_whitespace() || c.is_control())
}

/// The conversation's length and what the inbox knows of the person
/// answered: the other side of the message answered (the sender of an
/// inbound one, the first recipient of one the member sent). Nothing of the
/// person when the message is distrusted or their address is not plain.
///
/// Only the conversation's own inbox is read, never another one the member
/// can see: it is the inbox the reply is sent from, which the caller was
/// just authorized to read and send from.
async fn known_person(
    tx: &mut sqlx::PgConnection,
    inbox: InboxId,
    conversation: ConversationId,
    answered: &Answered,
) -> Result<Known> {
    let Answered {
        direction,
        sender_address,
        recipients,
        distrusted,
    } = answered;
    let total: i64 = sqlx::query_scalar(TOTAL)
        .bind(conversation)
        .bind(TOTAL_CAP)
        .fetch_one(&mut *tx)
        .await?;
    let total_messages = usize::try_from(total).unwrap_or_default();
    let first_to = recipients
        .pointer("/to/0")
        .and_then(|to| serde_json::from_value::<Address>(to.clone()).ok());
    let correspondent = if direction == "outbound" {
        first_to
    } else {
        // Their name, from the newest message they sent in this
        // conversation; the address alone when none gave one.
        let sender: Option<Value> = sqlx::query_scalar(
            "SELECT sender FROM conversations.messages \
             WHERE conversation_id = $1 AND direction = 'inbound' AND sender_address = $2 \
             ORDER BY sent_at DESC, id DESC LIMIT 1",
        )
        .bind(conversation)
        .bind(sender_address)
        .fetch_optional(&mut *tx)
        .await?;
        sender
            .and_then(|sender| serde_json::from_value::<Address>(sender).ok())
            .or_else(|| {
                Some(Address {
                    name: None,
                    address: sender_address.to_owned(),
                })
            })
    };
    let correspondent = correspondent.filter(|c| !distrusted && plain_address(c.address.trim()));
    let Some(correspondent) = correspondent else {
        return Ok(Known {
            total_messages,
            them: None,
        });
    };
    let address = correspondent.address.trim().to_lowercase();
    let (wrote_before, from_me): (bool, i64) = sqlx::query_as(WROTE_TO_THEM)
        .bind(inbox)
        .bind(&address)
        .bind(SENT_SCAN)
        .bind(COUNT_CAP)
        .fetch_one(&mut *tx)
        .await?;
    let from_them: i64 = sqlx::query_scalar(FROM_THEM)
        .bind(inbox)
        .bind(&address)
        .bind(COUNT_CAP)
        .fetch_one(&mut *tx)
        .await?;
    // A few more than are kept: some have no words of their own.
    let rows: PastRows = sqlx::query_as(PAST)
        .bind(inbox)
        .bind(&address)
        .bind(SENT_SCAN)
        .bind(conversation)
        .bind(i64::try_from(PAST_MESSAGES * 2).unwrap_or(6))
        .fetch_all(&mut *tx)
        .await?;
    let them = Them {
        name: shown_name(correspondent),
        i_wrote_to_them_before: wrote_before,
        from_them_last_year: from_them,
        from_me_last_year: from_me,
    };
    Ok(Known {
        total_messages,
        them: Some((them, rows)),
    })
}

/// A message's own words: its text part, else its HTML part reduced to
/// text, without quoted history. Text hygiene, not a decision.
async fn own_words(state: &AppState, workspace: &str, bodies: &Bodies) -> Result<String> {
    // Its text part, else its HTML read as text (campbooks-cloud#218).
    let (raw, _) = words(
        &state.blobs,
        workspace,
        (bodies.text.as_ref(), bodies.html.as_ref()),
        Budget {
            text: RAW_TEXT_BYTES,
            html: RAW_HTML_BYTES,
        },
        Quotes::Drop,
    )
    .await?;
    Ok(plain_text::own_words(&raw))
}

/// A sender as the model reads them: their name, else their address, capped.
fn shown_name(sender: Address) -> String {
    let name = sender
        .name
        .filter(|name| !name.trim().is_empty())
        .unwrap_or(sender.address);
    cut_bytes(&name, NAME_BYTES)
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

/// Ask the model, as a person waiting (the interactive share of the
/// workspace's daily AI limit), and read its answer as `T`.
async fn ask<T: DeserializeOwned>(
    state: &AppState,
    gateway: &Gateway,
    workspace: WorkspaceId,
    feature: &'static str,
    request: campbooks_ai::ChatRequest,
) -> Result<T> {
    let usage = Scope::workspace(state.db.clone(), workspace, feature).interactive();
    match usage.run(gateway.chat(Role::Complete, request)).await {
        Ok(response) => response.json::<T>().map_err(|_| {
            // Why it stopped and how long its answer was, never what it
            // wrote; nor the parser's message, which can quote it.
            let finish_reason = response.finish_kind();
            let reason = if response.content.trim().is_empty() {
                "empty"
            } else if finish_reason == "length" {
                "truncated"
            } else {
                "not_json"
            };
            tracing::warn!(
                feature,
                reason,
                finish_reason,
                completion_tokens = response.usage.map(|usage| usage.completion_tokens),
                content_chars = response.content.chars().count(),
                "reply: the model's answer is not the schema"
            );
            Error::Unavailable(crate::MESSAGES.reason("reply_failed"))
        }),
        // The workspace used its whole daily AI limit: 503, saying so.
        Err(error @ campbooks_ai::Error::OverDailyCap { .. }) => Err(error.into()),
        Err(error) => {
            // The kind only: a provider's error can echo the request.
            tracing::warn!(
                feature,
                error = error_kind(&error),
                "reply: the model call failed"
            );
            Err(Error::Unavailable(crate::MESSAGES.reason("reply_failed")))
        }
    }
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

#[cfg(test)]
mod tests;
