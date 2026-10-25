//! Holding time: a block on the caller's calendar, the task planned for its
//! day, moved by holding again, undone by Release; never for a bill, a stale
//! revision or someone else's task; freed when the task closes early.

use axum::http::StatusCode;
use campbooks_kernel::{
    RecordRef,
    tenancy::WorkspaceId,
    testing::{self, TestResponse},
};
use chrono::{DateTime, Duration, TimeDelta, Utc};
use serde_json::{Value, json};
use sqlx::{PgPool, types::Uuid};

use crate::testkit::{CAMPBOOKS_CALENDAR, Kit, SOMEONE_ELSES_CALENDAR};

/// Two days from now at 10:00 UTC (no time zone is set: days are UTC).
fn wednesday_ten() -> DateTime<Utc> {
    let day = (Utc::now() + TimeDelta::days(2)).date_naive();
    day.and_hms_opt(10, 0, 0).unwrap().and_utc()
}

/// Make a task as `token`; its URI.
async fn task(kit: &Kit, token: &str, workspace: WorkspaceId, body: Value) -> String {
    let made = kit.add(token, workspace, body).await;
    format!(
        "/api/v1/w/{workspace}/tasks/{}",
        made["id"].as_str().unwrap()
    )
}

async fn hold(
    kit: &Kit,
    token: &str,
    uri: &str,
    revision: i64,
    starts: DateTime<Utc>,
    minutes: i64,
    calendar: Option<Uuid>,
) -> TestResponse {
    let mut body = json!({
        "revision": revision,
        "starts": starts,
        "ends": starts + Duration::minutes(minutes),
    });
    if let Some(calendar) = calendar {
        body["calendar_id"] = json!(calendar);
    }
    kit.post(token, &format!("{uri}/hold"), Some(body)).await
}

async fn history(kit: &Kit, token: &str, uri: &str) -> Vec<(String, String)> {
    let read = kit.get(token, uri).await;
    read.json["history"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            (
                entry["what"].as_str().unwrap().to_owned(),
                entry["actor"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn holding_plans_its_day_and_holding_again_moves_it(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    let uri = task(
        &kit,
        &ana,
        workspace,
        json!({"title": "Send the signed lease"}),
    )
    .await;
    let starts = wednesday_ten();

    let held = hold(&kit, &ana, &uri, 1, starts, 30, None).await;
    assert_eq!(held.status, StatusCode::OK, "{:?}", held.json);
    assert_eq!(held.json["plan"], "date");
    assert_eq!(held.json["plan_on"], starts.date_naive().to_string());
    assert_eq!(
        held.json["hold"]["calendar_id"],
        CAMPBOOKS_CALENDAR.to_string()
    );
    let blocks = kit.holds.blocks();
    assert_eq!(blocks.len(), 1);
    let first = &blocks[0];
    assert_eq!(held.json["hold"]["event_id"], first.event.to_string());
    assert_eq!(first.member, ana_id);
    assert_eq!(first.calendar, None);
    assert_eq!(first.title, "Send the signed lease");
    assert_eq!(
        first.task,
        RecordRef::new("tasks.task", held.json["id"].as_str().unwrap())
    );
    assert_eq!(
        (first.starts, first.ends),
        (starts, starts + Duration::minutes(30))
    );
    let changed = kit.events("tasks.task_changed").await;
    assert_eq!(changed.last().unwrap()["change"], "planned");
    assert!(!changed.last().unwrap().to_string().contains("lease"));

    // Again, an hour on a provider calendar the next day: the first block goes.
    let thursday = starts + TimeDelta::days(1) + TimeDelta::hours(4);
    let provider = Uuid::now_v7();
    let moved = hold(&kit, &ana, &uri, 2, thursday, 60, Some(provider)).await;
    assert_eq!(moved.status, StatusCode::OK, "{:?}", moved.json);
    assert_eq!(moved.json["plan_on"], thursday.date_naive().to_string());
    assert_eq!(moved.json["hold"]["calendar_id"], provider.to_string());
    let blocks = kit.holds.blocks();
    assert_eq!(blocks.len(), 2);
    assert!(blocks[0].released);
    assert!(!blocks[1].released);
    assert_eq!(
        history(&kit, &ana, &uri).await,
        [
            ("created".to_owned(), "member".to_owned()),
            ("held".to_owned(), "member".to_owned()),
            ("held".to_owned(), "member".to_owned()),
        ]
    );

    let tracked = testing::tracked_events(&kit.db, "tasks.task_held").await;
    assert_eq!(tracked.len(), 2);
    assert_eq!(tracked[0].body["minutes"], 30);
    assert_eq!(tracked[0].body["moved"], false);
    assert_eq!(tracked[0].body["calendar"], "campbooks");
    assert_eq!(tracked[1].body["minutes"], 60);
    assert_eq!(tracked[1].body["moved"], true);
    assert_eq!(tracked[1].body["calendar"], "provider");
    assert_eq!(tracked[1].body["by"], "member");
    assert_eq!(tracked[1].envelope["operation"], "tasks.hold");
    assert!(!tracked[1].body.to_string().contains("lease"));
    // Holding is not a plain plan change.
    assert!(
        testing::tracked_events(&kit.db, "tasks.task_planned")
            .await
            .is_empty()
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn release_undoes_a_hold(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let uri = task(&kit, &ana, workspace, json!({"title": "Call the bank"})).await;
    let none = kit
        .post(
            &ana,
            &format!("{uri}/release"),
            Some(json!({"revision": 1})),
        )
        .await;
    assert_eq!(none.status, StatusCode::BAD_REQUEST);
    assert_eq!(none.json["code"], "not_held");

    hold(&kit, &ana, &uri, 1, wednesday_ten(), 30, None).await;
    let released = kit
        .post(
            &ana,
            &format!("{uri}/release"),
            Some(json!({"revision": 2})),
        )
        .await;
    assert_eq!(released.status, StatusCode::OK, "{:?}", released.json);
    assert_eq!(released.json["hold"], Value::Null);
    assert_eq!(released.json["plan"], "none");
    assert_eq!(released.json["plan_on"], Value::Null);
    assert!(kit.holds.blocks()[0].released);
    let whats: Vec<String> = history(&kit, &ana, &uri)
        .await
        .into_iter()
        .map(|(what, _)| what)
        .collect();
    assert_eq!(whats, ["created", "held", "released"]);
    let tracked = testing::tracked_events(&kit.db, "tasks.task_hold_released").await;
    assert_eq!(tracked.len(), 1);
    assert_eq!(tracked[0].body["minutes"], 30);
    assert_eq!(tracked[0].body["by"], "member");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn bills_bad_times_stale_revisions_and_other_peoples_tasks_are_refused(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (joao, _) = kit.member("joao", workspace);
    let bill = task(
        &kit,
        &ana,
        workspace,
        json!({"title": "Pay the water bill", "verb": "pay"}),
    )
    .await;
    let refused = hold(&kit, &ana, &bill, 1, wednesday_ten(), 30, None).await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST);
    assert_eq!(refused.json["code"], "pay_not_held");

    let uri = task(&kit, &ana, workspace, json!({"title": "Book the car"})).await;
    for (starts, minutes) in [
        (wednesday_ten(), 10),
        (wednesday_ten(), 300),
        (Utc::now() - TimeDelta::hours(1), 30),
    ] {
        let refused = hold(&kit, &ana, &uri, 1, starts, minutes, None).await;
        assert_eq!(refused.status, StatusCode::BAD_REQUEST, "{minutes}");
        assert_eq!(refused.json["code"], "invalid_hold");
    }
    let stale = hold(&kit, &ana, &uri, 7, wednesday_ten(), 30, None).await;
    assert_eq!(stale.status, StatusCode::CONFLICT);
    assert_eq!(stale.json["code"], "task_changed");
    let theirs = hold(&kit, &joao, &uri, 1, wednesday_ten(), 30, None).await;
    assert_eq!(theirs.status, StatusCode::NOT_FOUND);
    let elsewhere = hold(
        &kit,
        &ana,
        &uri,
        1,
        wednesday_ten(),
        30,
        Some(SOMEONE_ELSES_CALENDAR),
    )
    .await;
    assert_eq!(elsewhere.status, StatusCode::NOT_FOUND);
    assert!(kit.holds.blocks().is_empty());
    let read = kit.get(&ana, &uri).await;
    assert_eq!(read.json["task"]["revision"], 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn closing_or_handing_over_frees_time_ahead_and_keeps_time_past(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (_, joao) = kit.member("joao", workspace);

    // Done on Tuesday while held Wednesday: the block goes.
    let lease = task(&kit, &ana, workspace, json!({"title": "Send the lease"})).await;
    hold(&kit, &ana, &lease, 1, wednesday_ten(), 30, None).await;
    let done = kit
        .post(&ana, &format!("{lease}/done"), Some(json!({"revision": 2})))
        .await;
    assert_eq!(done.status, StatusCode::OK, "{:?}", done.json);
    assert_eq!(done.json["hold"], Value::Null);
    assert!(kit.holds.blocks()[0].released);

    // Handed to Joao: Ana's block goes too.
    let car = task(&kit, &ana, workspace, json!({"title": "Book the car"})).await;
    hold(&kit, &ana, &car, 1, wednesday_ten(), 30, None).await;
    let handed = kit
        .post(
            &ana,
            &format!("{car}/hand_off"),
            Some(json!({"revision": 2, "to": joao})),
        )
        .await;
    assert_eq!(handed.status, StatusCode::OK, "{:?}", handed.json);
    assert_eq!(handed.json["hold"], Value::Null);
    assert!(kit.holds.blocks()[1].released);

    // A hold that has started stays as a record.
    let call = task(&kit, &ana, workspace, json!({"title": "Call"})).await;
    hold(&kit, &ana, &call, 1, wednesday_ten(), 30, None).await;
    sqlx::query(
        "UPDATE tasks.tasks SET held_from = now() - interval '10 minutes',
                held_to = now() + interval '20 minutes'
          WHERE hold_event IS NOT NULL",
    )
    .execute(&kit.db)
    .await
    .unwrap();
    let done = kit
        .post(
            &ana,
            &format!("{call}/not_needed"),
            Some(json!({"revision": 2})),
        )
        .await;
    assert_eq!(done.status, StatusCode::OK, "{:?}", done.json);
    assert!(done.json["hold"].is_object());
    assert!(!kit.holds.blocks()[2].released);

    let released = testing::tracked_events(&kit.db, "tasks.task_hold_released").await;
    let operations: Vec<&str> = released
        .iter()
        .map(|event| event.envelope["operation"].as_str().unwrap())
        .collect();
    assert_eq!(operations, ["tasks.done", "tasks.hand_off"]);
}
