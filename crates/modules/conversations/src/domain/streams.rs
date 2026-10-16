//! Placing a conversation in a stream from an accepted answer: shared by
//! the rules pass as mail is stored (`apply_changes`) and the model pass
//! after it (`decide_message`).
//!
//! A conversation is sorted once, by its first inbound message, and only
//! while nobody placed it: a member's move (`member`) or the model's earlier
//! placement (`model`) is never overridden, and later messages never re-sort
//! it. A placement by the retired content rules (`rules`, before ADR 0018;
//! the facts never answer `stream`) gives way to the model's.

use campbooks_kernel::Result;
use sqlx::PgConnection;

use crate::model::{ConversationId, MessageId, Stream, StreamSource};

/// Which inbound message an answer is about, for [`sort_first_inbound`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Inbound {
    /// One being stored now: the conversation must have no inbound message
    /// yet.
    Arriving,
    /// One already stored: no other inbound message of the conversation
    /// may have been sent before it.
    Stored(MessageId),
}

/// Put `conversation` in `stream` (placed by `source`) when `message` is its
/// first inbound message and nobody but the retired rules placed it yet.
/// Returns whether it did.
pub async fn sort_first_inbound(
    conn: &mut PgConnection,
    conversation: ConversationId,
    stream: Stream,
    source: StreamSource,
    message: Inbound,
) -> Result<bool> {
    let stored = match message {
        Inbound::Arriving => None,
        Inbound::Stored(id) => Some(id),
    };
    let sorted = sqlx::query!(
        r#"
        UPDATE conversations.conversations c
           SET stream = $2, stream_source = $3, updated_at = now()
         WHERE c.id = $1 AND c.stream_source IN ('default', 'rules')
           AND NOT EXISTS (
               SELECT 1 FROM conversations.messages m
                WHERE m.conversation_id = c.id AND m.direction = 'inbound'
                  AND ($4::uuid IS NULL
                       OR (m.id <> $4 AND (m.sent_at, m.id) < (
                               SELECT f.sent_at, f.id FROM conversations.messages f
                                WHERE f.id = $4))))
        "#,
        conversation as ConversationId,
        stream.as_str(),
        source.as_str(),
        stored as Option<MessageId>,
    )
    .execute(conn)
    .await?
    .rows_affected();
    Ok(sorted > 0)
}
