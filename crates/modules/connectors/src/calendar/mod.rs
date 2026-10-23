//! The calendar capability: the documents the adapters produce and read
//! (`contracts/calendar-list.v1`, `calendar-change-batch.v1`,
//! `calendar-change.v1`), the provider calendars a stream knows, and the one
//! door ([`Source`]) a run reads and writes through whichever the provider.
//!
//! The schedule module reads the same documents with its own types; modules
//! never import each other, so the JSON Schema files are the contract and
//! both sides test against them.

use campbooks_kernel::{Result, tenancy::WorkspaceId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::{domain::model::StreamId, google::CalendarError};

pub mod changes;
pub mod google;
pub mod zoho;

/// Why a calendar stream cannot work yet: the calendar API is not enabled
/// for the Google project this instance is registered as.
pub const API_NOT_ENABLED: &str = "calendar_api_not_enabled";

/// The media type of a `calendar-list` v1 document.
pub const LIST_CONTENT_TYPE: &str = "application/vnd.campbooks.calendar-list+json; version=1";

/// The media type of a `calendar-change-batch` v1 document.
pub const BATCH_CONTENT_TYPE: &str = "application/vnd.campbooks.calendar-changes+json; version=1";

/// Events that ended this long ago are not read by a first read; a series
/// still recurring is.
pub const FIRST_READ_DAYS: i64 = 90;

/// Most events or removals one batch holds (`calendar-change-batch.v1`).
pub const BATCH_LIMIT: usize = 2_500;

/// The kinds a Campbooks event has, which `campbooks_kind` may name.
const KINDS: [&str; 11] = [
    "charge",
    "appointment",
    "meeting",
    "travel",
    "stay",
    "show",
    "reservation",
    "delivery",
    "expiry",
    "hold",
    "none",
];

/// The `calendar-list` v1 document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ListDocument {
    /// The format version, 1.
    pub version: u16,
    /// Whose calendars they are: `google` or `zoho`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<&'static str>,
    /// The calendars.
    pub calendars: Vec<ListedCalendar>,
}

/// One calendar of a [`ListDocument`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ListedCalendar {
    /// Its id at the provider.
    pub provider_id: String,
    /// Its name.
    pub name: String,
    /// Its colour, `#rrggbb`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Its IANA time zone.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_zone: Option<String>,
    /// The account's primary calendar.
    pub primary: bool,
    /// Whether events may be written on it.
    pub writable: bool,
}

/// When an event starts or ends: a date (all-day), or a time with an offset
/// and the zone it was written in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum When {
    /// A date.
    Date {
        /// `YYYY-MM-DD`.
        date: String,
    },
    /// A time.
    DateTime {
        /// RFC 3339.
        date_time: String,
        /// The IANA zone.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        time_zone: Option<String>,
    },
}

impl When {
    /// The moment as the provider names it in a query: the RFC 3339 time, or
    /// the date.
    pub fn as_text(&self) -> &str {
        match self {
            Self::Date { date } => date,
            Self::DateTime { date_time, .. } => date_time,
        }
    }
}

/// What Campbooks keeps on an event it created.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Private {
    /// The Campbooks event (a UUID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub campbooks_event: Option<String>,
    /// Its kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub campbooks_kind: Option<String>,
    /// Its source record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub campbooks_source: Option<String>,
}

impl Private {
    /// The properties as a provider stores them (`None` for none).
    pub fn pairs(&self) -> Vec<(&'static str, &str)> {
        let mut pairs = Vec::new();
        if let Some(value) = &self.campbooks_event {
            pairs.push(("campbooks_event", value.as_str()));
        }
        if let Some(value) = &self.campbooks_kind {
            pairs.push(("campbooks_kind", value.as_str()));
        }
        if let Some(value) = &self.campbooks_source {
            pairs.push(("campbooks_source", value.as_str()));
        }
        pairs
    }

    /// The properties a provider event holds, checked: an event id that is
    /// a UUID, a kind Campbooks has, a source of at most 200 characters.
    /// `None` when it holds none of them.
    pub fn read<'a>(mut get: impl FnMut(&str) -> Option<&'a str>) -> Option<Self> {
        let event = get("campbooks_event")
            .and_then(|id| Uuid::parse_str(id).ok())
            .map(|id| id.to_string());
        let kind = get("campbooks_kind")
            .filter(|kind| KINDS.contains(kind))
            .map(ToOwned::to_owned);
        let source = get("campbooks_source")
            .filter(|source| !source.is_empty() && source.chars().count() <= 200)
            .map(ToOwned::to_owned);
        (event.is_some() || kind.is_some() || source.is_some()).then_some(Self {
            campbooks_event: event,
            campbooks_kind: kind,
            campbooks_source: source,
        })
    }
}

/// A person on an event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PersonDocument {
    /// Their name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Their address.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Their answer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<&'static str>,
    /// The member.
    #[serde(rename = "self", skip_serializing_if = "std::ops::Not::not")]
    pub is_self: bool,
}

/// One event of a batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BatchEvent {
    /// Its id at the provider.
    pub provider_id: String,
    /// Its etag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    /// `confirmed`, `tentative` or `cancelled`.
    pub status: &'static str,
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
    pub start: When,
    /// When it ends.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<When>,
    /// The provider's own RRULE, RDATE and EXDATE lines.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub recurrence: Vec<String>,
    /// An exception: the id of its series.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series: Option<String>,
    /// An exception: the start of the occurrence it replaces.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_start: Option<When>,
    /// Its iCalendar UID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ical_uid: Option<String>,
    /// Who organises it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organizer: Option<PersonDocument>,
    /// Who is invited.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attendees: Vec<PersonDocument>,
    /// The member's own answer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<&'static str>,
    /// What Campbooks wrote on it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private: Option<Private>,
}

/// A removal in a batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Removal {
    /// The removed event's id at the provider.
    pub provider_id: String,
}

/// The `calendar-change-batch` v1 document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BatchDocument {
    /// The format version, 1.
    pub version: u16,
    /// The calendar at the provider.
    pub provider_calendar_id: String,
    /// A full read: events the calendar holds that are not here are gone.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub full: bool,
    /// Events created or changed.
    pub upserts: Vec<BatchEvent>,
    /// Events removed.
    pub removals: Vec<Removal>,
}

/// What reading one calendar gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Read {
    /// Events created or changed, in the provider's order.
    pub upserts: Vec<BatchEvent>,
    /// Ids of events removed.
    pub removals: Vec<String>,
    /// A full read (the first, or after the cursor was dropped).
    pub full: bool,
    /// The token to read changes from next; `None` for a source with no
    /// such token (the cursor then stays as it was).
    pub token: Option<String>,
}

impl Read {
    /// The batches of this read for `calendar`: one when it fits the
    /// contract's limits (and then `full` says the calendar was read whole),
    /// otherwise chunks of at most [`BATCH_LIMIT`], none of them `full`.
    pub fn batches(self, calendar: &str) -> Vec<BatchDocument> {
        let Self {
            upserts,
            removals,
            full,
            ..
        } = self;
        let whole = upserts.len() <= BATCH_LIMIT && removals.len() <= BATCH_LIMIT;
        if whole {
            if upserts.is_empty() && removals.is_empty() && !full {
                return Vec::new();
            }
            return vec![BatchDocument {
                version: 1,
                provider_calendar_id: calendar.to_owned(),
                full,
                upserts,
                removals: removals
                    .into_iter()
                    .map(|provider_id| Removal { provider_id })
                    .collect(),
            }];
        }
        let mut documents = Vec::new();
        let mut upserts = upserts.into_iter().peekable();
        let mut removals = removals.into_iter().peekable();
        while upserts.peek().is_some() || removals.peek().is_some() {
            documents.push(BatchDocument {
                version: 1,
                provider_calendar_id: calendar.to_owned(),
                full: false,
                upserts: upserts.by_ref().take(BATCH_LIMIT).collect(),
                removals: removals
                    .by_ref()
                    .take(BATCH_LIMIT)
                    .map(|provider_id| Removal { provider_id })
                    .collect(),
            });
        }
        documents
    }
}

/// A provider calendar a stream knows.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct KnownCalendar {
    /// The row.
    pub id: Uuid,
    /// Its id at the provider.
    pub provider_calendar_id: String,
    /// Whether it is read.
    pub syncing: bool,
    /// Where the provider's changes continue; none before the first read.
    pub sync_token: Option<String>,
}

/// The calendars `stream` knows, by id.
pub async fn known(conn: &mut PgConnection, stream: StreamId) -> Result<Vec<KnownCalendar>> {
    Ok(sqlx::query_as(
        "SELECT id, provider_calendar_id, syncing, sync_token
           FROM connectors.calendars WHERE stream_id = $1 ORDER BY provider_calendar_id, id",
    )
    .bind(stream)
    .fetch_all(conn)
    .await?)
}

/// Remember the calendars of a listing for `stream`: the primary one reads
/// at once, the others when the member turns them on. A calendar known
/// already is left as it is. Returns how many were new.
pub async fn remember(
    conn: &mut PgConnection,
    workspace: WorkspaceId,
    stream: StreamId,
    listed: &[ListedCalendar],
) -> Result<u64> {
    let mut added = 0;
    for calendar in listed {
        added += sqlx::query(
            "INSERT INTO connectors.calendars
                 (id, workspace_id, stream_id, provider_calendar_id, syncing)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (stream_id, provider_calendar_id) DO NOTHING",
        )
        .bind(Uuid::now_v7())
        .bind(workspace)
        .bind(stream)
        .bind(&calendar.provider_id)
        .bind(calendar.primary)
        .execute(&mut *conn)
        .await?
        .rows_affected();
    }
    Ok(added)
}

/// Record that `calendar` was read up to `token` at `now`.
pub async fn read_up_to(
    conn: &mut PgConnection,
    calendar: Uuid,
    token: Option<&str>,
    full: bool,
    now: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "UPDATE connectors.calendars
            SET sync_token = coalesce($2, sync_token),
                last_full_at = CASE WHEN $3 THEN $4 ELSE last_full_at END
          WHERE id = $1",
    )
    .bind(calendar)
    .bind(token)
    .bind(full)
    .bind(now)
    .execute(conn)
    .await?;
    Ok(())
}

/// How a change did not apply, for the run to record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteFailure {
    /// The event changed at the provider first (412).
    Superseded,
    /// Nothing was written, for this stable reason.
    Skip(&'static str),
    /// The provider could not be reached or asked for patience: the change
    /// stays pending.
    Trouble(crate::provider::ProviderError),
    /// The calendar API is not enabled for the app.
    ApiNotEnabled,
}

impl From<CalendarError> for WriteFailure {
    /// A failed call, as it ends a change.
    fn from(error: CalendarError) -> Self {
        use crate::provider::ProviderError;
        match error {
            CalendarError::PreconditionFailed => Self::Superseded,
            CalendarError::ApiNotEnabled => Self::ApiNotEnabled,
            CalendarError::Gone | CalendarError::Provider(ProviderError::NotFound) => {
                Self::Skip("not_found")
            }
            CalendarError::Exists | CalendarError::Provider(ProviderError::Unexpected(_)) => {
                Self::Skip("rejected")
            }
            CalendarError::Provider(other) => Self::Trouble(other),
        }
    }
}

/// What the provider answered to a change it made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    /// The event's id at the provider.
    pub provider_event_id: String,
    /// Its new etag.
    pub etag: Option<String>,
}

/// The `calendar-change` v1 document, as the schedule module wrote it.
#[derive(Debug, Clone, Deserialize)]
pub struct ChangeDocument {
    /// The format version, 1.
    pub version: u32,
    /// `create`, `update`, `update_occurrence`, `delete` or `respond`.
    pub change: String,
    /// The calendar at the provider.
    pub provider_calendar_id: String,
    /// The event at the provider (the series, for an occurrence).
    #[serde(default)]
    pub provider_event_id: Option<String>,
    /// The etag the member's version was based on.
    #[serde(default)]
    pub etag: Option<String>,
    /// `update_occurrence` only: the start of the occurrence.
    #[serde(default)]
    pub occurrence: Option<When>,
    /// `respond` only.
    #[serde(default)]
    pub response: Option<String>,
    /// The event to write.
    #[serde(default)]
    pub event: Option<ChangeEvent>,
}

/// The event to write.
#[derive(Debug, Clone, Deserialize)]
pub struct ChangeEvent {
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
    pub start: When,
    /// When it ends.
    #[serde(default)]
    pub end: Option<When>,
    /// What to write so it keeps its kind and source (creates only).
    #[serde(default)]
    pub private: Option<Private>,
}

impl ChangeDocument {
    /// Whether the document is one this module writes: a known version and
    /// kind, with what that kind needs, within the contract's sizes.
    pub fn is_sound(&self) -> bool {
        let sized = |text: Option<&str>, max: usize| text.is_none_or(|t| t.chars().count() <= max);
        let event_sound = self.event.as_ref().is_none_or(|event| {
            sized(event.title.as_deref(), 1_000)
                && sized(event.place.as_deref(), 1_000)
                && sized(event.description.as_deref(), 20_000)
        });
        let needs_event = matches!(
            self.change.as_str(),
            "create" | "update" | "update_occurrence"
        );
        let needs_id = !matches!(self.change.as_str(), "create");
        self.version == 1
            && !self.provider_calendar_id.is_empty()
            && self.provider_calendar_id.chars().count() <= 512
            && matches!(
                self.change.as_str(),
                "create" | "update" | "update_occurrence" | "delete" | "respond"
            )
            && event_sound
            && (!needs_event || self.event.is_some())
            && (!needs_id
                || self
                    .provider_event_id
                    .as_deref()
                    .is_some_and(|id| !id.is_empty() && id.chars().count() <= 1_024))
            && (self.change != "respond"
                || matches!(
                    self.response.as_deref(),
                    Some("accepted" | "tentative" | "declined")
                ))
            && (self.change != "update_occurrence" || self.occurrence.is_some())
    }
}

/// One provider's calendar API, for a run: the same few questions whichever
/// the provider.
#[derive(Debug)]
pub enum Source<'a> {
    /// Google Calendar.
    Google(crate::google::Calendar<'a>),
    /// Zoho Calendar (only while its flag is on).
    Zoho(zoho::ZohoCalendar<'a>),
}

impl Source<'_> {
    /// The account's calendars.
    pub async fn list(&self) -> Result<Vec<ListedCalendar>, CalendarError> {
        match self {
            Self::Google(api) => google::list(api).await,
            Self::Zoho(api) => zoho::list(api).await,
        }
    }

    /// One calendar's changes since `token` (all of it when none).
    pub async fn read(
        &self,
        calendar: &str,
        token: Option<&str>,
        now: DateTime<Utc>,
    ) -> Result<Read, CalendarError> {
        match self {
            Self::Google(api) => google::read(api, calendar, token, now).await,
            Self::Zoho(api) => zoho::read(api, calendar, token, now).await,
        }
    }

    /// Make `change` (request `request`) at the provider.
    pub async fn write(
        &self,
        change: &ChangeDocument,
        request: Uuid,
    ) -> Result<Written, WriteFailure> {
        match self {
            Self::Google(api) => google::write(api, change, request).await,
            Self::Zoho(api) => zoho::write(api, change, request).await,
        }
    }
}

/// `text` cut to at most `max` characters.
pub fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_owned()
    } else {
        text.chars().take(max).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(id: &str) -> BatchEvent {
        BatchEvent {
            provider_id: id.to_owned(),
            etag: None,
            status: "confirmed",
            title: Some("Dentist".to_owned()),
            place: None,
            description: None,
            start: When::Date {
                date: "2026-11-05".to_owned(),
            },
            end: None,
            recurrence: Vec::new(),
            series: None,
            original_start: None,
            ical_uid: None,
            organizer: None,
            attendees: Vec::new(),
            response: None,
            private: None,
        }
    }

    #[test]
    fn a_read_that_fits_is_one_batch_and_an_empty_incremental_read_is_none() {
        let read = Read {
            upserts: vec![event("a")],
            removals: vec!["b".to_owned()],
            full: true,
            token: Some("t".to_owned()),
        };
        let batches = read.batches("cal");
        assert_eq!(batches.len(), 1);
        assert!(batches[0].full);
        let empty = Read {
            upserts: Vec::new(),
            removals: Vec::new(),
            full: false,
            token: None,
        };
        assert!(empty.batches("cal").is_empty());
        // A first read of an empty calendar is still a full read: whatever
        // the calendar held is gone.
        let first = Read {
            upserts: Vec::new(),
            removals: Vec::new(),
            full: true,
            token: None,
        };
        assert_eq!(first.batches("cal").len(), 1);
    }

    #[test]
    fn a_read_over_the_limit_is_chunked_and_not_full() {
        let read = Read {
            upserts: (0..BATCH_LIMIT + 1)
                .map(|n| event(&n.to_string()))
                .collect(),
            removals: Vec::new(),
            full: true,
            token: None,
        };
        let batches = read.batches("cal");
        assert_eq!(batches.len(), 2);
        assert!(batches.iter().all(|b| !b.full));
        assert_eq!(batches[0].upserts.len(), BATCH_LIMIT);
        assert_eq!(batches[1].upserts.len(), 1);
    }

    #[test]
    fn private_properties_are_checked_before_they_are_believed() {
        let id = Uuid::now_v7().to_string();
        let props = [
            ("campbooks_event", id.as_str()),
            ("campbooks_kind", "meeting"),
            ("campbooks_source", "conversations.message:1"),
        ];
        let get = |key: &str| props.iter().find(|(k, _)| *k == key).map(|(_, v)| *v);
        let read = Private::read(get).unwrap();
        assert_eq!(read.campbooks_event.as_deref(), Some(id.as_str()));
        assert_eq!(read.campbooks_kind.as_deref(), Some("meeting"));
        let bad = [
            ("campbooks_event", "not-a-uuid"),
            ("campbooks_kind", "party"),
        ];
        let get = |key: &str| bad.iter().find(|(k, _)| *k == key).map(|(_, v)| *v);
        assert_eq!(Private::read(get), None);
    }

    #[test]
    fn a_change_document_must_hold_what_its_kind_needs() {
        let doc =
            |json: serde_json::Value| -> ChangeDocument { serde_json::from_value(json).unwrap() };
        let create = doc(serde_json::json!({
            "version": 1, "change": "create", "provider_calendar_id": "c",
            "event": {"start": {"date": "2026-11-05"}}
        }));
        assert!(create.is_sound());
        let update_without_etag = doc(serde_json::json!({
            "version": 1, "change": "update", "provider_calendar_id": "c",
            "provider_event_id": "e", "event": {"start": {"date": "2026-11-05"}}
        }));
        // The etag is checked when it is sent (If-Match), not required here.
        assert!(update_without_etag.is_sound());
        let delete_without_id = doc(serde_json::json!({
            "version": 1, "change": "delete", "provider_calendar_id": "c"
        }));
        assert!(!delete_without_id.is_sound());
        let respond = doc(serde_json::json!({
            "version": 1, "change": "respond", "provider_calendar_id": "c",
            "provider_event_id": "e", "response": "maybe"
        }));
        assert!(!respond.is_sound());
    }

    #[test]
    fn a_time_serialises_the_way_the_contract_names_it() {
        let when = When::DateTime {
            date_time: "2026-11-05T15:00:00+00:00".to_owned(),
            time_zone: Some("Europe/Lisbon".to_owned()),
        };
        assert_eq!(
            serde_json::to_value(&when).unwrap(),
            serde_json::json!({"date_time": "2026-11-05T15:00:00+00:00",
                               "time_zone": "Europe/Lisbon"})
        );
        let back: When = serde_json::from_value(serde_json::json!({"date": "2026-11-05"})).unwrap();
        assert_eq!(back.as_text(), "2026-11-05");
    }
}
