//! `GET /api/v1/w/{workspace_id}/conversations/{conversation_id}/messages/{message_id}/read`
//! — what Scout read in a message (campbooks-cloud#42): its items (things
//! that happen at a time) and asks (obligations), with their text (title,
//! place, quote, amount), from the sender's own data and from the model's
//! read, and where the model's read is.
//!
//! It is read with the same access checks as the conversation, so someone
//! who may not read the inbox gets 404, and it never asks the model. The
//! `conversations.message_read` event carries none of this text; a reader
//! comes here for it.

use axum::{Extension, Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, RecordRef, Result,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

use crate::{
    domain::{
        access::visible_inboxes,
        dated,
        needs::{self, NeedSettings},
        paths::MessagePath,
        read::{ReadAsk, ReadItem},
        resources,
    },
    events::MESSAGE_RECORD,
    features::read_message::{covers, store},
    model::{ConversationId, InboxId, MessageId, ReadLayer},
    scopes::ConversationsRead,
};

/// Where the model's read of a message is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReadStatus {
    /// Not asked for.
    None,
    /// Asked for; the answer arrives with a change to the conversation.
    Queued,
    /// The model gave no usable answer; it may be tried once more.
    Failed,
    /// Read.
    Done,
}

/// What Scout read in a message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MessageReadView {
    /// Where the model's read is.
    pub status: ReadStatus,
    /// The readers that produced a read: the sender's own data
    /// (`structured`), and Scout's call (`model`).
    pub layers: Vec<ReadLayer>,
    /// The things that happen at a time found, the sender's data first.
    pub items: Vec<ReadItem>,
    /// The obligations found.
    pub asks: Vec<ReadAsk>,
    /// Whether the decide pass flagged the message for a read.
    pub flagged: bool,
    /// Whether Scout can read this message for dates: the pass said it tells
    /// of something dated and the sender's own data did not cover it.
    pub can_read: bool,
    /// The kind of dated thing the pass named, when it flagged the message
    /// and no `complete` role may read mail here (the reader offers to add it
    /// by hand).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dated_kind: Option<String>,
}

/// What Scout read in a message.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/conversations/{conversation_id}/messages/{message_id}/read",
    operation_id = "conversations.get_message_read",
    tag = "conversations",
    security(("bearer" = ["conversations:read"])),
    extensions(("x-campbooks" = json!(Operation::query().records(["conversations.message"]).access("conversations.message", campbooks_kernel::authz::Action::Read)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("conversation_id" = ConversationId, Path, description = "The conversation."),
        ("message_id" = MessageId, Path, description = "The message."),
    ),
    responses(
        (status = OK, description = "The read.", body = MessageReadView),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such message, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    Extension(settings): Extension<NeedSettings>,
    workspace: Workspace<ConversationsRead>,
    extract::Path(path): extract::Path<MessagePath>,
) -> Result<Json<MessageReadView>> {
    let (conversation, message) = (path.conversation_id, path.message_id);
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    resources::authorize(
        &state,
        &mut tx,
        access,
        Action::Read,
        &RecordRef::new(MESSAGE_RECORD, &message),
    )
    .await?;
    let inboxes = visible_inboxes(&mut tx, access).await?;
    sqlx::query_scalar::<_, i32>(
        "SELECT 1 FROM conversations.messages
          WHERE id = $1 AND conversation_id = $2 AND inbox_id = ANY($3)",
    )
    .bind(message)
    .bind(conversation)
    .bind(&inboxes as &[InboxId])
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(Error::NotFound)?;
    let view = view_of(&mut tx, message, &settings).await?;
    tx.commit().await?;
    Ok(Json(view))
}

/// The read of `message` as stored, and whether it is flagged.
pub(crate) async fn view_of(
    conn: &mut sqlx::PgConnection,
    message: MessageId,
    settings: &NeedSettings,
) -> Result<MessageReadView> {
    let reads = store::reads_of(conn, message).await?;
    let model = reads.iter().find(|read| read.layer == "model");
    let status = match model.map(|read| (read.state.as_str(), read.read_at.is_some())) {
        Some(("queued", _)) => ReadStatus::Queued,
        Some(("failed", _)) => ReadStatus::Failed,
        // Not asked for, or made before items and asks were read, so not yet
        // read in full.
        None | Some((_, false)) => ReadStatus::None,
        Some(_) => ReadStatus::Done,
    };
    let mut layers = Vec::new();
    let mut items: Vec<ReadItem> = Vec::new();
    let mut asks: Vec<ReadAsk> = Vec::new();
    for read in reads.iter().filter(|read| read.state == "read") {
        let layer = if read.layer == "structured" {
            ReadLayer::Structured
        } else {
            ReadLayer::Model
        };
        let (read_items, read_asks) = (
            serde_json::from_value::<Vec<ReadItem>>(read.items.clone()).unwrap_or_default(),
            serde_json::from_value::<Vec<ReadAsk>>(read.asks.clone()).unwrap_or_default(),
        );
        if read.read_at.is_some() {
            layers.push(layer);
        }
        items.extend(read_items);
        asks.extend(read_asks);
    }
    let answers = needs::answers(conn, message).await?;
    let flag = dated::flag(&answers, &settings.dated);
    let structured: Option<(Value, Value)> = reads
        .iter()
        .find(|read| read.layer == "structured" && read.state == "read")
        .map(|read| (read.items.clone(), read.asks.clone()));
    let covered = covers(structured.as_ref(), flag.as_ref());
    Ok(MessageReadView {
        status,
        layers,
        items,
        asks,
        flagged: flag.is_some(),
        can_read: flag.as_ref().is_some_and(|flagged| flagged.by_dated_thing) && !covered,
        dated_kind: flag
            .and_then(|flagged| flagged.kind)
            .filter(|_| !settings.summaries),
    })
}
