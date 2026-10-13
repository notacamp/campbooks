//! Which inboxes a caller may see. Row-level security keeps workspaces
//! apart; within a workspace, a user sees the inboxes they are a member of,
//! and a client token (bound to the workspace, with no user) sees them all.

use campbooks_kernel::{Error, Result, tenancy::Access};

use crate::model::InboxRole;
use sqlx::PgConnection;

use crate::model::{ConversationId, InboxId};

/// The inboxes `access` may see in its workspace (the transaction's).
pub async fn visible_inboxes(conn: &mut PgConnection, access: &Access) -> Result<Vec<InboxId>> {
    let inboxes =
        match access.user {
            Some(user) => {
                sqlx::query_scalar!(
                    r#"SELECT inbox_id AS "id: InboxId" FROM conversations.inbox_members
                    WHERE workspace_id = $1 AND user_id = $2"#,
                    access.workspace.as_uuid(),
                    user.as_uuid(),
                )
                .fetch_all(conn)
                .await?
            }
            None => sqlx::query_scalar!(
                r#"SELECT id AS "id: InboxId" FROM conversations.inboxes WHERE workspace_id = $1"#,
                access.workspace.as_uuid(),
            )
            .fetch_all(conn)
            .await?,
        };
    Ok(inboxes)
}

/// The conversation `id`, locked for update, if `access` may change it:
/// 404 when the caller cannot see it, 403 (`inbox_read_only`) when they
/// only read its inbox. Changing takes the `sender` role or above; a client
/// token acts through its `conversations:write` scope. Returns its inbox.
pub async fn lock_conversation(
    conn: &mut PgConnection,
    access: &Access,
    id: ConversationId,
) -> Result<InboxId> {
    let inboxes = visible_inboxes(conn, access).await?;
    let ids: Vec<_> = inboxes.iter().map(|i| *i.as_uuid()).collect();
    let inbox = sqlx::query_scalar!(
        r#"SELECT inbox_id AS "inbox: InboxId" FROM conversations.conversations
            WHERE id = $1 AND inbox_id = ANY($2) FOR UPDATE"#,
        id as ConversationId,
        &ids,
    )
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(Error::NotFound)?;
    if let Some(user) = access.user {
        let role = sqlx::query_scalar!(
            "SELECT role FROM conversations.inbox_members WHERE inbox_id = $1 AND user_id = $2",
            inbox as InboxId,
            user.as_uuid(),
        )
        .fetch_optional(&mut *conn)
        .await?
        .map(|role| role.parse::<InboxRole>())
        .transpose()?;
        if !matches!(role, Some(InboxRole::Sender | InboxRole::Manager)) {
            return Err(Error::Denied(crate::MESSAGES.reason("inbox_read_only")));
        }
    }
    Ok(inbox)
}
