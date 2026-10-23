//! The events this module publishes (ADR 0012), each in the transaction of
//! the change it describes.
//!
//! **No personal data in events.** The event log is kept for 90 days, so
//! payloads carry ids, enums and times only: never a title, a place, notes,
//! attendees or an address. Whoever needs the text asks the API as the
//! member, or reads the blob an event points at (a blob is deleted with the
//! workspace).

use campbooks_kernel::{DomainEvent, RecordRef, blobs::BlobRef, tenancy::UserId};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::model::{
    BlobPointer, CalendarChange, CalendarId, ConnectionId, EventId, EventKind, EventOrigin,
    RequestId,
};

/// One of the module's events about an event of a calendar: the same facts
/// for a creation, a change and a cancellation
/// (`contracts/schedule.event_created.v1.schema.json` and its two siblings).
macro_rules! event_facts {
    ($(#[$meta:meta])* $name:ident => $wire:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
        pub struct $name {
            /// The workspace.
            pub workspace: String,
            /// The event.
            pub event: EventId,
            /// Its calendar.
            pub calendar: CalendarId,
            /// The member whose calendar it is.
            pub owner: UserId,
            /// What kind of dated thing it is.
            pub kind: EventKind,
            /// When it starts: an RFC 3339 time with an offset, or a
            /// `YYYY-MM-DD` date when it is all-day.
            pub starts: String,
            /// When it ends, in the same form, if it has an end.
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub ends: Option<String>,
            /// Whether it lasts whole days.
            pub all_day: bool,
            /// Who made it.
            pub origin: EventOrigin,
            /// The record it was read from, if any (an email).
            #[serde(default, skip_serializing_if = "Option::is_none")]
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

event_facts!(
    /// An event was added to a calendar: made by the member, read by Scout
    /// from an email, or applied from a provider's batch.
    EventCreated => "schedule.event_created"
);
event_facts!(
    /// An event changed: its time, its kind, its calendar, or the provider's
    /// version replaced the member's.
    EventChanged => "schedule.event_changed"
);
event_facts!(
    /// An event was cancelled, removed or deleted. Expanding the occurrences
    /// of a series publishes nothing.
    EventCancelled => "schedule.event_cancelled"
);

/// A member's change of an event on a provider calendar, for connectors to
/// make there (`contracts/schedule.outbound_requested.v1.schema.json`). The
/// text to write, the provider's ids and the etag the change was made
/// against travel in the blob, a `calendar-change` v1 document: the ids of a
/// calendar can be an address. Connectors asks the schedule module to
/// confirm the request through the kernel's calendar-requests port before it
/// acts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct OutboundRequested {
    /// The workspace.
    pub workspace: String,
    /// This request: unique, the connector records it once.
    pub request: RequestId,
    /// The calendar the event is on.
    pub calendar: CalendarId,
    /// The event.
    pub event: EventId,
    /// The connection whose grant makes the change.
    pub connection: ConnectionId,
    /// What to do.
    pub change: CalendarChange,
    /// The `calendar-change` v1 document, a blob of the workspace.
    #[schema(value_type = BlobPointer)]
    pub body: BlobRef,
}

impl DomainEvent for OutboundRequested {
    const NAME: &'static str = "schedule.outbound_requested";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}
