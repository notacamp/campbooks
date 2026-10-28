//! Free time: the stretches within a member's working hours that no event of
//! the calendars they see overlaps. Cancelled events and all-day events do
//! not count, nor does an event marked not busy.

use campbooks_kernel::{Result, tenancy::UserId};
use chrono::{DateTime, Datelike as _, NaiveTime, TimeDelta, TimeZone as _, Utc, Weekday};
use chrono_tz::Tz;

use super::{
    listing::{self, Filter},
    rows::CalendarRow,
    settings::{Settings, WorkingHours},
};
use crate::model::CalendarId;
use sqlx::PgConnection;

/// A stretch of time nothing is booked in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot {
    /// When it starts.
    pub starts: DateTime<Utc>,
    /// When it ends.
    pub ends: DateTime<Utc>,
}

/// The slots found, and the calendars whose events were counted.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Found {
    /// Earliest first.
    pub slots: Vec<Slot>,
    /// The calendars counted: the member's Campbooks calendar and the
    /// provider calendars that sync.
    pub calendars: Vec<CalendarId>,
}

const fn key(day: Weekday) -> &'static str {
    match day {
        Weekday::Mon => "mon",
        Weekday::Tue => "tue",
        Weekday::Wed => "wed",
        Weekday::Thu => "thu",
        Weekday::Fri => "fri",
        Weekday::Sat => "sat",
        Weekday::Sun => "sun",
    }
}

/// The working stretches within `from..to`, in UTC, earliest first.
pub fn working_stretches(
    hours: &WorkingHours,
    zone: Tz,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Vec<(DateTime<Utc>, DateTime<Utc>)> {
    let mut out = Vec::new();
    // A day before and after: the window's edges fall inside local days.
    let mut day = from.with_timezone(&zone).date_naive() - TimeDelta::days(1);
    let last = to.with_timezone(&zone).date_naive() + TimeDelta::days(1);
    while day <= last {
        for range in hours.get(key(day.weekday())).into_iter().flatten() {
            let (Ok(start), Ok(end)) = (
                NaiveTime::parse_from_str(&range.start, "%H:%M"),
                NaiveTime::parse_from_str(&range.end, "%H:%M"),
            ) else {
                continue;
            };
            let at = |time: NaiveTime| {
                zone.from_local_datetime(&day.and_time(time))
                    .earliest()
                    .map(|local| local.with_timezone(&Utc))
            };
            if let (Some(start), Some(end)) = (at(start), at(end)) {
                let (start, end) = (start.max(from), end.min(to));
                if end > start {
                    out.push((start, end));
                }
            }
        }
        day += TimeDelta::days(1);
    }
    out.sort();
    out
}

/// `stretches` without the `busy` spans, those of at least `duration`.
pub fn gaps(
    stretches: &[(DateTime<Utc>, DateTime<Utc>)],
    busy: &[(DateTime<Utc>, DateTime<Utc>)],
    duration: TimeDelta,
) -> Vec<Slot> {
    let mut busy = busy.to_vec();
    busy.sort();
    let mut slots = Vec::new();
    for &(start, end) in stretches {
        let mut cursor = start;
        for &(from, to) in &busy {
            if to <= cursor || from >= end {
                continue;
            }
            if from > cursor && from - cursor >= duration {
                slots.push(Slot {
                    starts: cursor,
                    ends: from,
                });
            }
            cursor = cursor.max(to);
        }
        if end > cursor && end - cursor >= duration {
            slots.push(Slot {
                starts: cursor,
                ends: end,
            });
        }
    }
    slots
}

/// The member's free slots of at least `duration` within `from..to`.
pub async fn free_slots(
    conn: &mut PgConnection,
    member: UserId,
    settings: &Settings,
    calendars: &[CalendarRow],
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    duration: TimeDelta,
) -> Result<Found> {
    let counted: Vec<&CalendarRow> = calendars
        .iter()
        .filter(|calendar| calendar.is_mine())
        .collect();
    let filter = Filter {
        busy_only: true,
        ..Filter::default()
    };
    let busy: Vec<(DateTime<Utc>, DateTime<Utc>)> = listing::list(conn, member, &filter, from, to)
        .await?
        .into_iter()
        .filter(|item| !item.span.start.is_day())
        .map(|item| (item.span.start.instant(), item.span.end.instant()))
        .collect();
    let stretches = working_stretches(&settings.hours(), settings.zone(), from, to);
    Ok(Found {
        slots: gaps(&stretches, &busy, duration),
        calendars: counted.iter().map(|calendar| calendar.id).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::settings::{HoursRange, default_hours};

    fn utc(text: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(text)
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn a_free_hour_on_tuesday_between_two_blocks() {
        // Tuesday 3 November 2026: 09:00-12:30 and 14:00-18:00 are booked.
        let zone: Tz = "Europe/Lisbon".parse().unwrap();
        let from = utc("2026-11-03T00:00:00Z");
        let to = utc("2026-11-04T00:00:00Z");
        let stretches = working_stretches(&default_hours(), zone, from, to);
        let busy = [
            (utc("2026-11-03T09:00:00Z"), utc("2026-11-03T12:30:00Z")),
            (utc("2026-11-03T14:00:00Z"), utc("2026-11-03T18:00:00Z")),
        ];
        let found = gaps(&stretches, &busy, TimeDelta::hours(1));
        assert_eq!(
            found,
            [Slot {
                starts: utc("2026-11-03T12:30:00Z"),
                ends: utc("2026-11-03T14:00:00Z"),
            }]
        );
        // Too long for the gap.
        assert!(gaps(&stretches, &busy, TimeDelta::hours(2)).is_empty());
    }

    #[test]
    fn a_weekend_has_no_working_hours() {
        let zone: Tz = "Europe/Lisbon".parse().unwrap();
        // Saturday 7 November 2026.
        let stretches = working_stretches(
            &default_hours(),
            zone,
            utc("2026-11-07T00:00:00Z"),
            utc("2026-11-08T00:00:00Z"),
        );
        assert!(stretches.is_empty());
    }

    #[test]
    fn working_hours_are_read_in_the_members_zone() {
        let zone: Tz = "Europe/Madrid".parse().unwrap();
        let mut hours = WorkingHours::new();
        hours.insert(
            "tue".to_owned(),
            vec![HoursRange {
                start: "09:00".to_owned(),
                end: "10:00".to_owned(),
            }],
        );
        let stretches = working_stretches(
            &hours,
            zone,
            utc("2026-11-03T00:00:00Z"),
            utc("2026-11-04T00:00:00Z"),
        );
        // 09:00 in Madrid in November is 08:00 UTC.
        assert_eq!(
            stretches,
            [(utc("2026-11-03T08:00:00Z"), utc("2026-11-03T09:00:00Z"))]
        );
    }
}
