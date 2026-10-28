//! The Zoho Calendar adapter, offered only while
//! `CAMPBOOKS_CONNECTORS__ZOHO_CALENDAR` is on. **Unverified against a live
//! Zoho grant**, as it was in the Rails app.
//!
//! Zoho lists events by date range (31 days at most) and has no change
//! token, so the adapter reads the window and keeps its own marker: each
//! event's etag, as a JSON map in the calendar's token. What differs from
//! the marker is upserted; what is gone from the window is removed.
//! Zoho keeps no private properties: an event Campbooks wrote is
//! recognised by the provider id the module stored when it wrote it.

use std::collections::BTreeMap;

use chrono::{DateTime, Days, NaiveDate, NaiveDateTime, TimeDelta, Utc};
use serde_json::{Value, json};
use uuid::Uuid;

use super::{
    BatchEvent, ChangeDocument, ChangeEvent, FIRST_READ_DAYS, ListedCalendar, Read, When,
    WriteFailure, Written, clip,
};
use crate::{google::CalendarError, provider::ProviderError, zoho::ZohoCalendar};

/// Zoho's time format for a time in UTC.
const TIME: &str = "%Y%m%dT%H%M%SZ";
/// Zoho's format for a date.
const DATE: &str = "%Y%m%d";

/// How far ahead the window reaches.
const AHEAD_DAYS: i64 = 365;

/// Days Zoho's range may span.
const RANGE_DAYS: u64 = 31;

/// The account's calendars.
pub async fn list(api: &ZohoCalendar<'_>) -> Result<Vec<ListedCalendar>, CalendarError> {
    let mut calendars: Vec<ListedCalendar> = api
        .calendars()
        .await?
        .iter()
        .filter_map(|calendar| {
            let uid = text(calendar, "uid")?;
            Some(ListedCalendar {
                name: clip(&text(calendar, "name").unwrap_or_else(|| uid.clone()), 200),
                color: text(calendar, "color").filter(|c| is_color(c)),
                time_zone: text(calendar, "timezone").filter(|z| z.chars().count() <= 64),
                primary: calendar
                    .get("isdefault")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                writable: !matches!(
                    text(calendar, "privilege").as_deref(),
                    Some("view" | "free_busy" | "freebusy")
                ),
                owned: super::owned_by(text(calendar, "privilege").as_deref()),
                provider_id: uid,
            })
        })
        .filter(|calendar| calendar.provider_id.chars().count() <= 512)
        .collect();
    // With no default marked, the first is the one that syncs.
    if !calendars.is_empty() && !calendars.iter().any(|c| c.primary) {
        calendars[0].primary = true;
    }
    Ok(calendars)
}

/// Whether `color` is `#rrggbb`.
fn is_color(color: &str) -> bool {
    color.len() == 7 && color.starts_with('#') && color[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// A string field of `value`.
fn text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .filter(|s| !s.is_empty())
}

/// The window's events, read in ranges of at most [`RANGE_DAYS`].
async fn window(
    api: &ZohoCalendar<'_>,
    calendar: &str,
    now: DateTime<Utc>,
) -> Result<Vec<Value>, CalendarError> {
    let first = now - TimeDelta::days(FIRST_READ_DAYS);
    let last = now + TimeDelta::days(AHEAD_DAYS);
    let mut events: Vec<Value> = Vec::new();
    let mut from = first;
    while from < last {
        let to = (from + Days::new(RANGE_DAYS)).min(last);
        let answer = api
            .events(
                calendar,
                &from.format(TIME).to_string(),
                &to.format(TIME).to_string(),
            )
            .await?;
        events.extend(answer);
        from = to;
    }
    Ok(events)
}

/// One calendar's changes against the marker in `token` (all of it when
/// there is none).
pub async fn read(
    api: &ZohoCalendar<'_>,
    calendar: &str,
    token: Option<&str>,
    now: DateTime<Utc>,
) -> Result<Read, CalendarError> {
    let before: BTreeMap<String, String> = token
        .and_then(|marker| serde_json::from_str(marker).ok())
        .unwrap_or_default();
    let mut after: BTreeMap<String, String> = BTreeMap::new();
    let mut upserts: Vec<BatchEvent> = Vec::new();
    for raw in window(api, calendar, now).await? {
        let Some(event) = normalise(&raw) else {
            continue;
        };
        let marker = event
            .etag
            .clone()
            .unwrap_or_else(|| serde_json::to_string(&event).unwrap_or_default());
        if before.get(&event.provider_id) != Some(&marker) {
            upserts.push(event.clone());
        }
        after.insert(event.provider_id, marker);
    }
    let removals: Vec<String> = before
        .keys()
        .filter(|id| !after.contains_key(*id))
        .cloned()
        .collect();
    Ok(Read {
        upserts,
        removals,
        full: token.is_none(),
        token: Some(serde_json::to_string(&after).map_err(|_| {
            CalendarError::Provider(ProviderError::Unexpected(
                "a calendar marker did not encode".into(),
            ))
        })?),
    })
}

/// A Zoho event as the contract's event.
fn normalise(raw: &Value) -> Option<BatchEvent> {
    let provider_id = text(raw, "uid").filter(|id| id.chars().count() <= 1_024)?;
    let all_day = raw
        .get("isallday")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let times = raw.get("dateandtime")?;
    let zone = text(times, "timezone").filter(|z| z.chars().count() <= 64);
    let start = moment(&text(times, "start")?, all_day, zone.as_deref())?;
    let end = text(times, "end").and_then(|end| moment(&end, all_day, zone.as_deref()));
    Some(BatchEvent {
        provider_id,
        etag: text(raw, "etag").filter(|etag| etag.chars().count() <= 256),
        status: "confirmed",
        title: text(raw, "title").map(|t| clip(&t, 1_000)),
        place: text(raw, "location").map(|t| clip(&t, 1_000)),
        description: text(raw, "description").map(|t| clip(&t, 20_000)),
        start,
        end,
        recurrence: Vec::new(),
        series: None,
        original_start: None,
        ical_uid: None,
        organizer: None,
        attendees: Vec::new(),
        response: None,
        private: None,
    })
}

/// A Zoho time as the contract's `When`.
fn moment(value: &str, all_day: bool, zone: Option<&str>) -> Option<When> {
    if all_day {
        let date = NaiveDate::parse_from_str(value.get(..8)?, DATE).ok()?;
        return Some(When::Date {
            date: date.format("%Y-%m-%d").to_string(),
        });
    }
    let time = NaiveDateTime::parse_from_str(value, TIME).ok()?.and_utc();
    Some(When::DateTime {
        date_time: time.to_rfc3339(),
        time_zone: zone.map(ToOwned::to_owned),
    })
}

/// Zoho's `dateandtime` for a start and an end.
fn times_json(start: &When, end: Option<&When>) -> Option<(Value, bool)> {
    let (all_day, zone) = match start {
        When::Date { .. } => (true, None),
        When::DateTime { time_zone, .. } => (false, time_zone.clone()),
    };
    let render = |when: &When| -> Option<String> {
        match when {
            When::Date { date } => Some(
                NaiveDate::parse_from_str(date, "%Y-%m-%d")
                    .ok()?
                    .format(DATE)
                    .to_string(),
            ),
            When::DateTime { date_time, .. } => Some(
                DateTime::parse_from_rfc3339(date_time)
                    .ok()?
                    .with_timezone(&Utc)
                    .format(TIME)
                    .to_string(),
            ),
        }
    };
    let start_text = render(start)?;
    let end_text = render(end.unwrap_or(start))?;
    Some((
        json!({
            "timezone": zone.unwrap_or_else(|| "UTC".to_owned()),
            "start": start_text,
            "end": end_text,
        }),
        all_day,
    ))
}

/// The `eventdata` for `event`.
fn event_data(event: &ChangeEvent) -> Option<Value> {
    let (times, all_day) = times_json(&event.start, event.end.as_ref())?;
    let mut data = json!({ "dateandtime": times, "isallday": all_day });
    if let Some(title) = &event.title {
        data["title"] = json!(title);
    }
    if let Some(place) = &event.place {
        data["location"] = json!(place);
    }
    if let Some(description) = &event.description {
        data["description"] = json!(description);
    }
    Some(data)
}

/// Make `change` at Zoho. Changes to one occurrence of a series and answers
/// to an invitation are not made (`rejected`).
pub async fn write(
    api: &ZohoCalendar<'_>,
    change: &ChangeDocument,
    _request: Uuid,
) -> Result<Written, WriteFailure> {
    let calendar = change.provider_calendar_id.as_str();
    let event_id = change.provider_event_id.as_deref().unwrap_or_default();
    match change.change.as_str() {
        "create" => {
            let event = change
                .event
                .as_ref()
                .ok_or(WriteFailure::Skip("rejected"))?;
            let data = event_data(event).ok_or(WriteFailure::Skip("rejected"))?;
            let made = api.create(calendar, &data).await?;
            written(&made).ok_or(WriteFailure::Skip("rejected"))
        }
        "update" => {
            let event = change
                .event
                .as_ref()
                .ok_or(WriteFailure::Skip("rejected"))?;
            let mut data = event_data(event).ok_or(WriteFailure::Skip("rejected"))?;
            if let Some(etag) = &change.etag {
                data["etag"] = json!(etag);
            }
            let updated = api.update(calendar, event_id, &data).await?;
            Ok(written(&updated).unwrap_or_else(|| Written {
                provider_event_id: event_id.to_owned(),
                etag: None,
            }))
        }
        "delete" => {
            let etag = change
                .etag
                .as_deref()
                .ok_or(WriteFailure::Skip("rejected"))?;
            match api.delete(calendar, event_id, etag).await {
                Ok(()) | Err(CalendarError::Provider(ProviderError::NotFound)) => Ok(Written {
                    provider_event_id: event_id.to_owned(),
                    etag: None,
                }),
                Err(other) => Err(other.into()),
            }
        }
        _ => Err(WriteFailure::Skip("rejected")),
    }
}

/// What Zoho answered to a create or update.
fn written(event: &Value) -> Option<Written> {
    Some(Written {
        provider_event_id: text(event, "uid")?,
        etag: text(event, "etag"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_zoho_calendar_says_whether_it_is_owned() {
        use super::super::owned_by;
        assert_eq!(owned_by(Some("owner")), Some(true));
        assert_eq!(owned_by(Some("view")), Some(false));
        assert_eq!(owned_by(None), None);
    }

    #[test]
    fn a_zoho_event_reads_like_a_google_one() {
        let event = normalise(&json!({
            "uid": "z1", "etag": "1700000000", "title": "Accountant, Av. da Liberdade",
            "isallday": false, "location": "Av. da Liberdade",
            "dateandtime": {"timezone": "Europe/Lisbon",
                            "start": "20261105T150000Z", "end": "20261105T160000Z"}
        }))
        .unwrap();
        assert_eq!(event.provider_id, "z1");
        assert_eq!(event.status, "confirmed");
        assert_eq!(
            event.start,
            When::DateTime {
                date_time: "2026-11-05T15:00:00+00:00".to_owned(),
                time_zone: Some("Europe/Lisbon".to_owned()),
            }
        );
        let all_day = normalise(&json!({
            "uid": "z2", "title": "Holiday", "isallday": true,
            "dateandtime": {"timezone": "UTC", "start": "20261105", "end": "20261106"}
        }))
        .unwrap();
        assert_eq!(
            all_day.start,
            When::Date {
                date: "2026-11-05".to_owned()
            }
        );
    }

    #[test]
    fn a_change_is_written_in_zohos_formats() {
        let event = ChangeEvent {
            title: Some("Dentist".to_owned()),
            place: None,
            description: None,
            start: When::DateTime {
                date_time: "2026-11-05T16:00:00+01:00".to_owned(),
                time_zone: Some("Europe/Lisbon".to_owned()),
            },
            end: Some(When::DateTime {
                date_time: "2026-11-05T17:00:00+01:00".to_owned(),
                time_zone: Some("Europe/Lisbon".to_owned()),
            }),
            private: None,
        };
        let data = event_data(&event).unwrap();
        assert_eq!(data["dateandtime"]["start"], "20261105T150000Z");
        assert_eq!(data["dateandtime"]["end"], "20261105T160000Z");
        assert_eq!(data["isallday"], false);
        assert_eq!(data["title"], "Dentist");
    }
}
