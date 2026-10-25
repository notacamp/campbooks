//! A hold follows its block: moved in Google, removed in Campbooks, put back
//! by the calendar's undo, turned all-day; nothing for another task's or
//! another member's block.

use campbooks_kernel::{
    RecordRef,
    tenancy::{UserId, WorkspaceId},
    testing,
};
use chrono::{DateTime, Duration, TimeDelta, Utc};
use serde_json::{Value, json};
use sqlx::{PgPool, types::Uuid};

use crate::{
    inbound::{CalendarEventCancelled, CalendarEventChanged, CalendarEventCreated},
    testkit::Kit,
};

fn wednesday_ten() -> DateTime<Utc> {
    let day = (Utc::now() + TimeDelta::days(2)).date_naive();
    day.and_hms_opt(10, 0, 0).unwrap().and_utc()
}

/// A task of `token`'s held 30 minutes from Wednesday 10:00; its URI, id
/// and the block's event and calendar.
async fn held(kit: &Kit, token: &str, workspace: WorkspaceId) -> (String, String, Uuid, Uuid) {
    let made = kit
        .add(token, workspace, json!({"title": "Send the signed lease"}))
        .await;
    let id = made["id"].as_str().unwrap().to_owned();
    let uri = format!("/api/v1/w/{workspace}/tasks/{id}");
    let starts = wednesday_ten();
    let held = kit
        .post(
            token,
            &format!("{uri}/hold"),
            Some(json!({"revision": 1, "starts": starts, "ends": starts + Duration::minutes(30)})),
        )
        .await;
    let event = held.json["hold"]["event_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let calendar = held.json["hold"]["calendar_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    (uri, id, event, calendar)
}

/// The facts of a block, as the schedule module publishes them.
fn facts(
    workspace: WorkspaceId,
    owner: UserId,
    (event, calendar): (Uuid, Uuid),
    task: &str,
    starts: &str,
    ends: Option<&str>,
) -> Value {
    json!({
        "workspace": workspace.to_string(),
        "event": event,
        "calendar": calendar,
        "owner": owner,
        "kind": "hold",
        "starts": starts,
        "ends": ends,
        "all_day": ends.is_none(),
        "origin": "member",
        "source": RecordRef::new("tasks.task", task),
    })
}

async fn task(kit: &Kit, token: &str, uri: &str) -> Value {
    kit.get(token, uri).await.json
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_block_moved_in_google_moves_the_hold_and_its_day(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    let (uri, id, event, calendar) = held(&kit, &ana, workspace).await;
    let thursday = wednesday_ten() + TimeDelta::days(1) - TimeDelta::hours(1);
    let moved: CalendarEventChanged = serde_json::from_value(facts(
        workspace,
        ana_id,
        (event, calendar),
        &id,
        &thursday.to_rfc3339(),
        Some(&(thursday + Duration::minutes(30)).to_rfc3339()),
    ))
    .unwrap();
    kit.publish(workspace, &moved).await;
    // Twice: nothing more.
    kit.publish(workspace, &moved).await;

    let read = task(&kit, &ana, &uri).await;
    assert_eq!(read["task"]["plan_on"], thursday.date_naive().to_string());
    let starts: DateTime<Utc> = read["task"]["hold"]["starts"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(starts, thursday);
    let last = read["history"].as_array().unwrap().last().unwrap().clone();
    assert_eq!(last["what"], "held");
    assert_eq!(last["actor"], "calendar");
    assert_eq!(read["history"].as_array().unwrap().len(), 3);
    let tracked = testing::tracked_events(&kit.db, "tasks.task_held").await;
    assert_eq!(tracked.len(), 2);
    assert_eq!(tracked[1].body["by"], "calendar");
    assert_eq!(tracked[1].body["moved"], true);
    assert!(tracked[1].body.get("calendar").is_none());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_block_removed_releases_the_hold_and_its_undo_brings_it_back(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    let (uri, id, event, calendar) = held(&kit, &ana, workspace).await;
    let starts = wednesday_ten();
    let block = facts(
        workspace,
        ana_id,
        (event, calendar),
        &id,
        &starts.to_rfc3339(),
        Some(&(starts + Duration::minutes(30)).to_rfc3339()),
    );
    let removed: CalendarEventCancelled = serde_json::from_value(block.clone()).unwrap();
    kit.publish(workspace, &removed).await;
    let read = task(&kit, &ana, &uri).await;
    assert_eq!(read["task"]["hold"], Value::Null);
    assert_eq!(read["task"]["plan"], "none");

    let back: CalendarEventCreated = serde_json::from_value(block).unwrap();
    kit.publish(workspace, &back).await;
    let read = task(&kit, &ana, &uri).await;
    assert_eq!(read["task"]["hold"]["event_id"], event.to_string());
    assert_eq!(read["task"]["plan"], "date");
    let whats: Vec<&str> = read["history"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["what"].as_str().unwrap())
        .collect();
    assert_eq!(whats, ["created", "held", "released", "held"]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_all_day_block_releases_and_others_blocks_change_nothing(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    let (_, joao) = kit.member("joao", workspace);
    let (uri, id, event, calendar) = held(&kit, &ana, workspace).await;
    let day = wednesday_ten().date_naive().to_string();

    // Someone else's calendar, another task, another event: nothing.
    for (owner, task_id, block) in [
        (joao, id.clone(), event),
        (ana_id, Uuid::now_v7().to_string(), event),
        (ana_id, id.clone(), Uuid::now_v7()),
    ] {
        let other: CalendarEventCancelled = serde_json::from_value(facts(
            workspace,
            owner,
            (block, calendar),
            &task_id,
            &day,
            None,
        ))
        .unwrap();
        kit.publish(workspace, &other).await;
    }
    let read = task(&kit, &ana, &uri).await;
    assert!(read["task"]["hold"].is_object());
    assert_eq!(read["task"]["revision"], 2);

    let all_day: CalendarEventChanged =
        serde_json::from_value(facts(workspace, ana_id, (event, calendar), &id, &day, None))
            .unwrap();
    kit.publish(workspace, &all_day).await;
    let read = task(&kit, &ana, &uri).await;
    assert_eq!(read["task"]["hold"], Value::Null);
    let last = read["history"].as_array().unwrap().last().unwrap().clone();
    assert_eq!(
        (last["what"].as_str(), last["actor"].as_str()),
        (Some("released"), Some("calendar"))
    );
}
