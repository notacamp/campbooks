//! The `text/calendar` parts of a message, read with an iCalendar parser
//! (RFC 5545; the `icalendar` crate), never with patterns.
//!
//! An event becomes a [`Candidate`]: `meeting` when it has attendees, else
//! `appointment`. `METHOD:CANCEL` (or `STATUS:CANCELLED`) cancels the event
//! with the same `UID`; a higher `SEQUENCE` updates it. Only typed properties
//! are read: `DTSTART`, `DTEND`, `DURATION`, `SUMMARY`, `LOCATION`, `UID`,
//! `SEQUENCE`, `STATUS`, `ATTENDEE` and the calendar's `METHOD`. A start with
//! no zone and no `Z` (a floating time) names no instant, so it is left out.

use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, TimeDelta, TimeZone as _, Utc};
use chrono_tz::Tz;
use icalendar::parser::{self, Component, Property};

use super::Candidate;
use crate::{
    domain::read::{MAX_PLACE, MAX_TITLE, When, tidy},
    model::ItemKind,
};

/// Most lines of a calendar part read.
const MAX_LINES: usize = 4_000;
/// Deepest nesting of components read (`VCALENDAR` > `VEVENT` > `VALARM`).
const MAX_DEPTH: usize = 6;
/// Most events read from one part.
const MAX_EVENTS: usize = 5;

/// What the calendar's `METHOD` asks of the reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Method {
    /// A request, a publication, or no method: an event to know about.
    Event,
    /// A cancellation.
    Cancel,
    /// A reply, a counter, a refresh: not an event of the reader's.
    Other,
}

impl Method {
    fn of(value: Option<&str>) -> Self {
        match value.map(str::to_ascii_uppercase).as_deref() {
            None | Some("REQUEST" | "PUBLISH") => Self::Event,
            Some("CANCEL") => Self::Cancel,
            Some(_) => Self::Other,
        }
    }
}

/// The events of a `text/calendar` part, in order. Nothing for a part that
/// is not a well-formed calendar, is too large, or nests too deeply.
pub fn events(text: &str) -> Vec<Candidate> {
    let unfolded = parser::unfold(text);
    if !shallow(&unfolded) {
        return Vec::new();
    }
    let Ok(calendar) = parser::read_calendar(&unfolded) else {
        return Vec::new();
    };
    let method = Method::of(
        calendar
            .properties
            .iter()
            .find(|p| p.name.as_str().eq_ignore_ascii_case("METHOD"))
            .map(|p| p.val.as_str().trim()),
    );
    if method == Method::Other {
        return Vec::new();
    }
    calendar
        .components
        .iter()
        .filter(|c| c.name.as_str().eq_ignore_ascii_case("VEVENT"))
        .filter_map(|event| candidate(event, method))
        .take(MAX_EVENTS)
        .collect()
}

/// Whether `unfolded` has few enough lines and nests few enough components
/// to hand to the parser, which recurses on nesting: counted from the
/// `BEGIN` and `END` lines of the format, nothing else.
fn shallow(unfolded: &str) -> bool {
    let starts = |line: &str, prefix: &str| {
        line.get(..prefix.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
    };
    let mut depth = 0_usize;
    for (number, line) in unfolded.lines().enumerate() {
        if number >= MAX_LINES {
            return false;
        }
        let line = line.trim_start();
        if starts(line, "BEGIN:") {
            depth += 1;
            if depth > MAX_DEPTH {
                return false;
            }
        } else if starts(line, "END:") {
            depth = depth.saturating_sub(1);
        }
    }
    true
}

/// The first property of `component` named `name`.
fn property<'a>(component: &'a Component<'a>, name: &str) -> Option<&'a Property<'a>> {
    component
        .properties
        .iter()
        .find(|p| p.name.as_str().eq_ignore_ascii_case(name))
}

/// A parameter of `property` (`TZID`, `VALUE`), without quotes.
fn parameter<'a>(property: &'a Property<'a>, name: &str) -> Option<&'a str> {
    property
        .params
        .iter()
        .find(|p| p.key.as_str().eq_ignore_ascii_case(name))
        .and_then(|p| p.val.as_ref())
        .map(|v| v.as_str().trim().trim_matches('"'))
}

/// The text of a `TEXT` property, unescaped.
fn text_of(property: &Property<'_>) -> String {
    property.val.clone().unescape_text().as_str().to_owned()
}

fn candidate(event: &Component<'_>, method: Method) -> Option<Candidate> {
    let cancelled = method == Method::Cancel
        || property(event, "STATUS")
            .is_some_and(|p| p.val.as_str().trim().eq_ignore_ascii_case("CANCELLED"));
    let title = tidy(&text_of(property(event, "SUMMARY")?), MAX_TITLE);
    if title.is_empty() {
        return None;
    }
    let start = property(event, "DTSTART")?;
    let (starts, zone) = moment(start)?;
    let ends = property(event, "DTEND")
        .and_then(moment)
        .map(|(ends, _)| ends)
        .or_else(|| {
            property(event, "DURATION")
                .and_then(|p| duration(p.val.as_str()))
                .and_then(|length| add(starts, length))
        })
        .filter(|ends| ends.instant() > starts.instant());
    let place = property(event, "LOCATION")
        .map(|p| tidy(&text_of(p), MAX_PLACE))
        .filter(|place| !place.is_empty());
    let attendees = event
        .properties
        .iter()
        .filter(|p| p.name.as_str().eq_ignore_ascii_case("ATTENDEE"))
        .count();
    Some(Candidate {
        kind: if attendees > 0 {
            ItemKind::Meeting
        } else {
            ItemKind::Appointment
        },
        title,
        starts,
        ends,
        time_zone: zone,
        place,
        uid: property(event, "UID")
            .map(|p| p.val.as_str().trim().to_owned())
            .filter(|uid| !uid.is_empty()),
        sequence: property(event, "SEQUENCE")
            .and_then(|p| p.val.as_str().trim().parse::<i64>().ok())
            .unwrap_or(0),
        cancelled,
    })
}

/// The moment a `DTSTART` or `DTEND` names, and its IANA zone when it was
/// written in one: a date, a UTC time (`…Z`), or a time with a `TZID`. A
/// floating time, and a `TZID` that is not an IANA name, name nothing.
fn moment(property: &Property<'_>) -> Option<(When, Option<String>)> {
    let value = property.val.as_str().trim();
    let is_date = parameter(property, "VALUE").is_some_and(|v| v.eq_ignore_ascii_case("DATE"))
        || (value.len() == 8 && value.bytes().all(|b| b.is_ascii_digit()));
    if is_date {
        let day = NaiveDate::parse_from_str(value, "%Y%m%d").ok()?;
        return Some((When::Day(day), None));
    }
    if let Some(utc) = value.strip_suffix(['Z', 'z']) {
        let naive = NaiveDateTime::parse_from_str(utc, "%Y%m%dT%H%M%S").ok()?;
        let time: DateTime<FixedOffset> = Utc.from_utc_datetime(&naive).fixed_offset();
        return Some((When::Time(time), None));
    }
    let zone_name = parameter(property, "TZID")?;
    let zone: Tz = zone_name.parse().ok()?;
    let naive = NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S").ok()?;
    // Twice in a fall-back hour: the first. Never in a spring-forward gap.
    let time = zone.from_local_datetime(&naive).earliest()?.fixed_offset();
    Some((When::Time(time), Some(zone_name.to_owned())))
}

/// `start` plus `length`: a date moves by whole days only.
fn add(start: When, length: TimeDelta) -> Option<When> {
    match start {
        When::Day(day) => {
            (length.num_seconds() % 86_400 == 0).then_some(())?;
            day.checked_add_signed(length).map(When::Day)
        }
        When::Time(time) => time.checked_add_signed(length).map(When::Time),
    }
}

/// An RFC 5545 `DURATION` (`P1D`, `PT1H30M`, `P2W`), positive only.
fn duration(value: &str) -> Option<TimeDelta> {
    let rest = value.trim().strip_prefix('P')?;
    let mut total = TimeDelta::zero();
    let mut number = String::new();
    let mut in_time = false;
    for c in rest.chars() {
        match c {
            'T' => in_time = true,
            '0'..='9' => number.push(c),
            unit => {
                let n: i64 = number.parse().ok()?;
                number.clear();
                let part = match (unit, in_time) {
                    ('W', false) => TimeDelta::try_weeks(n)?,
                    ('D', false) => TimeDelta::try_days(n)?,
                    ('H', true) => TimeDelta::try_hours(n)?,
                    ('M', true) => TimeDelta::try_minutes(n)?,
                    ('S', true) => TimeDelta::try_seconds(n)?,
                    _ => return None,
                };
                total = total.checked_add(&part)?;
            }
        }
    }
    (number.is_empty() && total > TimeDelta::zero()).then_some(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    const INVITE: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Example//EN\r\n\
METHOD:REQUEST\r\nBEGIN:VEVENT\r\nUID:site-visit-1@example.com\r\nSEQUENCE:0\r\n\
DTSTART;TZID=Europe/Lisbon:20261112T150000\r\nDTEND;TZID=Europe/Lisbon:20261112T160000\r\n\
SUMMARY:Site visit\\, Rua das Flores\r\nLOCATION:Rua das Flores 12\\, Lisboa\r\n\
ATTENDEE;CN=Ines:mailto:ines@example.com\r\nORGANIZER:mailto:pedro@example.com\r\n\
END:VEVENT\r\nEND:VCALENDAR\r\n";

    #[test]
    fn an_invitation_is_a_meeting_in_its_own_zone() {
        let found = events(INVITE);
        assert_eq!(found.len(), 1);
        let event = &found[0];
        assert_eq!(event.kind, ItemKind::Meeting);
        assert_eq!(event.title, "Site visit, Rua das Flores");
        assert_eq!(event.place.as_deref(), Some("Rua das Flores 12, Lisboa"));
        assert_eq!(event.uid.as_deref(), Some("site-visit-1@example.com"));
        assert_eq!(event.time_zone.as_deref(), Some("Europe/Lisbon"));
        // Lisbon is on UTC in November.
        assert_eq!(event.starts.to_text(), "2026-11-12T15:00:00+00:00");
        assert_eq!(
            event.ends.map(When::to_text).as_deref(),
            Some("2026-11-12T16:00:00+00:00")
        );
        assert!(!event.cancelled);
    }

    #[test]
    fn a_zone_with_an_offset_is_kept_as_written() {
        let summer = INVITE.replace("20261112", "20260712");
        let found = events(&summer);
        assert_eq!(found[0].starts.to_text(), "2026-07-12T15:00:00+01:00");
    }

    #[test]
    fn without_attendees_it_is_an_appointment_and_utc_and_dates_read() {
        let utc = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:a\nDTSTART:20261106T074000Z\n\
DURATION:PT1H30M\nSUMMARY:Dentist\nEND:VEVENT\nEND:VCALENDAR\n";
        let found = events(utc);
        assert_eq!(found[0].kind, ItemKind::Appointment);
        assert_eq!(found[0].starts.to_text(), "2026-11-06T07:40:00+00:00");
        assert_eq!(
            found[0].ends.map(When::to_text).as_deref(),
            Some("2026-11-06T09:10:00+00:00")
        );
        let day = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:b\nDTSTART;VALUE=DATE:20261106\n\
DTEND;VALUE=DATE:20261108\nSUMMARY:Conference\nEND:VEVENT\nEND:VCALENDAR\n";
        let found = events(day);
        assert_eq!(found[0].starts.to_text(), "2026-11-06");
        assert_eq!(
            found[0].ends.map(When::to_text).as_deref(),
            Some("2026-11-08")
        );
        assert!(found[0].starts.is_day());
    }

    #[test]
    fn a_floating_time_and_an_unknown_zone_name_nothing() {
        let floating = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:c\nDTSTART:20261106T074000\n\
SUMMARY:Floating\nEND:VEVENT\nEND:VCALENDAR\n";
        assert!(events(floating).is_empty());
        let windows = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:d\n\
DTSTART;TZID=W. Europe Standard Time:20261106T074000\nSUMMARY:Windows\nEND:VEVENT\n\
END:VCALENDAR\n";
        assert!(events(windows).is_empty());
    }

    #[test]
    fn a_cancellation_and_a_higher_sequence_are_marked() {
        let cancel = INVITE
            .replace("METHOD:REQUEST", "METHOD:CANCEL")
            .replace("SEQUENCE:0", "SEQUENCE:1");
        let found = events(&cancel);
        assert!(found[0].cancelled);
        assert_eq!(found[0].sequence, 1);
        let status = INVITE.replace("SEQUENCE:0", "SEQUENCE:2\r\nSTATUS:CANCELLED");
        assert!(events(&status)[0].cancelled);
    }

    #[test]
    fn a_reply_is_not_an_event_of_the_readers() {
        let reply = INVITE.replace("METHOD:REQUEST", "METHOD:REPLY");
        assert!(events(&reply).is_empty());
    }

    #[test]
    fn folded_lines_are_unfolded() {
        let folded = [
            "BEGIN:VCALENDAR",
            "BEGIN:VEVENT",
            "UID:e",
            "DTSTART:20261106T074000Z",
            "SUMMARY:A very long title that the sender folded onto a second line",
            "  for the parser",
            "END:VEVENT",
            "END:VCALENDAR",
            "",
        ]
        .join("\r\n");
        let found = events(&folded);
        assert_eq!(
            found[0].title,
            "A very long title that the sender folded onto a second line for the parser"
        );
    }

    #[test]
    fn nesting_and_size_are_bounded_before_the_parser_recurses() {
        let deep = format!(
            "BEGIN:VCALENDAR\n{}{}END:VCALENDAR\n",
            "BEGIN:X\n".repeat(50),
            "END:X\n".repeat(50)
        );
        assert!(!shallow(&deep));
        let long = format!(
            "BEGIN:VCALENDAR\n{}END:VCALENDAR\n",
            "X-A:b\n".repeat(5_000)
        );
        assert!(events(&long).is_empty());
        assert!(events("BEGIN:").is_empty());
        assert!(events("not a calendar").is_empty());
    }

    #[test]
    fn durations() {
        assert_eq!(duration("PT15M"), Some(TimeDelta::minutes(15)));
        assert_eq!(duration("P1DT2H"), Some(TimeDelta::hours(26)));
        assert_eq!(duration("P2W"), Some(TimeDelta::weeks(2)));
        for bad in ["-PT15M", "PT", "P", "1H", "PT1X", "P1H", "PT0S"] {
            assert_eq!(duration(bad), None, "{bad}");
        }
    }
}
