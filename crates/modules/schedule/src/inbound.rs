//! The other modules' events this module consumes, and the blob documents
//! they point at, as their published contracts describe them (modules never
//! import each other): `contracts/conversations.message_read.v1`,
//! `connectors.capability_granted.v1`, `connectors.calendar_changed.v1`,
//! `connectors.calendar_change_completed.v1`, `calendar-list.v1`,
//! `calendar-change-batch.v1` and `calendar-change.v1`.
//!
//! Each type reads only the fields this module uses; what the contract adds
//! later is ignored.

use campbooks_kernel::{DomainEvent, blobs::BlobRef, tenancy::UserId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::model::{CalendarId, ConnectionId, EventId, RequestId};

/// An item of a read, as `conversations.message_read` carries it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadItem {
    /// Stable for the same message, layer and position.
    pub key: String,
    /// The kind, as the contract names it (checked when used).
    pub kind: String,
    /// When it starts: an RFC 3339 time with an offset, or a date.
    pub starts: String,
    /// When it ends, in the same form.
    #[serde(default)]
    pub ends: Option<String>,
    /// Whether it lasts whole days.
    pub all_day: bool,
    /// Its IANA time zone, when the message gave one.
    #[serde(default)]
    pub time_zone: Option<String>,
    /// The key of an earlier item it changes.
    #[serde(default)]
    pub updates: Option<String>,
    /// The key of an earlier item it cancels.
    #[serde(default)]
    pub cancels: Option<String>,
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
    /// The inbox's personal owner; absent for a shared inbox.
    #[serde(default)]
    pub owner: Option<UserId>,
    /// `structured` or `model`.
    pub layer: String,
    /// The things that happen at a time.
    pub items: Vec<ReadItem>,
    /// The `texts` v1 document (titles, places and calendar UIDs of the
    /// items, by key), a blob of the workspace.
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

/// The text of the items of a read: the blob `texts` points at.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ReadTexts {
    /// One entry per item that has any, by its key.
    #[serde(default)]
    pub items: Vec<ReadItemText>,
}

/// The text of one item of a read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadItemText {
    /// The item's key.
    pub key: String,
    /// What to call it.
    #[serde(default)]
    pub title: String,
    /// Where it is.
    #[serde(default)]
    pub place: Option<String>,
    /// The iCalendar `UID` of the sender's own event, from the structured
    /// reader.
    #[serde(default)]
    pub ical_uid: Option<String>,
    /// The sentence of the email that gave the date.
    #[serde(default)]
    pub quote: Option<String>,
}

/// `connectors.capability_granted` v1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityGranted {
    /// The workspace.
    pub workspace: String,
    /// The connection.
    pub connection: ConnectionId,
    /// `messages`, `calendar`, `files` or `bank`.
    pub capability: String,
    /// The member whose consent it is.
    pub granted_by: UserId,
    /// The `calendar-list` v1 document (`calendar` only).
    #[serde(default)]
    pub calendars: Option<BlobRef>,
}

impl DomainEvent for CapabilityGranted {
    const NAME: &'static str = "connectors.capability_granted";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// `connectors.calendar_changed` v1: a batch of changes to one provider
/// calendar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarChanged {
    /// The workspace.
    pub workspace: String,
    /// The connection.
    pub connection: ConnectionId,
    /// The `calendar-change-batch` v1 document.
    pub batch: BlobRef,
}

impl DomainEvent for CalendarChanged {
    const NAME: &'static str = "connectors.calendar_changed";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// How a change asked for by `schedule.outbound_requested` ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// Written at the provider.
    Applied,
    /// The event had changed at the provider (412).
    Superseded,
    /// Nothing was written.
    Skipped,
    /// It failed.
    Failed,
}

/// `connectors.calendar_change_completed` v1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarChangeCompleted {
    /// The workspace.
    pub workspace: String,
    /// The request.
    pub request: RequestId,
    /// The calendar.
    pub calendar: CalendarId,
    /// The event.
    pub event: EventId,
    /// How it ended.
    pub outcome: Outcome,
    /// The event's id at the provider (applied only).
    #[serde(default)]
    pub provider_event_id: Option<String>,
    /// The new etag (applied only).
    #[serde(default)]
    pub etag: Option<String>,
    /// Why it did not apply.
    #[serde(default)]
    pub reason: Option<String>,
}

impl DomainEvent for CalendarChangeCompleted {
    const NAME: &'static str = "connectors.calendar_change_completed";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// `connectors.connection_removed` v1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionRemoved {
    /// The workspace.
    pub workspace: String,
    /// The connection.
    pub connection: ConnectionId,
}

impl DomainEvent for ConnectionRemoved {
    const NAME: &'static str = "connectors.connection_removed";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// `workspaces.member_left` v1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberLeft {
    /// The workspace.
    pub workspace: String,
    /// The former member.
    pub user: UserId,
}

impl DomainEvent for MemberLeft {
    const NAME: &'static str = "workspaces.member_left";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// `workspaces.workspace_deleting` v1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceDeleting {
    /// The workspace.
    pub workspace: String,
    /// When its data goes.
    pub purge_after: DateTime<Utc>,
}

impl DomainEvent for WorkspaceDeleting {
    const NAME: &'static str = "workspaces.workspace_deleting";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

// --- Blob documents -----------------------------------------------------

/// `calendar-list` v1.
#[derive(Debug, Clone, Deserialize)]
pub struct CalendarList {
    /// Whose calendars they are: `google` or `zoho`.
    #[serde(default)]
    pub provider: Option<String>,
    /// The account's calendars.
    pub calendars: Vec<ListedCalendar>,
}

/// One calendar of a `calendar-list` v1 document.
#[derive(Debug, Clone, Deserialize)]
pub struct ListedCalendar {
    /// Its id at the provider.
    pub provider_id: String,
    /// Its name.
    pub name: String,
    /// The provider's colour for it.
    #[serde(default)]
    pub color: Option<String>,
    /// Its IANA time zone.
    #[serde(default)]
    pub time_zone: Option<String>,
    /// The account's primary calendar.
    pub primary: bool,
    /// Whether events may be written on it.
    pub writable: bool,
}

/// When an event starts or ends in a `calendar-change-batch` v1 or
/// `calendar-change` v1 document: a date, or a time with an offset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ProviderWhen {
    /// A date (all-day).
    Date {
        /// `YYYY-MM-DD`.
        date: String,
    },
    /// A time with an offset, and the zone it was written in.
    DateTime {
        /// RFC 3339.
        date_time: String,
        /// The IANA zone.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        time_zone: Option<String>,
    },
}

/// What Campbooks wrote on an event it created.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivateProperties {
    /// The Campbooks event.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub campbooks_event: Option<String>,
    /// Its kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub campbooks_kind: Option<String>,
    /// Its source record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub campbooks_source: Option<String>,
}

/// One event of a `calendar-change-batch` v1 document.
#[derive(Debug, Clone, Deserialize)]
pub struct BatchEvent {
    /// Its id at the provider.
    pub provider_id: String,
    /// Its etag.
    #[serde(default)]
    pub etag: Option<String>,
    /// `confirmed`, `tentative` or `cancelled`.
    pub status: String,
    /// Its title.
    #[serde(default)]
    pub title: Option<String>,
    /// Its place.
    #[serde(default)]
    pub place: Option<String>,
    /// Its description.
    #[serde(default)]
    pub description: Option<String>,
    /// When it starts.
    pub start: ProviderWhen,
    /// When it ends.
    #[serde(default)]
    pub end: Option<ProviderWhen>,
    /// RRULE, RDATE and EXDATE lines, as given.
    #[serde(default)]
    pub recurrence: Vec<String>,
    /// An exception: the provider id of its series.
    #[serde(default)]
    pub series: Option<String>,
    /// An exception: the start of the occurrence it replaces.
    #[serde(default)]
    pub original_start: Option<ProviderWhen>,
    /// The event's iCalendar UID.
    #[serde(default)]
    pub ical_uid: Option<String>,
    /// The member's own answer to an invitation.
    #[serde(default)]
    pub response: Option<String>,
    /// What Campbooks wrote on it.
    #[serde(default)]
    pub private: Option<PrivateProperties>,
}

/// A removal in a `calendar-change-batch` v1 document.
#[derive(Debug, Clone, Deserialize)]
pub struct BatchRemoval {
    /// The removed event's id at the provider.
    pub provider_id: String,
}

/// `calendar-change-batch` v1.
#[derive(Debug, Clone, Deserialize)]
pub struct CalendarBatch {
    /// The format version, 1.
    pub version: u32,
    /// The calendar at the provider.
    pub provider_calendar_id: String,
    /// A full read: events not in `upserts` are gone.
    #[serde(default)]
    pub full: bool,
    /// Events created or changed.
    pub upserts: Vec<BatchEvent>,
    /// Events removed.
    pub removals: Vec<BatchRemoval>,
}

/// The event to write in a `calendar-change` v1 document.
#[derive(Debug, Clone, Serialize)]
pub struct ChangeEvent {
    /// Its title.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Its place.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    /// Its description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// When it starts.
    pub start: ProviderWhen,
    /// When it ends.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<ProviderWhen>,
    /// What to write so it keeps its kind and source (creates only).
    #[serde(rename = "private", skip_serializing_if = "Option::is_none")]
    pub private: Option<PrivateProperties>,
}

/// `calendar-change` v1, as this module writes it.
#[derive(Debug, Clone, Serialize)]
pub struct ChangeDocument {
    /// The format version, 1.
    pub version: u32,
    /// `create`, `update`, `update_occurrence`, `delete` or `respond`.
    pub change: String,
    /// The calendar at the provider.
    pub provider_calendar_id: String,
    /// The event at the provider (the series, for an occurrence).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_event_id: Option<String>,
    /// The etag the member's version was based on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    /// `update_occurrence` only: the start of the occurrence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub occurrence: Option<ProviderWhen>,
    /// `respond` only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<String>,
    /// The event to write.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<ChangeEvent>,
}
