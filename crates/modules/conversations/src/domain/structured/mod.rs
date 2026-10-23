//! Dates a sender structured for machines (ADR 0020, campbooks-cloud#42): an
//! iCalendar part (`text/calendar`) and schema.org markup (JSON-LD or
//! microdata) in a message's HTML.
//!
//! This is **not** reading content in the sense of ADR 0018. The sender wrote
//! typed fields for a parser, and a proper parser of each format reads them:
//! the `icalendar` crate for [`ical`], `serde_json` for JSON-LD and an HTML
//! parser (`scraper`) for microdata in [`schema_org`]. Only a closed list of
//! types and only their typed fields are read; the prose around the markup,
//! the subject, the text part and the attachments' names are never given to
//! this module and never read by it, and no pattern looks for a date or an
//! amount anywhere. A test keeps it so ([`tests::the_reader_never_reads_prose`]).
//!
//! The caller decides whether to run it at all: only for a sender whose
//! domain passed DMARC aligned (as `campbooks_ai::decide::signals::facts`
//! reads it) and for mail the decide pass did not call suspicious, because
//! markup is trivially forged.

mod ical;
mod schema_org;

use chrono::{DateTime, Utc};

use crate::{
    domain::read::{Amount, MAX_ASKS, MAX_ITEMS, ReadAsk, ReadItem, When, key},
    model::{ItemKind, MessageId, ReadLayer, Verb},
};

/// Most bytes of a calendar part read.
pub const MAX_CALENDAR_BYTES: usize = 128 * 1024;
/// Most bytes of the HTML part read for markup.
pub const MAX_HTML_BYTES: usize = 256 * 1024;
/// Most calendar parts read from one message.
pub const MAX_CALENDARS: usize = 3;

/// A dated thing the data names, before it has a key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// What kind of thing.
    pub kind: ItemKind,
    /// What it is.
    pub title: String,
    /// When it starts.
    pub starts: When,
    /// When it ends, if it says.
    pub ends: Option<When>,
    /// The IANA zone it was written in, when it was.
    pub time_zone: Option<String>,
    /// Where.
    pub place: Option<String>,
    /// The iCalendar `UID`.
    pub uid: Option<String>,
    /// The iCalendar `SEQUENCE`.
    pub sequence: i64,
    /// A cancellation of the event with this `UID`.
    pub cancelled: bool,
}

/// A payment the data says is due.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Due {
    /// Whom, or what.
    pub title: String,
    /// By when.
    pub due: When,
    /// How much, when it says.
    pub amount: Option<Amount>,
}

/// What the data of one message names.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Found {
    /// Dated things, in order.
    pub items: Vec<Candidate>,
    /// Payments due, in order.
    pub asks: Vec<Due>,
}

/// An item of the conversation already read, as far as the structured data
/// can tell it again: by its `UID`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Known {
    /// Its key.
    pub key: String,
    /// What kind it is.
    pub kind: ItemKind,
    /// Its title.
    pub title: String,
    /// When it starts, as kept.
    pub starts: String,
    /// The iCalendar `UID`, when it came from an event.
    pub uid: Option<String>,
    /// That event's `SEQUENCE`.
    pub sequence: Option<i64>,
    /// A later item cancelled it.
    pub cancelled: bool,
}

/// What `calendars` (the `text/calendar` parts, as text) and `html` (the
/// message's HTML part) name. Parsing is CPU work: run it off the async
/// workers.
pub fn read(calendars: &[String], html: Option<&str>) -> Found {
    let mut items: Vec<Candidate> = calendars
        .iter()
        .take(MAX_CALENDARS)
        .flat_map(|part| ical::events(part))
        .collect();
    let mut asks = Vec::new();
    // A document the sanitiser would refuse to parse (too much work, too
    // deep) is not parsed here either.
    if let Some(html) = html
        && crate::domain::sanitize::styles::measure(html).is_some()
    {
        let (found, due) = schema_org::interpret(&schema_org::values(html));
        items.extend(found);
        asks.extend(due);
    }
    Found { items, asks }
}

/// `found` as the items and asks of `message`'s structured read, against the
/// `known` items of its conversation: an event that cancels a known one
/// (same `UID`) becomes an item that `cancels` it; one with a higher
/// `SEQUENCE` an item that `updates` it; one that repeats a known item (same
/// `UID` and no newer, or the same kind, title and start) is dropped, and so
/// is a cancellation of nothing known. A thing that is not a cancellation
/// and lies outside the window around the message (a day before it was sent
/// to two years after), or had ended by `now`, is dropped, as is a payment
/// due outside the window. At most five items and three asks.
pub fn resolve(
    found: Found,
    known: &[Known],
    message: MessageId,
    sent_at: DateTime<Utc>,
    now: DateTime<Utc>,
) -> (Vec<ReadItem>, Vec<ReadAsk>) {
    let mut items: Vec<ReadItem> = Vec::new();
    for candidate in found.items {
        if items.len() >= MAX_ITEMS {
            break;
        }
        // What cannot be a thing to put on a calendar, whoever says so.
        if !candidate.cancelled
            && (!candidate.starts.in_window(sent_at) || candidate.starts.over(candidate.ends) < now)
        {
            continue;
        }
        let same_event = candidate.uid.as_deref().and_then(|uid| {
            known
                .iter()
                .rfind(|k| !k.cancelled && k.uid.as_deref() == Some(uid))
        });
        let (updates, cancels) = match (same_event, candidate.cancelled) {
            (Some(earlier), true) => (None, Some(earlier.key.clone())),
            // A cancellation of something this conversation never held.
            (None, true) => continue,
            (Some(earlier), false) if candidate.sequence > earlier.sequence.unwrap_or(0) => {
                (Some(earlier.key.clone()), None)
            }
            // The same invitation again, or an older version of it.
            (Some(_), false) => continue,
            (None, false) => (None, None),
        };
        let starts = candidate.starts.to_text();
        let repeats = |kind: ItemKind, title: &str, at: &str| {
            known
                .iter()
                .any(|k| !k.cancelled && k.kind == kind && k.title == title && k.starts == at)
                || items
                    .iter()
                    .any(|i| i.kind == kind && i.title == title && i.starts == at)
        };
        if updates.is_none()
            && cancels.is_none()
            && repeats(candidate.kind, &candidate.title, &starts)
        {
            continue;
        }
        items.push(ReadItem {
            key: key(message, ReadLayer::Structured, false, items.len() + 1),
            kind: candidate.kind,
            title: candidate.title,
            starts,
            ends: candidate.ends.map(When::to_text),
            all_day: candidate.starts.is_day(),
            time_zone: candidate.time_zone,
            place: candidate.place,
            quote: None,
            uid: candidate.uid,
            sequence: Some(candidate.sequence),
            updates,
            cancels,
        });
    }
    let mut asks: Vec<ReadAsk> = Vec::new();
    for due in found.asks {
        if asks.len() >= MAX_ASKS {
            break;
        }
        if !due.due.in_window(sent_at) {
            continue;
        }
        let due_text = due.due.to_text();
        if asks
            .iter()
            .any(|a| a.title == due.title && a.due.as_deref() == Some(due_text.as_str()))
        {
            continue;
        }
        asks.push(ReadAsk {
            key: key(message, ReadLayer::Structured, true, asks.len() + 1),
            title: due.title,
            quote: None,
            due: Some(due_text),
            verb: Verb::Pay,
            amount: due.amount,
            same_as: None,
        });
    }
    (items, asks)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The test's message was sent in the morning of 12 October 2026 and is
    /// read five minutes later.
    fn resolve_now(
        found: Found,
        known: &[Known],
        message: MessageId,
    ) -> (Vec<ReadItem>, Vec<ReadAsk>) {
        resolve(
            found,
            known,
            message,
            "2026-10-12T09:00:00Z".parse().unwrap(),
            "2026-10-12T09:05:00Z".parse().unwrap(),
        )
    }

    fn candidate(uid: Option<&str>, sequence: i64, cancelled: bool, start: &str) -> Candidate {
        Candidate {
            kind: ItemKind::Meeting,
            title: "Site visit".into(),
            starts: When::parse(start).unwrap(),
            ends: None,
            time_zone: None,
            place: None,
            uid: uid.map(str::to_owned),
            sequence,
            cancelled,
        }
    }

    fn known(key: &str, uid: &str, sequence: i64) -> Known {
        Known {
            key: key.into(),
            kind: ItemKind::Meeting,
            title: "Site visit".into(),
            starts: "2026-11-12T15:00:00+00:00".into(),
            uid: Some(uid.into()),
            sequence: Some(sequence),
            cancelled: false,
        }
    }

    fn found(items: Vec<Candidate>) -> Found {
        Found {
            items,
            asks: Vec::new(),
        }
    }

    #[test]
    fn a_new_event_gets_a_structured_key() {
        let message = MessageId::new();
        let (items, asks) = resolve_now(
            found(vec![candidate(
                Some("a"),
                0,
                false,
                "2026-11-12T15:00:00+00:00",
            )]),
            &[],
            message,
            SENT,
            NOW,
        );
        assert!(asks.is_empty());
        assert_eq!(items[0].key, format!("{message}:structured:item:1"));
        assert_eq!(items[0].uid.as_deref(), Some("a"));
        assert!(items[0].updates.is_none() && items[0].cancels.is_none());
    }

    #[test]
    fn a_cancellation_names_the_item_it_cancels() {
        let earlier = known("k1", "a", 0);
        let (items, _) = resolve_now(
            found(vec![candidate(
                Some("a"),
                1,
                true,
                "2026-11-12T15:00:00+00:00",
            )]),
            std::slice::from_ref(&earlier),
            MessageId::new(),
        );
        assert_eq!(items[0].cancels.as_deref(), Some("k1"));
        // A cancellation of something never held is nothing.
        let (items, _) = resolve_now(
            found(vec![candidate(
                Some("zzz"),
                1,
                true,
                "2026-11-12T15:00:00+00:00",
            )]),
            &[earlier],
            MessageId::new(),
        );
        assert!(items.is_empty());
    }

    #[test]
    fn a_higher_sequence_updates_and_the_same_one_is_dropped() {
        let earlier = known("k1", "a", 1);
        let moved = candidate(Some("a"), 2, false, "2026-11-12T16:00:00+00:00");
        let (items, _) = resolve_now(
            found(vec![moved]),
            std::slice::from_ref(&earlier),
            MessageId::new(),
        );
        assert_eq!(items[0].updates.as_deref(), Some("k1"));
        assert_eq!(items[0].starts, "2026-11-12T16:00:00+00:00");
        for same_or_older in [1, 0] {
            let again = candidate(Some("a"), same_or_older, false, "2026-11-12T15:00:00+00:00");
            let (items, _) = resolve_now(
                found(vec![again]),
                std::slice::from_ref(&earlier),
                MessageId::new(),
            );
            assert!(items.is_empty(), "sequence {same_or_older}");
        }
    }

    #[test]
    fn a_thing_read_twice_in_one_message_is_one_item() {
        let one = candidate(None, 0, false, "2026-11-12T15:00:00+00:00");
        let (items, _) = resolve_now(found(vec![one.clone(), one]), &[], MessageId::new());
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn what_cannot_go_on_a_calendar_is_dropped() {
        let sent_later = |start: &str| candidate(None, 0, false, start);
        for start in [
            // Over before it was read.
            "2026-10-12T07:40:00+00:00",
            // Years away, or long before the message.
            "2029-11-12",
            "2026-09-01",
        ] {
            let (items, _) = resolve_now(found(vec![sent_later(start)]), &[], MessageId::new());
            assert!(items.is_empty(), "{start}");
        }
        // A payment due in the past of the message is not one to make.
        let (_, asks) = resolve_now(
            Found {
                items: Vec::new(),
                asks: vec![Due {
                    title: "Old bill".into(),
                    due: When::parse("2026-01-31").unwrap(),
                    amount: None,
                }],
            },
            &[],
            MessageId::new(),
        );
        assert!(asks.is_empty());
        // A cancellation applies whenever the cancelled thing was.
        let earlier = known("k1", "a", 0);
        let (items, _) = resolve_now(
            found(vec![candidate(
                Some("a"),
                1,
                true,
                "2026-10-12T07:40:00+00:00",
            )]),
            &[earlier],
            MessageId::new(),
        );
        assert_eq!(items[0].cancels.as_deref(), Some("k1"));
    }

    #[test]
    fn at_most_five_items_and_three_asks() {
        let many: Vec<Candidate> = (0..8)
            .map(|n| candidate(None, 0, false, &format!("2026-11-{:02}", 10 + n)))
            .collect();
        let dues: Vec<Due> = (0..5)
            .map(|n| Due {
                title: format!("Bill {n}"),
                due: When::parse("2026-11-30").unwrap(),
                amount: None,
            })
            .collect();
        let (items, asks) = resolve_now(
            Found {
                items: many,
                asks: dues,
            },
            &[],
            MessageId::new(),
        );
        assert_eq!((items.len(), asks.len()), (5, 3));
        assert!(asks.iter().all(|a| a.verb == Verb::Pay));
        assert!(items.iter().all(|i| i.all_day));
    }

    #[test]
    fn the_reader_gets_parts_and_html_only() {
        // A calendar part and markup, nothing else: the signature is the
        // contract. Here both are present and both are read.
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:u\nDTSTART:20261106T074000Z\n\
SUMMARY:Dentist\nEND:VEVENT\nEND:VCALENDAR\n"
            .to_owned();
        let html = r#"<script type="application/ld+json">{"@type": "Invoice",
            "provider": {"name": "Water Co"}, "paymentDueDate": "2026-11-30"}</script>"#;
        let found = read(&[ics], Some(html));
        assert_eq!(found.items.len(), 1);
        assert_eq!(found.asks.len(), 1);
        assert_eq!(read(&[], None), Found::default());
    }

    /// No rule here may read what an email says (ADR 0018, ADR 0020): outside
    /// the tests, these files never name a subject, a body, the text part,
    /// plain-text conversion or a pattern. The only text taken from a tree is
    /// a JSON-LD block's JSON and a microdata property's value.
    #[test]
    fn the_reader_never_reads_prose() {
        let sources = [
            ("mod.rs", include_str!("mod.rs")),
            ("ical.rs", include_str!("ical.rs")),
            ("schema_org.rs", include_str!("schema_org.rs")),
        ];
        let forbidden = [
            "subject",
            "body",
            "bodies",
            "own_words",
            "plain_text",
            "Quotes",
            "snippet",
            "regex",
            "Regex",
            "Envelope",
            "decision_state",
            "embedding_text",
        ];
        for (file, source) in sources {
            let code = source
                .split("#[cfg(test)]")
                .next()
                .expect("the file has code");
            for (number, line) in code.lines().enumerate() {
                let line = line.split("//").next().unwrap_or_default();
                for word in forbidden {
                    assert!(
                        !line.contains(word),
                        "{file} line {}: names `{word}`, which is prose or content \
                         (ADR 0018, ADR 0020): leave understanding it to the model",
                        number + 1
                    );
                }
            }
        }
        // The one place text is taken from the parsed HTML.
        let schema = sources[2].1.split("#[cfg(test)]").next().unwrap();
        let takes: Vec<&str> = schema.lines().filter(|l| l.contains(".text()")).collect();
        assert_eq!(takes.len(), 1, "{takes:?}");
        let functions: Vec<&str> = schema.split("\nfn ").collect();
        let callers: Vec<&str> = functions
            .iter()
            .filter(|f| f.contains("text_of("))
            .filter_map(|f| f.split(['(', '<']).next())
            .collect();
        assert_eq!(callers, ["text_of", "json_ld", "microdata_value"]);
    }
}
