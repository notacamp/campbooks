//! `GET /api/v1/w/{workspace_id}/conversations/{conversation_id}` — a
//! conversation with its messages, as plain text. HTML bodies are fetched
//! one message at a time (`conversations.get_message_body`) and shown only
//! in the client's sandboxed frame.

use std::collections::HashMap;

use axum::{Extension, Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result,
    blobs::BlobRef,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    domain::paths::ConversationPath,
    domain::{
        access::visible_inboxes,
        blobs::read_text,
        conversation::{ConversationSummary, summary},
        policy::Policy,
        suggestions::{self, DecisionView},
    },
    model::{Address, AttachmentId, ConversationId, Direction, InboxId, MessageId},
    scopes::ConversationsRead,
};

/// Newest messages returned with a conversation.
pub const MAX_MESSAGES: i64 = 100;
/// Longest text body returned per message, in bytes.
pub const MAX_TEXT_BYTES: usize = 256 * 1024;

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
        .records(["conversations.conversation", "conversations.message"])))),
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
    workspace: Workspace<ConversationsRead>,
    extract::Path(path): extract::Path<ConversationPath>,
) -> Result<Json<ConversationDetail>> {
    let id = path.conversation_id;
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
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
    let conversation = summary(&mut tx, id).await?;
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
    let ids: Vec<MessageId> = rows.iter().map(|r| r.id).collect();
    let mut attachments = attachments_of(&mut tx, &ids).await?;
    tx.commit().await?;

    let ws = access.workspace.to_string();
    let mut messages = Vec::with_capacity(rows.len());
    for row in rows {
        let recipients: Recipients = serde_json::from_value(row.recipients)
            .map_err(|e| Error::internal(crate::domain::redact::json(&e)))?;
        let bodies: Bodies = serde_json::from_value(row.bodies)
            .map_err(|e| Error::internal(crate::domain::redact::json(&e)))?;
        let text = match &bodies.text {
            Some(blob) => {
                let (content, truncated) =
                    read_text(&state.blobs, &ws, blob, MAX_TEXT_BYTES).await?;
                Some(MessageText { content, truncated })
            }
            None => None,
        };
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
            has_html: bodies.html.is_some(),
            attachments: attachments.remove(&row.id).unwrap_or_default(),
        });
    }
    Ok(Json(ConversationDetail {
        conversation,
        messages,
        decisions,
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
               content_type, size, inline
          FROM conversations.attachments
         WHERE message_id = ANY($1)
         ORDER BY message_id, position
        "#,
        ids as &[MessageId],
    )
    .fetch_all(conn)
    .await?
    {
        attachments
            .entry(a.message_id)
            .or_default()
            .push(AttachmentView {
                id: a.id,
                filename: a.filename,
                content_type: a.content_type,
                size: u64::try_from(a.size).unwrap_or_default(),
                inline: a.inline,
            });
    }
    Ok(attachments)
}

#[cfg(test)]
mod tests {
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
        assert_eq!(kit.get(&client, &uri).await.status, StatusCode::OK);
        let junk = format!("/api/v1/w/{workspace}/conversations/not-an-id");
        assert_eq!(kit.get(&client, &junk).await.status, StatusCode::NOT_FOUND);
    }
}
