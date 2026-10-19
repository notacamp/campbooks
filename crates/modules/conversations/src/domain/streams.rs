//! Placing a conversation in a stream from an accepted answer: shared by
//! the rules pass as mail is stored (`apply_changes`) and the model pass
//! after it (`decide_message`).
//!
//! A conversation is sorted once, by its first inbound message, and only
//! while nobody placed it: a member's move (`member`) or the model's earlier
//! placement (`model`) is never overridden, and later messages never re-sort
//! it. A placement by the retired content rules (`rules`, before ADR 0018;
//! the facts never answer `stream`) gives way to the model's.
//!
//! Every placement is mirrored to the provider ([`mirror`]): each received
//! message whose stream at the provider is not its conversation's is asked
//! to move there, through the same recorded, bound requests as a member's
//! change. Mail stored before its inbox's first sync completed, or before
//! this was, is never asked (`mirrors_stream`).

use campbooks_kernel::{EventBus, Result, binding::Binding};
use sqlx::{PgConnection, Postgres, Transaction};

use super::conversation::{Changed, request};
use crate::{
    events::{Change, Origin, RequestedChange},
    model::{ConversationId, InboxId, MessageId, Stream, StreamSource},
};

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

/// Ask the provider to move `conversation`'s received messages to its
/// stream: each one that is mirrored (`mirrors_stream`) and whose stored
/// provider stream is another (`none`, or none stored, counting as
/// another), and `also` (a
/// message whose stream mark is stale) whatever it holds. One
/// `changes_requested` for them all, recorded and bound like a member's
/// (`conversation::request`); nothing when none is left to move.
pub async fn mirror(
    binding: &Binding,
    events: &EventBus,
    tx: &mut Transaction<'_, Postgres>,
    workspace: &str,
    conversation: ConversationId,
    also: Option<&str>,
) -> Result<()> {
    let Some((stream, inbox)): Option<(String, InboxId)> =
        sqlx::query_as("SELECT stream, inbox_id FROM conversations.conversations WHERE id = $1")
            .bind(conversation)
            .fetch_optional(&mut **tx)
            .await?
    else {
        return Ok(());
    };
    let mut moving: Vec<String> = sqlx::query_scalar(
        "SELECT provider_message_id FROM conversations.messages
          WHERE conversation_id = $1 AND direction = 'inbound' AND mirrors_stream
            AND provider_stream IS DISTINCT FROM $2
          ORDER BY sent_at, id",
    )
    .bind(conversation)
    .bind(&stream)
    .fetch_all(&mut **tx)
    .await?;
    if let Some(also) = also
        && !moving.iter().any(|id| id == also)
    {
        moving.push(also.to_owned());
    }
    if moving.is_empty() {
        return Ok(());
    }
    let changed = Changed {
        workspace: workspace.to_owned(),
        inbox,
        conversation,
        origin: Origin::Provider,
        changes: vec![Change::Stream],
        provider_message_ids: moving,
    };
    let to = RequestedChange::MoveToStream {
        stream: stream.parse()?,
    };
    request(binding, events, tx, &changed, &to).await
}
