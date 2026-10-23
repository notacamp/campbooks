//! The module's side of the contracts in `contracts/`: what it publishes
//! validates against its file, and the documents it consumes from
//! `connectors` (which has no types of its own for them yet) are accepted or
//! refused as the files say.

use campbooks_kernel::{
    DomainEvent, RecordRef,
    blobs::{BlobHash, BlobRef},
    tenancy::UserId,
};
use serde_json::{Value, json};

use crate::{
    events::{EventCancelled, EventChanged, EventCreated, OutboundRequested},
    model::{CalendarChange, CalendarId, ConnectionId, EventId, EventKind, EventOrigin, RequestId},
};

/// Whether `document` validates against the contract `file`.
fn accepts(file: &str, document: &Value) -> bool {
    let path = format!("{}/../../../contracts/{file}", env!("CARGO_MANIFEST_DIR"));
    let schema: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let mut schemas = boon::Schemas::new();
    let mut compiler = boon::Compiler::new();
    compiler.enable_format_assertions();
    compiler.add_resource(file, schema).unwrap();
    let index = compiler.compile(file, &mut schemas).unwrap();
    schemas.validate(document, index).is_ok()
}

const UUID: &str = "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c11";

fn blob() -> BlobRef {
    BlobRef {
        hash: BlobHash::of(b"a calendar change"),
        size: 17,
        content_type: "application/vnd.campbooks.calendar-change+json; version=1".into(),
    }
}

macro_rules! facts {
    ($event:ident) => {
        $event {
            workspace: UUID.to_owned(),
            event: EventId::new(),
            calendar: CalendarId::new(),
            owner: UserId::new(),
            kind: EventKind::Travel,
            starts: "2026-11-06T07:40:00+00:00".to_owned(),
            ends: Some("2026-11-06T11:55:00+01:00".to_owned()),
            all_day: false,
            origin: EventOrigin::Scout,
            source: Some(RecordRef::new("conversations.message", UUID)),
        }
    };
}

#[test]
fn the_event_events_follow_their_contracts() {
    let created = serde_json::to_value(facts!(EventCreated)).unwrap();
    let changed = serde_json::to_value(facts!(EventChanged)).unwrap();
    let cancelled = serde_json::to_value(facts!(EventCancelled)).unwrap();
    assert!(accepts("schedule.event_created.v1.schema.json", &created));
    assert!(accepts("schedule.event_changed.v1.schema.json", &changed));
    assert!(accepts(
        "schedule.event_cancelled.v1.schema.json",
        &cancelled
    ));
    assert_eq!(EventCreated::NAME, "schedule.event_created");
    assert_eq!(EventChanged::NAME, "schedule.event_changed");
    assert_eq!(EventCancelled::NAME, "schedule.event_cancelled");
}

#[test]
fn an_all_day_event_and_a_member_made_one_validate_too() {
    let mut event = facts!(EventCreated);
    event.starts = "2026-11-06".into();
    event.ends = Some("2026-11-07".into());
    event.all_day = true;
    event.origin = EventOrigin::Member;
    event.source = None;
    let value = serde_json::to_value(event).unwrap();
    assert!(accepts("schedule.event_created.v1.schema.json", &value));
    // `source` is left out, not null.
    let object = value.as_object().unwrap();
    assert!(!object.contains_key("source"));
}

#[test]
fn an_event_event_names_no_text() {
    let value = serde_json::to_value(facts!(EventCreated)).unwrap();
    let keys: Vec<&str> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    for text in [
        "title",
        "place",
        "notes",
        "attendees",
        "description",
        "quote",
    ] {
        assert!(!keys.contains(&text), "{text}");
    }
    // The contract refuses what it does not list.
    let mut with_title = value;
    with_title["title"] = json!("Flight LIS to AMS");
    assert!(!accepts(
        "schedule.event_created.v1.schema.json",
        &with_title
    ));
}

#[test]
fn an_outbound_request_follows_its_contract_and_carries_no_provider_id() {
    let request = OutboundRequested {
        workspace: UUID.to_owned(),
        request: RequestId::new(),
        calendar: CalendarId::new(),
        event: EventId::new(),
        connection: ConnectionId::new(),
        change: CalendarChange::UpdateOccurrence,
        body: blob(),
    };
    let value = serde_json::to_value(&request).unwrap();
    assert_eq!(value["change"], "update_occurrence");
    assert!(accepts(
        "schedule.outbound_requested.v1.schema.json",
        &value
    ));
    for field in ["provider_event_id", "provider_calendar_id", "etag", "title"] {
        assert!(value.get(field).is_none(), "{field}");
    }
}

#[test]
fn the_calendar_list_of_a_grant() {
    let list = json!({
        "version": 1,
        "calendars": [
            {"provider_id": "ines@example.com", "name": "Inês", "color": "#0b8043",
             "time_zone": "Europe/Lisbon", "primary": true, "writable": true},
            {"provider_id": "holidays-pt@example.com", "name": "Holidays in Portugal",
             "primary": false, "writable": false}
        ]
    });
    assert!(accepts("calendar-list.v1.schema.json", &list));
    let mut bad = list;
    bad["calendars"][0]["color"] = json!("green");
    assert!(!accepts("calendar-list.v1.schema.json", &bad));
}

#[test]
fn a_calendar_grant_names_its_calendar_list() {
    let granted = |with_list: bool| {
        let mut event = json!({
            "workspace": UUID, "connection": UUID, "capability": "calendar",
            "granted_by": UUID,
            "account": {"hash": "0".repeat(64), "size": 10, "content_type": "application/json"}
        });
        if with_list {
            event["calendars"] =
                json!({"hash": "1".repeat(64), "size": 90, "content_type": "application/json"});
        }
        event
    };
    assert!(accepts(
        "connectors.capability_granted.v1.schema.json",
        &granted(true)
    ));
    assert!(!accepts(
        "connectors.capability_granted.v1.schema.json",
        &granted(false)
    ));
}

#[test]
fn a_batch_of_calendar_changes() {
    let batch = json!({
        "version": 1,
        "provider_calendar_id": "ines@example.com",
        "upserts": [
            {
                "provider_id": "abc123", "etag": "\"3181\"", "status": "confirmed",
                "title": "Team call", "ical_uid": "abc123@example.com",
                "start": {"date_time": "2026-11-02T10:00:00+00:00", "time_zone": "Europe/Lisbon"},
                "end": {"date_time": "2026-11-02T10:30:00+00:00", "time_zone": "Europe/Lisbon"},
                "recurrence": ["RRULE:FREQ=WEEKLY;BYDAY=MO"],
                "attendees": [{"name": "Ana Costa", "email": "ana@example.com",
                               "response": "accepted"}, {"self": true, "response": "needs_action"}],
                "response": "needs_action",
                "private": {"campbooks_event": UUID, "campbooks_kind": "meeting"}
            },
            {
                "provider_id": "abc123_20261116", "status": "confirmed", "series": "abc123",
                "original_start": {"date_time": "2026-11-16T10:00:00+00:00"},
                "start": {"date_time": "2026-11-17T11:00:00+00:00"}
            },
            {"provider_id": "def456", "status": "confirmed", "start": {"date": "2026-11-12"}}
        ],
        "removals": [{"provider_id": "ghi789"}]
    });
    assert!(accepts("calendar-change-batch.v1.schema.json", &batch));
    let event = json!({
        "workspace": UUID, "connection": UUID,
        "batch": {"hash": "2".repeat(64), "size": 700,
                  "content_type": "application/vnd.campbooks.calendar-changes+json; version=1"}
    });
    assert!(accepts(
        "connectors.calendar_changed.v1.schema.json",
        &event
    ));
    // A start is a date or a time, not both and not neither.
    let mut both = batch.clone();
    both["upserts"][2]["start"] =
        json!({"date": "2026-11-12", "date_time": "2026-11-12T10:00:00Z"});
    assert!(!accepts("calendar-change-batch.v1.schema.json", &both));
    let mut neither = batch;
    neither["upserts"][2]["start"] = json!({});
    assert!(!accepts("calendar-change-batch.v1.schema.json", &neither));
}

#[test]
fn what_connectors_is_asked_to_write() {
    let create = json!({
        "version": 1, "change": "create", "provider_calendar_id": "ines@example.com",
        "event": {
            "title": "Dinner at Taberna", "start": {"date_time": "2026-11-12T20:00:00+00:00"},
            "private": {"campbooks_event": UUID, "campbooks_kind": "reservation"}
        }
    });
    assert!(accepts("calendar-change.v1.schema.json", &create));
    let update = json!({
        "version": 1, "change": "update", "provider_calendar_id": "ines@example.com",
        "provider_event_id": "abc123", "etag": "\"3181\"",
        "event": {"title": "Dentist", "start": {"date_time": "2026-11-12T16:00:00+00:00"}}
    });
    assert!(accepts("calendar-change.v1.schema.json", &update));
    // An update that names no etag cannot be guarded with If-Match.
    let mut unguarded = update.clone();
    unguarded.as_object_mut().unwrap().remove("etag");
    assert!(!accepts("calendar-change.v1.schema.json", &unguarded));
    // One occurrence names which.
    let mut occurrence = update;
    occurrence["change"] = json!("update_occurrence");
    assert!(!accepts("calendar-change.v1.schema.json", &occurrence));
    occurrence["occurrence"] = json!({"date_time": "2026-11-23T10:00:00+00:00"});
    assert!(accepts("calendar-change.v1.schema.json", &occurrence));
    let delete = json!({
        "version": 1, "change": "delete", "provider_calendar_id": "ines@example.com",
        "provider_event_id": "abc123", "etag": "\"3181\""
    });
    assert!(accepts("calendar-change.v1.schema.json", &delete));
    let respond = json!({
        "version": 1, "change": "respond", "provider_calendar_id": "ines@example.com",
        "provider_event_id": "abc123", "response": "accepted"
    });
    assert!(accepts("calendar-change.v1.schema.json", &respond));
    // A create has nothing at the provider to name.
    let mut create_with_id = create;
    create_with_id["provider_event_id"] = json!("abc123");
    assert!(!accepts("calendar-change.v1.schema.json", &create_with_id));
}

#[test]
fn how_a_calendar_change_ended() {
    let ended = |outcome: &str, extra: Value| {
        let mut event = json!({
            "workspace": UUID, "request": UUID, "calendar": UUID, "event": UUID,
            "outcome": outcome
        });
        for (key, value) in extra.as_object().unwrap() {
            event[key] = value.clone();
        }
        event
    };
    let file = "connectors.calendar_change_completed.v1.schema.json";
    assert!(accepts(
        file,
        &ended(
            "applied",
            json!({"provider_event_id": "abc123", "etag": "\"3182\""})
        )
    ));
    assert!(accepts(
        file,
        &ended("superseded", json!({"reason": "changed_at_provider"}))
    ));
    assert!(accepts(
        file,
        &ended("skipped", json!({"reason": "unverified"}))
    ));
    assert!(accepts(
        file,
        &ended("failed", json!({"reason": "unavailable"}))
    ));
    // Applied needs the provider's id; the others need a reason.
    assert!(!accepts(file, &ended("applied", json!({}))));
    assert!(!accepts(file, &ended("failed", json!({}))));
    assert!(!accepts(
        file,
        &ended("superseded", json!({"reason": "rejected"}))
    ));
    // Never event text.
    assert!(!accepts(
        file,
        &ended("failed", json!({"reason": "rejected", "title": "Dentist"}))
    ));
}

#[test]
fn a_message_read_is_ids_enums_and_times() {
    let message = UUID;
    let read = json!({
        "workspace": UUID, "message": message, "conversation": UUID, "inbox": UUID,
        "owner": UUID, "layer": "model", "summary_ready": false,
        "items": [
            {"key": format!("{message}:model:item:1"), "kind": "travel",
             "starts": "2026-11-06T07:40:00+00:00", "ends": "2026-11-06T11:55:00+01:00",
             "all_day": false, "time_zone": "Europe/Lisbon"},
            {"key": format!("{message}:model:item:2"), "kind": "stay", "starts": "2026-11-06",
             "all_day": true, "updates": format!("{UUID}:model:item:1")}
        ],
        "asks": [
            {"key": format!("{message}:model:ask:1"), "verb": "pay", "due": "2026-11-30",
             "has_amount": true, "same_as": "task-17"}
        ]
    });
    let file = "conversations.message_read.v1.schema.json";
    assert!(accepts(file, &read));
    // A shared inbox names no owner.
    let mut shared = read.clone();
    shared.as_object_mut().unwrap().remove("owner");
    assert!(accepts(file, &shared));
    // Not text: no title, quote, place or amount, and an item either updates or cancels.
    for (path, field) in [
        ("items", "title"),
        ("items", "quote"),
        ("items", "place"),
        ("asks", "amount"),
    ] {
        let mut text = read.clone();
        text[path][0][field] = json!("x");
        assert!(!accepts(file, &text), "{field}");
    }
    let mut both = read.clone();
    both["items"][1]["cancels"] = json!(format!("{UUID}:model:item:2"));
    assert!(!accepts(file, &both));
    // At most five items and three asks; kinds from the set.
    let mut many = read.clone();
    many["items"] = json!(
        (1..=6)
            .map(
                |n| json!({"key": format!("{message}:model:item:{n}"), "kind": "show",
                        "starts": "2026-11-06", "all_day": true})
            )
            .collect::<Vec<_>>()
    );
    assert!(!accepts(file, &many));
    let mut hold = read;
    hold["items"][0]["kind"] = json!("hold");
    assert!(!accepts(file, &hold));
}
