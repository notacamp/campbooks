//! The other modules' events this module consumes, and the blob document it
//! points at, as their published contracts describe them (modules never
//! import each other): `contracts/conversations.message_read.v1`,
//! `contracts/read-texts.v1`, `contracts/conversations.member_replied.v1`
//! and `contracts/schedule.event_created.v1` with its two siblings
//! (`event_changed`, `event_cancelled`).
//!
//! Each type reads only the fields this module uses; what the contract adds
//! later is ignored.

use campbooks_kernel::{DomainEvent, RecordRef, blobs::BlobRef, tenancy::UserId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;

/// An ask of a read, as `conversations.message_read` carries it: no title,
/// quote or amount.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadAsk {
    /// Stable for the same message, layer and position.
    pub key: String,
    /// `do` or `pay` (checked when used).
    pub verb: String,
    /// By when: a date, or an RFC 3339 time with an offset.
    #[serde(default)]
    pub due: Option<String>,
    /// The reference of an open task it repeats.
    #[serde(default)]
    pub same_as: Option<String>,
    /// `reply` or `elsewhere` (checked when used); absent from older reads.
    #[serde(default)]
    pub done_by: Option<String>,
}

/// `conversations.message_read` v1: a read of one message was stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageRead {
    /// The workspace.
    pub workspace: String,
    /// The message.
    pub message: String,
    /// Its conversation.
    pub conversation: String,
    /// Its inbox.
    pub inbox: String,
    /// The inbox's personal owner; absent for a shared inbox.
    #[serde(default)]
    pub owner: Option<UserId>,
    /// `structured` or `model`.
    pub layer: String,
    /// The obligations of the reader.
    #[serde(default)]
    pub asks: Vec<ReadAsk>,
    /// The `read-texts` v1 document, a blob of the workspace.
    #[serde(default)]
    pub texts: Option<BlobRef>,
}

impl DomainEvent for MessageRead {
    const NAME: &'static str = "conversations.message_read";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// `conversations.member_replied` v1: a message the member sent was stored
/// in a conversation of their personal inbox (sent from Campbooks, or from
/// the provider's own client). Ids and a time only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberReplied {
    /// The workspace.
    pub workspace: String,
    /// The inbox.
    pub inbox: String,
    /// The conversation the message belongs to.
    pub conversation: String,
    /// The message the member sent.
    pub message: Uuid,
    /// When it was sent.
    pub sent_at: DateTime<Utc>,
    /// The inbox's personal owner: the member who sent it.
    pub owner: UserId,
}

impl DomainEvent for MemberReplied {
    const NAME: &'static str = "conversations.member_replied";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// The words of a read: the blob `texts` points at. Only the asks are read.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
pub struct ReadTexts {
    /// One entry per ask that has any, by its key.
    #[serde(default)]
    pub asks: Vec<AskText>,
}

/// The words of one ask.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AskText {
    /// The ask's key.
    pub key: String,
    /// What to do.
    #[serde(default)]
    pub title: String,
    /// How much, for a payment.
    #[serde(default)]
    pub amount: Option<AskAmount>,
}

/// An amount in minor units.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AskAmount {
    /// Cents.
    pub minor: i64,
    /// ISO 4217 code.
    pub currency: String,
}

/// One of the schedule module's events about an event of a member's
/// calendar, as this module reads it: the same facts for a creation, a
/// change and a cancellation. No title.
macro_rules! calendar_event {
    ($(#[$meta:meta])* $name:ident => $wire:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub struct $name {
            /// The workspace.
            pub workspace: String,
            /// The event (the schedule module's id).
            pub event: Uuid,
            /// Its calendar.
            pub calendar: Uuid,
            /// The member whose calendar it is.
            pub owner: UserId,
            /// Its kind (`hold` for time held for a task).
            pub kind: String,
            /// When it starts: an RFC 3339 time, or a date when all-day.
            pub starts: String,
            /// When it ends, in the same form.
            #[serde(default)]
            pub ends: Option<String>,
            /// Whether it lasts whole days.
            pub all_day: bool,
            /// The record it was made for or read from (`tasks.task` for a hold).
            #[serde(default)]
            pub source: Option<RecordRef>,
        }

        impl DomainEvent for $name {
            const NAME: &'static str = $wire;
            const VERSION: u16 = 1;

            fn workspace(&self) -> Option<&str> {
                Some(&self.workspace)
            }
        }
    };
}

calendar_event!(
    /// `schedule.event_created` v1: an event was added to a calendar (made,
    /// read from an email, synced, or put back by an undo).
    CalendarEventCreated => "schedule.event_created"
);

calendar_event!(
    /// `schedule.event_changed` v1: an event changed (moved, in Campbooks or
    /// at the provider).
    CalendarEventChanged => "schedule.event_changed"
);

calendar_event!(
    /// `schedule.event_cancelled` v1: an event was removed or cancelled.
    CalendarEventCancelled => "schedule.event_cancelled"
);
