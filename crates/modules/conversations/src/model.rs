//! The module's ids and vocabulary, shared by its features, its events and
//! its ingest contract.

use std::{fmt, str::FromStr};

use campbooks_kernel::{Error, blobs::BlobRef, define_id};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

define_id!(
    /// An inbox: one address the workspace receives and sends on.
    InboxId
);
define_id!(
    /// A conversation: the exchange a message belongs to.
    ConversationId
);
define_id!(
    /// A connection, owned by the connectors module (referenced by id only).
    ConnectionId
);
define_id!(
    /// A message: one item of a conversation.
    MessageId
);
define_id!(
    /// A label on a conversation.
    LabelId
);
define_id!(
    /// An attachment of a message.
    AttachmentId
);
define_id!(
    /// One answer the decide pass gave about a message.
    DecisionId
);
define_id!(
    /// A member's correction of where a conversation was sorted.
    CorrectionId
);
define_id!(
    /// A taught rule: a condition → action pair a member taught Campbooks.
    RuleId
);
define_id!(
    /// One pass of a rule over existing mail.
    RuleRunId
);
define_id!(
    /// Something Scout noticed about mail an inbox gets a lot of, with the
    /// smart rule it offers ("Scout noticed").
    RuleNoticeId
);
define_id!(
    /// The rules one teaching created, across inboxes.
    TeachingId
);
define_id!(
    /// A message being written: new, or a reply or forward of a message.
    DraftId
);
define_id!(
    /// One send of a draft, minted when it leaves its undo window: the
    /// connector's idempotency key.
    SendId
);

/// What a draft answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DraftKind {
    /// A new message.
    New,
    /// A reply to the sender.
    Reply,
    /// A reply to the sender and every other recipient.
    ReplyAll,
    /// A forward, with the message's attachments.
    Forward,
}

impl DraftKind {
    /// The wire and database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Reply => "reply",
            Self::ReplyAll => "reply_all",
            Self::Forward => "forward",
        }
    }
}

impl FromStr for DraftKind {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "new" => Ok(Self::New),
            "reply" => Ok(Self::Reply),
            "reply_all" => Ok(Self::ReplyAll),
            "forward" => Ok(Self::Forward),
            other => Err(Error::internal(format!("unknown draft kind `{other}`"))),
        }
    }
}

/// Where a draft is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DraftState {
    /// Being written.
    Draft,
    /// Sent, waiting out its undo window.
    Scheduled,
    /// Handed to the connector.
    Sending,
    /// The provider did not send it; editable and sendable again.
    Failed,
}

impl DraftState {
    /// The wire and database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Scheduled => "scheduled",
            Self::Sending => "sending",
            Self::Failed => "failed",
        }
    }

    /// Whether the member may change, discard or send it.
    pub const fn editable(self) -> bool {
        matches!(self, Self::Draft | Self::Failed)
    }
}

impl FromStr for DraftState {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "draft" => Ok(Self::Draft),
            "scheduled" => Ok(Self::Scheduled),
            "sending" => Ok(Self::Sending),
            "failed" => Ok(Self::Failed),
            other => Err(Error::internal(format!("unknown draft state `{other}`"))),
        }
    }
}

/// A kind of messaging. Email is the first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    /// Email.
    Email,
}

impl Channel {
    /// The wire and database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Email => "email",
        }
    }
}

impl FromStr for Channel {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "email" => Ok(Self::Email),
            other => Err(Error::internal(format!("unknown channel `{other}`"))),
        }
    }
}

/// Whether a message came in or went out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// Received by the inbox.
    Inbound,
    /// Sent from the inbox (from Campbooks or from the provider's own client).
    Outbound,
}

impl Direction {
    /// The wire and database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Inbound => "inbound",
            Self::Outbound => "outbound",
        }
    }
}

impl FromStr for Direction {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "inbound" => Ok(Self::Inbound),
            "outbound" => Ok(Self::Outbound),
            other => Err(Error::internal(format!("unknown direction `{other}`"))),
        }
    }
}

/// The one place Campbooks sorts a conversation into. The built-in set; the
/// workspace's own streams come later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Stream {
    /// Mail from people: what the member reads first.
    Primary,
    /// Notifications, alerts and automatic messages.
    Updates,
    /// Receipts, invoices and bills.
    Receipts,
    /// Newsletters, mailing lists and marketing.
    Newsletters,
}

impl Stream {
    /// Every built-in stream, in display order.
    pub const ALL: [Self; 4] = [
        Self::Primary,
        Self::Updates,
        Self::Receipts,
        Self::Newsletters,
    ];

    /// The wire and database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Updates => "updates",
            Self::Receipts => "receipts",
            Self::Newsletters => "newsletters",
        }
    }
}

impl fmt::Display for Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Stream {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|stream| stream.as_str() == s)
            .ok_or_else(|| Error::internal(format!("unknown stream `{s}`")))
    }
}

/// Who put a conversation in its stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum StreamSource {
    /// Nobody: no accepted answer, so Primary.
    Default,
    /// An accepted rules answer (a test policy's; the facts never answer
    /// the stream).
    Rules,
    /// An accepted model answer.
    Model,
    /// A rule a member taught.
    Taught,
    /// A member moved it.
    Member,
}

impl StreamSource {
    /// The wire and database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Rules => "rules",
            Self::Model => "model",
            Self::Taught => "taught",
            Self::Member => "member",
        }
    }
}

impl FromStr for StreamSource {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "default" => Ok(Self::Default),
            "rules" => Ok(Self::Rules),
            "model" => Ok(Self::Model),
            "taught" => Ok(Self::Taught),
            "member" => Ok(Self::Member),
            other => Err(Error::internal(format!("unknown stream source `{other}`"))),
        }
    }
}

/// A member's access to an inbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum InboxRole {
    /// Reads and tidies its conversations.
    Reader,
    /// Also sends from it.
    Sender,
    /// Also manages who may use it.
    Manager,
}

impl InboxRole {
    /// The wire and database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Reader => "reader",
            Self::Sender => "sender",
            Self::Manager => "manager",
        }
    }
}

impl FromStr for InboxRole {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "reader" => Ok(Self::Reader),
            "sender" => Ok(Self::Sender),
            "manager" => Ok(Self::Manager),
            other => Err(Error::internal(format!("unknown inbox role `{other}`"))),
        }
    }
}

/// One way to reach someone on a channel: a display name and an address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Address {
    /// The display name, when the message gave one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "Ana Lopes")]
    pub name: Option<String>,
    /// The address, as given.
    #[schema(example = "ana@example.com")]
    pub address: String,
}

/// The schema of a stored blob reference (`campbooks_kernel::blobs::BlobRef`)
/// as it appears in events and documents: read the bytes with
/// `BlobStore::get` in the event's workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct BlobPointer {
    /// SHA-256 of the bytes, 64 lower-case hex digits.
    #[schema(example = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08")]
    pub hash: String,
    /// Length in bytes.
    pub size: u64,
    /// The media type recorded with it.
    #[schema(example = "text/plain; charset=utf-8")]
    pub content_type: String,
}

impl From<&BlobRef> for BlobPointer {
    fn from(blob: &BlobRef) -> Self {
        Self {
            hash: blob.hash.to_string(),
            size: blob.size,
            content_type: blob.content_type.clone(),
        }
    }
}

/// A small closed vocabulary with wire and database names.
macro_rules! vocabulary {
    ($(#[$meta:meta])* $name:ident { $($(#[$vmeta:meta])* $variant:ident => $wire:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            $($(#[$vmeta])* $variant),+
        }

        impl $name {
            /// Every value.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// The wire and database name.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $wire),+
                }
            }
        }

        impl FromStr for $name {
            type Err = Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::ALL
                    .iter()
                    .copied()
                    .find(|value| value.as_str() == s)
                    .ok_or_else(|| {
                        Error::internal(format!(concat!("unknown ", stringify!($name), " `{}`"), s))
                    })
            }
        }
    };
}

vocabulary!(
    /// Why a conversation needs its inbox's members (attention).
    NeedReason {
        /// Someone is waiting for an answer.
        Reply => "reply",
        /// Someone asked for something to be done.
        Do => "do",
        /// Money is owed.
        Pay => "pay",
        /// Look before acting: a warning, or a question to the member.
        Review => "review",
        /// A rule the member taught keeps this in front of them until they
        /// have opened it once (campbooks-cloud#274).
        WorthALook => "worth_a_look",
    }
);

vocabulary!(
    /// How much Campbooks stands behind a need.
    NeedTrust {
        /// An answer the policy lets act (or a member confirmed).
        Sure => "sure",
        /// A suggestion put to the member as a question.
        Check => "check",
    }
);

vocabulary!(
    /// Whose answer raised a need.
    NeedBasis {
        /// The facts (headers and addresses; only ever a "no", so only in
        /// answers stored before ADR 0018).
        Rules => "rules",
        /// The `decide` model.
        Model => "model",
        /// A rule a member taught.
        Taught => "taught",
        /// A member confirmed it.
        Member => "member",
    }
);

vocabulary!(
    /// The question whose answer raised a need.
    NeedQuestion {
        /// `needs_reply`.
        NeedsReply => "needs_reply",
        /// `asks_action`.
        AsksAction => "asks_action",
        /// `suspicious`.
        Suspicious => "suspicious",
        /// `money`: it asks for a payment still owed (a Pay need, always a
        /// question until a member answers it, campbooks-cloud#167).
        Money => "money",
        /// `worth_a_look`: a rule the member taught asked to see it.
        WorthALook => "worth_a_look",
    }
);

vocabulary!(
    /// One action of the catalogue a need may offer (campbooks-cloud#166),
    /// as `campbooks_ai::decide::signals::actions::ActionId` names it.
    NeedAction {
        /// Write back.
        Reply => "reply",
        /// Bring it back at a set time.
        Remind => "remind",
        /// Open the attachment.
        OpenAttachment => "open_attachment",
        /// Forward it.
        Forward => "forward",
        /// Report it as phishing and delete it.
        Report => "report",
        /// Say it is safe.
        Safe => "safe",
        /// The bill is already paid.
        AlreadyPaid => "already_paid",
        /// The bill is still to pay.
        StillToPay => "still_to_pay",
    }
);

vocabulary!(
    /// Why a need was cleared.
    ClearCause {
        /// A message was sent in the conversation after the one that raised it.
        Replied => "replied",
        /// The conversation was archived.
        Archived => "archived",
        /// Every message went to the trash or spam, or was removed.
        Trashed => "trashed",
        /// A member said it is done.
        Done => "done",
        /// A member said it was never needed.
        NotNeeded => "not_needed",
        /// A newer message replaced it.
        Superseded => "superseded",
        /// A rule a member taught says this mail never needs anyone.
        Rule => "rule",
        /// The member opened it: all "Worth a look" asks.
        Opened => "opened",
    }
);

vocabulary!(
    /// A member's verdict on a need.
    NeedVerdict {
        /// It is done.
        Done => "done",
        /// It was never needed (stored as a correction).
        NotNeeded => "not_needed",
        /// A question answered yes (stored as a correction).
        Confirmed => "confirmed",
        /// A Pay need is already paid: it is cleared as done.
        AlreadyPaid => "already_paid",
        /// A Pay need is still to pay: Campbooks stands behind it (stored
        /// as a correction confirming the money answer).
        StillToPay => "still_to_pay",
    }
);

vocabulary!(
    /// Where a taught rule came from.
    RuleOrigin {
        /// A member acted on one conversation and said "do the same to these".
        Action => "action",
        /// An answer to an onboarding question.
        Onboarding => "onboarding",
        /// The member's own words.
        Instruction => "instruction",
        /// A member's repeated corrections.
        Correction => "correction",
        /// Something Campbooks noticed and proposed.
        Suggestion => "suggestion",
    }
);

vocabulary!(
    /// What Scout noticed about a sender's mail, and so the rule it offers.
    NoticeKind {
        /// Mail the member gets a lot of and rarely opens: keep it off Home
        /// and file it under a stream.
        KeepOffHome => "keep_off_home",
        /// Mail the member archives or deletes unread: archive it as it
        /// arrives.
        ArchiveOnArrival => "archive_on_arrival",
    }
);

vocabulary!(
    /// A member's answer to a suggestion Scout made.
    NoticeAnswer {
        /// Ask another time.
        NotNow => "not_now",
        /// Never suggest this for that sender again.
        Never => "never",
    }
);

vocabulary!(
    /// Whether a taught rule acts.
    RuleState {
        /// Waiting for a member to say yes; does nothing yet.
        Proposed => "proposed",
        /// Acts on existing and new mail.
        Active => "active",
        /// Stopped: its author may no longer change the inbox or is away for
        /// now, its inbox is kept read-only, or a member paused it.
        Paused => "paused",
        /// Gone.
        Removed => "removed",
    }
);

vocabulary!(
    /// Why a taught rule is paused: the one that would still hold once the
    /// most passing one clears.
    PausedReason {
        /// Its inbox is kept read-only: no rule acts there any more.
        InboxReadOnly => "inbox_read_only",
        /// It names a sender domain that is a public suffix (`github.io`), so
        /// it would cover every site under it and matches nothing. Only
        /// removing it clears this.
        DomainTooBroad => "domain_too_broad",
        /// Its author is no longer a sender or manager of the inbox, until
        /// a manager adopts or removes it.
        AuthorCannotWrite => "author_cannot_write",
        /// A member paused it.
        PausedByMember => "paused_by_member",
        /// Its author is away for now; it acts again on its own once they
        /// are back. Deliberately says no more: why someone is away is not
        /// other members' business.
        AuthorUnavailable => "author_unavailable",
    }
);

vocabulary!(
    /// What a run over existing mail does.
    RunKind {
        /// Applies the rule.
        Apply => "apply",
        /// Puts back what the rule changed.
        Revert => "revert",
    }
);

vocabulary!(
    /// Where a run is.
    RunState {
        /// Working through the inbox.
        Running => "running",
        /// Finished.
        Done => "done",
        /// Stopped before the end.
        Cancelled => "cancelled",
    }
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for stream in Stream::ALL {
            assert_eq!(stream.as_str().parse::<Stream>().unwrap(), stream);
            assert_eq!(
                serde_json::to_value(stream).unwrap(),
                serde_json::json!(stream.as_str())
            );
        }
        for source in [
            StreamSource::Default,
            StreamSource::Rules,
            StreamSource::Model,
            StreamSource::Taught,
            StreamSource::Member,
        ] {
            assert_eq!(source.as_str().parse::<StreamSource>().unwrap(), source);
        }
        for role in [InboxRole::Reader, InboxRole::Sender, InboxRole::Manager] {
            assert_eq!(role.as_str().parse::<InboxRole>().unwrap(), role);
        }
        for direction in [Direction::Inbound, Direction::Outbound] {
            assert_eq!(direction.as_str().parse::<Direction>().unwrap(), direction);
        }
        for kind in [
            DraftKind::New,
            DraftKind::Reply,
            DraftKind::ReplyAll,
            DraftKind::Forward,
        ] {
            assert_eq!(kind.as_str().parse::<DraftKind>().unwrap(), kind);
        }
        for state in [
            DraftState::Draft,
            DraftState::Scheduled,
            DraftState::Sending,
            DraftState::Failed,
        ] {
            assert_eq!(state.as_str().parse::<DraftState>().unwrap(), state);
        }
        for reason in NeedReason::ALL {
            assert_eq!(reason.as_str().parse::<NeedReason>().unwrap(), *reason);
        }
        for cause in ClearCause::ALL {
            assert_eq!(
                serde_json::to_value(cause).unwrap(),
                serde_json::json!(cause.as_str())
            );
        }
        assert!("later".parse::<NeedVerdict>().is_err());
        assert_eq!("email".parse::<Channel>().unwrap(), Channel::Email);
        assert!("sms".parse::<Channel>().is_err());
        assert!("spam".parse::<Stream>().is_err());
    }
}
