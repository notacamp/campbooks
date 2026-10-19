//! What the reply operations take and answer. Nothing here is stored.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::model::MessageId;

/// Which message to suggest replies to.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SuggestReplies {
    /// The message answered; the conversation's newest by default.
    #[serde(default)]
    pub message_id: Option<MessageId>,
}

/// Likely replies to a message, as short intents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ReplySuggestions {
    /// The message answered.
    pub message_id: MessageId,
    /// At most three, in your language; empty when Scout has none.
    pub intents: Vec<String>,
}

/// What the reply should say, or how to change the draft.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct WriteReply {
    /// The message answered; the conversation's newest by default.
    #[serde(default)]
    pub message_id: Option<MessageId>,
    /// What it should say: a likely reply or your own words (at most 300 characters).
    #[schema(example = "Yes, Thursday 10:00 works")]
    pub intent: String,
    /// The draft as it stands, to rewrite with `change` or `shorter`.
    #[serde(default)]
    pub draft: Option<String>,
    /// How to change the draft, in plain words (at most 300 characters).
    #[serde(default)]
    #[schema(example = "warmer")]
    pub change: Option<String>,
    /// Make the draft shorter.
    #[serde(default)]
    pub shorter: bool,
    /// The likely replies shown, in order (at most 3, each at most 80
    /// characters), so `intent` may pick or combine them ("1 and 3").
    #[serde(default)]
    pub suggestions: Vec<String>,
}

/// How to write the nudge to the person a conversation waits on
/// (campbooks-cloud#293): Scout's own, or your draft rewritten.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct WriteNudge {
    /// The nudge as it stands, to rewrite with `change` or `shorter`.
    #[serde(default)]
    pub draft: Option<String>,
    /// How to change the draft, in plain words (at most 300 characters).
    #[serde(default)]
    #[schema(example = "warmer")]
    pub change: Option<String>,
    /// Make the draft shorter.
    #[serde(default)]
    pub shorter: bool,
}

/// Scout's reply, as plain text. Nothing is saved or sent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ReplyText {
    /// The message answered.
    pub message_id: MessageId,
    /// The reply, signed with your name when it is known.
    pub text: String,
}
