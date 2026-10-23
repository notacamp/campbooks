//! Reads at rest: asking for one, the reads a message has, and the items
//! already read in a conversation.
//!
//! `conversations.message_reads` holds one row per message and layer. A
//! `queued` row says a read of the `model` layer was asked for (so it is not
//! asked for twice, and a read that is lost can be asked for again); the job
//! then writes it as `read` or `failed`.

use std::collections::HashSet;

use campbooks_kernel::{
    jobs::{EnqueueOptions, Enqueued},
    tenancy::{UserId, WorkspaceId},
};
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{PgConnection, Postgres, Transaction};

use super::{ReadMessage, ReadMessagePayload};
use crate::{
    domain::read::ReadItem,
    model::{ConversationId, InboxId, MessageId},
};

/// Why a read was asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// A need opened for the message.
    Need,
    /// A member opened it.
    Open,
}

impl Trigger {
    /// As stored in `message_reads.triggered_by`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Need => "need",
            Self::Open => "open",
        }
    }
}

/// Ask for the model read of `message`: a `queued` row when it has none, and
/// one pending job per message. A message that already has a read is not
/// asked again (the job checks). Ids only.
pub async fn enqueue(
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    message: MessageId,
    trigger: Trigger,
) -> Result<Enqueued, sqlx::Error> {
    sqlx::query(
        "INSERT INTO conversations.message_reads
             (message_id, workspace_id, layer, state, attempts, triggered_by)
         VALUES ($1, $2, 'model', 'queued', 0, $3)
         ON CONFLICT (message_id, layer) DO NOTHING",
    )
    .bind(message)
    .bind(workspace.as_uuid())
    .bind(trigger.as_str())
    .execute(&mut **tx)
    .await?;
    campbooks_kernel::jobs::enqueue_to::<ReadMessage>(
        tx,
        &ReadMessagePayload { message },
        EnqueueOptions::for_workspace(workspace.to_string())
            .unique(format!("conversations.read_message:{message}"))
            .priority(-10),
    )
    .await
}

/// A stored read.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StoredRead {
    /// `structured` or `model`.
    pub layer: String,
    /// `queued`, `read` or `failed`.
    pub state: String,
    /// Calls made for it.
    pub attempts: i16,
    /// Its items.
    pub items: Value,
    /// Its asks.
    pub asks: Value,
    /// When it was read in full; none for one made before items and asks.
    pub read_at: Option<DateTime<Utc>>,
}

/// The reads `message` has, structured first.
pub async fn reads_of(
    conn: &mut PgConnection,
    message: MessageId,
) -> Result<Vec<StoredRead>, sqlx::Error> {
    sqlx::query_as(
        "SELECT layer, state, attempts, items, asks, read_at
           FROM conversations.message_reads WHERE message_id = $1
          ORDER BY layer DESC",
    )
    .bind(message)
    .fetch_all(conn)
    .await
}

/// The items read from the messages of `conversation` that no later item
/// cancelled or replaced, oldest first: what a newer message can update or cancel, and
/// what a read is told is known. A stored item that cannot be read back is
/// left out.
pub async fn known_items(
    conn: &mut PgConnection,
    conversation: ConversationId,
) -> Result<Vec<ReadItem>, sqlx::Error> {
    let rows: Vec<Value> = sqlx::query_scalar(
        "SELECT r.items
           FROM conversations.message_reads r
           JOIN conversations.messages m ON m.id = r.message_id
          WHERE m.conversation_id = $1 AND r.state = 'read'
            AND jsonb_array_length(r.items) > 0
          ORDER BY m.sent_at, m.id, r.layer DESC",
    )
    .bind(conversation)
    .fetch_all(conn)
    .await?;
    let all: Vec<ReadItem> = rows
        .into_iter()
        .filter_map(|items| serde_json::from_value::<Vec<ReadItem>>(items).ok())
        .flatten()
        .collect();
    // Gone: cancelled, or replaced by a later version of itself.
    let gone: HashSet<String> = all
        .iter()
        .flat_map(|i| i.cancels.iter().chain(&i.updates))
        .cloned()
        .collect();
    Ok(all
        .into_iter()
        .filter(|item| item.cancels.is_none() && !gone.contains(&item.key))
        .collect())
}

/// The inbox's personal owner: the member whose connection feeds it when
/// nobody else reads it, or the one member who reads an inbox with no owner
/// on record. None for an inbox several members read (a shared one).
pub async fn owner(conn: &mut PgConnection, inbox: InboxId) -> Result<Option<UserId>, sqlx::Error> {
    let owner: Option<UserId> =
        sqlx::query_scalar("SELECT owner FROM conversations.inboxes WHERE id = $1")
            .bind(inbox)
            .fetch_optional(&mut *conn)
            .await?
            .flatten();
    let members: Vec<UserId> =
        sqlx::query_scalar("SELECT user_id FROM conversations.inbox_members WHERE inbox_id = $1")
            .bind(inbox)
            .fetch_all(conn)
            .await?;
    Ok(match (owner, members.as_slice()) {
        (Some(owner), members) if members.iter().all(|member| *member == owner) => Some(owner),
        (None, [only]) => Some(*only),
        _ => None,
    })
}
