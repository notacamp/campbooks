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
//! - **Where it goes** (campbooks-cloud#242). Both send bindings name the
//!   inbox's [`Destination`] as the connector resolved it when they were
//!   signed (the connection, stream and mailbox it goes out through): the
//!   author's request when they asked, the stored send at its release.
//!   `release_send` and the port verify each against the destination as it
//!   is resolved again then, so a stream repointed by SQL after the member
//!   asked does not verify, and nothing goes out. A binding signed before
//!   destinations were bound (`…/v1`) verifies only while the destination
//!   is still the one recorded at that deploy ([`Resolved::as_at_deploy`]).
//! - **A change request.** `conversation::request` signs what it records:
//!   workspace, request (the event's id), inbox, conversation and the items.
//!   Stored as `change_requests.mac`; the port verifies it before it says
//!   any item stands.
//!
//! - **What a member was shown** (campbooks-cloud#242). Every response that
//!   shows a member a draft or a rule carries a [`ContentToken`]: a tag over
//!   the workspace, the member, the object (kind, id, version or binding
//!   generation) and the same content hash the signing step binds
//!   ([`content_hash`] for a draft; for a rule the hash of its conditions
//!   and actions with its inbox, stored state and author). Sending a draft
//!   and saying yes to a rule take the token back, and the server signs only
//!   when it still holds for the row as it is under its lock. The token is
//!   never stored: the client holds it.
//!
//! SQL in the workspace can still edit or plant these rows; it cannot make
//! the tags verify, so nothing it planted or changed is sent or made.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use campbooks_kernel::{
    Error, Result,
    binding::{Binding, Fields},
    blobs::BlobRef,
    outbound::{Destination, Resolved},
    tenancy::{Access, UserId, WorkspaceId},
};
use sqlx::types::Uuid;

use crate::{
    domain::{drafts::DraftRow, redact},
    events::{DRAFT_RECORD, RULE_RECORD, RequestedItem},
    model::{Address, ConversationId, DraftId, InboxId, MessageId, RuleId, RuleState, SendId},
};

/// The encoding of a draft's outgoing content. A new field takes a new
/// version.
const DRAFT_CONTENT: &str = "campbooks/conversations.draft-content/v1";
/// The author's request to send a draft, as signed before destinations were
/// bound: still verified for the destination recorded at that deploy.
const SEND_REQUEST: &str = "campbooks/conversations.send-request/v1";
/// The author's request to send a draft, with its destination.
const SEND_REQUEST_V2: &str = "campbooks/conversations.send-request/v2";
/// A released send, as stored before destinations were bound.
const STORED_SEND: &str = "campbooks/conversations.stored-send/v1";
/// A released send, as stored, with its destination.
const STORED_SEND_V2: &str = "campbooks/conversations.stored-send/v2";
/// A change request, as recorded.
const CHANGE_REQUEST: &str = "campbooks/conversations.change-request/v1";
/// What a member was shown: a draft or a rule, as a response showed it.
const CONTENT_TOKEN: &str = "campbooks/conversations.content-token/v1";

/// SHA-256 of everything of `row` that goes out: the sender, the
/// recipients (To, Cc, Bcc), subject, body and quoted history, the reply
/// headers and thread, what it answers, and each attachment with its blob
/// (hash, size, type), and its signature when it carries one. Canonical (length-prefixed, versioned), never a
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
    // The signature the draft carries (campbooks-cloud#393), only when there
    // is one: a draft without it hashes as it always did, so a content token
    // issued before signatures existed stays valid for it.
    if !row.signature.is_empty() {
        fields.str(&row.signature);
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

/// Which destination a send's binding names (campbooks-cloud#242).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bound<'a> {
    /// Signed before destinations were bound (`…/v1`): it names none.
    Before,
    /// Signed for this destination (`None`: no connection fed the inbox).
    To(Option<&'a Destination>),
}

impl<'a> Bound<'a> {
    /// Bound to `at`, the destination resolved now.
    pub fn at(at: Option<&'a Resolved>) -> Self {
        Self::To(at.map(|resolved| &resolved.destination))
    }
}

/// Append `destination`: a presence byte, then its connection, stream and
/// mailbox.
fn destination(fields: &mut Fields, destination: Option<&Destination>) {
    match destination {
        None => {
            fields.bytes(&[0]);
        }
        Some(to) => {
            fields
                .bytes(&[1])
                .uuid(&to.connection)
                .uuid(&to.stream)
                .str(&to.mailbox);
        }
    }
}

/// Whether `tag` holds for `at`, the destination resolved now: it is the
/// tag of `now` (the binding for `at`, `…/v2`), or of `before` (the same
/// binding as signed before destinations were bound) while `at` is still
/// the destination recorded then.
fn holds_at(
    binding: &Binding,
    now: &Fields,
    before: &Fields,
    tag: Option<&[u8]>,
    at: Option<&Resolved>,
) -> bool {
    binding.verify(now, tag)
        || (at.is_some_and(|at| at.as_at_deploy) && binding.verify(before, tag))
}

/// The tag under the current key of `now` or `before`, whichever `tag`
/// verifies under the previous key only (campbooks-cloud#191).
fn retag_at(
    binding: &Binding,
    now: &Fields,
    before: &Fields,
    tag: Option<&[u8]>,
) -> Option<Vec<u8>> {
    binding
        .retag(now, tag)
        .or_else(|| binding.retag(before, tag))
}

fn blob(fields: &mut Fields, blob: &BlobRef) {
    fields
        .bytes(blob.hash.as_bytes())
        .uint(blob.size)
        .str(&blob.content_type);
}

/// The author's request to send `draft` at `generation`, from `inbox`,
/// with content `content` ([`content_hash`]), to `destination`.
#[derive(Debug, Clone, Copy)]
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
    /// Where it goes.
    pub destination: Bound<'a>,
}

impl SendRequest<'_> {
    fn fields(&self) -> Fields {
        let label = match self.destination {
            Bound::Before => SEND_REQUEST,
            Bound::To(_) => SEND_REQUEST_V2,
        };
        let mut fields = Fields::new(label);
        fields
            .uuid(self.workspace.as_uuid())
            .uuid(self.draft.as_uuid())
            .int(i64::from(self.generation))
            .opt_uuid(self.author.as_ref().map(UserId::as_uuid))
            .uuid(self.inbox.as_uuid())
            .bytes(self.content);
        if let Bound::To(to) = self.destination {
            destination(&mut fields, to);
        }
        fields
    }

    /// Whether `tag` is this request's, sent to `at` (the destination
    /// resolved now); `self.destination` is not read.
    pub fn holds_at(&self, binding: &Binding, tag: Option<&[u8]>, at: Option<&Resolved>) -> bool {
        let now = SendRequest {
            destination: Bound::at(at),
            ..*self
        };
        let before = SendRequest {
            destination: Bound::Before,
            ..*self
        };
        holds_at(binding, &now.fields(), &before.fields(), tag, at)
    }

    /// The tag under the current key, when `tag` is this one's under the
    /// previous key only (campbooks-cloud#191), for `at`, whichever
    /// encoding it is in.
    pub fn retag_at(
        &self,
        binding: &Binding,
        tag: Option<&[u8]>,
        at: Option<&Resolved>,
    ) -> Option<Vec<u8>> {
        let now = SendRequest {
            destination: Bound::at(at),
            ..*self
        };
        let before = SendRequest {
            destination: Bound::Before,
            ..*self
        };
        retag_at(binding, &now.fields(), &before.fields(), tag)
    }

    /// The tag to store with the scheduled draft; `None` without a key.
    pub fn sign(&self, binding: &Binding) -> Option<Vec<u8>> {
        binding.tag(&self.fields())
    }

    /// Whether `tag` is this request's, as signed (for `self.destination`).
    #[cfg(test)]
    pub fn holds(&self, binding: &Binding, tag: Option<&[u8]>) -> bool {
        binding.verify(&self.fields(), tag)
    }
}

/// A released send as stored: `send` from `inbox`, asked for by `author`,
/// written as `document`, going to `destination`.
#[derive(Debug, Clone, Copy)]
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
    /// Where it goes.
    pub destination: Bound<'a>,
}

impl StoredSend<'_> {
    fn fields(&self) -> Fields {
        let label = match self.destination {
            Bound::Before => STORED_SEND,
            Bound::To(_) => STORED_SEND_V2,
        };
        let mut fields = Fields::new(label);
        fields
            .uuid(self.workspace.as_uuid())
            .uuid(self.send.as_uuid())
            .uuid(self.inbox.as_uuid())
            .opt_uuid(self.author.as_ref().map(UserId::as_uuid));
        blob(&mut fields, self.document);
        if let Bound::To(to) = self.destination {
            destination(&mut fields, to);
        }
        fields
    }

    /// Whether `tag` is this send's, going to `at` (the destination the
    /// sender resolved); `self.destination` is not read.
    pub fn holds_at(&self, binding: &Binding, tag: Option<&[u8]>, at: Option<&Resolved>) -> bool {
        let now = StoredSend {
            destination: Bound::at(at),
            ..*self
        };
        let before = StoredSend {
            destination: Bound::Before,
            ..*self
        };
        holds_at(binding, &now.fields(), &before.fields(), tag, at)
    }

    /// The tag under the current key, when `tag` is this one's under the
    /// previous key only (campbooks-cloud#191), for `at`, whichever
    /// encoding it is in.
    pub fn retag_at(
        &self,
        binding: &Binding,
        tag: Option<&[u8]>,
        at: Option<&Resolved>,
    ) -> Option<Vec<u8>> {
        let now = StoredSend {
            destination: Bound::at(at),
            ..*self
        };
        let before = StoredSend {
            destination: Bound::Before,
            ..*self
        };
        retag_at(binding, &now.fields(), &before.fields(), tag)
    }

    /// The tag to store with the send; `None` without a key.
    pub fn sign(&self, binding: &Binding) -> Option<Vec<u8>> {
        binding.tag(&self.fields())
    }

    /// Whether `tag` is this send's, as signed (for `self.destination`).
    #[cfg(test)]
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

    /// The tag under the current key, when `tag` is this request's under
    /// the previous key only (campbooks-cloud#191).
    pub fn retag(&self, binding: &Binding, tag: Option<&[u8]>) -> Result<Option<Vec<u8>>> {
        Ok(binding.retag(&self.fields()?, tag))
    }
}

/// What a response showed a member, as its [`ContentToken`] binds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shown {
    /// A draft at `version`, with everything that goes out ([`content_hash`]).
    Draft {
        /// The draft.
        id: DraftId,
        /// The inbox it is sent from (the send request binds it too).
        inbox: InboxId,
        /// Its version (bumped by every save).
        version: i32,
        /// [`content_hash`] of the row.
        content: [u8; 32],
    },
    /// A rule at its binding `generation`.
    Rule {
        /// The rule.
        id: RuleId,
        /// Its binding generation (one more at every signing).
        generation: i64,
        /// Its inbox.
        inbox: InboxId,
        /// Its stored state (a proposal is `proposed`).
        state: RuleState,
        /// Who it is taught by (whose rights it acts with).
        taught_by: Option<UserId>,
        /// The hash of its conditions and actions
        /// (`domain::rules::binding::content_hash`).
        content: [u8; 32],
    },
}

/// A token the server hands a member's client with what it shows them, and
/// takes back when the member sends a draft or says yes to a rule: `0x01`
/// then an HMAC-SHA256 under the binding key, over the workspace, the
/// member and [`Shown`], as URL-safe base64. Bound to the member: another
/// member's token does not hold. Never stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentToken {
    /// The workspace.
    pub workspace: WorkspaceId,
    /// Who was shown it (`None` for a client token: those share the drafts
    /// client tokens write).
    pub member: Option<UserId>,
    /// What was shown.
    pub shown: Shown,
}

impl ContentToken {
    /// The token of `shown`, shown to the caller of `access`.
    pub fn for_caller(access: &Access, shown: Shown) -> Self {
        Self {
            workspace: access.workspace,
            member: access.user,
            shown,
        }
    }

    /// The token of draft `row` as it is now, shown to `access`.
    pub fn of_draft(access: &Access, row: &DraftRow) -> Result<Self> {
        Ok(Self::for_caller(
            access,
            Shown::Draft {
                id: row.id,
                inbox: row.inbox_id,
                version: row.version,
                content: content_hash(row)?,
            },
        ))
    }

    fn fields(&self) -> Fields {
        let mut fields = Fields::new(CONTENT_TOKEN);
        fields
            .uuid(self.workspace.as_uuid())
            .opt_uuid(self.member.as_ref().map(UserId::as_uuid));
        match &self.shown {
            Shown::Draft {
                id,
                inbox,
                version,
                content,
            } => {
                fields
                    .str(DRAFT_RECORD)
                    .uuid(id.as_uuid())
                    .uuid(inbox.as_uuid())
                    .int(i64::from(*version))
                    .bytes(content);
            }
            Shown::Rule {
                id,
                generation,
                inbox,
                state,
                taught_by,
                content,
            } => {
                fields
                    .str(RULE_RECORD)
                    .uuid(id.as_uuid())
                    .int(*generation)
                    .bytes(content)
                    .uuid(inbox.as_uuid())
                    .str(state.as_str())
                    .opt_uuid(taught_by.as_ref().map(UserId::as_uuid));
            }
        }
        fields
    }

    /// The token to put in the response; `None` without a key.
    pub fn issue(&self, binding: &Binding) -> Option<String> {
        binding
            .tag(&self.fields())
            .map(|tag| URL_SAFE_NO_PAD.encode(tag))
    }

    /// Whether `token` (as the client sent it back) is this one: the HMAC
    /// compared in constant time. `false` without a key or a token, or for
    /// one that does not decode.
    pub fn holds(&self, binding: &Binding, token: Option<&str>) -> bool {
        let tag = token.and_then(|t| URL_SAFE_NO_PAD.decode(t).ok());
        binding.verify(&self.fields(), tag.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use super::*;
    use crate::events::RequestedChange;

    fn binding() -> Binding {
        campbooks_kernel::testing::binding()
    }

    /// A draft row with nothing random in it.
    fn row(signature: &str) -> DraftRow {
        let at = chrono::DateTime::UNIX_EPOCH;
        DraftRow {
            id: DraftId::from_uuid(sqlx::types::Uuid::from_u128(1)),
            inbox_id: InboxId::from_uuid(sqlx::types::Uuid::from_u128(2)),
            inbox_address: "sam@example.com".into(),
            inbox_name: Some("Sam Rivers".into()),
            kind: "new".into(),
            conversation_id: None,
            source_message_id: None,
            to_recipients: serde_json::json!([{"address": "leo@example.com"}]),
            cc_recipients: serde_json::json!([]),
            bcc_recipients: serde_json::json!([]),
            subject: "Lease".into(),
            body: "See you Thursday.".into(),
            quoted: String::new(),
            in_reply_to: None,
            reference_ids: vec![],
            thread_key: None,
            attachments: serde_json::json!([]),
            signature: signature.into(),
            version: 1,
            state: "draft".into(),
            send_generation: 0,
            send_at: None,
            failure_reason: None,
            created_at: at,
            updated_at: at,
        }
    }

    /// What the content hash of a draft without a signature was before
    /// signatures existed (campbooks-cloud#393): a content token issued then
    /// must stay valid for it, so this value may never change.
    const UNSIGNED_BEFORE_SIGNATURES: &str =
        "888ed6b177dba6880ae4845a584de94f667f8a955a6912a6ba5928a6fb938969";

    #[test]
    fn a_draft_without_a_signature_hashes_as_it_always_did() {
        let mut hex = String::new();
        for byte in content_hash(&row("")).unwrap() {
            write!(hex, "{byte:02x}").unwrap();
        }
        assert_eq!(hex, UNSIGNED_BEFORE_SIGNATURES);
        // One that carries a signature is another content, and so is each text.
        let signed = content_hash(&row("Sam Rivers")).unwrap();
        assert_ne!(signed, content_hash(&row("")).unwrap());
        assert_ne!(signed, content_hash(&row("Sam R.")).unwrap());
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
            destination: Bound::To(None),
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
            destination: Bound::To(None),
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

    fn resolved(as_at_deploy: bool) -> Resolved {
        Resolved {
            destination: Destination {
                connection: Uuid::now_v7(),
                stream: Uuid::now_v7(),
                mailbox: "mailbox-1".into(),
            },
            as_at_deploy,
        }
    }

    /// campbooks-cloud#242: a send's bindings name where it goes. Signed
    /// for one destination, they hold for it only: another connection,
    /// stream or mailbox (a stream repointed by SQL), or none, does not
    /// verify.
    #[test]
    fn a_send_holds_only_for_the_destination_it_was_signed_for() {
        let content = [3_u8; 32];
        let at = resolved(false);
        let request = SendRequest {
            workspace: WorkspaceId::new(),
            draft: DraftId::new(),
            generation: 1,
            author: Some(UserId::new()),
            inbox: InboxId::new(),
            content: &content,
            destination: Bound::at(Some(&at)),
        };
        let tag = request.sign(&binding()).unwrap();
        assert!(request.holds_at(&binding(), Some(&tag), Some(&at)));
        let elsewhere = |change: fn(&mut Destination)| {
            let mut other = at.clone();
            change(&mut other.destination);
            other
        };
        for other in [
            elsewhere(|d| d.connection = Uuid::now_v7()),
            elsewhere(|d| d.stream = Uuid::now_v7()),
            elsewhere(|d| d.mailbox = "mailbox-2".into()),
        ] {
            assert!(!request.holds_at(&binding(), Some(&tag), Some(&other)));
        }
        assert!(!request.holds_at(&binding(), Some(&tag), None));
        // Recorded at the deploy changes nothing for a v2 binding.
        let pinned = Resolved {
            as_at_deploy: true,
            ..elsewhere(|d| d.stream = Uuid::now_v7())
        };
        assert!(!request.holds_at(&binding(), Some(&tag), Some(&pinned)));

        let document = BlobRef {
            hash: campbooks_kernel::blobs::BlobHash::of(b"{}"),
            size: 2,
            content_type: "application/json".into(),
        };
        let send = StoredSend {
            workspace: request.workspace,
            send: SendId::new(),
            inbox: request.inbox,
            author: request.author,
            document: &document,
            destination: Bound::at(Some(&at)),
        };
        let tag = send.sign(&binding()).unwrap();
        assert!(send.holds_at(&binding(), Some(&tag), Some(&at)));
        assert!(!send.holds_at(
            &binding(),
            Some(&tag),
            Some(&elsewhere(|d| d.connection = Uuid::now_v7()))
        ));
        assert!(!send.holds_at(&binding(), Some(&tag), None));
    }

    /// A binding signed before destinations were (`…/v1`) holds only while
    /// the destination is still the one recorded at the deploy; a v2 tag
    /// never verifies as a v1 one, so SQL cannot choose the encoding.
    #[test]
    fn a_send_signed_before_destinations_holds_only_as_at_the_deploy() {
        let content = [3_u8; 32];
        let request = SendRequest {
            workspace: WorkspaceId::new(),
            draft: DraftId::new(),
            generation: 1,
            author: Some(UserId::new()),
            inbox: InboxId::new(),
            content: &content,
            destination: Bound::Before,
        };
        let tag = request.sign(&binding()).unwrap();
        assert!(request.holds_at(&binding(), Some(&tag), Some(&resolved(true))));
        assert!(!request.holds_at(&binding(), Some(&tag), Some(&resolved(false))));
        assert!(!request.holds_at(&binding(), Some(&tag), None));
        // A v2 tag for no destination is not a v1 tag.
        let v2 = SendRequest {
            destination: Bound::To(None),
            ..request
        }
        .sign(&binding())
        .unwrap();
        assert_ne!(v2, tag);
        assert!(!request.holds_at(&binding(), Some(&v2), Some(&resolved(true))));
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

    #[test]
    fn a_content_token_holds_only_for_what_was_shown_to_whom() {
        let workspace = WorkspaceId::new();
        let member = Some(UserId::new());
        let (id, inbox) = (DraftId::new(), InboxId::new());
        let draft = Shown::Draft {
            id,
            inbox,
            version: 3,
            content: [5_u8; 32],
        };
        let token = ContentToken {
            workspace,
            member,
            shown: draft,
        };
        let issued = token.issue(&binding()).unwrap();
        assert!(token.holds(&binding(), Some(&issued)));
        // Base64 of the version byte and the HMAC: no padding, URL-safe.
        assert_eq!(issued.len(), 44);
        assert!(!issued.contains(['=', '+', '/']));
        for other in [
            ContentToken {
                member: Some(UserId::new()),
                ..token
            },
            ContentToken {
                member: None,
                ..token
            },
            ContentToken {
                workspace: WorkspaceId::new(),
                ..token
            },
            ContentToken {
                shown: Shown::Draft {
                    id,
                    inbox,
                    version: 4,
                    content: [5_u8; 32],
                },
                ..token
            },
            ContentToken {
                shown: Shown::Draft {
                    id,
                    inbox,
                    version: 3,
                    content: [6_u8; 32],
                },
                ..token
            },
            ContentToken {
                shown: Shown::Draft {
                    id: DraftId::new(),
                    inbox,
                    version: 3,
                    content: [5_u8; 32],
                },
                ..token
            },
            ContentToken {
                shown: Shown::Draft {
                    id,
                    inbox: InboxId::new(),
                    version: 3,
                    content: [5_u8; 32],
                },
                ..token
            },
        ] {
            assert!(!other.holds(&binding(), Some(&issued)));
        }
        // No token, garbage, another key, no key.
        assert!(!token.holds(&binding(), None));
        assert!(!token.holds(&binding(), Some("")));
        assert!(!token.holds(&binding(), Some("not base64!")));
        assert!(!token.holds(&Binding::disabled(), Some(&issued)));
        assert!(token.issue(&Binding::disabled()).is_none());
        let other_key = Binding::new(&campbooks_kernel::crypto::SecretKey::from_bytes(
            [9; campbooks_kernel::crypto::SECRET_KEY_LEN],
        ));
        assert!(!token.holds(&other_key, Some(&issued)));
    }

    #[test]
    fn a_rule_token_binds_its_inbox_state_and_author() {
        let (id, inbox, content) = (RuleId::new(), InboxId::new(), [7_u8; 32]);
        let rule = Shown::Rule {
            id,
            generation: 0,
            inbox,
            state: RuleState::Proposed,
            taught_by: None,
            content,
        };
        let token = ContentToken {
            workspace: WorkspaceId::new(),
            member: Some(UserId::new()),
            shown: rule,
        };
        let issued = token.issue(&binding()).unwrap();
        assert!(token.holds(&binding(), Some(&issued)));
        let changed = |shown| ContentToken { shown, ..token };
        for other in [
            Shown::Rule {
                id,
                generation: 1,
                inbox,
                state: RuleState::Proposed,
                taught_by: None,
                content,
            },
            Shown::Rule {
                id,
                generation: 0,
                inbox: InboxId::new(),
                state: RuleState::Proposed,
                taught_by: None,
                content,
            },
            Shown::Rule {
                id,
                generation: 0,
                inbox,
                state: RuleState::Active,
                taught_by: None,
                content,
            },
            Shown::Rule {
                id,
                generation: 0,
                inbox,
                state: RuleState::Proposed,
                taught_by: Some(UserId::new()),
                content,
            },
            Shown::Rule {
                id,
                generation: 0,
                inbox,
                state: RuleState::Proposed,
                taught_by: None,
                content: [8_u8; 32],
            },
        ] {
            assert!(!changed(other).holds(&binding(), Some(&issued)));
        }
        // A draft's token is never a rule's, even with the same id bytes.
        let draft = ContentToken {
            shown: Shown::Draft {
                id: DraftId::from_uuid(*id.as_uuid()),
                inbox,
                version: 0,
                content,
            },
            ..token
        };
        assert!(!draft.holds(&binding(), Some(&issued)));
    }
}
