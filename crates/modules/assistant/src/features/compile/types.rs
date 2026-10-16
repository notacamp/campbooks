//! What compiling answers. Conditions and actions have exactly the wire
//! shape `conversations.create_rule` takes (the taught-rule vocabulary of
//! campbooks-cloud#125), limited to what a sentence can say: no `list_id`
//! or subject template, whose exact values only a message knows.

use campbooks_ai::decide::taught;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// What came of the sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CompileOutcome {
    /// Rules to check.
    Rules,
    /// It asks for something rules can't do; `proposals` holds the closest
    /// rules, if any.
    Unsupported,
    /// Scout couldn't turn it into a rule.
    Unclear,
}

/// Why a sentence can't be a rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum UnsupportedReason {
    /// Sending, forwarding or replying: rules never send anything.
    SendsMail,
    /// Deleting: rules never delete anything.
    DeletesMail,
    /// It is not about mail.
    NotAboutMail,
    /// It doesn't say which mail.
    TooVague,
}

/// A stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum StreamKey {
    /// Primary.
    Primary,
    /// Updates.
    Updates,
    /// Receipts.
    Receipts,
    /// Newsletters.
    Newsletters,
}

impl StreamKey {
    /// The wire name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Updates => "updates",
            Self::Receipts => "receipts",
            Self::Newsletters => "newsletters",
        }
    }
}

/// A header a condition may test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum HeaderKey {
    /// `List-Id`: newsletters and mailing lists.
    ListId,
    /// `List-Unsubscribe`.
    ListUnsubscribe,
    /// `Auto-Submitted`: automatic notifications.
    AutoSubmitted,
    /// `Precedence`.
    Precedence,
}

/// A header test: the header is present.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ProposedHeader {
    /// Which header.
    pub name: HeaderKey,
}

/// Always or never show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceKey {
    /// Keep it in the inbox and never file it away.
    Always,
    /// It never needs you.
    Never,
}

/// One test on a message; all of a rule's conditions must hold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "field", content = "value", rename_all = "snake_case")]
pub enum ProposedCondition {
    /// The sender's address is this one.
    SenderAddress(String),
    /// The sender's domain is this one, or a subdomain of it.
    SenderDomain(String),
    /// The subject holds these words, in this order.
    SubjectContains(String),
    /// A header is present.
    Header(ProposedHeader),
    /// The conversation has this label (its id).
    Label(String),
    /// The conversation is in this stream.
    Stream(StreamKey),
}

/// What a rule does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ProposedAction {
    /// Put the conversation in this stream.
    MoveToStream {
        /// The stream.
        stream: StreamKey,
    },
    /// Take it out of the inbox.
    Archive,
    /// Put this label on it (its id).
    AddLabel {
        /// The label.
        label: String,
    },
    /// Mark it read.
    MarkRead,
    /// Whether it needs a reply.
    NeedsReply {
        /// Yes or no.
        value: bool,
    },
    /// Whether it asks for an action.
    AsksAction {
        /// Yes or no.
        value: bool,
    },
    /// Always or never show it.
    Surface {
        /// Which.
        value: SurfaceKey,
    },
}

impl From<&ProposedCondition> for taught::Condition {
    fn from(condition: &ProposedCondition) -> Self {
        match condition {
            ProposedCondition::SenderAddress(v) => Self::SenderAddress(v.clone()),
            ProposedCondition::SenderDomain(v) => Self::SenderDomain(v.clone()),
            ProposedCondition::SubjectContains(v) => Self::SubjectContains(v.clone()),
            ProposedCondition::Header(h) => Self::Header(taught::HeaderTest {
                name: match h.name {
                    HeaderKey::ListId => taught::HeaderName::ListId,
                    HeaderKey::ListUnsubscribe => taught::HeaderName::ListUnsubscribe,
                    HeaderKey::AutoSubmitted => taught::HeaderName::AutoSubmitted,
                    HeaderKey::Precedence => taught::HeaderName::Precedence,
                },
                value: None,
            }),
            ProposedCondition::Label(v) => Self::Label(v.clone()),
            ProposedCondition::Stream(s) => Self::Stream(s.as_str().to_owned()),
        }
    }
}

impl From<&ProposedAction> for taught::Action {
    fn from(action: &ProposedAction) -> Self {
        match action {
            ProposedAction::MoveToStream { stream } => Self::MoveToStream {
                stream: stream.as_str().to_owned(),
            },
            ProposedAction::Archive => Self::Archive,
            ProposedAction::AddLabel { label } => Self::AddLabel {
                label: label.clone(),
            },
            ProposedAction::MarkRead => Self::MarkRead,
            ProposedAction::NeedsReply { value } => Self::NeedsReply { value: *value },
            ProposedAction::AsksAction { value } => Self::AsksAction { value: *value },
            ProposedAction::Surface { value } => Self::Surface {
                value: match value {
                    SurfaceKey::Always => taught::Surface::Always,
                    SurfaceKey::Never => taught::Surface::Never,
                },
            },
        }
    }
}

/// An inbox a rule can be taught in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ProposalInbox {
    /// The inbox.
    pub id: String,
    /// Its address.
    pub address: String,
    /// Someone else uses it too: the rule would act for everyone there.
    pub shared: bool,
}

/// Someone the sentence names without an address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Candidate {
    /// Their address.
    pub address: String,
    /// Their display name.
    pub name: Option<String>,
    /// Messages from them.
    pub received: u32,
    /// Their conversations you replied to.
    pub replied: u32,
}

/// "Who do you mean by …?": the person must pick or type the sender.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct WhoQuestion {
    /// The words from the sentence.
    #[schema(example = "my accountant")]
    pub phrase: String,
    /// Senders whose name, address or domain matches the phrase, replied
    /// to most first; when none match, the people replied to most.
    pub candidates: Vec<Candidate>,
    /// Whether the candidates matched the phrase.
    pub matched: bool,
}

/// One rule to check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Proposal {
    /// All must hold; without the sender while `who` is unanswered.
    pub conditions: Vec<ProposedCondition>,
    /// What it does.
    pub actions: Vec<ProposedAction>,
    /// A sender to pick before saving: add it as `sender_address` (or
    /// `sender_domain`).
    pub who: Option<WhoQuestion>,
    /// The inboxes it can be taught in (where you may send).
    pub inboxes: Vec<ProposalInbox>,
}

/// The sentence to compile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CompileRequest {
    /// What you want, in your own words (at most 300 characters).
    #[schema(example = "anything from my accountant always matters")]
    pub text: String,
}

/// What Scout understood. Nothing is saved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CompileResult {
    /// Rules to check, something rules can't do, or unclear.
    pub outcome: CompileOutcome,
    /// Why not, when `unsupported`.
    pub unsupported: Option<UnsupportedReason>,
    /// The sentence as used, to keep as the rule's `origin_text`.
    pub text: String,
    /// The rules, at most three.
    pub proposals: Vec<Proposal>,
}
