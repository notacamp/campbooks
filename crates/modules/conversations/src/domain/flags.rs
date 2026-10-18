//! Read, star, archive, trash and spam: operations that set a flag on a conversation's
//! messages, so the provider can be told message by message.

use campbooks_kernel::{AppState, Error, Result, db::BeginFor as _, tenancy::Access};

use super::{
    access::lock_conversation,
    conversation::{Changed, ConversationSummary, request_each, summary},
    needs::{self, NeedSettings},
    rules,
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
    /// Move every message to the trash.
    Trash,
    /// Take every message out of the trash, back where it was.
    Untrash,
    /// Report every message as spam: out of the inbox, into spam.
    Spam,
    /// Take every message out of spam, back where it was (in the inbox only
    /// if it was there).
    Unspam,
}

impl Flag {
    /// The change the provider is asked to make, the same for every
    /// message. `None` for untrash and not spam: theirs carries each
    /// message's direction, so only [`change_items`] builds them.
    pub const fn requested(self) -> Option<RequestedChange> {
        Some(match self {
            Self::Read => RequestedChange::MarkRead,
            Self::Unread => RequestedChange::MarkUnread,
            Self::Star => RequestedChange::Star,
            Self::Unstar => RequestedChange::Unstar,
            Self::Archive => RequestedChange::Archive,
            Self::Unarchive => RequestedChange::Unarchive,
            Self::Trash => RequestedChange::Trash,
            Self::Spam => RequestedChange::Spam,
            Self::Untrash | Self::Unspam => return None,
        })
    }

    /// [`Flag::requested`], or an internal error for untrash and not spam.
    pub fn requested_for_all(self) -> Result<RequestedChange> {
        self.requested().ok_or_else(|| {
            Error::internal("untrash and not spam carry each message's direction (change_items)")
        })
    }

    pub const fn change(self) -> Change {
        match self {
            Self::Read | Self::Unread => Change::Read,
            Self::Star | Self::Unstar => Change::Starred,
            Self::Archive | Self::Unarchive => Change::Archived,
            Self::Trash | Self::Untrash | Self::Spam | Self::Unspam => Change::Messages,
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
    let inbox = lock_conversation(state, &mut tx, access, id).await?;
    let items = change_items(&mut tx, id, flag).await?;
    let changed: Vec<String> = items.iter().map(|(message, _)| message.clone()).collect();
    let undone = match flag {
        Flag::Unarchive => Some(rules::Undone::Archive),
        Flag::Unread => Some(rules::Undone::Read),
        Flag::Untrash | Flag::Unspam => Some(rules::Undone::Bin),
        _ => None,
    };
    let exception = match undone {
        Some(undone) if !changed.is_empty() => {
            rules::except(&mut tx, id, undone, access.user).await?
        }
        _ => None,
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
        request_each(&state.binding, &state.events, &mut tx, &changed, &items).await?;
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
    let mut summary = summary(&mut tx, access.user, id).await?;
    summary.rule_exception = exception;
    tx.commit().await?;
    Ok(summary)
}

/// Set `flag` on conversation `id`'s messages in `conn`, returning each
/// message that changed with the change the provider is asked to make: for
/// untrash and not spam, with the message's own direction.
pub async fn change_items(
    conn: &mut sqlx::PgConnection,
    id: ConversationId,
    flag: Flag,
) -> Result<Vec<(String, RequestedChange)>> {
    if matches!(flag, Flag::Untrash | Flag::Unspam) {
        return out_of_bin(conn, id, flag).await;
    }
    let requested = flag.requested_for_all()?;
    Ok(change(conn, id, flag)
        .await?
        .into_iter()
        .map(|message| (message, requested.clone()))
        .collect())
}

/// Set `flag` on conversation `id`'s messages in `conn`, returning the
/// provider ids of the messages that changed (none when it already was so).
pub async fn change(
    conn: &mut sqlx::PgConnection,
    id: ConversationId,
    flag: Flag,
) -> Result<Vec<String>> {
    let changed = match flag {
        Flag::Read => {
            sqlx::query_scalar!(
                "UPDATE conversations.messages SET read = true, updated_at = now()
                  WHERE conversation_id = $1 AND NOT read RETURNING provider_message_id",
                id as ConversationId
            )
            .fetch_all(&mut *conn)
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
            .fetch_all(&mut *conn)
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
            .fetch_all(&mut *conn)
            .await?
        }
        Flag::Unstar => {
            sqlx::query_scalar!(
                "UPDATE conversations.messages SET starred = false, updated_at = now()
                  WHERE conversation_id = $1 AND starred RETURNING provider_message_id",
                id as ConversationId
            )
            .fetch_all(&mut *conn)
            .await?
        }
        Flag::Archive => {
            sqlx::query_scalar!(
                "UPDATE conversations.messages SET in_inbox = false, updated_at = now()
                  WHERE conversation_id = $1 AND in_inbox RETURNING provider_message_id",
                id as ConversationId
            )
            .fetch_all(&mut *conn)
            .await?
        }
        Flag::Unarchive => {
            sqlx::query_scalar!(
                "UPDATE conversations.messages SET in_inbox = true, updated_at = now()
                  WHERE conversation_id = $1 AND NOT in_inbox AND NOT in_trash AND NOT in_spam
                 RETURNING provider_message_id",
                id as ConversationId
            )
            .fetch_all(&mut *conn)
            .await?
        }
        Flag::Trash | Flag::Untrash | Flag::Spam | Flag::Unspam => bin(conn, id, flag).await?,
    };
    Ok(changed)
}

/// [`change`] for the trash and spam (into either, and out of either).
async fn bin(conn: &mut sqlx::PgConnection, id: ConversationId, flag: Flag) -> Result<Vec<String>> {
    match flag {
        // Trash and spam exclude each other. Trash leaves the inbox flag as
        // it is, so untrash puts the message back where it was.
        Flag::Trash => Ok(sqlx::query_scalar!(
            "UPDATE conversations.messages SET in_trash = true, in_spam = false,
                    updated_at = now()
              WHERE conversation_id = $1 AND (NOT in_trash OR in_spam)
             RETURNING provider_message_id",
            id as ConversationId
        )
        .fetch_all(&mut *conn)
        .await?),
        // The whole conversation, as providers report a thread: into spam,
        // out of the inbox.
        Flag::Spam => Ok(sqlx::query_scalar!(
            "UPDATE conversations.messages
                SET in_spam = true, in_inbox = false, in_trash = false, updated_at = now()
              WHERE conversation_id = $1 AND NOT in_spam RETURNING provider_message_id",
            id as ConversationId
        )
        .fetch_all(&mut *conn)
        .await?),
        Flag::Untrash | Flag::Unspam => Ok(out_of_bin(conn, id, flag)
            .await?
            .into_iter()
            .map(|(message, _)| message)
            .collect()),
        Flag::Read | Flag::Unread | Flag::Star | Flag::Unstar | Flag::Archive | Flag::Unarchive => {
            Err(Error::internal("only the trash and spam are binned"))
        }
    }
}

/// Take `messages` (provider ids) of conversation `id` out of the trash or
/// spam, as a rule's undo does: only the messages it binned. Each comes
/// with the change the provider is asked to make, with its own direction.
pub async fn unbin_messages(
    conn: &mut sqlx::PgConnection,
    id: ConversationId,
    flag: Flag,
    messages: &[String],
) -> Result<Vec<(String, RequestedChange)>> {
    let rows = match flag {
        Flag::Untrash => sqlx::query!(
            r#"UPDATE conversations.messages SET in_trash = false, updated_at = now()
                WHERE conversation_id = $1 AND in_trash AND provider_message_id = ANY($2)
               RETURNING provider_message_id, direction = 'inbound' AS "inbound!""#,
            id as ConversationId,
            messages,
        )
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(|r| {
            (
                r.provider_message_id,
                RequestedChange::Untrash { inbound: r.inbound },
            )
        })
        .collect(),
        Flag::Unspam => sqlx::query!(
            r#"UPDATE conversations.messages
                  SET in_spam = false, in_inbox = direction = 'inbound', updated_at = now()
                WHERE conversation_id = $1 AND in_spam AND provider_message_id = ANY($2)
               RETURNING provider_message_id, direction = 'inbound' AS "inbound!""#,
            id as ConversationId,
            messages,
        )
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(|r| {
            (
                r.provider_message_id,
                RequestedChange::Unspam { inbound: r.inbound },
            )
        })
        .collect(),
        Flag::Read
        | Flag::Unread
        | Flag::Star
        | Flag::Unstar
        | Flag::Archive
        | Flag::Unarchive
        | Flag::Trash
        | Flag::Spam => {
            return Err(Error::internal("only untrash and not spam take mail out"));
        }
    };
    Ok(rows)
}

/// Take conversation `id`'s messages out of the trash (untrash) or spam (not
/// spam), returning each message that changed with its change and whether
/// it is mail the inbox received. Untrash leaves the inbox flag as it was;
/// not spam puts received mail back in the inbox (as a provider's own "Not
/// spam" does, whoever reported it) and never the member's own mail.
async fn out_of_bin(
    conn: &mut sqlx::PgConnection,
    id: ConversationId,
    flag: Flag,
) -> Result<Vec<(String, RequestedChange)>> {
    let rows = match flag {
        Flag::Untrash => sqlx::query!(
            r#"UPDATE conversations.messages SET in_trash = false, updated_at = now()
                WHERE conversation_id = $1 AND in_trash
               RETURNING provider_message_id, direction = 'inbound' AS "inbound!""#,
            id as ConversationId
        )
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(|r| {
            (
                r.provider_message_id,
                RequestedChange::Untrash { inbound: r.inbound },
            )
        })
        .collect(),
        Flag::Unspam => sqlx::query!(
            r#"UPDATE conversations.messages
                  SET in_spam = false, in_inbox = direction = 'inbound', updated_at = now()
                WHERE conversation_id = $1 AND in_spam
               RETURNING provider_message_id, direction = 'inbound' AS "inbound!""#,
            id as ConversationId
        )
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(|r| {
            (
                r.provider_message_id,
                RequestedChange::Unspam { inbound: r.inbound },
            )
        })
        .collect(),
        Flag::Read
        | Flag::Unread
        | Flag::Star
        | Flag::Unstar
        | Flag::Archive
        | Flag::Unarchive
        | Flag::Trash
        | Flag::Spam => {
            return Err(Error::internal("only untrash and not spam take mail out"));
        }
    };
    Ok(rows)
}
