//! What a read of a message holds (campbooks-cloud#42): the **items** (things
//! that happen at a time) and **asks** (obligations of the reader, dated or
//! not) found in it, whether by the sender's own machine-readable data
//! ([`super::structured`], ADR 0020) or by Scout's one `complete` call
//! (`conversations.read_message`).
//!
//! These are the shapes kept in `conversations.message_reads` and shown by
//! `conversations.get_message_read`. They carry the text of the read (titles,
//! places, quotes, amounts); the `conversations.message_read` event carries
//! none of it.

use chrono::{DateTime, FixedOffset, NaiveDate, SecondsFormat, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::model::{ItemKind, MessageId, ReadLayer, Verb};

/// Most items a read keeps.
pub const MAX_ITEMS: usize = 5;
/// Most asks a read keeps.
pub const MAX_ASKS: usize = 3;
/// Longest title kept, in characters.
pub const MAX_TITLE: usize = 80;
/// Longest place kept, in characters.
pub const MAX_PLACE: usize = 120;
/// Longest quote kept, in characters.
pub const MAX_QUOTE: usize = 400;
/// How long before a message was sent something it tells of may lie, in days.
pub const DAYS_BEFORE: i64 = 1;
/// How long after a message was sent something it tells of may lie, in days.
pub const DAYS_AHEAD: i64 = 731;

/// When something starts, ends or is due: a date, or a time with an offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum When {
    /// A whole day (no hour is ever guessed).
    Day(NaiveDate),
    /// An instant, with the offset it was written in.
    Time(DateTime<FixedOffset>),
}

impl When {
    /// `text` as an RFC 3339 time with an offset, or a `YYYY-MM-DD` date.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        if let Ok(time) = DateTime::parse_from_rfc3339(text) {
            return Some(Self::Time(time));
        }
        NaiveDate::parse_from_str(text, "%Y-%m-%d")
            .ok()
            .map(Self::Day)
    }

    /// As kept and shown: RFC 3339 with its offset, or the date.
    pub fn to_text(self) -> String {
        match self {
            Self::Day(day) => day.format("%Y-%m-%d").to_string(),
            Self::Time(time) => time.to_rfc3339_opts(SecondsFormat::Secs, false),
        }
    }

    /// The instant it names, a date taken as its midnight in UTC: for
    /// comparing with other times, never for showing.
    pub fn instant(self) -> DateTime<Utc> {
        match self {
            Self::Day(day) => day.and_time(chrono::NaiveTime::MIN).and_utc(),
            Self::Time(time) => time.with_timezone(&Utc),
        }
    }

    /// The date it falls on, in the offset it was written in.
    pub fn date(self) -> NaiveDate {
        match self {
            Self::Day(day) => day,
            Self::Time(time) => time.date_naive(),
        }
    }

    /// Whether it is a date.
    pub const fn is_day(self) -> bool {
        matches!(self, Self::Day(_))
    }

    /// Whether it lies in `[sent_at - 1 day, sent_at + 731 days]`: a date by
    /// its day, a time by its instant.
    pub fn in_window(self, sent_at: DateTime<Utc>) -> bool {
        match self {
            Self::Day(day) => {
                let sent = sent_at.date_naive();
                day >= sent - TimeDelta::days(DAYS_BEFORE)
                    && day <= sent + TimeDelta::days(DAYS_AHEAD)
            }
            Self::Time(_) => {
                let at = self.instant();
                at >= sent_at - TimeDelta::days(DAYS_BEFORE)
                    && at <= sent_at + TimeDelta::days(DAYS_AHEAD)
            }
        }
    }

    /// When it is over, if it starts at `self` and ends at `ends`: the end,
    /// else a day for a date (the whole day) or the start itself for a time.
    pub fn over(self, ends: Option<Self>) -> DateTime<Utc> {
        match (ends, self) {
            (Some(ends), _) => ends.instant(),
            (None, Self::Day(day)) => (day + TimeDelta::days(1))
                .and_time(chrono::NaiveTime::MIN)
                .and_utc(),
            (None, time @ Self::Time(_)) => time.instant(),
        }
    }
}

/// An amount of money, in minor units.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Amount {
    /// Minor units (cents).
    pub minor: i64,
    /// The ISO 4217 currency code.
    pub currency: String,
}

/// Something of the reader's that happens at a time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ReadItem {
    /// Stable for the same message, layer and position
    /// (`<message>:<layer>:item:<n>`).
    pub key: String,
    /// What kind of thing it is.
    pub kind: ItemKind,
    /// What it is, at most 80 characters; never the subject.
    pub title: String,
    /// When it starts: an RFC 3339 time with an offset, or a date when
    /// all-day.
    pub starts: String,
    /// When it ends, if the message says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ends: Option<String>,
    /// Whether it lasts whole days.
    pub all_day: bool,
    /// The IANA time zone it is in, when the message says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_zone: Option<String>,
    /// Where, at most 120 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    /// The sentence that gives its date, as written (a model read only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote: Option<String>,
    /// The `UID` of the iCalendar event it came from, if it came from one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uid: Option<String>,
    /// That event's `SEQUENCE`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sequence: Option<i64>,
    /// The key of an earlier item of the conversation that this one changes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updates: Option<String>,
    /// The key of an earlier item of the conversation that this one cancels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cancels: Option<String>,
}

impl ReadItem {
    /// When it starts, if it parses.
    pub fn when(&self) -> Option<When> {
        When::parse(&self.starts)
    }
}

/// An obligation of the reader, dated or not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ReadAsk {
    /// Stable for the same message, layer and position
    /// (`<message>:<layer>:ask:<n>`).
    pub key: String,
    /// What to do, at most 80 characters.
    pub title: String,
    /// The sentence that asks, as written (a model read only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote: Option<String>,
    /// By when: a date, or an RFC 3339 time with an offset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<String>,
    /// Do something, or pay something.
    pub verb: Verb,
    /// How much, for a payment, when the message says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<Amount>,
    /// The reference of an open task this ask repeats.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub same_as: Option<String>,
}

impl ReadAsk {
    /// The day it is due, in the offset it was written in.
    pub fn due_on(&self) -> Option<NaiveDate> {
        self.due.as_deref().and_then(When::parse).map(When::date)
    }
}

/// The key of item or ask number `position` (from 1) of `message`'s `layer`.
pub fn key(message: MessageId, layer: ReadLayer, ask: bool, position: usize) -> String {
    format!(
        "{message}:{}:{}:{position}",
        layer.as_str(),
        if ask { "ask" } else { "item" }
    )
}

/// `text` on one line, whitespace collapsed, at most `max` characters.
pub fn tidy(text: &str, max: usize) -> String {
    let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let shown = campbooks_kernel::text::displayable(&one_line);
    shown
        .chars()
        .take(max)
        .collect::<String>()
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whens_round_trip_and_keep_their_offset() {
        let time = When::parse("2026-11-06T07:40:00+01:00").unwrap();
        assert_eq!(time.to_text(), "2026-11-06T07:40:00+01:00");
        assert_eq!(time.instant().to_rfc3339(), "2026-11-06T06:40:00+00:00");
        assert_eq!(When::parse("2026-11-06").unwrap().to_text(), "2026-11-06");
        for bad in ["soon", "2026-11-06T07:40:00", "06/11/2026", ""] {
            assert_eq!(When::parse(bad), None, "{bad:?}");
        }
        assert_eq!(
            When::parse("2026-11-06T23:30:00-03:00").unwrap().date(),
            NaiveDate::from_ymd_opt(2026, 11, 6).unwrap()
        );
    }

    #[test]
    fn the_window_runs_from_a_day_before_the_message_to_two_years_after() {
        let sent: DateTime<Utc> = "2026-10-12T09:00:00Z".parse().unwrap();
        let at = |text: &str| When::parse(text).unwrap().in_window(sent);
        assert!(at("2026-10-11") && at("2026-10-12") && at("2028-10-12"));
        assert!(!at("2026-10-10") && !at("2028-10-13"));
        assert!(at("2026-10-11T09:00:00+00:00") && !at("2026-10-11T08:59:59+00:00"));
        assert!(at("2028-10-13T09:00:00+00:00") && !at("2028-10-13T09:00:01+00:00"));
    }

    #[test]
    fn a_date_with_no_end_lasts_the_day_and_a_time_with_none_ends_when_it_starts() {
        let day = When::parse("2026-11-06").unwrap();
        assert_eq!(day.over(None).to_rfc3339(), "2026-11-07T00:00:00+00:00");
        let time = When::parse("2026-11-06T07:40:00+00:00").unwrap();
        assert_eq!(time.over(None), time.instant());
        let end = When::parse("2026-11-06T11:55:00+00:00");
        assert_eq!(time.over(end).to_rfc3339(), "2026-11-06T11:55:00+00:00");
    }

    #[test]
    fn keys_name_the_message_the_layer_and_the_position() {
        let message = MessageId::new();
        assert_eq!(
            key(message, ReadLayer::Model, false, 2),
            format!("{message}:model:item:2")
        );
        assert_eq!(
            key(message, ReadLayer::Structured, true, 1),
            format!("{message}:structured:ask:1")
        );
    }

    #[test]
    fn text_is_one_clean_line() {
        assert_eq!(
            tidy("  Flight\nLIS \u{202E}to AMS  ", 80),
            "Flight LIS to AMS"
        );
        assert_eq!(tidy("abcdef", 3), "abc");
    }
}
