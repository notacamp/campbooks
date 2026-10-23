//! The Google Calendar adapter: the account's calendars, a calendar read
//! (the first time, then by sync token, series once and exceptions as their
//! own events), and the changes members ask for.
//!
//! Google's text is normalised into the contract's documents; nothing here
//! reads what an event says to decide anything (ADR 0018): only its
//! structured fields.

use chrono::{DateTime, NaiveDate, SecondsFormat, TimeDelta, Utc};
use serde_json::{Value, json};
use uuid::Uuid;

use super::{
    BatchEvent, ChangeDocument, ChangeEvent, FIRST_READ_DAYS, ListedCalendar, PersonDocument,
    Private, Read, When, WriteFailure, Written, clip,
};
use crate::google::{Calendar, CalendarError, EventTime, EventsQuery, GoogleEvent, Person};
use crate::provider::ProviderError;

/// Events per page asked of `events.list`.
const PAGE_SIZE: u32 = 250;

/// Pages one read walks at most: 25,000 events. A calendar larger than that
/// is not read (its stream reports trouble) rather than read in part.
const MAX_PAGES: usize = 100;

/// The account's calendars.
pub async fn list(api: &Calendar<'_>) -> Result<Vec<ListedCalendar>, CalendarError> {
    Ok(api
        .calendars()
        .await?
        .into_iter()
        .filter(|entry| entry.id.chars().count() <= 512)
        .map(|entry| ListedCalendar {
            name: clip(&entry.name(), 200),
            color: entry.background_color.clone().filter(|c| is_color(c)),
            time_zone: entry.time_zone.clone().filter(|z| z.chars().count() <= 64),
            primary: entry.primary,
            writable: entry.writable(),
            provider_id: entry.id,
        })
        .collect())
}

/// Whether `color` is `#rrggbb`.
fn is_color(color: &str) -> bool {
    color.len() == 7 && color.starts_with('#') && color[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// One calendar's changes since `token`, or all of it (the events that end
/// from [`FIRST_READ_DAYS`] ago, and every series still recurring) when
/// there is none. A token Google no longer accepts is `Gone`: the caller
/// reads again without one.
pub async fn read(
    api: &Calendar<'_>,
    calendar: &str,
    token: Option<&str>,
    now: DateTime<Utc>,
) -> Result<Read, CalendarError> {
    let since = (now - TimeDelta::days(FIRST_READ_DAYS)).to_rfc3339_opts(SecondsFormat::Secs, true);
    let mut items: Vec<GoogleEvent> = Vec::new();
    let mut page: Option<String> = None;
    let mut sync: Option<String> = None;
    let mut finished = false;
    for _ in 0..MAX_PAGES {
        let answer = api
            .events(
                calendar,
                &EventsQuery {
                    sync_token: token,
                    time_min: token.is_none().then_some(since.as_str()),
                    page_token: page.as_deref(),
                    max: PAGE_SIZE,
                },
            )
            .await?;
        items.extend(answer.items);
        match answer.next_page_token {
            Some(next) => page = Some(next),
            None => {
                sync = answer.next_sync_token;
                finished = true;
                break;
            }
        }
    }
    if !finished || sync.is_none() {
        return Err(CalendarError::Provider(ProviderError::Unexpected(
            "the calendar could not be read to its end".into(),
        )));
    }
    let mut upserts: Vec<BatchEvent> = Vec::new();
    let mut removals: Vec<String> = Vec::new();
    for event in items {
        match normalise(event) {
            Normalised::Upsert(event) => {
                // An event listed twice (it changed between pages) keeps its
                // last version.
                upserts.retain(|seen| seen.provider_id != event.provider_id);
                removals.retain(|id| *id != event.provider_id);
                upserts.push(*event);
            }
            Normalised::Removal(id) => {
                upserts.retain(|seen| seen.provider_id != id);
                if !removals.contains(&id) {
                    removals.push(id);
                }
            }
            Normalised::Skip => {}
        }
    }
    Ok(Read {
        upserts,
        removals,
        full: token.is_none(),
        token: sync,
    })
}

/// What an event of the list means to the batch.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Normalised {
    /// The event exists in this state.
    Upsert(Box<BatchEvent>),
    /// The event is gone.
    Removal(String),
    /// Nothing for the calendar (an event the contract has no place for).
    Skip,
}

/// A Google event as the contract's event, or what else it means. A
/// cancelled event is a removal, except a cancelled occurrence of a series,
/// which is an exception that says so.
fn normalise(event: GoogleEvent) -> Normalised {
    if event.id.is_empty() || event.id.chars().count() > 1_024 {
        return Normalised::Skip;
    }
    // Where someone works from is not an event of the calendar.
    if event.event_type.as_deref() == Some("workingLocation") {
        return Normalised::Skip;
    }
    let cancelled = event.status.as_deref() == Some("cancelled");
    let original = event.original_start_time.as_ref().and_then(when);
    let series = event
        .recurring_event_id
        .clone()
        .filter(|id| !id.is_empty() && id.chars().count() <= 1_024);
    let exception = series.is_some() && original.is_some();
    if cancelled && !exception {
        return Normalised::Removal(event.id);
    }
    let start = event
        .start
        .as_ref()
        .and_then(when)
        .or_else(|| cancelled.then(|| original.clone()).flatten());
    let Some(start) = start else {
        // An event with no start cannot be placed; if it is gone, it is gone.
        return if cancelled {
            Normalised::Removal(event.id)
        } else {
            Normalised::Skip
        };
    };
    let status = match event.status.as_deref() {
        Some("cancelled") => "cancelled",
        Some("tentative") => "tentative",
        _ => "confirmed",
    };
    let private = event
        .extended_properties
        .as_ref()
        .and_then(|props| Private::read(|key| props.private.get(key).map(String::as_str)));
    let response = event
        .attendees
        .iter()
        .find(|person| person.is_self)
        .and_then(|person| response_of(person.response_status.as_deref()));
    Normalised::Upsert(Box::new(BatchEvent {
        provider_id: event.id,
        etag: event.etag.filter(|etag| etag.chars().count() <= 256),
        status,
        title: event.summary.as_deref().map(|t| clip(t, 1_000)),
        place: event.location.as_deref().map(|t| clip(t, 1_000)),
        description: event.description.as_deref().map(|t| clip(t, 20_000)),
        start,
        end: event.end.as_ref().and_then(when),
        recurrence: event
            .recurrence
            .iter()
            .take(20)
            .map(|line| clip(line, 1_000))
            .collect(),
        series: series.filter(|_| exception),
        original_start: original.filter(|_| exception),
        ical_uid: event
            .ical_uid
            .filter(|uid| !uid.is_empty() && uid.chars().count() <= 1_024),
        organizer: event.organizer.as_ref().map(person),
        attendees: event.attendees.iter().take(200).map(person).collect(),
        response,
        private,
    }))
}

/// `time` as the contract's `When`, when it is a date or a time with an
/// offset.
fn when(time: &EventTime) -> Option<When> {
    if let Some(date) = &time.date {
        NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()?;
        return Some(When::Date { date: date.clone() });
    }
    let date_time = time.date_time.as_ref()?;
    DateTime::parse_from_rfc3339(date_time).ok()?;
    Some(When::DateTime {
        date_time: date_time.clone(),
        time_zone: time
            .time_zone
            .clone()
            .filter(|zone| !zone.is_empty() && zone.chars().count() <= 64),
    })
}

/// Google's answer as the contract's.
fn response_of(status: Option<&str>) -> Option<&'static str> {
    match status? {
        "needsAction" => Some("needs_action"),
        "accepted" => Some("accepted"),
        "tentative" => Some("tentative"),
        "declined" => Some("declined"),
        _ => None,
    }
}

/// A person as the contract's.
fn person(person: &Person) -> PersonDocument {
    PersonDocument {
        name: person.display_name.as_deref().map(|n| clip(n, 200)),
        email: person
            .email
            .as_deref()
            .filter(|e| e.chars().count() <= 320)
            .map(ToOwned::to_owned),
        response: response_of(person.response_status.as_deref()),
        is_self: person.is_self,
    }
}

/// Make `change` (request `request`) at Google.
///
/// A create carries the request's id as the event's id, so a create made
/// before (the run died after Google took it) finds its event instead of
/// making another. An update or delete sends `If-Match` with the etag the
/// member's version was based on: a 412 means the event changed at Google
/// first, and Google's version wins.
pub async fn write(
    api: &Calendar<'_>,
    change: &ChangeDocument,
    request: Uuid,
) -> Result<Written, WriteFailure> {
    let calendar = change.provider_calendar_id.as_str();
    let event_id = change.provider_event_id.as_deref().unwrap_or_default();
    match change.change.as_str() {
        "create" => {
            let event = change
                .event
                .as_ref()
                .ok_or(WriteFailure::Skip("rejected"))?;
            let id = request.simple().to_string();
            let mut body = event_body(event, true);
            body["id"] = Value::String(id.clone());
            match api.insert(calendar, &body).await {
                Ok(created) => Ok(written(created)),
                Err(CalendarError::Exists) => Ok(written(api.event(calendar, &id).await?)),
                Err(other) => Err(other.into()),
            }
        }
        "update" => {
            let event = change
                .event
                .as_ref()
                .ok_or(WriteFailure::Skip("rejected"))?;
            let updated = api
                .patch(
                    calendar,
                    event_id,
                    &event_body(event, false),
                    change.etag.as_deref(),
                    false,
                )
                .await?;
            Ok(written(updated))
        }
        "update_occurrence" => {
            let event = change
                .event
                .as_ref()
                .ok_or(WriteFailure::Skip("rejected"))?;
            let occurrence = change
                .occurrence
                .as_ref()
                .ok_or(WriteFailure::Skip("rejected"))?;
            // The series must still be the version the member saw.
            let series = api.event(calendar, event_id).await?;
            if change.etag.is_some() && series.etag != change.etag {
                return Err(WriteFailure::Superseded);
            }
            let instance = api
                .instance(calendar, event_id, occurrence.as_text())
                .await?
                .ok_or(WriteFailure::Skip("not_found"))?;
            let updated = api
                .patch(
                    calendar,
                    &instance.id,
                    &event_body(event, false),
                    instance.etag.as_deref(),
                    false,
                )
                .await?;
            Ok(written(updated))
        }
        "delete" => match api.delete(calendar, event_id, change.etag.as_deref()).await {
            Ok(()) => Ok(Written {
                provider_event_id: event_id.to_owned(),
                etag: None,
            }),
            // Gone already: what was asked is so.
            Err(CalendarError::Gone | CalendarError::Provider(ProviderError::NotFound)) => {
                Ok(Written {
                    provider_event_id: event_id.to_owned(),
                    etag: None,
                })
            }
            Err(other) => Err(other.into()),
        },
        "respond" => {
            let answer = change
                .response
                .as_deref()
                .ok_or(WriteFailure::Skip("rejected"))?;
            let current = api.event_raw(calendar, event_id).await?;
            let attendees = answered(&current, answer).ok_or(WriteFailure::Skip("rejected"))?;
            let etag = current.get("etag").and_then(Value::as_str);
            // Tells the organiser: that is what an answer is for.
            let updated = api
                .patch(
                    calendar,
                    event_id,
                    &json!({ "attendees": attendees }),
                    etag,
                    true,
                )
                .await?;
            Ok(written(updated))
        }
        _ => Err(WriteFailure::Skip("rejected")),
    }
}

/// What Google answered, as the change's outcome.
fn written(event: GoogleEvent) -> Written {
    Written {
        provider_event_id: event.id,
        etag: event.etag,
    }
}

/// The attendees of `event` (as Google has them) with the member's own
/// answer set to `answer`, or `None` when the member is not among them.
fn answered(event: &Value, answer: &str) -> Option<Vec<Value>> {
    let mut attendees = event.get("attendees")?.as_array()?.clone();
    let mut found = false;
    for attendee in &mut attendees {
        if attendee.get("self").and_then(Value::as_bool) == Some(true) {
            attendee["responseStatus"] = Value::String(answer.to_owned());
            found = true;
        }
    }
    found.then_some(attendees)
}

/// Google's JSON for a time.
fn time_json(when: &When) -> Value {
    match when {
        When::Date { date } => json!({ "date": date, "dateTime": null, "timeZone": null }),
        When::DateTime {
            date_time,
            time_zone,
        } => json!({ "dateTime": date_time, "timeZone": time_zone, "date": null }),
    }
}

/// The body of an insert or patch for `event`. Only what the member's
/// version has is sent; `creating` adds what makes the event come back with
/// its kind and source.
fn event_body(event: &ChangeEvent, creating: bool) -> Value {
    let mut body = json!({});
    if let Some(title) = &event.title {
        body["summary"] = json!(title);
    }
    if let Some(place) = &event.place {
        body["location"] = json!(place);
    }
    if let Some(description) = &event.description {
        body["description"] = json!(description);
    }
    body["start"] = time_json(&event.start);
    if let Some(end) = &event.end {
        body["end"] = time_json(end);
    }
    if creating && let Some(private) = &event.private {
        let pairs: serde_json::Map<String, Value> = private
            .pairs()
            .into_iter()
            .map(|(key, value)| (key.to_owned(), Value::String(value.to_owned())))
            .collect();
        if !pairs.is_empty() {
            body["extendedProperties"] = json!({ "private": pairs });
        }
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(json: serde_json::Value) -> GoogleEvent {
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn an_event_is_normalised_with_its_series_response_and_private_properties() {
        let id = Uuid::now_v7().to_string();
        let normalised = normalise(event(serde_json::json!({
            "id": "e1", "etag": "\"1\"", "status": "confirmed",
            "summary": "Accountant", "location": "Av. da Liberdade",
            "start": {"dateTime": "2026-11-05T15:00:00+00:00", "timeZone": "Europe/Lisbon"},
            "end": {"dateTime": "2026-11-05T16:00:00+00:00"},
            "recurrence": ["RRULE:FREQ=WEEKLY"],
            "iCalUID": "uid-1@example.com",
            "organizer": {"email": "clinic@example.com", "displayName": "Clinic"},
            "attendees": [
                {"email": "ines@example.com", "self": true, "responseStatus": "needsAction"},
                {"email": "clinic@example.com", "responseStatus": "accepted"}
            ],
            "extendedProperties": {"private": {"campbooks_event": id, "campbooks_kind": "appointment"}}
        })));
        let Normalised::Upsert(event) = normalised else {
            panic!("an upsert");
        };
        assert_eq!(event.provider_id, "e1");
        assert_eq!(event.status, "confirmed");
        assert_eq!(event.response, Some("needs_action"));
        assert_eq!(event.attendees.len(), 2);
        assert!(event.attendees[0].is_self);
        assert_eq!(event.recurrence, ["RRULE:FREQ=WEEKLY"]);
        let private = event.private.unwrap();
        assert_eq!(private.campbooks_event.as_deref(), Some(id.as_str()));
        assert_eq!(private.campbooks_kind.as_deref(), Some("appointment"));
        assert_eq!(
            event.start,
            When::DateTime {
                date_time: "2026-11-05T15:00:00+00:00".to_owned(),
                time_zone: Some("Europe/Lisbon".to_owned()),
            }
        );
    }

    #[test]
    fn a_cancelled_event_is_a_removal_and_a_cancelled_occurrence_an_exception() {
        assert_eq!(
            normalise(event(
                serde_json::json!({"id": "gone", "status": "cancelled"})
            )),
            Normalised::Removal("gone".to_owned())
        );
        let Normalised::Upsert(skipped) = normalise(event(serde_json::json!({
            "id": "e1_20261105", "status": "cancelled", "recurringEventId": "e1",
            "originalStartTime": {"dateTime": "2026-11-05T15:00:00+00:00"}
        }))) else {
            panic!("an exception");
        };
        assert_eq!(skipped.status, "cancelled");
        assert_eq!(skipped.series.as_deref(), Some("e1"));
        assert!(skipped.original_start.is_some());
    }

    #[test]
    fn an_event_that_cannot_be_placed_is_left_out() {
        assert_eq!(
            normalise(event(serde_json::json!({
                "id": "e1", "start": {"dateTime": "tomorrow at noon"}
            }))),
            Normalised::Skip
        );
        assert_eq!(
            normalise(event(serde_json::json!({
                "id": "wl", "eventType": "workingLocation",
                "start": {"date": "2026-11-05"}
            }))),
            Normalised::Skip
        );
    }

    #[test]
    fn a_create_carries_its_kind_and_source_and_a_patch_clears_the_other_form_of_time() {
        let create = ChangeEvent {
            title: Some("Dentist".to_owned()),
            place: None,
            description: None,
            start: When::Date {
                date: "2026-11-05".to_owned(),
            },
            end: Some(When::Date {
                date: "2026-11-06".to_owned(),
            }),
            private: Some(Private {
                campbooks_event: Some(Uuid::now_v7().to_string()),
                campbooks_kind: Some("appointment".to_owned()),
                campbooks_source: Some("conversations.message:1".to_owned()),
            }),
        };
        let body = event_body(&create, true);
        assert_eq!(body["summary"], "Dentist");
        assert_eq!(
            body["extendedProperties"]["private"]["campbooks_kind"],
            "appointment"
        );
        assert_eq!(body["start"]["date"], "2026-11-05");
        // A patch never re-sends the private properties, and moving an
        // all-day event to a time clears its date.
        let timed = ChangeEvent {
            start: When::DateTime {
                date_time: "2026-11-05T15:00:00+00:00".to_owned(),
                time_zone: None,
            },
            ..create
        };
        let patch = event_body(&timed, false);
        assert!(patch.get("extendedProperties").is_none());
        assert!(patch["start"]["date"].is_null());
        assert_eq!(patch["start"]["dateTime"], "2026-11-05T15:00:00+00:00");
    }

    #[test]
    fn an_answer_changes_only_the_members_attendee() {
        let event = serde_json::json!({"attendees": [
            {"email": "ines@example.com", "self": true, "responseStatus": "needsAction", "optional": true},
            {"email": "clinic@example.com", "responseStatus": "accepted"}
        ]});
        let attendees = answered(&event, "declined").unwrap();
        assert_eq!(attendees[0]["responseStatus"], "declined");
        assert_eq!(attendees[0]["optional"], true);
        assert_eq!(attendees[1]["responseStatus"], "accepted");
        assert!(
            answered(
                &serde_json::json!({"attendees": [{"email": "x@example.com"}]}),
                "declined"
            )
            .is_none()
        );
    }
}
