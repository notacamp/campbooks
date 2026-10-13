//! Drafts as the features share them: who may see and change one, how a
//! row reads, and the limits a draft keeps to.
//!
//! A draft is private to its author: a user sees only their own, a client
//! token only those client tokens wrote. Writing or sending one takes the
//! `sender` role or above on its inbox.

use campbooks_kernel::{Error, Result, blobs::BlobRef, tenancy::Access};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use utoipa::ToSchema;

use crate::{
    domain::redact,
    model::{
        Address, AttachmentId, ConversationId, DraftId, DraftKind, DraftState, InboxId, InboxRole,
        MessageId,
    },
};

/// Most recipients a draft may have, To, Cc and Bcc together.
pub const MAX_RECIPIENTS: usize = 100;
/// Longest subject, in characters (RFC 5322's line limit).
pub const MAX_SUBJECT_CHARS: usize = 998;
/// Most bytes of body and quoted history together.
pub const MAX_TEXT_BYTES: usize = 1024 * 1024;
/// Most bytes of attachments a message may carry (Gmail's limit).
pub const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
/// Most message ids kept in `References`.
pub const MAX_REFERENCES: usize = 20;

/// A forwarded attachment, as the draft shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct DraftAttachment {
    /// The original message's attachment.
    pub id: AttachmentId,
    /// Its file name.
    pub filename: String,
    /// Its media type, as sent (not trusted).
    pub content_type: String,
    /// Its size in bytes.
    pub size: u64,
}

/// A forwarded attachment as the row keeps it: with its blob, which the
/// draft holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredAttachment {
    /// The original message's attachment.
    pub id: AttachmentId,
    /// Its file name.
    pub filename: String,
    /// Its media type.
    pub content_type: String,
    /// Its size in bytes.
    pub size: u64,
    /// The bytes.
    pub blob: BlobRef,
}

/// A draft.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct DraftView {
    /// The draft.
    pub id: DraftId,
    /// The inbox it is sent from.
    pub inbox_id: InboxId,
    /// What it answers.
    pub kind: DraftKind,
    /// The conversation it answers, if any.
    pub conversation_id: Option<ConversationId>,
    /// The message it replies to or forwards, if any.
    pub message_id: Option<MessageId>,
    /// The inbox's address and name.
    pub from: Address,
    /// Recipients.
    pub to: Vec<Address>,
    /// Copied recipients.
    pub cc: Vec<Address>,
    /// Blind-copied recipients.
    pub bcc: Vec<Address>,
    /// The subject.
    pub subject: String,
    /// What the member writes: plain text with basic formatting.
    pub body: String,
    /// The quoted history under it.
    pub quoted: String,
    /// The forwarded attachments.
    pub attachments: Vec<DraftAttachment>,
    /// Bumped by every save; name it when saving.
    pub version: u32,
    /// Where it is in its life.
    pub state: DraftState,
    /// When a scheduled send leaves (until then it can be cancelled).
    pub send_at: Option<DateTime<Utc>>,
    /// Why the last send failed (`not_connected`, `needs_reauth`,
    /// `rejected`, `too_large`, `unknown_outcome`).
    pub failure_reason: Option<String>,
    /// When it was started.
    pub created_at: DateTime<Utc>,
    /// When it was last saved.
    pub updated_at: DateTime<Utc>,
}

/// A draft row with its inbox's address.
pub struct DraftRow {
    pub id: DraftId,
    pub inbox_id: InboxId,
    pub inbox_address: String,
    pub inbox_name: Option<String>,
    pub kind: String,
    pub conversation_id: Option<ConversationId>,
    pub source_message_id: Option<MessageId>,
    pub to_recipients: serde_json::Value,
    pub cc_recipients: serde_json::Value,
    pub bcc_recipients: serde_json::Value,
    pub subject: String,
    pub body: String,
    pub quoted: String,
    pub in_reply_to: Option<String>,
    pub reference_ids: Vec<String>,
    pub thread_key: Option<String>,
    pub attachments: serde_json::Value,
    pub version: i32,
    pub state: String,
    pub send_generation: i32,
    pub send_at: Option<DateTime<Utc>>,
    pub failure_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A JSON column as `T`.
pub fn decode<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> Result<T> {
    serde_json::from_value(value).map_err(|e| Error::internal(redact::json(&e)))
}

impl DraftRow {
    /// The draft's state.
    pub fn state(&self) -> Result<DraftState> {
        self.state.parse()
    }

    /// Its forwarded attachments, with their blobs.
    pub fn stored_attachments(&self) -> Result<Vec<StoredAttachment>> {
        decode(self.attachments.clone())
    }

    /// Its recipients: To, Cc and Bcc.
    pub fn recipients(&self) -> Result<(Vec<Address>, Vec<Address>, Vec<Address>)> {
        Ok((
            decode(self.to_recipients.clone())?,
            decode(self.cc_recipients.clone())?,
            decode(self.bcc_recipients.clone())?,
        ))
    }

    /// The inbox's address and name.
    pub fn from(&self) -> Address {
        Address {
            name: self.inbox_name.clone(),
            address: self.inbox_address.clone(),
        }
    }

    /// The draft as the API shows it.
    pub fn view(&self) -> Result<DraftView> {
        let (to, cc, bcc) = self.recipients()?;
        Ok(DraftView {
            id: self.id,
            inbox_id: self.inbox_id,
            kind: self.kind.parse()?,
            conversation_id: self.conversation_id,
            message_id: self.source_message_id,
            from: self.from(),
            to,
            cc,
            bcc,
            subject: self.subject.clone(),
            body: self.body.clone(),
            quoted: self.quoted.clone(),
            attachments: self
                .stored_attachments()?
                .into_iter()
                .map(|a| DraftAttachment {
                    id: a.id,
                    filename: a.filename,
                    content_type: a.content_type,
                    size: a.size,
                })
                .collect(),
            version: u32::try_from(self.version).unwrap_or_default(),
            state: self.state()?,
            send_at: self.send_at.filter(|_| self.state == "scheduled"),
            failure_reason: self.failure_reason.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}

/// Draft `id` as `access` may see it (`None` otherwise), locked for update
/// when `lock`.
pub async fn find(
    conn: &mut PgConnection,
    access: &Access,
    id: DraftId,
    lock: bool,
) -> Result<Option<DraftRow>> {
    let author = access.user.map(|u| *u.as_uuid());
    let row = if lock {
        sqlx::query_as!(
            DraftRow,
            r#"
            SELECT d.id AS "id: DraftId", d.inbox_id AS "inbox_id: InboxId",
                   i.address AS inbox_address, i.display_name AS inbox_name, d.kind,
                   d.conversation_id AS "conversation_id: ConversationId",
                   d.source_message_id AS "source_message_id: MessageId",
                   d.to_recipients, d.cc_recipients, d.bcc_recipients, d.subject, d.body,
                   d.quoted, d.in_reply_to, d.reference_ids, d.thread_key, d.attachments,
                   d.version, d.state, d.send_generation, d.send_at,
                   d.failure_reason, d.created_at, d.updated_at
              FROM conversations.drafts d
              JOIN conversations.inboxes i ON i.id = d.inbox_id
             WHERE d.id = $1 AND d.author IS NOT DISTINCT FROM $2
               FOR UPDATE OF d
            "#,
            id as DraftId,
            author,
        )
        .fetch_optional(&mut *conn)
        .await?
    } else {
        sqlx::query_as!(
            DraftRow,
            r#"
            SELECT d.id AS "id: DraftId", d.inbox_id AS "inbox_id: InboxId",
                   i.address AS inbox_address, i.display_name AS inbox_name, d.kind,
                   d.conversation_id AS "conversation_id: ConversationId",
                   d.source_message_id AS "source_message_id: MessageId",
                   d.to_recipients, d.cc_recipients, d.bcc_recipients, d.subject, d.body,
                   d.quoted, d.in_reply_to, d.reference_ids, d.thread_key, d.attachments,
                   d.version, d.state, d.send_generation, d.send_at,
                   d.failure_reason, d.created_at, d.updated_at
              FROM conversations.drafts d
              JOIN conversations.inboxes i ON i.id = d.inbox_id
             WHERE d.id = $1 AND d.author IS NOT DISTINCT FROM $2
            "#,
            id as DraftId,
            author,
        )
        .fetch_optional(&mut *conn)
        .await?
    };
    Ok(row)
}

/// Draft `id`, if `access` may see it and still send from its inbox: 404
/// when they cannot see it (or no longer belong to its inbox), 403
/// `inbox_read_only` when they only read the inbox.
pub async fn load(
    conn: &mut PgConnection,
    access: &Access,
    id: DraftId,
    lock: bool,
) -> Result<DraftRow> {
    let row = find(conn, access, id, lock).await?.ok_or(Error::NotFound)?;
    require_sender(conn, access, row.inbox_id).await?;
    Ok(row)
}

/// `access` may send from `inbox`: a user needs the `sender` or `manager`
/// role (a `reader` gets 403 `inbox_read_only`, a non-member 404); a client
/// token acts through its scope.
pub async fn require_sender(
    conn: &mut PgConnection,
    access: &Access,
    inbox: InboxId,
) -> Result<()> {
    let Some(user) = access.user else {
        return Ok(());
    };
    let role = sqlx::query_scalar!(
        "SELECT role FROM conversations.inbox_members WHERE inbox_id = $1 AND user_id = $2",
        inbox as InboxId,
        user.as_uuid(),
    )
    .fetch_optional(conn)
    .await?
    .map(|role| role.parse::<InboxRole>())
    .transpose()?;
    match role {
        None => Err(Error::NotFound),
        Some(InboxRole::Reader) => Err(Error::Denied(crate::MESSAGES.reason("inbox_read_only"))),
        Some(InboxRole::Sender | InboxRole::Manager) => Ok(()),
    }
}

/// Whether `address` looks like one mail can be sent to: one `@`, a local
/// part and a dotted domain, no spaces, angle brackets or separators.
pub fn valid_address(address: &str) -> bool {
    let Some((local, domain)) = address.rsplit_once('@') else {
        return false;
    };
    let clean = |part: &str| {
        !part.is_empty()
            && part
                .chars()
                .all(|c| !c.is_whitespace() && !c.is_control() && !"<>,;\"()[]\\@".contains(c))
    };
    address.len() <= 320
        && clean(local)
        && clean(domain)
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
}

/// Trimmed addresses and names, duplicates (by address, ignoring case)
/// dropped from each list and from later lists, first occurrence kept.
pub fn tidy(lists: [Vec<Address>; 3]) -> [Vec<Address>; 3] {
    let mut seen = std::collections::HashSet::new();
    lists.map(|list| {
        list.into_iter()
            .filter_map(|a| {
                let address = a.address.trim().to_owned();
                let name = a
                    .name
                    .map(|n| n.trim().to_owned())
                    .filter(|n| !n.is_empty());
                (!address.is_empty() && seen.insert(address.to_lowercase()))
                    .then_some(Address { name, address })
            })
            .collect()
    })
}

/// Check a draft's sizes: recipients, subject and text.
pub fn check_limits(recipients: usize, subject: &str, body: &str, quoted: &str) -> Result<()> {
    if recipients > MAX_RECIPIENTS {
        return Err(Error::BadRequest(
            crate::MESSAGES
                .reason("too_many_recipients")
                .arg("max", MAX_RECIPIENTS),
        ));
    }
    if subject.chars().count() > MAX_SUBJECT_CHARS || subject.contains(['\r', '\n']) {
        return Err(Error::BadRequest(
            crate::MESSAGES
                .reason("invalid_subject")
                .arg("max", MAX_SUBJECT_CHARS),
        ));
    }
    if body.len() + quoted.len() > MAX_TEXT_BYTES {
        return Err(Error::BadRequest(crate::MESSAGES.reason("draft_too_long")));
    }
    Ok(())
}

/// Bytes of attachments in all.
pub fn attachment_bytes(attachments: &[StoredAttachment]) -> u64 {
    attachments.iter().map(|a| a.size).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_are_checked() {
        for good in ["ana@example.com", "a.b+c@mail.example.org", "x@sub.example"] {
            assert!(valid_address(good), "{good}");
        }
        for bad in [
            "",
            "ana",
            "ana@",
            "@example.com",
            "ana@example",
            "ana @example.com",
            "Ana <ana@example.com>",
            "ana@example.com,leo@example.com",
            "ana@.example.com",
            "ana@example.com.",
        ] {
            assert!(!valid_address(bad), "{bad}");
        }
    }

    #[test]
    fn tidy_drops_duplicates_across_lists() {
        let a = |address: &str| Address {
            name: Some(format!(" {address} ")),
            address: format!(" {address} "),
        };
        let [to, cc, bcc] = tidy([
            vec![a("ana@example.com"), a("ANA@example.com")],
            vec![a("leo@example.com"), a("ana@example.com")],
            vec![a("leo@example.com"), a("mo@example.com")],
        ]);
        let addresses =
            |list: &[Address]| list.iter().map(|a| a.address.clone()).collect::<Vec<_>>();
        assert_eq!(addresses(&to), ["ana@example.com"]);
        assert_eq!(addresses(&cc), ["leo@example.com"]);
        assert_eq!(addresses(&bcc), ["mo@example.com"]);
        assert_eq!(to[0].name.as_deref(), Some("ana@example.com"));
    }
}
