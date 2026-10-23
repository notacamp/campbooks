//! `POST /api/v1/w/{workspace_id}/conversations/{conversation_id}/messages/{message_id}/read`
//! — queue Scout's read of a message when a member opens it
//! (campbooks-cloud#42).
//!
//! It queues the read of a **flagged** message (see [`crate::domain::dated`])
//! that has none and no read queued, and otherwise does nothing and says so.
//! It never waits for the model: the reader shows "Scout is reading this…"
//! until the result arrives with a change to the conversation, and opening the
//! message again queues nothing. A message the decide pass did not flag, one
//! that may be phishing, one the sender's own data covered, and a server where
//! no `complete` role may read mail (or where dated things are switched off)
//! get nothing queued. The client calls this when a member opens a message.

use axum::{Extension, Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, RecordRef, Result, Risk,
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
        resources,
    },
    events::MESSAGE_RECORD,
    features::read_message::{
        covers, enqueue,
        store::{self, Trigger},
    },
    model::{ConversationId, InboxId, MessageId},
    scopes::ConversationsRead,
};

/// What asking for the read did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// The read was queued now.
    Queued,
    /// It was queued already: nothing more was done.
    AlreadyQueued,
    /// It was read already.
    Read,
    /// The model gave no usable answer both times it was asked: no more.
    Failed,
    /// The decide pass did not flag the message (or it may be phishing):
    /// nothing to read.
    NotFlagged,
    /// The sender's own data holds its dates: nothing to ask the model.
    Covered,
    /// No `complete` role may read mail on this server, or dated things are
    /// switched off.
    Unavailable,
}

/// What asking for the read did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ReadRequested {
    /// The outcome.
    pub outcome: Outcome,
}

/// Queue Scout's read of a message, when it has none.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/conversations/{conversation_id}/messages/{message_id}/read",
    operation_id = "conversations.request_message_read",
    tag = "conversations",
    security(("bearer" = ["conversations:read"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).records(["conversations.message"]).access("conversations.message", campbooks_kernel::authz::Action::Read)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("conversation_id" = ConversationId, Path, description = "The conversation."),
        ("message_id" = MessageId, Path, description = "The message."),
    ),
    responses(
        (status = OK, description = "What was done.", body = ReadRequested),
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
) -> Result<Json<ReadRequested>> {
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
          WHERE id = $1 AND conversation_id = $2 AND inbox_id = ANY($3)
            AND direction = 'inbound' AND NOT in_spam AND NOT in_trash",
    )
    .bind(message)
    .bind(conversation)
    .bind(&inboxes as &[InboxId])
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(Error::NotFound)?;
    let outcome = request(&mut tx, access.workspace, message, &settings).await?;
    tx.commit().await?;
    Ok(Json(ReadRequested { outcome }))
}

/// Queue the read of `message` if it should be, and say what was done.
async fn request(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    workspace: WorkspaceId,
    message: MessageId,
    settings: &NeedSettings,
) -> Result<Outcome> {
    // Nothing to ask while no model may read mail here, or dated things are
    // switched off.
    if !settings.summaries || !settings.dated.enabled {
        return Ok(Outcome::Unavailable);
    }
    let answers = needs::answers(&mut **tx, message).await?;
    let flag = dated::flag(&answers, &settings.dated);
    // A warning on the message (a suggestion the pass raised too) is not read
    // either: a read would retell the scam.
    let warned: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM conversations.needs
                         WHERE message_id = $1 AND state = 'open' AND question = 'suspicious')",
    )
    .bind(message)
    .fetch_one(&mut **tx)
    .await?;
    if flag.is_none() || warned {
        return Ok(Outcome::NotFlagged);
    }
    let reads = store::reads_of(&mut **tx, message).await?;
    let structured: Option<(Value, Value)> = reads
        .iter()
        .find(|read| read.layer == "structured" && read.state == "read")
        .map(|read| (read.items.clone(), read.asks.clone()));
    let need_open: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM conversations.needs
                         WHERE message_id = $1 AND state = 'open')",
    )
    .bind(message)
    .fetch_one(&mut **tx)
    .await?;
    // Covered by the sender's data: the model is asked only for the summary
    // of an open need, which a need asked for when it opened.
    if covers(structured.as_ref(), flag.as_ref()) && !need_open {
        return Ok(Outcome::Covered);
    }
    match reads
        .iter()
        .find(|read| read.layer == "model")
        .map(|read| (read.state.as_str(), read.attempts, read.read_at.is_some()))
    {
        Some(("read", _, true)) => return Ok(Outcome::Read),
        Some(("failed", attempts, _)) if attempts >= crate::features::read_message::MAX_CALLS => {
            return Ok(Outcome::Failed);
        }
        Some(("queued", ..)) => {
            // Its job may have been lost: queueing again is a no-op while one
            // is pending.
            enqueue(tx, workspace, message, Trigger::Open).await?;
            return Ok(Outcome::AlreadyQueued);
        }
        _ => {}
    }
    enqueue(tx, workspace, message, Trigger::Open).await?;
    Ok(Outcome::Queued)
}
