//! What binds a send, and a change request, to the authenticated request
//! that made it (campbooks-cloud#227): MACs under the kernel's
//! [`Binding`], whose key the database never holds.
//!
//! - **The author's request.** `send_draft` signs the draft as the author
//!   asked for it: workspace, draft, send generation, author, inbox and a
//!   hash of everything that goes out ([`content_hash`]). Stored as
//!   `drafts.request_mac`; `release_send` verifies it before it writes
//!   anything, and clears it once the send is released or cancelled.
//! - **The stored send.** `release_send` signs the send it wrote:
//!   workspace, send id, inbox, author and the outgoing document's blob
//!   (hash, size, type). Stored as `drafts.send_mac`; the outbound-requests
//!   port verifies it before it hands the send to the connector.
//! - **A change request.** `conversation::request` signs what it records:
//!   workspace, request (the event's id), inbox, conversation and the items.
//!   Stored as `change_requests.mac`; the port verifies it before it says
//!   any item stands.
//!
//! SQL in the workspace can still edit or plant these rows; it cannot make
//! the tags verify, so nothing it planted or changed is sent or made.

use campbooks_kernel::{
    Error, Result,
    binding::{Binding, Fields},
    blobs::BlobRef,
    tenancy::{UserId, WorkspaceId},
};
use sqlx::types::Uuid;

use crate::{
    domain::{drafts::DraftRow, redact},
    events::RequestedItem,
    model::{Address, ConversationId, DraftId, InboxId, MessageId, SendId},
};

/// The encoding of a draft's outgoing content. A new field takes a new
/// version.
const DRAFT_CONTENT: &str = "campbooks/conversations.draft-content/v1";
/// The author's request to send a draft.
const SEND_REQUEST: &str = "campbooks/conversations.send-request/v1";
/// A released send, as stored.
const STORED_SEND: &str = "campbooks/conversations.stored-send/v1";
/// A change request, as recorded.
const CHANGE_REQUEST: &str = "campbooks/conversations.change-request/v1";

/// SHA-256 of everything of `row` that goes out: the sender, the
/// recipients (To, Cc, Bcc), subject, body and quoted history, the reply
/// headers and thread, what it answers, and each attachment with its blob
/// (hash, size, type). Canonical (length-prefixed, versioned), never a
/// concatenation.
pub fn content_hash(row: &DraftRow) -> Result<[u8; 32]> {
    let (to, cc, bcc) = row.recipients()?;
    let attachments = row.stored_attachments()?;
    let mut fields = Fields::new(DRAFT_CONTENT);
    fields
        .str(&row.inbox_address)
        .opt_str(row.inbox_name.as_deref())
        .str(&row.kind)
        .opt_uuid(row.conversation_id.as_ref().map(ConversationId::as_uuid))
        .opt_uuid(row.source_message_id.as_ref().map(MessageId::as_uuid));
    for list in [&to, &cc, &bcc] {
        addresses(&mut fields, list);
    }
    fields
        .str(&row.subject)
        .str(&row.body)
        .str(&row.quoted)
        .opt_str(row.in_reply_to.as_deref())
        .count(row.reference_ids.len());
    for reference in &row.reference_ids {
        fields.str(reference);
    }
    fields
        .opt_str(row.thread_key.as_deref())
        .count(attachments.len());
    for attachment in &attachments {
        fields
            .uuid(attachment.id.as_uuid())
            .str(&attachment.filename)
            .str(&attachment.content_type)
            .uint(attachment.size);
        blob(&mut fields, &attachment.blob);
    }
    Ok(fields.digest())
}

fn addresses(fields: &mut Fields, list: &[Address]) {
    fields.count(list.len());
    for address in list {
        fields
            .str(&address.address)
            .opt_str(address.name.as_deref());
    }
}

fn blob(fields: &mut Fields, blob: &BlobRef) {
    fields
        .bytes(blob.hash.as_bytes())
        .uint(blob.size)
        .str(&blob.content_type);
}

/// The author's request to send `draft` at `generation`, from `inbox`,
/// with content `content` ([`content_hash`]).
pub struct SendRequest<'a> {
    /// The workspace.
    pub workspace: WorkspaceId,
    /// The draft.
    pub draft: DraftId,
    /// The send generation the request scheduled.
    pub generation: i32,
    /// Who asked (the draft's author).
    pub author: Option<UserId>,
    /// The inbox it is sent from.
    pub inbox: InboxId,
    /// What goes out.
    pub content: &'a [u8; 32],
}

impl SendRequest<'_> {
    fn fields(&self) -> Fields {
        let mut fields = Fields::new(SEND_REQUEST);
        fields
            .uuid(self.workspace.as_uuid())
            .uuid(self.draft.as_uuid())
            .int(i64::from(self.generation))
            .opt_uuid(self.author.as_ref().map(UserId::as_uuid))
            .uuid(self.inbox.as_uuid())
            .bytes(self.content);
        fields
    }

    /// The tag to store with the scheduled draft; `None` without a key.
    pub fn sign(&self, binding: &Binding) -> Option<Vec<u8>> {
        binding.tag(&self.fields())
    }

    /// Whether `tag` is this request's.
    pub fn holds(&self, binding: &Binding, tag: Option<&[u8]>) -> bool {
        binding.verify(&self.fields(), tag)
    }
}

/// A released send as stored: `send` from `inbox`, asked for by `author`,
/// written as `document`.
pub struct StoredSend<'a> {
    /// The workspace.
    pub workspace: WorkspaceId,
    /// The send.
    pub send: SendId,
    /// The inbox it is sent from.
    pub inbox: InboxId,
    /// Who asked (the draft's author).
    pub author: Option<UserId>,
    /// The outgoing document.
    pub document: &'a BlobRef,
}

impl StoredSend<'_> {
    fn fields(&self) -> Fields {
        let mut fields = Fields::new(STORED_SEND);
        fields
            .uuid(self.workspace.as_uuid())
            .uuid(self.send.as_uuid())
            .uuid(self.inbox.as_uuid())
            .opt_uuid(self.author.as_ref().map(UserId::as_uuid));
        blob(&mut fields, self.document);
        fields
    }

    /// The tag to store with the send; `None` without a key.
    pub fn sign(&self, binding: &Binding) -> Option<Vec<u8>> {
        binding.tag(&self.fields())
    }

    /// Whether `tag` is this send's.
    pub fn holds(&self, binding: &Binding, tag: Option<&[u8]>) -> bool {
        binding.verify(&self.fields(), tag)
    }
}

/// A change request as recorded: `items` for `conversation` in `inbox`,
/// under request (event) `request`.
pub struct ChangeRequest<'a> {
    /// The workspace.
    pub workspace: WorkspaceId,
    /// The request: the id of the event that carried it.
    pub request: Uuid,
    /// The inbox.
    pub inbox: InboxId,
    /// The conversation.
    pub conversation: ConversationId,
    /// The changes, in order.
    pub items: &'a [RequestedItem],
}

impl ChangeRequest<'_> {
    /// Each item is its typed value serialized (the field order of its
    /// type, so a stored copy that parses back encodes the same).
    fn fields(&self) -> Result<Fields> {
        let mut fields = Fields::new(CHANGE_REQUEST);
        fields
            .uuid(self.workspace.as_uuid())
            .uuid(&self.request)
            .uuid(self.inbox.as_uuid())
            .uuid(self.conversation.as_uuid())
            .count(self.items.len());
        for item in self.items {
            let encoded =
                serde_json::to_vec(item).map_err(|e| Error::internal(redact::json(&e)))?;
            fields.bytes(&encoded);
        }
        Ok(fields)
    }

    /// The tag to record with the request; `None` without a key.
    pub fn sign(&self, binding: &Binding) -> Result<Option<Vec<u8>>> {
        Ok(binding.tag(&self.fields()?))
    }

    /// Whether `tag` is this request's.
    pub fn holds(&self, binding: &Binding, tag: Option<&[u8]>) -> Result<bool> {
        Ok(binding.verify(&self.fields()?, tag))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::RequestedChange;

    fn binding() -> Binding {
        campbooks_kernel::testing::binding()
    }

    #[test]
    fn a_send_request_holds_only_for_its_own_fields() {
        let content = [3_u8; 32];
        let request = SendRequest {
            workspace: WorkspaceId::new(),
            draft: DraftId::new(),
            generation: 2,
            author: Some(UserId::new()),
            inbox: InboxId::new(),
            content: &content,
        };
        let tag = request.sign(&binding()).unwrap();
        assert!(request.holds(&binding(), Some(&tag)));
        let other_content = [4_u8; 32];
        for other in [
            SendRequest {
                generation: 3,
                ..request
            },
            SendRequest {
                draft: DraftId::new(),
                ..request
            },
            SendRequest {
                author: Some(UserId::new()),
                ..request
            },
            SendRequest {
                author: None,
                ..request
            },
            SendRequest {
                inbox: InboxId::new(),
                ..request
            },
            SendRequest {
                workspace: WorkspaceId::new(),
                ..request
            },
            SendRequest {
                content: &other_content,
                ..request
            },
        ] {
            assert!(!other.holds(&binding(), Some(&tag)));
        }
        assert!(!request.holds(&Binding::disabled(), Some(&tag)));
        assert!(request.sign(&Binding::disabled()).is_none());
    }

    #[test]
    fn a_stored_send_holds_only_for_its_document() {
        let document = BlobRef {
            hash: campbooks_kernel::blobs::BlobHash::of(b"{}"),
            size: 2,
            content_type: "application/json".into(),
        };
        let send = StoredSend {
            workspace: WorkspaceId::new(),
            send: SendId::new(),
            inbox: InboxId::new(),
            author: Some(UserId::new()),
            document: &document,
        };
        let tag = send.sign(&binding()).unwrap();
        assert!(send.holds(&binding(), Some(&tag)));
        let other = BlobRef {
            hash: campbooks_kernel::blobs::BlobHash::of(b"{\"to\":[]}"),
            size: 9,
            content_type: "application/json".into(),
        };
        assert!(
            !StoredSend {
                document: &other,
                ..send
            }
            .holds(&binding(), Some(&tag))
        );
        assert!(
            !StoredSend {
                send: SendId::new(),
                ..send
            }
            .holds(&binding(), Some(&tag))
        );
    }

    #[test]
    fn a_change_request_holds_only_for_its_items() {
        let items = [RequestedItem {
            provider_message_id: "m-1".into(),
            change: RequestedChange::Archive,
        }];
        let request = ChangeRequest {
            workspace: WorkspaceId::new(),
            request: Uuid::now_v7(),
            inbox: InboxId::new(),
            conversation: ConversationId::new(),
            items: &items,
        };
        let tag = request.sign(&binding()).unwrap().unwrap();
        assert!(request.holds(&binding(), Some(&tag)).unwrap());
        let other = [RequestedItem {
            provider_message_id: "m-1".into(),
            change: RequestedChange::Trash,
        }];
        assert!(
            !ChangeRequest {
                items: &other,
                ..request
            }
            .holds(&binding(), Some(&tag))
            .unwrap()
        );
        assert!(
            !ChangeRequest {
                request: Uuid::now_v7(),
                ..request
            }
            .holds(&binding(), Some(&tag))
            .unwrap()
        );
    }
}
