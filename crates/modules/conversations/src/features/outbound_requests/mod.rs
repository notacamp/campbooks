//! The kernel's outbound-requests port (campbooks-cloud#226): before the
//! connector sends a message or mirrors a change at the provider, it asks
//! this module whether its own records still say so. An event alone proves
//! nothing: any transaction working in a workspace can append that
//! workspace's `send_requested` or `changes_requested`.
//!
//! - A send stands while its draft is `sending` as that very send, with the
//!   outgoing document `release_send` recorded, and its author may still
//!   send it (a member, allowed to send from the inbox). The connector sends
//!   that document, never one an event names.
//! - A requested change stands while the conversation in that inbox shows
//!   it: the message is read for `mark_read`, the label is on it with the
//!   key named for `add_label`, the conversation is in the stream for
//!   `move_to_stream`, and so on. A member's own change is applied here in
//!   the transaction that requests it, so it always stands when the
//!   connector asks, unless a later change undid it (which then stands in
//!   its place).

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use campbooks_kernel::{
    AppState,
    blobs::BlobRef,
    outbound::{OutboundPort, OutboundRequests, StandingFuture, StoredSend, StoredSendFuture},
    tenancy::{UserId, WorkspaceId},
};
use serde::Deserialize as _;
use sqlx::{PgConnection, types::Uuid};

use crate::{
    events::{RequestedChange, RequestedItem},
    features::release_send::author_may_send,
    model::{ConversationId, DraftId, DraftState, InboxId, LabelId, SendId},
};

/// The port, for the server to register.
pub fn outbound_requests() -> OutboundPort {
    Arc::new(StoredRequests)
}

#[derive(Debug, Clone, Copy)]
struct StoredRequests;

impl OutboundRequests for StoredRequests {
    fn stored_send<'a>(
        &'a self,
        state: &'a AppState,
        conn: &'a mut PgConnection,
        workspace: WorkspaceId,
        send: Uuid,
    ) -> StoredSendFuture<'a> {
        Box::pin(async move {
            // Held until the asking transaction ends: the send's outcome
            // (`record_send`) waits for it.
            let Some(draft) = sqlx::query!(
                r#"SELECT id AS "id: DraftId", inbox_id AS "inbox: InboxId",
                          author AS "author: UserId", state, send_document
                     FROM conversations.drafts
                    WHERE send_id = $1
                      FOR SHARE"#,
                SendId::from(send) as SendId,
            )
            .fetch_optional(&mut *conn)
            .await?
            else {
                return Ok(None);
            };
            if draft.state != DraftState::Sending.as_str() {
                return Ok(None);
            }
            let Some(message) = draft
                .send_document
                .and_then(|document| serde_json::from_value::<BlobRef>(document).ok())
            else {
                return Ok(None);
            };
            if !author_may_send(state, conn, workspace, draft.author, draft.id).await? {
                return Ok(None);
            }
            Ok(Some(StoredSend {
                inbox: draft.inbox.into(),
                message,
            }))
        })
    }

    fn changes_standing<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        _workspace: WorkspaceId,
        inbox: Uuid,
        conversation: Uuid,
        items: &'a [serde_json::Value],
    ) -> StandingFuture<'a> {
        Box::pin(async move {
            let parsed: Vec<Option<RequestedItem>> = items
                .iter()
                .map(|item| RequestedItem::deserialize(item).ok())
                .collect();
            let none = vec![false; items.len()];
            let conversation = ConversationId::from(conversation);
            let Some(stored) = sqlx::query!(
                r#"SELECT inbox_id AS "inbox: InboxId", stream
                     FROM conversations.conversations
                    WHERE id = $1"#,
                conversation as ConversationId,
            )
            .fetch_optional(&mut *conn)
            .await?
            else {
                return Ok(none);
            };
            if Uuid::from(stored.inbox) != inbox {
                return Ok(none);
            }
            let named: Vec<String> = parsed
                .iter()
                .flatten()
                .map(|item| item.provider_message_id.clone())
                .collect();
            let messages: HashMap<String, Flags> = sqlx::query!(
                r#"SELECT provider_message_id, read, starred, in_inbox
                     FROM conversations.messages
                    WHERE conversation_id = $1 AND provider_message_id = ANY($2)"#,
                conversation as ConversationId,
                &named,
            )
            .fetch_all(&mut *conn)
            .await?
            .into_iter()
            .map(|m| {
                (
                    m.provider_message_id,
                    Flags {
                        read: m.read,
                        starred: m.starred,
                        in_inbox: m.in_inbox,
                    },
                )
            })
            .collect();
            let on: HashSet<LabelId> = sqlx::query_scalar!(
                r#"SELECT label_id AS "label: LabelId"
                     FROM conversations.conversation_labels
                    WHERE conversation_id = $1"#,
                conversation as ConversationId,
            )
            .fetch_all(&mut *conn)
            .await?
            .into_iter()
            .collect();
            let asked: Vec<LabelId> = parsed
                .iter()
                .flatten()
                .filter_map(|item| match &item.change {
                    RequestedChange::AddLabel { label, .. }
                    | RequestedChange::RemoveLabel { label, .. } => Some(*label),
                    _ => None,
                })
                .collect();
            let keys: HashMap<LabelId, Option<String>> = sqlx::query!(
                r#"SELECT id AS "id: LabelId", provider_key
                     FROM conversations.labels
                    WHERE inbox_id = $1 AND id = ANY($2)"#,
                stored.inbox as InboxId,
                &asked as &[LabelId],
            )
            .fetch_all(&mut *conn)
            .await?
            .into_iter()
            .map(|l| (l.id, l.provider_key))
            .collect();
            let shown = Shown {
                stream: &stored.stream,
                messages: &messages,
                on: &on,
                keys: &keys,
            };
            Ok(parsed
                .iter()
                .map(|item| item.as_ref().is_some_and(|item| shown.stands(item)))
                .collect())
        })
    }
}

/// A message's flags as stored.
#[derive(Debug, Clone, Copy)]
struct Flags {
    read: bool,
    starred: bool,
    in_inbox: bool,
}

/// The conversation as stored: what a requested change is checked against.
struct Shown<'a> {
    stream: &'a str,
    messages: &'a HashMap<String, Flags>,
    on: &'a HashSet<LabelId>,
    keys: &'a HashMap<LabelId, Option<String>>,
}

impl Shown<'_> {
    /// Whether `item` is what the conversation shows now.
    fn stands(&self, item: &RequestedItem) -> bool {
        let Some(flags) = self.messages.get(&item.provider_message_id) else {
            return false; // Not a message of this conversation.
        };
        let key_is = |label: &LabelId, key: Option<&str>| {
            self.keys
                .get(label)
                .is_some_and(|stored| stored.as_deref() == key)
        };
        match &item.change {
            RequestedChange::MarkRead => flags.read,
            RequestedChange::MarkUnread => !flags.read,
            RequestedChange::Star => flags.starred,
            RequestedChange::Unstar => !flags.starred,
            RequestedChange::Archive => !flags.in_inbox,
            RequestedChange::Unarchive => flags.in_inbox,
            RequestedChange::AddLabel {
                label,
                provider_key,
            } => self.on.contains(label) && key_is(label, provider_key.as_deref()),
            RequestedChange::RemoveLabel {
                label,
                provider_key,
            } => !self.on.contains(label) && key_is(label, provider_key.as_deref()),
            RequestedChange::MoveToStream { stream } => stream.as_str() == self.stream,
        }
    }
}

#[cfg(test)]
mod tests;
