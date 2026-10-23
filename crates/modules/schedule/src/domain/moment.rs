//! Times: an event is timed (instants) or all-day (dates, the end exclusive
//! as the providers write it), and is shown in the member's time zone.

use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use chrono_tz::Tz;

use crate::inbound::ProviderWhen;

/// The IANA zone named `name`.
pub fn parse_zone(name: &str) -> Option<Tz> {
    name.parse::<Tz>().ok()
}

/// The zone named `name`, or UTC when there is none (or it is unknown).
pub fn zone_or_utc(name: Option<&str>) -> Tz {
    name.and_then(parse_zone).unwrap_or(Tz::UTC)
}

/// A point in an event's span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Moment {
    /// An instant.
    At(DateTime<Utc>),
    /// A whole day.
    On(NaiveDate),
}

impl Moment {
    /// An RFC 3339 time with an offset, or a `YYYY-MM-DD` date.
    pub fn parse(text: &str) -> Option<Self> {
        if let Ok(at) = DateTime::parse_from_rfc3339(text) {
            return Some(Self::At(at.with_timezone(&Utc)));
        }
        NaiveDate::parse_from_str(text, "%Y-%m-%d")
            .ok()
            .map(Self::On)
    }

    /// Whether it is a day.
    pub const fn is_day(self) -> bool {
        matches!(self, Self::On(_))
    }

    /// It as the API shows it: RFC 3339 with `zone`'s offset, or the date.
    pub fn render(self, zone: Tz) -> String {
        match self {
            Self::At(at) => at.with_timezone(&zone).to_rfc3339(),
            Self::On(day) => day.format("%Y-%m-%d").to_string(),
        }
    }

    /// It as the events carry it: UTC with an offset, or the date.
    pub fn render_utc(self) -> String {
        self.render(Tz::UTC)
    }

    /// The first instant of it (a day starts at midnight UTC).
    pub fn instant(self) -> DateTime<Utc> {
        match self {
            Self::At(at) => at,
            Self::On(day) => day.and_time(chrono::NaiveTime::MIN).and_utc(),
        }
    }

    /// It as the name of an occurrence: UTC to the second, or the date. The
    /// provider's `original_start` is normalised to the same form.
    pub fn key(self) -> String {
        match self {
            Self::At(at) => at.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            Self::On(day) => day.format("%Y-%m-%d").to_string(),
        }
    }

    /// It as a provider's start or end, written in `zone` (a day has none).
    pub fn to_provider(self, zone: Option<&str>) -> ProviderWhen {
        match self {
            Self::On(day) => ProviderWhen::Date {
                date: day.format("%Y-%m-%d").to_string(),
            },
            Self::At(at) => {
                let tz = zone_or_utc(zone);
                ProviderWhen::DateTime {
                    date_time: at.with_timezone(&tz).to_rfc3339(),
                    time_zone: zone
                        .filter(|name| parse_zone(name).is_some())
                        .map(str::to_owned),
                }
            }
        }
    }
}

impl ProviderWhen {
    /// The moment it names, and the zone it was written in.
    pub fn moment(&self) -> Option<(Moment, Option<String>)> {
        match self {
            Self::Date { date } => NaiveDate::parse_from_str(date, "%Y-%m-%d")
                .ok()
                .map(|day| (Moment::On(day), None)),
            Self::DateTime {
                date_time,
                time_zone,
            } => DateTime::parse_from_rfc3339(date_time).ok().map(|at| {
                (
                    Moment::At(at.with_timezone(&Utc)),
                    time_zone.clone().filter(|name| parse_zone(name).is_some()),
                )
            }),
        }
    }
}

/// The four columns that hold a span: `starts_at`, `ends_at`, `starts_on`,
/// `ends_on`.
pub type Columns = (
    Option<DateTime<Utc>>,
    Option<DateTime<Utc>>,
    Option<NaiveDate>,
    Option<NaiveDate>,
);

/// A start and an end of the same form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// When it starts.
    pub start: Moment,
    /// When it ends (exclusive for a day).
    pub end: Moment,
}

/// The default length of a timed event with no end.
pub const DEFAULT_LENGTH: TimeDelta = TimeDelta::hours(1);

impl Span {
    /// A span from `start` to `end`, when both are of the same form and the
    /// end comes after the start. With no end: an hour, or a day.
    pub fn new(start: Moment, end: Option<Moment>) -> Option<Self> {
        let end = match (start, end) {
            (Moment::At(at), None) => Moment::At(at + DEFAULT_LENGTH),
            (Moment::On(day), None) => Moment::On(day.succ_opt()?),
            (start, Some(end)) => {
                if start.is_day() != end.is_day() {
                    return None;
                }
                end
            }
        };
        (end > start).then_some(Self { start, end })
    }

    /// Its length.
    pub fn length(self) -> TimeDelta {
        match (self.start, self.end) {
            (Moment::At(a), Moment::At(b)) => b - a,
            (Moment::On(a), Moment::On(b)) => b - a,
            _ => TimeDelta::zero(),
        }
    }

    /// The same length, starting at `start` (of the same form).
    pub fn shifted_to(self, start: Moment) -> Option<Self> {
        let length = self.length();
        let end = match start {
            Moment::At(at) => Moment::At(at + length),
            Moment::On(day) => Moment::On(day.checked_add_signed(length)?),
        };
        Some(Self { start, end })
    }

    /// The four columns that hold it: `starts_at`, `ends_at`, `starts_on`,
    /// `ends_on`.
    pub fn columns(self) -> Columns {
        match (self.start, self.end) {
            (Moment::At(a), Moment::At(b)) => (Some(a), Some(b), None, None),
            (Moment::On(a), Moment::On(b)) => (None, None, Some(a), Some(b)),
            _ => (None, None, None, None),
        }
    }

    /// Whether it overlaps `from..to`.
    pub fn overlaps(self, from: DateTime<Utc>, to: DateTime<Utc>) -> bool {
        self.start.instant() < to && self.end.instant() > from
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_moment_is_a_time_with_an_offset_or_a_date() {
        let at = Moment::parse("2026-11-06T07:40:00+01:00").unwrap();
        assert_eq!(at.render_utc(), "2026-11-06T06:40:00+00:00");
        assert_eq!(
            at.render(parse_zone("Europe/Lisbon").unwrap()),
            "2026-11-06T06:40:00+00:00"
        );
        assert_eq!(
            at.render(parse_zone("Europe/Madrid").unwrap()),
            "2026-11-06T07:40:00+01:00"
        );
        let day = Moment::parse("2026-11-06").unwrap();
        assert!(day.is_day());
        assert_eq!(day.render_utc(), "2026-11-06");
        assert!(Moment::parse("tomorrow").is_none());
        // A time with no offset is not a time.
        assert!(Moment::parse("2026-11-06T07:40:00").is_none());
    }

    #[test]
    fn a_span_defaults_its_end_and_refuses_mixed_forms() {
        let start = Moment::parse("2026-11-06T07:40:00Z").unwrap();
        let span = Span::new(start, None).unwrap();
        assert_eq!(span.length(), TimeDelta::hours(1));
        let day = Moment::parse("2026-11-06").unwrap();
        assert_eq!(Span::new(day, None).unwrap().length(), TimeDelta::days(1));
        assert!(Span::new(start, Some(day)).is_none());
        // The end comes after the start.
        assert!(Span::new(start, Some(start)).is_none());
    }

    #[test]
    fn an_occurrence_is_named_in_utc_or_by_its_date() {
        let at = Moment::parse("2026-11-16T11:00:00+01:00").unwrap();
        assert_eq!(at.key(), "2026-11-16T10:00:00Z");
        assert_eq!(Moment::parse("2026-11-16").unwrap().key(), "2026-11-16");
    }
}
