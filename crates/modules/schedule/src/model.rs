//! The module's ids and vocabulary, shared by its features and its events.

use campbooks_kernel::define_id;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

define_id!(
    /// A calendar: a member's own Campbooks calendar, or one of a provider's.
    CalendarId
);
define_id!(
    /// An event of a calendar.
    EventId
);
define_id!(
    /// A connection, owned by the connectors module (referenced by id only).
    ConnectionId
);
define_id!(
    /// One change of an event that connectors is asked to make at the
    /// provider.
    RequestId
);

/// What kind of dated thing an event is. A fixed set: obligations (pay by,
/// submit by, renew by) are never events, they are asks owned by `tasks`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    /// Money is taken automatically on a date.
    Charge,
    /// An appointment or a booked slot.
    Appointment,
    /// A meeting with other people.
    Meeting,
    /// A trip: a flight, a train, a bus, a rental car.
    Travel,
    /// A stay: a hotel or a rental.
    Stay,
    /// A ticketed event.
    Show,
    /// A table or a visit booked.
    Reservation,
    /// A parcel arriving.
    Delivery,
    /// Something expiring.
    Expiry,
    /// Time held for a task.
    Hold,
    /// A provider event nobody typed.
    None,
}

impl EventKind {
    /// Every kind, in the order the settings list them.
    pub const ALL: [Self; 11] = [
        Self::Charge,
        Self::Appointment,
        Self::Meeting,
        Self::Travel,
        Self::Stay,
        Self::Show,
        Self::Reservation,
        Self::Delivery,
        Self::Expiry,
        Self::Hold,
        Self::None,
    ];

    /// The wire and database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Charge => "charge",
            Self::Appointment => "appointment",
            Self::Meeting => "meeting",
            Self::Travel => "travel",
            Self::Stay => "stay",
            Self::Show => "show",
            Self::Reservation => "reservation",
            Self::Delivery => "delivery",
            Self::Expiry => "expiry",
            Self::Hold => "hold",
            Self::None => "none",
        }
    }
}

impl std::str::FromStr for EventKind {
    type Err = ();

    fn from_str(text: &str) -> Result<Self, ()> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.as_str() == text)
            .ok_or(())
    }
}

/// Who made an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EventOrigin {
    /// It came from a provider calendar.
    Provider,
    /// The member made it.
    Member,
    /// Scout read it from an email.
    Scout,
}

impl EventOrigin {
    /// The database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Provider => "provider",
            Self::Member => "member",
            Self::Scout => "scout",
        }
    }
}

impl std::str::FromStr for EventOrigin {
    type Err = ();

    fn from_str(text: &str) -> Result<Self, ()> {
        match text {
            "provider" => Ok(Self::Provider),
            "member" => Ok(Self::Member),
            "scout" => Ok(Self::Scout),
            _ => Err(()),
        }
    }
}

/// Whether an event stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EventStatus {
    /// It stands.
    Confirmed,
    /// It may happen.
    Tentative,
    /// It will not happen.
    Cancelled,
}

impl EventStatus {
    /// The database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Confirmed => "confirmed",
            Self::Tentative => "tentative",
            Self::Cancelled => "cancelled",
        }
    }
}

impl std::str::FromStr for EventStatus {
    type Err = ();

    fn from_str(text: &str) -> Result<Self, ()> {
        match text {
            "confirmed" => Ok(Self::Confirmed),
            "tentative" => Ok(Self::Tentative),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(()),
        }
    }
}

/// The member's answer to an invitation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Response {
    /// Not answered yet.
    NeedsAction,
    /// Going.
    Accepted,
    /// Maybe.
    Tentative,
    /// Not going.
    Declined,
}

impl Response {
    /// The database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NeedsAction => "needs_action",
            Self::Accepted => "accepted",
            Self::Tentative => "tentative",
            Self::Declined => "declined",
        }
    }
}

impl std::str::FromStr for Response {
    type Err = ();

    fn from_str(text: &str) -> Result<Self, ()> {
        match text {
            "needs_action" => Ok(Self::NeedsAction),
            "accepted" => Ok(Self::Accepted),
            "tentative" => Ok(Self::Tentative),
            "declined" => Ok(Self::Declined),
            _ => Err(()),
        }
    }
}

/// Why an event does not show what its member last asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EventNotice {
    /// The event changed at the provider meanwhile: theirs was kept.
    KeptTheirs,
    /// The provider did not take the change.
    NotSaved,
}

impl EventNotice {
    /// The database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::KeptTheirs => "kept_theirs",
            Self::NotSaved => "not_saved",
        }
    }
}

impl std::str::FromStr for EventNotice {
    type Err = ();

    fn from_str(text: &str) -> Result<Self, ()> {
        match text {
            "kept_theirs" => Ok(Self::KeptTheirs),
            "not_saved" => Ok(Self::NotSaved),
            _ => Err(()),
        }
    }
}

/// Where a calendar lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CalendarSource {
    /// The member's own Campbooks calendar: no provider sees it.
    Campbooks,
    /// A calendar of a connected provider account.
    Provider,
}

/// Record type of a calendar (the access catalogue and `x-campbooks`).
pub const CALENDAR_RECORD: &str = "schedule.calendar";
/// Record type of an event.
pub const EVENT_RECORD: &str = "schedule.event";

/// The change connectors is asked to make at the provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CalendarChange {
    /// Create an event.
    Create,
    /// Change one event.
    Update,
    /// Change one occurrence of a series.
    UpdateOccurrence,
    /// Delete an event.
    Delete,
    /// Answer an invitation.
    Respond,
}

impl CalendarChange {
    /// The wire and database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Update => "update",
            Self::UpdateOccurrence => "update_occurrence",
            Self::Delete => "delete",
            Self::Respond => "respond",
        }
    }
}

/// The schema of a stored blob reference (`campbooks_kernel::blobs::BlobRef`)
/// as it appears in events and documents: read the bytes with
/// `BlobStore::get` in the event's workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct BlobPointer {
    /// SHA-256 of the bytes, 64 lower-case hex digits.
    pub hash: String,
    /// Length in bytes.
    pub size: u64,
    /// The media type recorded with it.
    pub content_type: String,
}

impl From<&campbooks_kernel::blobs::BlobRef> for BlobPointer {
    fn from(blob: &campbooks_kernel::blobs::BlobRef) -> Self {
        Self {
            hash: blob.hash.to_string(),
            size: blob.size,
            content_type: blob.content_type.clone(),
        }
    }
}
