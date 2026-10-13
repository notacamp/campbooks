//! Which inboxes a caller may see. Row-level security keeps workspaces
//! apart; within a workspace, a user sees the inboxes they are a member of
//! (and the ones they own), and a client token (bound to the workspace,
//! with no user) sees them all: the access catalogue's `read` on an inbox
//! (`domain::resources`), listed in one query for lists and search.

use campbooks_kernel::{AppState, Error, RecordRef, Result, authz::Action, tenancy::Access};
use sqlx::PgConnection;

use crate::{
    domain::resources,
    events::CONVERSATION_RECORD,
    model::{ConversationId, InboxId},
};

/// The inboxes `access` may see in its workspace (the transaction's).
pub async fn visible_inboxes(conn: &mut PgConnection, access: &Access) -> Result<Vec<InboxId>> {
    let inboxes = match access.user {
        Some(user) => {
            sqlx::query_scalar!(
                r#"SELECT inbox_id AS "id!: InboxId" FROM conversations.inbox_members
                    WHERE workspace_id = $1 AND user_id = $2
                   UNION
                   SELECT id AS "id!: InboxId" FROM conversations.inboxes
                    WHERE workspace_id = $1 AND owner = $2"#,
                access.workspace.as_uuid(),
                user.as_uuid(),
            )
            .fetch_all(conn)
            .await?
        }
        None => {
            sqlx::query_scalar!(
                r#"SELECT id AS "id: InboxId" FROM conversations.inboxes WHERE workspace_id = $1"#,
                access.workspace.as_uuid(),
            )
            .fetch_all(conn)
            .await?
        }
    };
    Ok(inboxes)
}

/// The conversation `id`, locked for update, if `access` may change it
/// (the catalogue's `write` on a conversation): 404 when the caller cannot
/// see it, 403 (`inbox_read_only`) when they only read its inbox or the
/// inbox is kept read-only. Returns its inbox.
pub async fn lock_conversation(
    state: &AppState,
    conn: &mut PgConnection,
    access: &Access,
    id: ConversationId,
) -> Result<InboxId> {
    resources::authorize(
        state,
        conn,
        access,
        Action::Write,
        &RecordRef::new(CONVERSATION_RECORD, &id),
    )
    .await?;
    let inbox = sqlx::query_scalar!(
        r#"SELECT inbox_id AS "inbox: InboxId" FROM conversations.conversations
            WHERE id = $1 FOR UPDATE"#,
        id as ConversationId,
    )
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(Error::NotFound)?;
    Ok(inbox)
}
