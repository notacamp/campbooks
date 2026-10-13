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
    /// An accepted rules answer.
    Rules,
    /// An accepted model answer.
    Model,
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
        assert_eq!("email".parse::<Channel>().unwrap(), Channel::Email);
        assert!("sms".parse::<Channel>().is_err());
        assert!("spam".parse::<Stream>().is_err());
    }
}
