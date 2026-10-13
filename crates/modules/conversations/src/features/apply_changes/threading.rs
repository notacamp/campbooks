//! Which conversation a new message joins.
//!
//! Per inbox, in order: the provider's thread key; else the conversation of
//! an ancestor named in `In-Reply-To` / `References`; else the conversation
//! of a descendant that arrived first and names this message. Several
//! descendant conversations (replies stored before their common parent) are
//! merged into the one it joins. No subject-based threading.

use sqlx::PgConnection;

use campbooks_kernel::JobError;

use crate::{
    ingest::NormalizedMessage,
    model::{ConversationId, InboxId},
};

/// Where a new message goes.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Placement {
    /// The conversation it joins; `None` for a new one.
    pub conversation: Option<ConversationId>,
    /// Conversations to merge into it.
    pub merge: Vec<ConversationId>,
}

/// Place `message` in `inbox`.
pub async fn place(
    conn: &mut PgConnection,
    inbox: InboxId,
    message: &NormalizedMessage,
) -> Result<Placement, JobError> {
    if let Some(key) = &message.thread_key {
        let found = sqlx::query_scalar!(
            r#"SELECT id AS "id: ConversationId" FROM conversations.conversations
                WHERE inbox_id = $1 AND thread_key = $2"#,
            inbox as InboxId,
            key,
        )
        .fetch_optional(&mut *conn)
        .await?;
        return Ok(Placement {
            conversation: found,
            merge: Vec::new(),
        });
    }

    let ancestors: Vec<&str> = message
        .in_reply_to
        .iter()
        .chain(&message.references)
        .map(String::as_str)
        .collect();
    let ancestor = if ancestors.is_empty() {
        None
    } else {
        sqlx::query_scalar!(
            r#"SELECT conversation_id AS "id: ConversationId" FROM conversations.messages
                WHERE inbox_id = $1 AND rfc_message_id = ANY($2)
                ORDER BY sent_at DESC, id DESC LIMIT 1"#,
            inbox as InboxId,
            &ancestors as &[&str],
        )
        .fetch_optional(&mut *conn)
        .await?
    };

    let descendants = match &message.rfc_message_id {
        Some(rfc_id) => {
            sqlx::query_scalar!(
                r#"
                SELECT c.id AS "id: ConversationId"
                  FROM conversations.conversations c
                 WHERE c.inbox_id = $1 AND c.thread_key IS NULL
                   AND EXISTS (SELECT 1 FROM conversations.messages m
                                WHERE m.conversation_id = c.id
                                  AND (m.in_reply_to = $2 OR m.reference_ids @> ARRAY[$2]))
                 ORDER BY c.created_at, c.id
                "#,
                inbox as InboxId,
                rfc_id,
            )
            .fetch_all(&mut *conn)
            .await?
        }
        None => Vec::new(),
    };

    let conversation = ancestor.or_else(|| descendants.first().copied());
    let merge = descendants
        .into_iter()
        .filter(|d| Some(*d) != conversation)
        .collect();
    Ok(Placement {
        conversation,
        merge,
    })
}

/// Move everything of conversation `from` into `into`; `from` is left empty
/// (the caller's recount deletes it).
pub async fn merge(
    conn: &mut PgConnection,
    from: ConversationId,
    into: ConversationId,
) -> Result<(), JobError> {
    sqlx::query!(
        r#"
        WITH moved_messages AS (
            UPDATE conversations.messages SET conversation_id = $2, updated_at = now()
             WHERE conversation_id = $1
        ), moved_decisions AS (
            UPDATE conversations.decisions SET conversation_id = $2 WHERE conversation_id = $1
        ), moved_corrections AS (
            UPDATE conversations.corrections SET conversation_id = $2 WHERE conversation_id = $1
        )
        INSERT INTO conversations.conversation_labels (workspace_id, conversation_id, label_id)
        SELECT workspace_id, $2, label_id FROM conversations.conversation_labels
         WHERE conversation_id = $1
        ON CONFLICT DO NOTHING
        "#,
        from as ConversationId,
        into as ConversationId,
    )
    .execute(conn)
    .await?;
    Ok(())
}
