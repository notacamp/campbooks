//! A conversation's derived state (counts, flags) and how features read it
//! and announce its changes.

use campbooks_kernel::{
    Error, EventBus, Result,
    binding::Binding,
    tenancy::{UserId, WorkspaceId},
};
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

use super::{
    needs::{self, NeedView},
    rules::{self, RuleException, RuleMark},
    suggestions::{self, StreamSuggestion},
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
#[expect(
    clippy::struct_excessive_bools,
    reason = "the wire shape: independent facts about one conversation"
)]
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
    /// Whether every message is in spam or the trash.
    pub trashed: bool,
    /// Whether it is binned as spam (in Spam rather than in Trash); only
    /// ever true when `trashed`.
    pub spam: bool,
    /// When the inbox last wrote in it, outside spam and the trash; none
    /// when it wrote nothing there. Sent lists it by this.
    pub last_sent_at: Option<DateTime<Utc>>,
    /// Who the inbox's latest message there went to (to, then cc); empty
    /// when it wrote nothing there.
    pub sent_to: Vec<Address>,
    /// Whether someone wrote back after the inbox's latest message (an
    /// inbound message newer than `last_sent_at`, outside spam and the trash).
    pub replied_since_sent: bool,
    /// The sender of the newest message.
    pub latest_from: Option<Address>,
    /// Whether the newest message is one the inbox sent (outbound).
    pub latest_from_you: bool,
    /// The start of the newest message's text.
    pub snippet: String,
    /// Its labels, by name.
    pub labels: Vec<LabelView>,
    /// Where the decide pass would put it instead, when it would: a
    /// suggestion, never applied by itself.
    pub suggestion: Option<StreamSuggestion>,
    /// What it needs from its inbox's members, while it needs something.
    pub need: Option<NeedView>,
    /// The taught rule that placed or changed it, while no member changed
    /// that back.
    pub rule: Option<RuleMark>,
    /// In the response of an operation that changed back what a rule did:
    /// that rule, now leaving this conversation alone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_exception: Option<RuleException>,
}

/// The columns of a [`SummaryRow`] from `conversations.conversations c`, for
/// every query that lists conversations: callers add their `WHERE`, `ORDER
/// BY` and `LIMIT`.
pub const SUMMARY_SELECT: &str = r"
    SELECT c.id, c.inbox_id, c.subject, c.stream, c.stream_source, c.last_message_at,
           c.message_count, c.unread_count, c.starred, c.archived_at IS NOT NULL AS archived,
           c.trashed, c.spam, c.last_sent_at,
           coalesce(o.recipients -> 'to', '[]') || coalesce(o.recipients -> 'cc', '[]')
               AS sent_to,
           c.last_sent_at IS NOT NULL AND EXISTS (
               SELECT 1 FROM conversations.messages r
                WHERE r.conversation_id = c.id AND r.direction = 'inbound'
                  AND NOT r.in_spam AND NOT r.in_trash AND r.sent_at > c.last_sent_at
           ) AS replied_since_sent,
           m.sender, m.snippet, coalesce(m.direction = 'outbound', false) AS latest_from_you,
           coalesce((SELECT jsonb_agg(jsonb_build_object('id', l.id, 'name', l.name)
                                      ORDER BY l.name)
                       FROM conversations.conversation_labels cl
                       JOIN conversations.labels l ON l.id = cl.label_id
                      WHERE cl.conversation_id = c.id), '[]') AS labels
      FROM conversations.conversations c
      LEFT JOIN LATERAL (
          SELECT sender, snippet, direction FROM conversations.messages
           WHERE conversation_id = c.id ORDER BY sent_at DESC, id DESC LIMIT 1
      ) m ON true
      LEFT JOIN LATERAL (
          SELECT recipients FROM conversations.messages
           WHERE conversation_id = c.id AND direction = 'outbound'
             AND NOT in_spam AND NOT in_trash
           ORDER BY sent_at DESC, id DESC LIMIT 1
      ) o ON true";

/// A row of [`SUMMARY_SELECT`].
#[derive(sqlx::FromRow)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "one column each of the summary's facts"
)]
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
    pub trashed: bool,
    pub spam: bool,
    pub last_sent_at: Option<DateTime<Utc>>,
    pub sent_to: serde_json::Value,
    pub replied_since_sent: bool,
    pub sender: Option<serde_json::Value>,
    pub snippet: Option<String>,
    pub latest_from_you: bool,
    pub labels: serde_json::Value,
}

impl TryFrom<SummaryRow> for ConversationSummary {
    type Error = Error;

    fn try_from(row: SummaryRow) -> Result<Self> {
        let json = |e: serde_json::Error| Error::internal(crate::domain::redact::json(&e));
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
            trashed: row.trashed,
            spam: row.spam,
            last_sent_at: row.last_sent_at,
            sent_to: serde_json::from_value(row.sent_to).map_err(json)?,
            replied_since_sent: row.replied_since_sent,
            latest_from: row
                .sender
                .map(serde_json::from_value)
                .transpose()
                .map_err(json)?,
            latest_from_you: row.latest_from_you,
            snippet: row.snippet.unwrap_or_default(),
            labels: serde_json::from_value(row.labels).map_err(json)?,
            suggestion: None,
            need: None,
            rule: None,
            rule_exception: None,
        })
    }
}

fn count(n: i32) -> u32 {
    u32::try_from(n).unwrap_or_default()
}

/// The summary of conversation `id` (already checked visible).
pub async fn summary(
    conn: &mut PgConnection,
    user: Option<UserId>,
    id: ConversationId,
) -> Result<ConversationSummary> {
    let row: SummaryRow = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "{SUMMARY_SELECT} WHERE c.id = $1"
    )))
    .bind(id)
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(Error::NotFound)?;
    let mut summary = [ConversationSummary::try_from(row)?];
    suggestions::attach(conn, &mut summary).await?;
    needs::attach(conn, &mut summary).await?;
    rules::attach(conn, user, &mut summary).await?;
    let [summary] = summary;
    Ok(summary)
}

/// What [`recount`] works out of the messages, and the state it returns.
/// `spam` and `last_sent_at` are what the Spam, Trash and Sent lists page on
/// (campbooks-cloud#240): a binned conversation is in Spam when a message is
/// in spam and not in the trash, and it was last written in at the newest
/// outbound message in neither.
const RECOUNT: &str = r"
    WITH s AS (
        SELECT count(*)::int AS n,
               count(*) FILTER (WHERE direction = 'inbound' AND NOT read)::int AS unread,
               coalesce(bool_or(starred), false) AS starred,
               max(sent_at) AS last,
               coalesce(bool_or(in_inbox), false) AS in_inbox,
               coalesce(bool_and(in_trash OR in_spam), false) AS trashed,
               coalesce(bool_or(in_spam AND NOT in_trash), false) AS in_spam,
               max(sent_at) FILTER (WHERE direction = 'outbound'
                                      AND NOT in_spam AND NOT in_trash) AS last_sent
          FROM conversations.messages WHERE conversation_id = $1
    )
    UPDATE conversations.conversations c
       SET message_count = s.n, unread_count = s.unread, starred = s.starred,
           last_message_at = coalesce(s.last, c.last_message_at),
           archived_at = CASE WHEN s.in_inbox THEN NULL ELSE coalesce(c.archived_at, now()) END,
           trashed = s.trashed, spam = s.trashed AND s.in_spam, last_sent_at = s.last_sent,
           updated_at = now()
      FROM s
     WHERE c.id = $1
    RETURNING s.n, c.stream, c.unread_count, c.starred,
              c.archived_at IS NOT NULL AS archived, c.trashed";

/// A row of [`RECOUNT`].
#[derive(sqlx::FromRow)]
struct Recounted {
    n: i32,
    stream: String,
    unread_count: i32,
    starred: bool,
    archived: bool,
    trashed: bool,
}

/// Recompute conversation `id`'s counts and flags from its messages; delete
/// it when it has none. Returns its state, or `None` once deleted.
pub async fn recount(
    conn: &mut PgConnection,
    id: ConversationId,
) -> Result<Option<ConversationState>> {
    let row: Option<Recounted> = sqlx::query_as(RECOUNT)
        .bind(id)
        .fetch_optional(&mut *conn)
        .await?;
    let row = row.ok_or(Error::NotFound)?;
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
        let empty = sqlx::query_scalar!(
            r#"SELECT NOT EXISTS (SELECT 1 FROM conversations.messages
                                   WHERE conversation_id = $1) AS "empty!""#,
            self.conversation as ConversationId,
        )
        .fetch_one(&mut **tx)
        .await?;
        if empty {
            // Announce its need as gone before the row goes with it.
            needs::conversation_gone(tx, events, &self.workspace, self.inbox, self.conversation)
                .await?;
        }
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

/// Publish `changes_requested` for a member's change of `changed`'s
/// conversation: each item's message with its own change. Nothing is
/// published when there is none.
pub async fn request_each(
    binding: &Binding,
    events: &EventBus,
    tx: &mut Transaction<'_, Postgres>,
    changed: &Changed,
    items: &[(String, RequestedChange)],
) -> Result<()> {
    let items = items
        .iter()
        .map(|(message, change)| RequestedItem {
            provider_message_id: message.clone(),
            change: change.clone(),
        })
        .collect();
    publish_request(binding, events, tx, changed, items).await
}

/// Publish `changes_requested` for a member's change: `change` for each of
/// `provider_message_ids`. Nothing is published when the list is empty.
pub async fn request(
    binding: &Binding,
    events: &EventBus,
    tx: &mut Transaction<'_, Postgres>,
    changed: &Changed,
    change: &RequestedChange,
) -> Result<()> {
    let items = changed
        .provider_message_ids
        .iter()
        .map(|id| RequestedItem {
            provider_message_id: id.clone(),
            change: change.clone(),
        })
        .collect();
    publish_request(binding, events, tx, changed, items).await
}

/// Publish `changes_requested` with `items` for `changed`'s conversation,
/// and record it under the event's id, which the connector asks about
/// before making it (the outbound-requests port). The only place the event
/// is published. Requests on the same conversation older than seven days go
/// with it. Nothing is published when there is no item. The record is
/// bound with `binding` (campbooks-cloud#227); without a key it is recorded
/// unbound, and none of its changes is made at the provider.
async fn publish_request(
    binding: &Binding,
    events: &EventBus,
    tx: &mut Transaction<'_, Postgres>,
    changed: &Changed,
    items: Vec<RequestedItem>,
) -> Result<()> {
    if items.is_empty() {
        return Ok(());
    }
    let recorded = serde_json::to_value(&items).map_err(Error::internal)?;
    let workspace: WorkspaceId = changed
        .workspace
        .parse()
        .map_err(|_| Error::internal("a change request outside a workspace"))?;
    let signed = items.clone();
    let id = events
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
    sqlx::query!(
        "DELETE FROM conversations.change_requests
          WHERE conversation_id = $1 AND created_at < now() - interval '7 days'",
        changed.conversation as ConversationId,
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query!(
        r#"INSERT INTO conversations.change_requests
               (id, workspace_id, inbox_id, conversation_id, items)
           VALUES ($1, $2::text::uuid, $3, $4, $5)"#,
        id.as_uuid(),
        changed.workspace,
        changed.inbox as InboxId,
        changed.conversation as ConversationId,
        recorded,
    )
    .execute(&mut **tx)
    .await?;
    let mac = super::binding::ChangeRequest {
        workspace,
        request: *id.as_uuid(),
        inbox: changed.inbox,
        conversation: changed.conversation,
        items: &signed,
    }
    .sign(binding)?;
    sqlx::query("UPDATE conversations.change_requests SET mac = $2 WHERE id = $1")
        .bind(*id.as_uuid())
        .bind(mac)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
