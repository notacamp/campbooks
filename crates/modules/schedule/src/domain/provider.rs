//! An event as a provider has it: read out of a batch, and kept as the last
//! provider version for a change the provider did not take.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;

use super::{moment::Span, rows::EventRow};
use crate::{
    inbound::BatchEvent,
    model::{EventId, EventKind},
};

/// An event of a batch, in the module's own terms.
#[derive(Debug, Clone)]
pub struct Parsed {
    /// Its title.
    pub title: String,
    /// Its place.
    pub place: Option<String>,
    /// Its notes.
    pub notes: Option<String>,
    /// When it starts and ends.
    pub span: Span,
    /// The zone it was written in.
    pub time_zone: Option<String>,
    /// `confirmed`, `tentative` or `cancelled`.
    pub status: String,
    /// RRULE, RDATE and EXDATE lines.
    pub recurrence: Vec<String>,
    /// For an exception, the name of the occurrence it replaces.
    pub original: Option<String>,
    /// The kind Campbooks wrote on it, if it did.
    pub kind: Option<EventKind>,
}

/// The module's reading of `event`; `None` when its times cannot be read (it
/// is skipped, and logged by id elsewhere).
pub fn parse(event: &BatchEvent) -> Option<Parsed> {
    let (start, zone) = event.start.moment()?;
    let end = event
        .end
        .as_ref()
        .and_then(crate::inbound::ProviderWhen::moment)
        .map(|(m, _)| m);
    // A provider event with an end of another form, or one that does not
    // come after the start, is shown with the default length.
    let span = Span::new(start, end).or_else(|| Span::new(start, None))?;
    let original = event
        .original_start
        .as_ref()
        .and_then(crate::inbound::ProviderWhen::moment)
        .map(|(moment, _)| moment.key());
    Some(Parsed {
        title: event.title.clone().unwrap_or_default(),
        place: event.place.clone().filter(|place| !place.is_empty()),
        notes: event.description.clone().filter(|notes| !notes.is_empty()),
        span,
        time_zone: zone,
        status: match event.status.as_str() {
            "tentative" => "tentative",
            "cancelled" => "cancelled",
            _ => "confirmed",
        }
        .to_owned(),
        recurrence: event.recurrence.clone(),
        original,
        kind: event
            .private
            .as_ref()
            .and_then(|private| private.campbooks_kind.as_deref())
            .and_then(|kind| kind.parse().ok()),
    })
}

/// The fields a provider owns, as the provider last had them.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Snapshot {
    title: String,
    place: Option<String>,
    notes: Option<String>,
    status: String,
    starts_at: Option<DateTime<Utc>>,
    ends_at: Option<DateTime<Utc>>,
    starts_on: Option<NaiveDate>,
    ends_on: Option<NaiveDate>,
    all_day: bool,
    time_zone: Option<String>,
    recurrence: Vec<String>,
}

impl Snapshot {
    /// `row` as it is now.
    pub fn of(row: &EventRow) -> Self {
        Self {
            title: row.title.clone(),
            place: row.place.clone(),
            notes: row.notes.clone(),
            status: row.status.clone(),
            starts_at: row.starts_at,
            ends_at: row.ends_at,
            starts_on: row.starts_on,
            ends_on: row.ends_on,
            all_day: row.all_day,
            time_zone: row.time_zone.clone(),
            recurrence: row.recurrence.clone(),
        }
    }

    /// The provider fields of `parsed`.
    pub fn from_parsed(parsed: &Parsed) -> Self {
        let (starts_at, ends_at, starts_on, ends_on) = parsed.span.columns();
        Self {
            title: parsed.title.clone(),
            place: parsed.place.clone(),
            notes: parsed.notes.clone(),
            status: parsed.status.clone(),
            starts_at,
            ends_at,
            starts_on,
            ends_on,
            all_day: parsed.span.start.is_day(),
            time_zone: parsed.time_zone.clone(),
            recurrence: parsed.recurrence.clone(),
        }
    }

    /// Whether it says the event is cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.status == "cancelled"
    }

    /// The snapshot as `jsonb`.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    /// Write it back over event `id`.
    pub async fn restore(&self, conn: &mut PgConnection, id: EventId) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE schedule.events
                SET title = $2, place = $3, notes = $4, status = $5, starts_at = $6,
                    ends_at = $7, starts_on = $8, ends_on = $9, all_day = $10,
                    time_zone = $11, recurrence = $12, updated_at = now()
              WHERE id = $1",
        )
        .bind(id)
        .bind(&self.title)
        .bind(&self.place)
        .bind(&self.notes)
        .bind(&self.status)
        .bind(self.starts_at)
        .bind(self.ends_at)
        .bind(self.starts_on)
        .bind(self.ends_on)
        .bind(self.all_day)
        .bind(&self.time_zone)
        .bind(&self.recurrence)
        .execute(conn)
        .await?;
        Ok(())
    }
}

/// The key of the start of `row` (what an exception names), if it has times.
pub fn start_key(row: &EventRow) -> Option<String> {
    row.span().map(|span| span.start.key())
}
