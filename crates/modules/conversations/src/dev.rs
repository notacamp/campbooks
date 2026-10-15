//! Development and test helpers: create an inbox (in production the
//! connectors module does, when a connection grants the `messages`
//! capability) and feed it recorded change batches.
//!
//! A **recorded batch** is a [`MessageChangeBatch`](crate::ingest::MessageChangeBatch)
//! whose bodies and attachments are written inline, so fixtures stay
//! readable: `text`, `html` and `raw` are strings, and each attachment has a
//! `content` string instead of a `blob`. [`record`] stores them as blobs and
//! returns the real batch, exactly as a connector would produce it.

use campbooks_kernel::{
    AppState, Error, Result,
    blobs::BlobRef,
    db::BeginFor as _,
    tenancy::{UserId, WorkspaceId},
};
use serde_json::{Map, Value, json};

use crate::{
    features::apply_changes,
    ingest::{BATCH_VERSION, MessageChangeBatch},
    model::{Channel, InboxId, InboxRole},
};

/// Create an email inbox for `address` in `workspace`, readable by
/// `members`.
pub async fn create_inbox(
    state: &AppState,
    workspace: WorkspaceId,
    address: &str,
    members: &[(UserId, InboxRole)],
) -> Result<InboxId> {
    let id = InboxId::new();
    // Its first manager owns it, as the member whose connection feeds one does.
    let owner = members
        .iter()
        .find(|(_, role)| *role == InboxRole::Manager)
        .map(|(user, _)| *user.as_uuid());
    let mut tx = state.db.begin_for(workspace).await?;
    sqlx::query!(
        "INSERT INTO conversations.inboxes (id, workspace_id, channel, address, owner)
         VALUES ($1, $2, $3, $4, $5)",
        id as InboxId,
        workspace.as_uuid(),
        Channel::Email.as_str(),
        address.to_lowercase(),
        owner,
    )
    .execute(&mut *tx)
    .await?;
    for (user, role) in members {
        sqlx::query!(
            "INSERT INTO conversations.inbox_members (workspace_id, inbox_id, user_id, role)
             VALUES ($1, $2, $3, $4)",
            workspace.as_uuid(),
            id as InboxId,
            user.as_uuid(),
            role.as_str(),
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(id)
}

/// Store the inline bodies and attachments of `recorded` (a recorded batch's
/// `changes`, see the module docs) in `workspace` and return the batch for
/// `inbox`.
pub async fn record(
    state: &AppState,
    workspace: WorkspaceId,
    inbox: InboxId,
    recorded: &Value,
) -> Result<MessageChangeBatch> {
    let ws = workspace.to_string();
    let bad = |what: &str| Error::internal(format!("recorded batch: {what}"));
    let mut changes = recorded
        .get("changes")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| bad("no `changes` array"))?;
    for change in &mut changes {
        let Some(message) = change.get_mut("message").and_then(Value::as_object_mut) else {
            continue;
        };
        for (part, content_type) in [
            ("text", "text/plain; charset=utf-8"),
            ("html", "text/html; charset=utf-8"),
            ("raw", "message/rfc822"),
        ] {
            if let Some(Value::String(content)) = message.get(part) {
                let blob = put(state, &ws, content, content_type).await?;
                message.insert(part.to_owned(), json!(blob));
            }
        }
        if let Some(Value::Array(attachments)) = message.get_mut("attachments") {
            for attachment in attachments {
                let attachment = attachment
                    .as_object_mut()
                    .ok_or_else(|| bad("an attachment is not an object"))?;
                let content = take_string(attachment, "content")
                    .ok_or_else(|| bad("an attachment has no `content`"))?;
                let content_type = attachment
                    .get("content_type")
                    .and_then(Value::as_str)
                    .unwrap_or("application/octet-stream")
                    .to_owned();
                let blob = put(state, &ws, &content, &content_type).await?;
                attachment.insert("blob".to_owned(), json!(blob));
            }
        }
    }
    let batch = json!({ "version": BATCH_VERSION, "inbox": inbox, "changes": changes });
    serde_json::from_value(batch).map_err(|e| bad(&crate::domain::redact::json(&e)))
}

/// Store `batch` and enqueue its application, as a connector does.
pub async fn submit(
    state: &AppState,
    workspace: WorkspaceId,
    batch: &MessageChangeBatch,
) -> Result<BlobRef> {
    let mut tx = state.db.begin_for(workspace).await?;
    let blob = apply_changes::submit(state, &mut tx, workspace, batch).await?;
    tx.commit().await?;
    Ok(blob)
}

async fn put(state: &AppState, ws: &str, content: &str, content_type: &str) -> Result<BlobRef> {
    Ok(state
        .blobs
        .put(ws, content.as_bytes().to_vec(), content_type)
        .await?)
}

fn take_string(object: &mut Map<String, Value>, key: &str) -> Option<String> {
    match object.remove(key) {
        Some(Value::String(s)) => Some(s),
        _ => None,
    }
}
