//! A conversation's derived state (counts, flags) and how features read it
//! and announce its changes.

use campbooks_kernel::{Error, EventBus, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, Postgres, Transaction};
use utoipa::ToSchema;

use crate::{
    events::{
        Change, ChangesRequested, ConversationChanged, ConversationState, Origin, RequestOrigin,
        RequestedChange, RequestedItem,
    },
    model::{Address, ConversationId, InboxId, LabelId, Stream, StreamSource},
};

/// A label as shown on a conversation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct LabelView {
    /// The label.
    pub id: LabelId,
    /// Its name.
    pub name: String,
}

/// A conversation as lists show it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ConversationSummary {
    /// The conversation.
    pub id: ConversationId,
    /// Its inbox.
    pub inbox_id: InboxId,
    /// The first message's subject.
    pub subject: String,
    /// Its stream.
    pub stream: Stream,
    /// Who put it there.
    pub stream_source: StreamSource,
    /// When its newest message was sent.
    pub last_message_at: DateTime<Utc>,
    /// Messages in it.
    pub message_count: u32,
    /// Unread inbound messages.
    pub unread_count: u32,
    /// Whether any message is starred.
    pub starred: bool,
    /// Whether no message is in the inbox.
    pub archived: bool,
    /// The sender of the newest message.
    pub latest_from: Option<Address>,
    /// The start of the newest message's text.
    pub snippet: String,
    /// Its labels, by name.
    pub labels: Vec<LabelView>,
}

/// A row of the summary queries.
pub struct SummaryRow {
    pub id: ConversationId,
    pub inbox_id: InboxId,
    pub subject: String,
    pub stream: String,
    pub stream_source: String,
    pub last_message_at: DateTime<Utc>,
    pub message_count: i32,
    pub unread_count: i32,
    pub starred: bool,
    pub archived: bool,
    pub sender: Option<serde_json::Value>,
    pub snippet: Option<String>,
    pub labels: serde_json::Value,
}

impl TryFrom<SummaryRow> for ConversationSummary {
    type Error = Error;

    fn try_from(row: SummaryRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            inbox_id: row.inbox_id,
            subject: row.subject,
            stream: row.stream.parse()?,
            stream_source: row.stream_source.parse()?,
            last_message_at: row.last_message_at,
            message_count: count(row.message_count),
            unread_count: count(row.unread_count),
            starred: row.starred,
            archived: row.archived,
            latest_from: row
                .sender
                .map(serde_json::from_value)
                .transpose()
                .map_err(|e| Error::internal(crate::domain::redact::json(&e)))?,
            snippet: row.snippet.unwrap_or_default(),
            labels: serde_json::from_value(row.labels)
                .map_err(|e| Error::internal(crate::domain::redact::json(&e)))?,
        })
    }
}

fn count(n: i32) -> u32 {
    u32::try_from(n).unwrap_or_default()
}

/// The summary of conversation `id` (already checked visible).
pub async fn summary(conn: &mut PgConnection, id: ConversationId) -> Result<ConversationSummary> {
    let row = sqlx::query_as!(
        SummaryRow,
        r#"
        SELECT c.id AS "id: ConversationId", c.inbox_id AS "inbox_id: InboxId", c.subject,
               c.stream, c.stream_source, c.last_message_at, c.message_count, c.unread_count,
               c.starred, c.archived_at IS NOT NULL AS "archived!",
               m.sender AS "sender?", m.snippet AS "snippet?",
               coalesce((SELECT jsonb_agg(jsonb_build_object('id', l.id, 'name', l.name)
                                          ORDER BY l.name)
                           FROM conversations.conversation_labels cl
                           JOIN conversations.labels l ON l.id = cl.label_id
                          WHERE cl.conversation_id = c.id), '[]') AS "labels!"
          FROM conversations.conversations c
          LEFT JOIN LATERAL (
              SELECT sender, snippet FROM conversations.messages
               WHERE conversation_id = c.id ORDER BY sent_at DESC, id DESC LIMIT 1
          ) m ON true
         WHERE c.id = $1
        "#,
        id as ConversationId,
    )
    .fetch_optional(conn)
    .await?
    .ok_or(Error::NotFound)?;
    row.try_into()
}

/// Recompute conversation `id`'s counts and flags from its messages; delete
/// it when it has none. Returns its state, or `None` once deleted.
pub async fn recount(
    conn: &mut PgConnection,
    id: ConversationId,
) -> Result<Option<ConversationState>> {
    let row = sqlx::query!(
        r#"
        WITH s AS (
            SELECT count(*)::int AS n,
                   count(*) FILTER (WHERE direction = 'inbound' AND NOT read)::int AS unread,
                   coalesce(bool_or(starred), false) AS starred,
                   max(sent_at) AS last,
                   coalesce(bool_or(in_inbox), false) AS in_inbox,
                   coalesce(bool_and(in_trash OR in_spam), false) AS trashed
              FROM conversations.messages WHERE conversation_id = $1
        )
        UPDATE conversations.conversations c
           SET message_count = s.n, unread_count = s.unread, starred = s.starred,
               last_message_at = coalesce(s.last, c.last_message_at),
               archived_at = CASE WHEN s.in_inbox THEN NULL ELSE coalesce(c.archived_at, now()) END,
               trashed = s.trashed, updated_at = now()
          FROM s
         WHERE c.id = $1
        RETURNING s.n AS "n!", c.stream, c.unread_count, c.starred,
                  c.archived_at IS NOT NULL AS "archived!", c.trashed
        "#,
        id as ConversationId,
    )
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(Error::NotFound)?;
    if row.n == 0 {
        sqlx::query!(
            "DELETE FROM conversations.conversations WHERE id = $1",
            id as ConversationId
        )
        .execute(conn)
        .await?;
        return Ok(None);
    }
    Ok(Some(ConversationState {
        stream: row.stream.parse()?,
        unread_count: count(row.unread_count),
        starred: row.starred,
        archived: row.archived,
        trashed: row.trashed,
    }))
}

/// The provider ids of conversation `id`'s messages.
pub async fn provider_message_ids(
    conn: &mut PgConnection,
    id: ConversationId,
) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT provider_message_id FROM conversations.messages
          WHERE conversation_id = $1 ORDER BY sent_at, id",
        id as ConversationId
    )
    .fetch_all(conn)
    .await?)
}

/// A change to announce, before its state is known.
#[derive(Debug)]
pub struct Changed {
    /// The workspace, as the event bus keys it.
    pub workspace: String,
    /// The inbox.
    pub inbox: InboxId,
    /// The conversation.
    pub conversation: ConversationId,
    /// Where it came from.
    pub origin: Origin,
    /// What changed.
    pub changes: Vec<Change>,
    /// The messages it applies to.
    pub provider_message_ids: Vec<String>,
}

impl Changed {
    /// Recount the conversation and publish `conversation_changed` in `tx`.
    pub async fn publish(
        mut self,
        events: &EventBus,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<()> {
        let state = recount(tx, self.conversation).await?;
        if state.is_none() && !self.changes.contains(&Change::Removed) {
            self.changes.push(Change::Removed);
        }
        self.changes.sort_unstable();
        self.changes.dedup();
        events
            .publish(
                tx,
                &ConversationChanged {
                    workspace: self.workspace,
                    inbox: self.inbox,
                    conversation: self.conversation,
                    origin: self.origin,
                    changes: self.changes,
                    provider_message_ids: self.provider_message_ids,
                    state,
                },
            )
            .await?;
        Ok(())
    }
}

/// Publish `changes_requested` for a member's change: `change` for each of
/// `provider_message_ids`. Nothing is published when the list is empty.
pub async fn request(
    events: &EventBus,
    tx: &mut Transaction<'_, Postgres>,
    changed: &Changed,
    change: &RequestedChange,
) -> Result<()> {
    if changed.provider_message_ids.is_empty() {
        return Ok(());
    }
    let items = changed
        .provider_message_ids
        .iter()
        .map(|id| RequestedItem {
            provider_message_id: id.clone(),
            change: change.clone(),
        })
        .collect();
    events
        .publish(
            tx,
            &ChangesRequested {
                workspace: changed.workspace.clone(),
                inbox_id: changed.inbox,
                conversation: changed.conversation,
                origin: RequestOrigin::User,
                items,
            },
        )
        .await?;
    Ok(())
}
