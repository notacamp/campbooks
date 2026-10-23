//! A member's calendar settings: their time zone, working hours, the kinds
//! Scout adds, and per kind the calendar an event is also added to.

use std::collections::BTreeMap;

use campbooks_kernel::{
    Error, Result,
    tenancy::{UserId, WorkspaceId},
};
use chrono::NaiveTime;
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, types::Json};
use utoipa::ToSchema;

use super::moment::{parse_zone, zone_or_utc};
use crate::model::{CalendarId, EventKind};

/// The weekdays, as working hours name them.
pub const WEEKDAYS: [&str; 7] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

/// One stretch of a working day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct HoursRange {
    /// `HH:MM`, 24 hours.
    #[schema(example = "09:00")]
    pub start: String,
    /// `HH:MM`, after the start.
    #[schema(example = "18:00")]
    pub end: String,
}

/// Working hours by weekday (`mon` … `sun`); a missing weekday is a day off.
pub type WorkingHours = BTreeMap<String, Vec<HoursRange>>;

/// Monday to Friday, 09:00 to 18:00: the hours of a member who set none.
pub fn default_hours() -> WorkingHours {
    WEEKDAYS[..5]
        .iter()
        .map(|day| {
            (
                (*day).to_owned(),
                vec![HoursRange {
                    start: "09:00".to_owned(),
                    end: "18:00".to_owned(),
                }],
            )
        })
        .collect()
}

/// The kinds Scout adds until the member chooses.
pub fn default_scout_kinds() -> Vec<EventKind> {
    EventKind::ALL
        .into_iter()
        .filter(|kind| scout_can_add(*kind))
        .collect()
}

/// Whether Scout reads events of `kind` out of mail (not holds, not none).
pub const fn scout_can_add(kind: EventKind) -> bool {
    !matches!(kind, EventKind::Hold | EventKind::None)
}

/// A member's settings, as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// Their IANA time zone, once set.
    pub time_zone: Option<String>,
    /// Working hours; empty until they set some (then [`default_hours`]).
    pub working_hours: WorkingHours,
    /// The kinds Scout adds.
    pub scout_kinds: Vec<EventKind>,
    /// Per kind, the provider calendar an event is also added to.
    pub also_add_to: BTreeMap<String, CalendarId>,
    /// The provider calendar they last pushed to.
    pub last_push_calendar: Option<CalendarId>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            time_zone: None,
            working_hours: WorkingHours::new(),
            scout_kinds: default_scout_kinds(),
            also_add_to: BTreeMap::new(),
            last_push_calendar: None,
        }
    }
}

impl Settings {
    /// The zone events are shown in: theirs, or UTC.
    pub fn zone(&self) -> Tz {
        zone_or_utc(self.time_zone.as_deref())
    }

    /// The working hours in force.
    pub fn hours(&self) -> WorkingHours {
        if self.working_hours.is_empty() {
            default_hours()
        } else {
            self.working_hours.clone()
        }
    }
}

type Row = (
    Option<String>,
    Json<WorkingHours>,
    Vec<String>,
    Json<BTreeMap<String, CalendarId>>,
    Option<CalendarId>,
);

/// The member's settings (the defaults when they have none yet).
pub async fn load(conn: &mut PgConnection, member: UserId) -> Result<Settings> {
    let row = sqlx::query_as::<_, Row>(
        "SELECT time_zone, working_hours, scout_kinds, also_add_to, last_push_calendar
           FROM schedule.member_settings WHERE member_id = $1",
    )
    .bind(member)
    .fetch_optional(conn)
    .await?;
    Ok(
        row.map_or_else(Settings::default, |(zone, hours, kinds, also, last)| {
            Settings {
                time_zone: zone,
                working_hours: hours.0,
                scout_kinds: kinds.iter().filter_map(|k| k.parse().ok()).collect(),
                also_add_to: also.0,
                last_push_calendar: last,
            }
        }),
    )
}

/// Store the member's settings.
pub async fn save(
    conn: &mut PgConnection,
    workspace: WorkspaceId,
    member: UserId,
    settings: &Settings,
) -> Result<()> {
    let kinds: Vec<&str> = settings.scout_kinds.iter().map(|k| k.as_str()).collect();
    sqlx::query(
        "INSERT INTO schedule.member_settings
             (workspace_id, member_id, time_zone, working_hours, scout_kinds, also_add_to,
              last_push_calendar)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (workspace_id, member_id) DO UPDATE
            SET time_zone = EXCLUDED.time_zone, working_hours = EXCLUDED.working_hours,
                scout_kinds = EXCLUDED.scout_kinds, also_add_to = EXCLUDED.also_add_to,
                last_push_calendar = EXCLUDED.last_push_calendar, updated_at = now()",
    )
    .bind(workspace)
    .bind(member)
    .bind(&settings.time_zone)
    .bind(Json(&settings.working_hours))
    .bind(&kinds)
    .bind(Json(&settings.also_add_to))
    .bind(settings.last_push_calendar)
    .execute(conn)
    .await?;
    Ok(())
}

/// Check working hours: known weekdays, `HH:MM` times, ends after starts.
pub fn check_hours(hours: &WorkingHours) -> Result<()> {
    let invalid = || Error::BadRequest(crate::MESSAGES.reason("invalid_hours"));
    for (day, ranges) in hours {
        if !WEEKDAYS.contains(&day.as_str()) {
            return Err(invalid());
        }
        for range in ranges {
            let start = NaiveTime::parse_from_str(&range.start, "%H:%M").map_err(|_| invalid())?;
            let end = NaiveTime::parse_from_str(&range.end, "%H:%M").map_err(|_| invalid())?;
            if end <= start {
                return Err(invalid());
            }
        }
    }
    Ok(())
}

/// Check a time zone name.
pub fn check_zone(name: &str) -> Result<()> {
    parse_zone(name)
        .map(drop)
        .ok_or_else(|| Error::BadRequest(crate::MESSAGES.reason("invalid_time_zone")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hours(day: &str, start: &str, end: &str) -> WorkingHours {
        WorkingHours::from([(
            day.to_owned(),
            vec![HoursRange {
                start: start.to_owned(),
                end: end.to_owned(),
            }],
        )])
    }

    #[test]
    fn working_hours_end_after_they_start() {
        assert!(check_hours(&hours("mon", "09:00", "18:00")).is_ok());
        assert!(check_hours(&hours("mon", "18:00", "09:00")).is_err());
        assert!(check_hours(&hours("mon", "09:00", "09:00")).is_err());
        assert!(check_hours(&hours("monday", "09:00", "18:00")).is_err());
        assert!(check_hours(&hours("mon", "9am", "6pm")).is_err());
    }

    #[test]
    fn a_time_zone_is_an_iana_name() {
        assert!(check_zone("Europe/Lisbon").is_ok());
        assert!(check_zone("GMT+1").is_err());
        assert!(check_zone("").is_err());
    }

    #[test]
    fn until_a_member_sets_hours_they_work_weekdays() {
        let settings = Settings::default();
        let hours = settings.hours();
        assert_eq!(hours.len(), 5);
        assert!(!hours.contains_key("sat"));
        assert_eq!(settings.zone(), Tz::UTC);
    }
}
