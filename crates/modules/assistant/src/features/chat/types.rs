//! What the chat operations take and answer.

use campbooks_kernel::{RecordRef, define_id};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

define_id!(
    /// A member's chat with Scout.
    ChatId
);

define_id!(
    /// A question or an answer in a chat.
    ChatMessageId
);

define_id!(
    /// A change, a rule or a reply Scout proposed in a chat.
    ProposalId
);

/// Start a chat, or continue the one you have about the same record.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct StartChat {
    /// What the chat is about: a record of a type Scout can read
    /// (`conversations.conversation`, `conversations.rule`,
    /// `conversations.rule_notice`). When absent, a new chat about nothing
    /// in particular, every time.
    #[serde(default)]
    pub about: Option<RecordRef>,
}

/// A question.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AskChat {
    /// What you ask, 1 to 2,000 characters (line breaks kept).
    pub text: String,
    /// Your time zone, an IANA name (`Europe/Lisbon`), for "today" and
    /// "Thursday afternoon"; UTC when absent or unknown.
    #[serde(default)]
    pub time_zone: Option<String>,
}

/// Who wrote a chat message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChatAuthor {
    /// You asked it.
    Member,
    /// Scout answered it.
    Scout,
}

impl ChatAuthor {
    /// The database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Member => "member",
            Self::Scout => "scout",
        }
    }
}

/// How Scout's answer ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChatFinish {
    /// Answered.
    Done,
    /// Scout couldn't answer (the model failed, or the answer was cut off
    /// before it was written).
    Failed,
    /// Answered, but cut short: the model ran out of room.
    CutShort,
}

impl ChatFinish {
    /// The database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Done => "done",
            Self::Failed => "failed",
            Self::CutShort => "cut_short",
        }
    }

    /// From the database name.
    pub fn from_db(name: &str) -> Self {
        match name {
            "done" => Self::Done,
            "cut_short" => Self::CutShort,
            _ => Self::Failed,
        }
    }
}

/// A tool Scout used. Each reads as the member, and changes nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChatTool {
    /// Looked for conversations.
    FindConversations,
    /// Read a conversation.
    ReadConversation,
    /// Read what happened to a conversation.
    ConversationHistory,
    /// Looked for people who write to you.
    FindPeople,
    /// Read what needs you.
    WhatNeedsMe,
    /// Read your inboxes and their streams.
    MyInboxes,
    /// Read your tasks (campbooks-cloud#43).
    MyTasks,
    /// Read one task and what changed in it.
    ReadTask,
}

/// One thing Scout looked at, built by the server from the tool that ran,
/// never from the model's words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ChatStep {
    /// The tool.
    pub tool: ChatTool,
    /// The records it returned, at most 10.
    pub records: Vec<RecordRef>,
    /// How many it found.
    pub count: u32,
}

/// A question or an answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ChatMessageView {
    /// The message.
    pub id: ChatMessageId,
    /// Who wrote it.
    pub author: ChatAuthor,
    /// What was asked or answered, as plain text: show it as text only,
    /// never as HTML and never with links made of it.
    pub text: String,
    /// What Scout looked at to answer (Scout's answers only).
    pub steps: Vec<ChatStep>,
    /// How Scout's answer ended (Scout's answers only).
    pub finish: Option<ChatFinish>,
    /// What Scout proposed in this answer, at most three, each a card the
    /// member confirms or declines (Scout's answers only). Nothing in them
    /// has happened unless its state says so.
    pub proposals: Vec<ChatProposal>,
    /// When.
    pub created_at: DateTime<Utc>,
}

/// A chat and its messages, oldest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ChatView {
    /// The chat.
    pub id: ChatId,
    /// What it is about.
    pub about: Option<RecordRef>,
    /// Questions asked so far.
    pub questions: u32,
    /// Questions it may hold; past them, start over.
    pub max_questions: u32,
    /// When it was started.
    pub created_at: DateTime<Utc>,
    /// When its last message was written; it is deleted 30 days after.
    pub last_message_at: DateTime<Utc>,
    /// Its messages, oldest first.
    pub messages: Vec<ChatMessageView>,
}

/// One of your chats, as the list shows it: never what it is about read
/// again, only what you asked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ChatSummary {
    /// The chat.
    pub id: ChatId,
    /// What it is about; none for a chat about nothing in particular.
    pub about: Option<RecordRef>,
    /// Your first question in it, your own words, at most 120 characters:
    /// show it as text only.
    pub first_question: String,
    /// Questions asked so far.
    pub questions: u32,
    /// When its last message was written.
    pub last_message_at: DateTime<Utc>,
}

/// Your chats with Scout, the newest message first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ChatList {
    /// At most 30, each with at least one question.
    pub chats: Vec<ChatSummary>,
}

/// What kind of thing Scout proposed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProposalKind {
    /// One low-risk change to one conversation, made by the member's own
    /// request on Confirm.
    Change,
    /// A sentence for Teach Scout, where the member checks and saves the
    /// rule.
    Rule,
    /// An intent for a reply draft, written in the Reply dialog; the member
    /// sends it, or not.
    Reply,
}

/// Where a proposal stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProposalState {
    /// Waiting for the member.
    Pending,
    /// The member confirmed it and it was done (a rule saved, a draft
    /// opened).
    Done,
    /// The member said Not now.
    Declined,
    /// The member confirmed it, and it could not be done.
    Failed,
    /// Done, then undone by the member.
    Undone,
}

impl ProposalState {
    /// Whether a proposal in this state may be settled as `to`: `pending`
    /// to `done`, `declined` or `failed`; `done` to `undone`.
    pub const fn may_become(self, to: Settlement) -> bool {
        matches!(
            (self, to),
            (
                Self::Pending,
                Settlement::Done | Settlement::Declined | Settlement::Failed
            ) | (Self::Done, Settlement::Undone)
        )
    }
}

/// What the member did with a card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Settlement {
    /// Confirmed and done (a rule saved, a draft opened).
    Done,
    /// Not now.
    Declined,
    /// Confirmed, and the request was refused.
    Failed,
    /// Undone after it was done.
    Undone,
}

impl From<Settlement> for ProposalState {
    fn from(settlement: Settlement) -> Self {
        match settlement {
            Settlement::Done => Self::Done,
            Settlement::Declined => Self::Declined,
            Settlement::Failed => Self::Failed,
            Settlement::Undone => Self::Undone,
        }
    }
}

/// The change a change proposal names, from a fixed list, each mapped by
/// the server to one catalogue operation and its undo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProposalAction {
    /// Archive the conversation.
    Archive,
    /// Back to the inbox.
    Unarchive,
    /// Mark it read.
    MarkRead,
    /// Mark it unread.
    MarkUnread,
    /// Star it.
    Star,
    /// Unstar it.
    Unstar,
    /// Add one of its inbox's labels (`detail.label`).
    AddLabel,
    /// Take a label off it (`detail.label`).
    RemoveLabel,
    /// Move it to another stream (`detail.stream`).
    MoveToStream,
    /// Move it to the trash.
    Trash,
    /// Out of the trash.
    Untrash,
    /// Report it as spam.
    ReportSpam,
    /// Not spam.
    NotSpam,
    /// Its need: Done.
    Done,
    /// Its need: Not needed.
    NotNeeded,
    /// Its Pay need: Already paid.
    AlreadyPaid,
    /// Its Pay need: Still to pay.
    StillToPay,
    /// Its question: Yes.
    YesToQuestion,
    /// Its need: Later, until `detail.until`.
    Later,
}

/// A conversation's own facts, read from its record when the proposal was
/// made: what a card names, never the model's words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ProposalFacts {
    /// Who wrote its newest message in: a name, else an address.
    pub sender: String,
    /// Its subject (empty when it has none).
    pub subject: String,
    /// Its stream then.
    pub stream: String,
}

/// What a change takes besides its conversation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ProposalDetail {
    /// The stream it moves to (`move_to_stream`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<String>,
    /// The label's name, as its inbox has it (`add_label`, `remove_label`):
    /// Scout's words, show them quoted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Until when (`later`), RFC 3339, UTC.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<DateTime<Utc>>,
}

/// An HTTP method a proposal's request may use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum ProposalMethod {
    /// `POST`.
    Post,
    /// `DELETE`.
    Delete,
}

/// A request the member's client sends, as the server built it from the
/// catalogue: the client checks it is one of its own actions before
/// sending it, with the member's token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ProposalRequest {
    /// Its `operationId`.
    pub operation: String,
    /// Its method.
    pub method: ProposalMethod,
    /// Its path, relative to the workspace's API root
    /// (`conversations/{id}/archive` under `/api/v1/w/{workspace_id}/`).
    pub path: String,
    /// Its JSON body, or none.
    pub body: Option<serde_json::Value>,
    /// For an undo: the top-level fields of the change's answer to copy
    /// into this body, and nothing else.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub from_response: Vec<String>,
}

/// Something Scout proposed in a chat: a card under its answer. Nothing
/// happens until the member confirms it, and then by their own request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ChatProposal {
    /// The proposal: the change's `Idempotency-Key` (`<id>:undo` for its
    /// undo).
    pub id: ProposalId,
    /// What kind.
    pub kind: ProposalKind,
    /// Where it stands.
    pub state: ProposalState,
    /// The change (a change only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<ProposalAction>,
    /// The conversation it is about (a change or a reply).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record: Option<RecordRef>,
    /// The conversation's own facts (a change or a reply).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facts: Option<ProposalFacts>,
    /// What the change takes (a change only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<ProposalDetail>,
    /// The change's request (a change only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<ProposalRequest>,
    /// Its undo (a change only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub undo: Option<ProposalRequest>,
    /// The rule's sentence, Scout's words: show it quoted (a rule only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sentence: Option<String>,
    /// What the reply should say, Scout's words: show it quoted (a reply
    /// only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
}

/// What the member did with a card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SettleProposal {
    /// `done`, `declined` or `failed` for a pending card; `undone` for a
    /// done one.
    pub state: Settlement,
}
