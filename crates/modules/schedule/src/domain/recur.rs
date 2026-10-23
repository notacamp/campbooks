//! Recurring series: stored once with the provider's own RRULE, RDATE and
//! EXDATE lines, expanded when events are listed, within the asked window
//! (no stored occurrence rows). The expansion is the `rrule` crate's; this
//! module only builds its input and reads its output.

use std::str::FromStr as _;

use chrono::{DateTime, TimeDelta, Utc};
use chrono_tz::Tz;
use rrule::RRuleSet;

use super::moment::{Moment, Span, parse_zone};

/// Most occurrences one series gives in one window.
pub const MAX_OCCURRENCES: u16 = 1_000;

/// One occurrence of a series.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Occurrence {
    /// When it starts and ends.
    pub span: Span,
    /// Its name: what an exception's `original_start` names.
    pub key: String,
}

/// The occurrences of the series that starts as `first` (written in `zone`)
/// with recurrence `lines`, that overlap `from..to`. A series whose lines the
/// library does not accept is shown as its first occurrence only.
pub fn expand(
    first: Span,
    zone: Option<&str>,
    lines: &[String],
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Vec<Occurrence> {
    let single = || {
        if first.overlaps(from, to) {
            vec![Occurrence {
                span: first,
                key: first.start.key(),
            }]
        } else {
            Vec::new()
        }
    };
    let Some(input) = input_of(first, zone, lines) else {
        return single();
    };
    let Ok(set) = RRuleSet::from_str(&input) else {
        tracing::warn!("a series has recurrence lines the library does not accept");
        return single();
    };
    let length = first.length();
    let utc = rrule::Tz::UTC;
    // An occurrence that started before the window may still overlap it.
    let after = (from - length - TimeDelta::seconds(1)).with_timezone(&utc);
    let before = to.with_timezone(&utc);
    let result = set.after(after).before(before).all(MAX_OCCURRENCES);
    if result.limited {
        tracing::warn!("a series was cut at {MAX_OCCURRENCES} occurrences in one window");
    }
    result
        .dates
        .into_iter()
        .filter_map(|date| {
            let start = date.with_timezone(&Utc);
            let moment = match first.start {
                Moment::At(_) => Moment::At(start),
                Moment::On(_) => Moment::On(start.date_naive()),
            };
            let span = first.shifted_to(moment)?;
            span.overlaps(from, to).then(|| Occurrence {
                key: span.start.key(),
                span,
            })
        })
        .collect()
}

/// The text the library parses: a `DTSTART` in the event's zone, then the
/// series' own lines (the ones that make dates), made explicit about the
/// zone of dates that carry none.
fn input_of(first: Span, zone: Option<&str>, lines: &[String]) -> Option<String> {
    let (start_line, zone_name) = match first.start {
        Moment::At(at) => {
            let tz = zone.and_then(parse_zone).unwrap_or(Tz::UTC);
            let local = at.with_timezone(&tz).format("%Y%m%dT%H%M%S");
            (
                format!("DTSTART;TZID={}:{local}", tz.name()),
                tz.name().to_owned(),
            )
        }
        Moment::On(day) => (
            format!("DTSTART;TZID=UTC:{}T000000", day.format("%Y%m%d")),
            "UTC".to_owned(),
        ),
    };
    let all_day = first.start.is_day();
    let mut text = start_line;
    let mut makes_dates = false;
    for line in lines {
        let line = line.trim();
        let name = line
            .split([';', ':'])
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        match name.as_str() {
            "RRULE" => {
                makes_dates = true;
                text.push('\n');
                text.push_str(line);
            }
            "RDATE" | "EXDATE" => {
                if name == "RDATE" {
                    makes_dates = true;
                }
                let (_, values) = line.split_once(':')?;
                let zoned = values.trim_end().ends_with('Z');
                text.push('\n');
                text.push_str(&name);
                if !zoned {
                    text.push_str(";TZID=");
                    text.push_str(if all_day { "UTC" } else { &zone_name });
                }
                text.push(':');
                text.push_str(values);
            }
            _ => {}
        }
    }
    makes_dates.then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(start: &str, end: &str) -> Span {
        Span::new(Moment::parse(start).unwrap(), Moment::parse(end)).unwrap()
    }

    fn utc(text: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(text)
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn a_weekly_series_has_four_mondays_in_four_weeks() {
        let first = span("2026-11-02T10:00:00+00:00", "2026-11-02T10:30:00+00:00");
        let lines = vec!["RRULE:FREQ=WEEKLY;BYDAY=MO".to_owned()];
        let found = expand(
            first,
            Some("Europe/Lisbon"),
            &lines,
            utc("2026-11-02T00:00:00Z"),
            utc("2026-11-30T00:00:00Z"),
        );
        assert_eq!(found.len(), 4);
        assert_eq!(found[1].key, "2026-11-09T10:00:00Z");
        assert_eq!(found[1].span.length(), TimeDelta::minutes(30));
    }

    #[test]
    fn an_excluded_date_is_not_an_occurrence() {
        let first = span("2026-11-02T10:00:00+00:00", "2026-11-02T10:30:00+00:00");
        let lines = vec![
            "RRULE:FREQ=WEEKLY;BYDAY=MO".to_owned(),
            "EXDATE;TZID=Europe/Lisbon:20261109T100000".to_owned(),
        ];
        let found = expand(
            first,
            Some("Europe/Lisbon"),
            &lines,
            utc("2026-11-02T00:00:00Z"),
            utc("2026-11-30T00:00:00Z"),
        );
        let keys: Vec<&str> = found.iter().map(|o| o.key.as_str()).collect();
        assert_eq!(keys.len(), 3);
        assert!(!keys.contains(&"2026-11-09T10:00:00Z"));
    }

    #[test]
    fn an_all_day_series_stays_on_its_dates() {
        let first = span("2026-11-12", "2026-11-13");
        let lines = vec!["RRULE:FREQ=DAILY;COUNT=3".to_owned()];
        let found = expand(
            first,
            None,
            &lines,
            utc("2026-11-01T00:00:00Z"),
            utc("2026-12-01T00:00:00Z"),
        );
        let keys: Vec<&str> = found.iter().map(|o| o.key.as_str()).collect();
        assert_eq!(keys, ["2026-11-12", "2026-11-13", "2026-11-14"]);
    }

    #[test]
    fn lines_that_make_no_dates_leave_the_first_occurrence() {
        let first = span("2026-11-02T10:00:00+00:00", "2026-11-02T10:30:00+00:00");
        let found = expand(
            first,
            None,
            &["X-NOTE:none".to_owned()],
            utc("2026-11-01T00:00:00Z"),
            utc("2026-12-01T00:00:00Z"),
        );
        assert_eq!(found.len(), 1);
        let outside = expand(
            first,
            None,
            &[],
            utc("2027-01-01T00:00:00Z"),
            utc("2027-02-01T00:00:00Z"),
        );
        assert!(outside.is_empty());
    }
}
