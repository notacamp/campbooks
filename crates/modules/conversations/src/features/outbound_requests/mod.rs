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
//! - A requested change stands only when this module recorded it, under
//!   the id of the event that carried it, for that conversation and inbox
//!   (`conversation::request` records every request it publishes). A member's
//!   change and its undo both stand, as before; the connector merges them.

use std::sync::Arc;

use campbooks_kernel::{
    AppState,
    blobs::BlobRef,
    outbound::{OutboundPort, OutboundRequests, StandingFuture, StoredSend, StoredSendFuture},
    tenancy::{UserId, WorkspaceId},
};
use serde::Deserialize as _;
use sqlx::{PgConnection, types::Uuid};

use crate::{
    events::RequestedItem,
    features::release_send::author_may_send,
    model::{ConversationId, DraftId, DraftState, InboxId, SendId},
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
        request: Uuid,
        inbox: Uuid,
        conversation: Uuid,
        items: &'a [serde_json::Value],
    ) -> StandingFuture<'a> {
        Box::pin(async move {
            let recorded = sqlx::query_scalar!(
                r#"SELECT items FROM conversations.change_requests
                    WHERE id = $1 AND conversation_id = $2 AND inbox_id = $3"#,
                request,
                ConversationId::from(conversation) as ConversationId,
                InboxId::from(inbox) as InboxId,
            )
            .fetch_optional(&mut *conn)
            .await?;
            let recorded: Vec<RequestedItem> = recorded
                .and_then(|items| Vec::<RequestedItem>::deserialize(&items).ok())
                .unwrap_or_default();
            Ok(items
                .iter()
                .map(|item| {
                    RequestedItem::deserialize(item).is_ok_and(|item| recorded.contains(&item))
                })
                .collect())
        })
    }
}

#[cfg(test)]
mod tests;
