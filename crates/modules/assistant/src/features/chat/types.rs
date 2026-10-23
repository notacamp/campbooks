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

/// Start a chat, or continue the one you have about the same record.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct StartChat {
    /// What the chat is about: a record of a type Scout can read
    /// (`conversations.conversation`); nothing when absent.
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
