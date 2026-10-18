//! `GET /api/v1/w/{workspace_id}/conversations/{conversation_id}` — a
//! conversation with its messages, as plain text. HTML bodies are fetched
//! one message at a time (`conversations.get_message_body`) and shown only
//! in the client's sandboxed frame.

use std::{collections::HashMap, time::Instant};

use axum::{Extension, Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result,
    blobs::BlobRef,
    db::BeginFor as _,
    extract,
    i18n::Locale,
    tenancy::{Workspace, WorkspaceId},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::domain::resources;
use crate::events::CONVERSATION_RECORD;
use crate::{
    config::ActionSettings,
    domain::paths::ConversationPath,
    domain::{
        access::visible_inboxes,
        actions,
        attachments::in_body,
        blobs::{Budget, cut, words_until},
        conversation::{ConversationSummary, summary},
        plain_text::Quotes,
        policy::Policy,
        suggestions::{self, DecisionView},
    },
    model::{Address, AttachmentId, ConversationId, Direction, InboxId, MessageId},
    scopes::ConversationsRead,
};
use campbooks_kernel::RecordRef;
use campbooks_kernel::authz::Action;

/// Newest messages returned with a conversation.
pub const MAX_MESSAGES: i64 = 100;
/// Longest text body returned per message, in bytes.
pub const MAX_TEXT_BYTES: usize = 256 * 1024;
/// Bytes of an HTML part read when the text part has no words; the words
/// read from it are cut to [`MAX_TEXT_BYTES`].
const MAX_HTML_BYTES: usize = 256 * 1024;
/// How long one answer may spend reading messages' HTML as text, all its
/// messages together: crafted HTML threaded into one conversation cannot
/// hold a request for long. Messages past it come without `text`, marked
/// `text_unread`.
const TEXT_DEADLINE: std::time::Duration = std::time::Duration::from_secs(1);

/// An attachment of a message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct AttachmentView {
    /// The attachment.
    pub id: AttachmentId,
    /// Its file name, as sent.
    pub filename: String,
    /// Its media type, as sent (not trusted).
    pub content_type: String,
    /// Its size in bytes.
    pub size: u64,
    /// Shown inside the body rather than attached.
    pub inline: bool,
    /// Shown by the HTML body (an inline raster image its `cid:` source
    /// names), so a reader need not list it with the files.
    pub in_body: bool,
}

/// A message's plain-text body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MessageText {
    /// The text, up to 256 KiB.
    pub content: String,
    /// Whether it was cut short.
    pub truncated: bool,
}

/// A message, with its plain-text body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "the wire shape: independent facts about one message"
)]
pub struct MessageView {
    /// The message.
    pub id: MessageId,
    /// Received or sent.
    pub direction: Direction,
    /// The sender.
    pub from: Address,
    /// Recipients.
    pub to: Vec<Address>,
    /// Copied recipients.
    pub cc: Vec<Address>,
    /// Where replies go, when not to the sender.
    pub reply_to: Vec<Address>,
    /// The subject.
    pub subject: String,
    /// When it was sent.
    pub sent_at: DateTime<Utc>,
    /// Read.
    pub read: bool,
    /// Starred.
    pub starred: bool,
    /// The plain-text body, when it has one.
    pub text: Option<MessageText>,
    /// Its words were to be read from its HTML and this answer ran out of
    /// time first: `text` is absent, and the HTML is still whole
    /// (`conversations.get_message_body`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub text_unread: bool,
    /// Whether it has an HTML body (`conversations.get_message_body`).
    pub has_html: bool,
    /// Its attachments, in order.
    pub attachments: Vec<AttachmentView>,
}

/// A conversation and its newest messages, oldest first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct ConversationDetail {
    /// The conversation.
    pub conversation: ConversationSummary,
    /// Up to the 100 newest messages, oldest first.
    pub messages: Vec<MessageView>,
    /// The decide pass's answers about the newest inbound message, with
    /// their reasons: suggestions, except an answer marked `acted`.
    pub decisions: Vec<DecisionView>,
    /// Scout's summary of the email behind the conversation's open need, in
    /// your language (English when it has none in yours), written once when
    /// the need was raised. None when there is no open need, the summary is
    /// not written yet (it arrives with a change to this conversation), or no
    /// model may read mail on this server.
    pub need_summary: Option<String>,
}

/// The summary of the email behind the conversation's open need; none for
/// a phishing warning, even one written before the need became one.
const NEED_SUMMARY: &str = "SELECT s.texts FROM conversations.needs n \
       JOIN conversations.need_summaries s ON s.message_id = n.message_id \
      WHERE n.conversation_id = $1 AND n.state = 'open' AND s.state = 'written' \
        AND n.question <> 'suspicious'";

/// The summary of the email behind the conversation's open need, in
/// `locale`, else in English.
async fn need_summary(
    conn: &mut sqlx::PgConnection,
    id: ConversationId,
    locale: Locale,
) -> Result<Option<String>> {
    let texts: Option<serde_json::Value> = sqlx::query_scalar(NEED_SUMMARY)
        .bind(id)
        .fetch_optional(&mut *conn)
        .await?;
    Ok(texts.and_then(|texts| {
        [locale, Locale::En]
            .into_iter()
            .find_map(|l| texts.get(l.tag())?.as_str().map(ToOwned::to_owned))
    }))
}

/// `messages.recipients`.
#[derive(Deserialize)]
struct Recipients {
    #[serde(default)]
    to: Vec<Address>,
    #[serde(default)]
    cc: Vec<Address>,
    #[serde(default)]
    reply_to: Vec<Address>,
}

/// `messages.bodies`.
#[derive(Deserialize)]
pub struct Bodies {
    /// The plain-text part.
    #[serde(default)]
    pub text: Option<BlobRef>,
    /// The HTML part.
    #[serde(default)]
    pub html: Option<BlobRef>,
}

/// Read a conversation and its messages.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/conversations/{conversation_id}",
    operation_id = "conversations.get",
    tag = "conversations",
    security(("bearer" = ["conversations:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant()
        .records(["conversations.conversation", "conversations.message", "conversations.rule"]).access("conversations.conversation", campbooks_kernel::authz::Action::Read)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("conversation_id" = ConversationId, Path, description = "The conversation."),
    ),
    responses(
        (status = OK, description = "The conversation.", body = ConversationDetail),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such conversation, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    Extension(policy): Extension<Policy>,
    Extension(settings): Extension<ActionSettings>,
    workspace: Workspace<ConversationsRead>,
    locale: Locale,
    extract::Path(path): extract::Path<ConversationPath>,
) -> Result<Json<ConversationDetail>> {
    let id = path.conversation_id;
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    resources::authorize(
        &state,
        &mut tx,
        access,
        Action::Read,
        &RecordRef::new(CONVERSATION_RECORD, &id),
    )
    .await?;
    let inboxes = visible_inboxes(&mut tx, access).await?;
    let visible = sqlx::query_scalar!(
        "SELECT 1 AS one FROM conversations.conversations WHERE id = $1 AND inbox_id = ANY($2)",
        id as ConversationId,
        &inboxes as &[InboxId],
    )
    .fetch_optional(&mut *tx)
    .await?;
    if visible.is_none() {
        return Err(Error::NotFound);
    }
    let mut conversation = summary(&mut tx, access.user, id).await?;
    actions::attach(&mut tx, settings, std::slice::from_mut(&mut conversation)).await?;
    let mut rows = sqlx::query!(
        r#"
        SELECT id AS "id: MessageId", direction, sender, recipients, subject, sent_at, read,
               starred, bodies
          FROM conversations.messages
         WHERE conversation_id = $1
         ORDER BY sent_at DESC, id DESC
         LIMIT $2
        "#,
        id as ConversationId,
        MAX_MESSAGES,
    )
    .fetch_all(&mut *tx)
    .await?;
    rows.reverse();
    let newest_inbound = rows
        .iter()
        .rev()
        .find(|r| r.direction == crate::model::Direction::Inbound.as_str())
        .map(|r| r.id);
    let mut decisions = match newest_inbound {
        Some(message) => suggestions::for_message(&mut tx, message).await?,
        None => Vec::new(),
    };
    for view in &mut decisions {
        view.threshold = suggestions::threshold(&policy, view);
    }
    let need_summary = need_summary(&mut tx, id, locale).await?;
    let ids: Vec<MessageId> = rows.iter().map(|r| r.id).collect();
    let mut attachments = attachments_of(&mut tx, &ids).await?;
    tx.commit().await?;

    let ws = access.workspace.to_string();
    // One budget for reading HTML as text, all messages together.
    let deadline = Instant::now() + text_deadline();
    let mut messages = Vec::with_capacity(rows.len());
    for row in rows {
        let recipients: Recipients = serde_json::from_value(row.recipients)
            .map_err(|e| Error::internal(crate::domain::redact::json(&e)))?;
        let bodies: Bodies = serde_json::from_value(row.bodies)
            .map_err(|e| Error::internal(crate::domain::redact::json(&e)))?;
        let (text, text_unread) = text_of(&state, &ws, &bodies, deadline).await?;
        messages.push(MessageView {
            id: row.id,
            direction: row.direction.parse()?,
            from: serde_json::from_value(row.sender)
                .map_err(|e| Error::internal(crate::domain::redact::json(&e)))?,
            to: recipients.to,
            cc: recipients.cc,
            reply_to: recipients.reply_to,
            subject: row.subject,
            sent_at: row.sent_at,
            read: row.read,
            starred: row.starred,
            text,
            text_unread,
            has_html: bodies.html.is_some(),
            attachments: attachments.remove(&row.id).unwrap_or_default(),
        });
    }
    Ok(Json(ConversationDetail {
        conversation,
        messages,
        decisions,
        need_summary,
    }))
}

/// The attachments of `ids`, by message, in order.
async fn attachments_of(
    conn: &mut sqlx::PgConnection,
    ids: &[MessageId],
) -> Result<HashMap<MessageId, Vec<AttachmentView>>> {
    let mut attachments: HashMap<MessageId, Vec<AttachmentView>> = HashMap::new();
    for a in sqlx::query!(
        r#"
        SELECT id AS "id: AttachmentId", message_id AS "message_id: MessageId", filename,
               content_type, size, inline, content_id IS NOT NULL AS "has_content_id!"
          FROM conversations.attachments
         WHERE message_id = ANY($1)
         ORDER BY message_id, position
        "#,
        ids as &[MessageId],
    )
    .fetch_all(conn)
    .await?
    {
        let in_body = in_body(a.inline, a.has_content_id, &a.content_type);
        attachments
            .entry(a.message_id)
            .or_default()
            .push(AttachmentView {
                id: a.id,
                filename: a.filename,
                content_type: a.content_type,
                size: u64::try_from(a.size).unwrap_or_default(),
                inline: a.inline,
                in_body,
            });
    }
    Ok(attachments)
}

/// [`TEXT_DEADLINE`]; in a debug build only, `CAMPBOOKS_DEV_TEXT_DEADLINE_MS`
/// instead when set (0 marks every message read from HTML `text_unread`), so
/// a recording can show what the reader does then. Release builds never
/// read it.
fn text_deadline() -> std::time::Duration {
    if cfg!(debug_assertions)
        && let Some(ms) = std::env::var("CAMPBOOKS_DEV_TEXT_DEADLINE_MS")
            .ok()
            .and_then(|ms| ms.parse::<u64>().ok())
    {
        return std::time::Duration::from_millis(ms);
    }
    TEXT_DEADLINE
}

/// A message's text: its text part, else its HTML read as text, also when
/// the text part is blanks only (campbooks-cloud#218); replies and forwards
/// quote it. `None` for a message with neither part; `(None, true)` when its
/// HTML was not read by `deadline`.
async fn text_of(
    state: &AppState,
    ws: &str,
    bodies: &Bodies,
    deadline: Instant,
) -> Result<(Option<MessageText>, bool)> {
    if bodies.text.is_none() && bodies.html.is_none() {
        return Ok((None, false));
    }
    let read = words_until(
        &state.blobs,
        ws,
        (bodies.text.as_ref(), bodies.html.as_ref()),
        Budget {
            text: MAX_TEXT_BYTES,
            html: MAX_HTML_BYTES,
        },
        Quotes::Keep,
        Some(deadline),
    )
    .await?;
    if read.late {
        return Ok((None, true));
    }
    // Cut short only when the text part was: words read from HTML are
    // capped too, but the HTML the reader shows is whole.
    Ok((
        Some(MessageText {
            content: cut(read.text, MAX_TEXT_BYTES),
            truncated: read.cut,
        }),
        false,
    ))
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use axum::http::StatusCode;
    use sqlx::PgPool;

    use crate::{model::ConversationId, testkit::Kit};

    #[sqlx::test(migrations = "../../../migrations")]
    async fn reads_a_conversation_with_plain_text_bodies(db: PgPool) {
        let kit = Kit::new(db).await;
        let (workspace, _, sam) = kit.sams_inbox().await;
        let id = kit.conversation_of("en-1").await;
        let response = kit
            .get(&sam, &format!("/api/v1/w/{workspace}/conversations/{id}"))
            .await;
        assert_eq!(response.status, StatusCode::OK, "{:?}", response.json);
        let messages = response.json["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0]["direction"], "inbound");
        assert_eq!(messages[0]["from"]["name"], "Maya Okafor");
        assert_eq!(
            messages[0]["cc"][0]["address"],
            "leo@northwind-studio.example"
        );
        assert!(
            messages[0]["text"]["content"]
                .as_str()
                .unwrap()
                .contains("by Friday 10 October")
        );
        assert_eq!(messages[0]["text"]["truncated"], false);
        assert_eq!(messages[0]["has_html"], true);
        assert_eq!(messages[0]["attachments"][0]["filename"], "q4-plan.pdf");
        assert_eq!(messages[1]["direction"], "outbound");
        assert_eq!(messages[1]["has_html"], false);
        assert_eq!(response.json["conversation"]["unread_count"], 1);
        // No HTML in this response.
        assert!(!response.json.to_string().contains("<p>"));
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn html_alone_or_a_blank_text_part_gives_the_words_of_the_html(db: PgPool) {
        // campbooks-cloud#218: quotes and forwards are built from this text.
        let kit = Kit::new(db).await;
        let (workspace, inbox, sam) = kit.sams_inbox().await;
        let html = "<head><meta charset=utf-8><title>Quay Office</title>\
                    <style>p{color:red}</style><p>Your berth at the harbourlight pontoon \
                    is kept.</p><script>alert(1)</script>";
        for (id, text) in [("html-only-1", None), ("blank-text-1", Some("\n\n\n"))] {
            let mut message = serde_json::json!({
                "provider_message_id": id,
                "from": {"name": "Quay Office", "address": "office@quay-moorings.example"},
                "to": [{"address": "sam@example.com"}],
                "subject": "Your berth for next season",
                "sent_at": "2026-10-13T06:30:00Z",
                "direction": "inbound",
                "html": html,
                "mailbox": {"in_inbox": true},
                "read": false, "starred": false
            });
            if let Some(text) = text {
                message["text"] = text.into();
            }
            kit.ingest_value(
                workspace,
                inbox,
                &serde_json::json!({"changes": [{"op": "upsert", "message": message}]}),
            )
            .await;
            let conversation = kit.conversation_of(id).await;
            let response = kit
                .get(
                    &sam,
                    &format!("/api/v1/w/{workspace}/conversations/{conversation}"),
                )
                .await;
            assert_eq!(response.status, StatusCode::OK, "{:?}", response.json);
            let content = response.json["messages"][0]["text"]["content"]
                .as_str()
                .unwrap_or_default()
                .to_owned();
            assert!(
                content.contains("harbourlight pontoon"),
                "{id}: {content:?}"
            );
            for never in ["Quay Office", "color", "alert", "<"] {
                assert!(!content.contains(never), "{id}: {never} in {content:?}");
            }
        }
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn crafted_html_threaded_into_one_conversation_cannot_hold_an_open(db: PgPool) {
        // Twelve HTML-only messages in one thread, each a single tag with
        // tens of thousands of attributes (slow to tokenize, within the
        // HTML budget): each would take the whole per-message reading time,
        // about 6 s in all. The answer shares one second instead, off the
        // async workers, and says which texts it did not read.
        let kit = Kit::new(db).await;
        let (workspace, inbox, sam) = kit.sams_inbox().await;
        let mut attributes = String::new();
        for i in 0..34_000 {
            let _ = write!(attributes, "a{i} ");
        }
        let changes: Vec<serde_json::Value> = (0..12)
            .map(|n| {
                serde_json::json!({"op": "upsert", "message": {
                    "provider_message_id": format!("crafted-{n}"),
                    "thread_key": "thr-crafted",
                    "from": {"address": "mass@crafted.example"},
                    "to": [{"address": "sam@example.com"}],
                    "subject": "Crafted",
                    "sent_at": format!("2026-10-13T06:{n:02}:00Z"),
                    "direction": "inbound",
                    "html": format!("<p>Word {n}</p><p {attributes}>tail</p>"),
                    "mailbox": {"in_inbox": true},
                    "read": false, "starred": false
                }})
            })
            .collect();
        kit.ingest_value(workspace, inbox, &serde_json::json!({ "changes": changes }))
            .await;
        let id = kit.conversation_of("crafted-0").await;
        let started = std::time::Instant::now();
        let response = kit
            .get(&sam, &format!("/api/v1/w/{workspace}/conversations/{id}"))
            .await;
        let took = started.elapsed();
        assert_eq!(response.status, StatusCode::OK, "{:?}", response.json);
        assert!(took < std::time::Duration::from_secs(4), "took {took:?}");
        let messages = response.json["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 12);
        let unread = messages.iter().filter(|m| m["text_unread"] == true).count();
        assert!(unread > 0, "{messages:?}");
        for message in messages {
            // Either its words, or said not read: never a "truncated" text.
            let read = message["text"].is_object();
            assert!(read != (message["text_unread"] == true), "{message}");
            if read {
                assert_eq!(message["text"]["truncated"], false);
            }
            assert_eq!(message["has_html"], true);
        }
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn a_conversation_the_caller_cannot_see_is_not_found(db: PgPool) {
        let kit = Kit::new(db).await;
        let (workspace, _, _) = kit.sams_inbox().await;
        let (ana, _) = kit.member("ana", workspace);
        let id = kit.conversation_of("en-1").await;
        let uri = format!("/api/v1/w/{workspace}/conversations/{id}");
        assert_eq!(kit.get(&ana, &uri).await.status, StatusCode::NOT_FOUND);
        let unknown = format!(
            "/api/v1/w/{workspace}/conversations/{}",
            ConversationId::new()
        );
        let client = kit.client(workspace, "conversations:read");
        assert_eq!(
            kit.get(&client, &unknown).await.status,
            StatusCode::NOT_FOUND
        );
        // A client acting for no member sees no inbox's conversation.
        assert_eq!(kit.get(&client, &uri).await.status, StatusCode::NOT_FOUND);
        let junk = format!("/api/v1/w/{workspace}/conversations/not-an-id");
        assert_eq!(kit.get(&client, &junk).await.status, StatusCode::NOT_FOUND);
    }
}
