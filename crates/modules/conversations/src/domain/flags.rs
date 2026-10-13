//! Read, star and archive: operations that set a flag on a conversation's
//! messages, so the provider can be told message by message.

use campbooks_kernel::{AppState, Result, db::BeginFor as _, tenancy::Access};

use super::{
    access::lock_conversation,
    needs::{self, NeedSettings},
    conversation::{Changed, ConversationSummary, request, summary},
};
use crate::{
    events::{Change, Origin, RequestedChange},
    model::ConversationId,
};

/// A flag operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flag {
    /// Mark every message read.
    Read,
    /// Mark the newest message unread.
    Unread,
    /// Star the newest message, unless one is starred.
    Star,
    /// Unstar every message.
    Unstar,
    /// Take every message out of the inbox.
    Archive,
    /// Put every message not in the trash or spam back in the inbox.
    Unarchive,
}

impl Flag {
    const fn requested(self) -> RequestedChange {
        match self {
            Self::Read => RequestedChange::MarkRead,
            Self::Unread => RequestedChange::MarkUnread,
            Self::Star => RequestedChange::Star,
            Self::Unstar => RequestedChange::Unstar,
            Self::Archive => RequestedChange::Archive,
            Self::Unarchive => RequestedChange::Unarchive,
        }
    }

    const fn change(self) -> Change {
        match self {
            Self::Read | Self::Unread => Change::Read,
            Self::Star | Self::Unstar => Change::Starred,
            Self::Archive | Self::Unarchive => Change::Archived,
        }
    }
}

/// Apply `flag` to conversation `id` for `access`; announce it when anything
/// changed, and work out its need again when given `needs` (archiving
/// clears Reply and Review). Returns the conversation afterwards. Applying
/// it twice is a no-op the second time.
pub async fn apply(
    state: &AppState,
    access: &Access,
    id: ConversationId,
    flag: Flag,
    needs: Option<&NeedSettings>,
) -> Result<ConversationSummary> {
    let mut tx = state.db.begin_for(access.workspace).await?;
    let inbox = lock_conversation(&mut tx, access, id).await?;
    let changed = match flag {
        Flag::Read => {
            sqlx::query_scalar!(
                "UPDATE conversations.messages SET read = true, updated_at = now()
                  WHERE conversation_id = $1 AND NOT read RETURNING provider_message_id",
                id as ConversationId
            )
            .fetch_all(&mut *tx)
            .await?
        }
        Flag::Unread => {
            sqlx::query_scalar!(
                "UPDATE conversations.messages SET read = false, updated_at = now()
                  WHERE id = (SELECT id FROM conversations.messages WHERE conversation_id = $1
                               ORDER BY direction = 'inbound' DESC, sent_at DESC, id DESC LIMIT 1)
                    AND read
                 RETURNING provider_message_id",
                id as ConversationId
            )
            .fetch_all(&mut *tx)
            .await?
        }
        Flag::Star => {
            sqlx::query_scalar!(
                "UPDATE conversations.messages SET starred = true, updated_at = now()
                  WHERE id = (SELECT id FROM conversations.messages WHERE conversation_id = $1
                               ORDER BY sent_at DESC, id DESC LIMIT 1)
                    AND NOT EXISTS (SELECT 1 FROM conversations.messages
                                     WHERE conversation_id = $1 AND starred)
                 RETURNING provider_message_id",
                id as ConversationId
            )
            .fetch_all(&mut *tx)
            .await?
        }
        Flag::Unstar => {
            sqlx::query_scalar!(
                "UPDATE conversations.messages SET starred = false, updated_at = now()
                  WHERE conversation_id = $1 AND starred RETURNING provider_message_id",
                id as ConversationId
            )
            .fetch_all(&mut *tx)
            .await?
        }
        Flag::Archive => {
            sqlx::query_scalar!(
                "UPDATE conversations.messages SET in_inbox = false, updated_at = now()
                  WHERE conversation_id = $1 AND in_inbox RETURNING provider_message_id",
                id as ConversationId
            )
            .fetch_all(&mut *tx)
            .await?
        }
        Flag::Unarchive => {
            sqlx::query_scalar!(
                "UPDATE conversations.messages SET in_inbox = true, updated_at = now()
                  WHERE conversation_id = $1 AND NOT in_inbox AND NOT in_trash AND NOT in_spam
                 RETURNING provider_message_id",
                id as ConversationId
            )
            .fetch_all(&mut *tx)
            .await?
        }
    };
    if !changed.is_empty() {
        let changed = Changed {
            workspace: access.workspace.to_string(),
            inbox,
            conversation: id,
            origin: Origin::Member,
            changes: vec![flag.change()],
            provider_message_ids: changed,
        };
        request(&state.events, &mut tx, &changed, &flag.requested()).await?;
        changed.publish(&state.events, &mut tx).await?;
        if let Some(needs) = needs {
            needs::refresh(
                &mut tx,
                &state.events,
                &access.workspace.to_string(),
                id,
                needs,
            )
            .await?;
        }
    }
    let summary = summary(&mut tx, id).await?;
    tx.commit().await?;
    Ok(summary)
}
