//! The dates a sender structured for machines (ADR 0021), end to end: an
//! airline's JSON-LD, an invitation's iCalendar part, a bill's markup, and
//! every reason the message is not believed. A `complete` role on a mock
//! server proves none of it calls a model.

use axum::http::StatusCode;
use campbooks_ai::{AiConfig, Gateway, ProviderConfig, Role};
use campbooks_kernel::tenancy::WorkspaceId;
use chrono::{TimeDelta, Utc};
use serde_json::{Value, json};
use sqlx::PgPool;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

use crate::{
    ConversationsDeps,
    model::{InboxId, InboxRole},
    testkit::{Kit, Scripted, contract, dated, money, script, sorting_deps},
};

const AIRLINE: &str = "bookings@airline.example";
const PASSED: &str = "mx.example; dmarc=pass header.from=airline.example";

/// A `complete` role on a mock server, to count how often it is asked.
async fn model_on_a_mock() -> (MockServer, Gateway) {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": "never", "choices": [{"message": {"role": "assistant",
            "content": "{}"}, "finish_reason": "stop"}], "usage": {"prompt_tokens": 1,
            "completion_tokens": 1}
        })))
        .mount(&server)
        .await;
    let provider = ProviderConfig::new(format!("{}/v1", server.uri()).parse().unwrap(), "test");
    let gateway = Gateway::new(AiConfig {
        roles: [(Role::Complete, provider)].into(),
        ..AiConfig::default()
    })
    .unwrap();
    (server, gateway)
}

async fn kit_for(
    db: PgPool,
    deps: ConversationsDeps,
) -> (Kit, MockServer, WorkspaceId, InboxId, String) {
    let (server, gateway) = model_on_a_mock().await;
    let kit = Kit::with_deps(
        db,
        ConversationsDeps {
            ai: Some(gateway),
            ..deps
        },
    )
    .await;
    let workspace = WorkspaceId::new();
    let (token, sam) = kit.member("sam", workspace);
    let inbox = kit
        .inbox(workspace, "sam@example.com", &[(sam, InboxRole::Manager)])
        .await;
    (kit, server, workspace, inbox, token)
}

/// A message to Sam from `from`, in thread `thread`, sent `hours_ago` hours
/// ago, with the sender checks `results` (when given), an HTML part and some
/// attachments `(media type, content)`.
fn mail(
    id: &str,
    thread: &str,
    from: &str,
    results: Option<&str>,
    html: Option<&str>,
    attachments: &[(&str, &str)],
) -> Value {
    let mut message = json!({
        "provider_message_id": id,
        "thread_key": thread,
        "from": {"name": "Sender", "address": from},
        "to": [{"name": "Sam Rivers", "address": "sam@example.com"}],
        "subject": "Your booking",
        "sent_at": (Utc::now() - TimeDelta::hours(1)).to_rfc3339(),
        "direction": "inbound",
        "text": "Thank you for your booking. Your flight leaves on the 6th of November at noon.",
        "mailbox": {"in_inbox": true},
        "read": false, "starred": false
    });
    if let Some(results) = results {
        message["headers"] = json!({"authentication_results": results});
    }
    if let Some(html) = html {
        message["html"] = json!(html);
    }
    if !attachments.is_empty() {
        message["attachments"] = attachments
            .iter()
            .map(|(kind, content)| {
                json!({"filename": "booking", "content_type": kind, "content": content})
            })
            .collect();
    }
    json!({"changes": [{"op": "upsert", "message": message}]})
}

/// A day `days` from now.
fn day(days: i64) -> String {
    (Utc::now() + TimeDelta::days(days))
        .date_naive()
        .format("%Y-%m-%d")
        .to_string()
}

/// JSON-LD for a flight leaving in `days` days.
fn flight(days: i64) -> String {
    format!(
        r#"<html><head><script type="application/ld+json">
        {{"@context": "http://schema.org", "@type": "FlightReservation",
          "reservationFor": {{"@type": "Flight", "flightNumber": "1234",
            "airline": {{"iataCode": "AE"}},
            "departureAirport": {{"iataCode": "LIS"}}, "arrivalAirport": {{"iataCode": "AMS"}},
            "departureTime": "{d}T07:40:00+00:00", "arrivalTime": "{d}T11:55:00+01:00"}}}}
        </script></head><body><p>Your flight leaves on the 6th at noon.</p></body></html>"#,
        d = day(days)
    )
}

/// `(layer, state)` and the items of every structured read.
async fn structured(kit: &Kit) -> Vec<(String, Value, Value)> {
    sqlx::query_as(
        "SELECT m.provider_message_id, s.items, s.asks FROM conversations.message_reads s
           JOIN conversations.messages m ON m.id = s.message_id
          WHERE s.layer = 'structured' AND s.state = 'read' ORDER BY m.sent_at, 1",
    )
    .fetch_all(&kit.db)
    .await
    .unwrap()
}

async fn model_calls(server: &MockServer) -> usize {
    server.received_requests().await.unwrap().len()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_authenticated_airline_confirmation_is_read_with_no_model(db: PgPool) {
    let (kit, server, workspace, inbox, _) = kit_for(db, sorting_deps()).await;
    kit.ingest_value(
        workspace,
        inbox,
        &mail(
            "air-1",
            "thr-air",
            AIRLINE,
            Some(PASSED),
            Some(&flight(30)),
            &[],
        ),
    )
    .await;

    let reads = structured(&kit).await;
    assert_eq!(reads.len(), 1);
    let items = reads[0].1.as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["kind"], "travel");
    assert_eq!(items[0]["title"], "AE1234 LIS → AMS");
    assert_eq!(items[0]["starts"], format!("{}T07:40:00+00:00", day(30)));
    assert_eq!(items[0]["all_day"], false);
    // No prose quoted: the data is the source, not a sentence.
    assert!(items[0].get("quote").is_none());
    let message = kit.message_of("air-1").await;
    assert_eq!(items[0]["key"], format!("{message}:structured:item:1"));

    // Published once, as a structured read of the message's owner, no model.
    let events = kit.events("conversations.message_read").await;
    assert_eq!(events.len(), 1);
    let (schemas, index) = contract("conversations.message_read.v1.schema.json");
    schemas.validate(&events[0], index).unwrap();
    assert_eq!(events[0]["layer"], "structured");
    assert_eq!(events[0]["items"][0]["kind"], "travel");
    assert_eq!(events[0]["summary_ready"], false);
    assert!(events[0].get("owner").is_some());
    let text = events[0].to_string();
    assert!(!text.contains("LIS") && !text.contains("AE1234"), "{text}");
    assert_eq!(model_calls(&server).await, 0);

    // The same message again (a replayed delivery) changes nothing.
    kit.ingest_value(
        workspace,
        inbox,
        &mail(
            "air-1",
            "thr-air",
            AIRLINE,
            Some(PASSED),
            Some(&flight(30)),
            &[],
        ),
    )
    .await;
    assert_eq!(kit.events("conversations.message_read").await.len(), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn markup_from_a_sender_that_failed_its_checks_is_not_believed(db: PgPool) {
    let (kit, server, workspace, inbox, _) = kit_for(db, sorting_deps()).await;
    let html = flight(30);
    for (id, results) in [
        // A look-alike domain: DMARC failed.
        (
            "fake-1",
            Some(
                "mx.example; dkim=fail header.d=airline.example; dmarc=fail header.from=airline.example",
            ),
        ),
        // Passed for another domain.
        (
            "fake-2",
            Some("mx.example; dmarc=pass header.from=other-airline.example"),
        ),
        // Nothing checked or nothing said.
        ("fake-3", Some("mx.example; none")),
        ("fake-4", None),
    ] {
        kit.ingest_value(
            workspace,
            inbox,
            &mail(id, &format!("thr-{id}"), AIRLINE, results, Some(&html), &[]),
        )
        .await;
    }
    assert!(structured(&kit).await.is_empty());
    assert!(kit.events("conversations.message_read").await.is_empty());
    assert_eq!(model_calls(&server).await, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn mail_the_pass_called_suspicious_is_not_believed(db: PgPool) {
    // Authenticated, with markup, and the decide model sure it is phishing.
    let deps = sorting_deps().with_model(std::sync::Arc::new(Scripted::model([(
        AIRLINE,
        script("updates", false, true, true, 2.0, true),
    )])));
    let (kit, _, workspace, inbox, _) = kit_for(db, deps).await;
    kit.ingest_value(
        workspace,
        inbox,
        &mail(
            "air-1",
            "thr-air",
            AIRLINE,
            Some(PASSED),
            Some(&flight(30)),
            &[],
        ),
    )
    .await;
    assert!(structured(&kit).await.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_suspicion_the_policy_did_not_accept_is_only_a_suggestion(db: PgPool) {
    let mut unsure = script("updates", false, false, false, 0.0, false);
    unsure.retain(|(id, ..)| *id != "suspicious");
    unsure.push((
        "suspicious",
        campbooks_ai::decide::AnswerValue::Noul(true),
        0.5,
    ));
    let deps = sorting_deps().with_model(std::sync::Arc::new(Scripted::model([(AIRLINE, unsure)])));
    let (kit, _, workspace, inbox, _) = kit_for(db, deps).await;
    kit.ingest_value(
        workspace,
        inbox,
        &mail(
            "air-1",
            "thr-air",
            AIRLINE,
            Some(PASSED),
            Some(&flight(30)),
            &[],
        ),
    )
    .await;
    assert_eq!(structured(&kit).await.len(), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_invitation_is_a_meeting_and_a_later_cancellation_cancels_it(db: PgPool) {
    let (kit, server, workspace, inbox, _) = kit_for(db, sorting_deps()).await;
    let when = day(20).replace('-', "");
    let invite = |method: &str, sequence: u32, hour: u32| {
        format!(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Example//EN\r\nMETHOD:{method}\r\n\
BEGIN:VEVENT\r\nUID:site-visit-1@studio.example\r\nSEQUENCE:{sequence}\r\n\
DTSTART;TZID=Europe/Lisbon:{when}T{hour:02}0000\r\nDTEND;TZID=Europe/Lisbon:{when}T{end:02}0000\r\n\
SUMMARY:Site visit\r\nLOCATION:Rua das Flores 12\r\nATTENDEE:mailto:sam@example.com\r\n\
END:VEVENT\r\nEND:VCALENDAR\r\n",
            end = hour + 1
        )
    };
    let organiser = "pedro@studio.example";
    let passed = "mx.example; dmarc=pass header.from=studio.example";
    let send = |id: &str, ics: String| {
        mail(
            id,
            "thr-visit",
            organiser,
            Some(passed),
            None,
            &[("text/calendar; method=REQUEST", &ics)],
        )
    };

    kit.ingest_value(workspace, inbox, &send("visit-1", invite("REQUEST", 0, 15)))
        .await;
    let reads = structured(&kit).await;
    assert_eq!(reads.len(), 1);
    let first = &reads[0].1[0];
    assert_eq!(first["kind"], "meeting");
    assert_eq!(first["title"], "Site visit");
    assert_eq!(first["place"], "Rua das Flores 12");
    assert_eq!(first["time_zone"], "Europe/Lisbon");
    assert_eq!(first["uid"], "site-visit-1@studio.example");
    let first_key = first["key"].as_str().unwrap().to_owned();

    // The same invitation again changes nothing; a higher sequence moves it.
    kit.ingest_value(workspace, inbox, &send("visit-2", invite("REQUEST", 0, 15)))
        .await;
    assert_eq!(structured(&kit).await.len(), 1);
    kit.ingest_value(workspace, inbox, &send("visit-3", invite("REQUEST", 1, 16)))
        .await;
    let reads = structured(&kit).await;
    assert_eq!(reads.len(), 2);
    let moved = &reads[1].1[0];
    assert_eq!(moved["updates"], first_key);
    assert!(moved["starts"].as_str().unwrap().contains("T16:00:00"));
    let moved_key = moved["key"].as_str().unwrap().to_owned();

    // The cancellation names the item as it stands now.
    kit.ingest_value(workspace, inbox, &send("visit-4", invite("CANCEL", 2, 16)))
        .await;
    let reads = structured(&kit).await;
    assert_eq!(reads.len(), 3);
    assert_eq!(reads[2].1[0]["cancels"], moved_key);

    let events = kit.events("conversations.message_read").await;
    assert_eq!(events.len(), 3);
    let (schemas, index) = contract("conversations.message_read.v1.schema.json");
    for event in &events {
        schemas.validate(event, index).unwrap();
        assert_eq!(event["layer"], "structured");
        assert!(!event.to_string().contains("Site visit"));
    }
    assert_eq!(events[2]["items"][0]["cancels"], moved_key);
    assert_eq!(model_calls(&server).await, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_invoice_with_a_due_date_gives_the_pay_need_its_date(db: PgPool) {
    // The scripted model reads the bill as owing a payment: a Pay need.
    let deps = sorting_deps().with_model(std::sync::Arc::new(Scripted::model([(
        "billing@water.example",
        money(script("receipts", false, true, true, 1.0, false), "owed"),
    )])));
    let (kit, server, workspace, inbox, _) = kit_for(db, deps).await;
    let due = day(25);
    let html = format!(
        r#"<script type="application/ld+json">{{"@type": "Invoice",
          "provider": {{"@type": "Organization", "name": "Águas do Vale"}},
          "paymentDueDate": "{due}",
          "totalPaymentDue": {{"@type": "PriceSpecification", "price": "47.30",
                              "priceCurrency": "EUR"}}}}</script>"#
    );
    kit.ingest_value(
        workspace,
        inbox,
        &mail(
            "bill-1",
            "thr-bill",
            "billing@water.example",
            Some("mx.example; dmarc=pass header.from=water.example"),
            Some(&html),
            &[],
        ),
    )
    .await;
    let reads = structured(&kit).await;
    assert_eq!(reads.len(), 1);
    let ask = &reads[0].2[0];
    assert_eq!(ask["verb"], "pay");
    assert_eq!(ask["title"], "Águas do Vale");
    assert_eq!(ask["due"], due);
    assert_eq!(ask["amount"]["minor"], 4730);
    let events = kit.events("conversations.message_read").await;
    assert_eq!(events[0]["asks"][0]["has_amount"], true);
    assert_eq!(events[0]["asks"][0]["due"], due);
    assert!(!events[0].to_string().contains("4730"));
    // Its date is the Pay need's, with no model.
    let need: (String, Option<chrono::NaiveDate>) =
        sqlx::query_as("SELECT reason, due_on FROM conversations.needs WHERE state = 'open'")
            .fetch_one(&kit.db)
            .await
            .unwrap();
    assert_eq!(need.0, "pay");
    assert_eq!(need.1.map(|d| d.to_string()), Some(due));
    // The model is asked for the need's summary only: the dates are the
    // data's.
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let chat: Value = requests[0].body_json().unwrap();
    let sent: Value =
        serde_json::from_str(chat["messages"][1]["content"].as_str().unwrap()).unwrap();
    assert_eq!(sent["dates"], false);
    assert_eq!(sent["need"], "pay");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_date_in_the_prose_is_never_read(db: PgPool) {
    let (kit, server, workspace, inbox, _) = kit_for(db, sorting_deps()).await;
    // Authenticated; the text and the HTML both say when, in words, and no
    // markup says anything.
    let html = "<html><body><p>Your flight leaves on 6 November at 07:40.</p>\
                <p>Pay by 30 November.</p></body></html>";
    kit.ingest_value(
        workspace,
        inbox,
        &mail(
            "prose-1",
            "thr-prose",
            AIRLINE,
            Some(PASSED),
            Some(html),
            &[],
        ),
    )
    .await;
    assert!(structured(&kit).await.is_empty());
    assert!(kit.events("conversations.message_read").await.is_empty());
    assert_eq!(model_calls(&server).await, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn mail_older_than_the_window_is_not_read(db: PgPool) {
    let mut deps = sorting_deps();
    deps.config.decide_backfill_days = 14;
    let (kit, _, workspace, inbox, _) = kit_for(db, deps).await;
    let mut old = mail(
        "old-1",
        "thr-old",
        AIRLINE,
        Some(PASSED),
        Some(&flight(30)),
        &[],
    );
    old["changes"][0]["message"]["sent_at"] =
        json!((Utc::now() - TimeDelta::days(30)).to_rfc3339());
    kit.ingest_value(workspace, inbox, &old).await;
    assert!(structured(&kit).await.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_message_the_data_covers_is_not_read_by_the_model_for_dates(db: PgPool) {
    // The pass flags the confirmation as a trip; the data holds the flight.
    let deps = sorting_deps().with_model(std::sync::Arc::new(Scripted::model([(
        AIRLINE,
        dated(
            script("updates", false, false, false, 0.0, false),
            "travel",
            0.9,
        ),
    )])));
    let (kit, server, workspace, inbox, sam) = kit_for(db, deps).await;
    kit.ingest_value(
        workspace,
        inbox,
        &mail(
            "air-1",
            "thr-air",
            AIRLINE,
            Some(PASSED),
            Some(&flight(30)),
            &[],
        ),
    )
    .await;
    assert_eq!(structured(&kit).await.len(), 1);

    let conversation = kit.conversation_of("air-1").await;
    let message = kit.message_of("air-1").await;
    let path =
        format!("/api/v1/w/{workspace}/conversations/{conversation}/messages/{message}/read");
    // Flagged, but there is nothing for Scout to read.
    let view = kit.get(&sam, &path).await;
    assert_eq!(view.status, StatusCode::OK, "{:?}", view.json);
    assert_eq!(view.json["flagged"], true);
    assert_eq!(view.json["can_read"], false);
    assert_eq!(view.json["layers"], json!(["structured"]));
    assert_eq!(view.json["items"][0]["kind"], "travel");
    assert_eq!(view.json["status"], "none");
    let asked = kit.post(&sam, &path, None).await;
    assert_eq!(asked.json["outcome"], "covered");
    kit.run_jobs().await;
    assert_eq!(model_calls(&server).await, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_kind_the_data_did_not_find_is_still_for_the_model(db: PgPool) {
    // The pass says the email also holds a deadline; the data holds a flight.
    let deps = sorting_deps().with_model(std::sync::Arc::new(Scripted::model([(
        AIRLINE,
        dated(
            script("updates", false, false, false, 0.0, false),
            "show",
            0.9,
        ),
    )])));
    let (kit, _, workspace, inbox, sam) = kit_for(db, deps).await;
    kit.ingest_value(
        workspace,
        inbox,
        &mail(
            "air-1",
            "thr-air",
            AIRLINE,
            Some(PASSED),
            Some(&flight(30)),
            &[],
        ),
    )
    .await;
    let conversation = kit.conversation_of("air-1").await;
    let message = kit.message_of("air-1").await;
    let path =
        format!("/api/v1/w/{workspace}/conversations/{conversation}/messages/{message}/read");
    let view = kit.get(&sam, &path).await;
    assert_eq!(view.json["can_read"], true);
}
