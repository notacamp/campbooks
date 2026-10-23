//! Planning: a day, Later, or nothing.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::WorkspaceId;
use chrono::{TimeDelta, Utc};
use serde_json::json;
use sqlx::PgPool;

use crate::testkit::Kit;

#[sqlx::test(migrations = "../../../migrations")]
async fn later_needs_a_future_time_and_ends_by_itself(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let task = kit
        .add(&ana, workspace, json!({"title": "Book the car inspection"}))
        .await;
    let id = task["id"].as_str().unwrap().to_owned();
    let plan = format!("/api/v1/w/{workspace}/tasks/{id}/plan");

    for until in [
        Utc::now() - TimeDelta::hours(1),
        Utc::now() + TimeDelta::days(400),
    ] {
        let refused = kit
            .post(
                &ana,
                &plan,
                Some(json!({"revision": 1, "plan": "later", "later_until": until})),
            )
            .await;
        assert_eq!(refused.status, StatusCode::BAD_REQUEST);
        assert_eq!(refused.json["code"], "invalid_later");
    }
    let missing = kit
        .post(&ana, &plan, Some(json!({"revision": 1, "plan": "date"})))
        .await;
    assert_eq!(missing.json["code"], "plan_needs_day");

    let monday = Utc::now() + TimeDelta::days(4);
    let later = kit
        .post(
            &ana,
            &plan,
            Some(json!({"revision": 1, "plan": "later", "later_until": monday})),
        )
        .await;
    assert_eq!(later.status, StatusCode::OK, "{:?}", later.json);
    assert_eq!(later.json["plan"], "later");
    let events = kit.events("tasks.task_changed").await;
    assert_eq!(events[1]["change"], "planned");
    assert_eq!(events[1]["plan"], "later");

    // Once the time has passed it reads as undated, with no plan.
    sqlx::query("UPDATE tasks.tasks SET later_until = now() - interval '1 minute' WHERE id = $1")
        .bind(id.parse::<sqlx::types::Uuid>().unwrap())
        .execute(&kit.db)
        .await
        .unwrap();
    let after = kit
        .get(&ana, &format!("/api/v1/w/{workspace}/tasks/{id}"))
        .await;
    assert_eq!(after.json["task"]["plan"], "none");
    assert_eq!(after.json["task"]["later_until"], serde_json::Value::Null);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_day_and_no_plan_undo_each_other(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let task = kit.add(&ana, workspace, json!({"title": "Call"})).await;
    let plan = format!(
        "/api/v1/w/{workspace}/tasks/{}/plan",
        task["id"].as_str().unwrap()
    );
    let day = kit
        .post(
            &ana,
            &plan,
            Some(json!({"revision": 1, "plan": "date", "plan_on": "2026-10-29"})),
        )
        .await;
    assert_eq!(day.json["plan"], "date");
    assert_eq!(day.json["plan_on"], "2026-10-29");
    let none = kit
        .post(&ana, &plan, Some(json!({"revision": 2, "plan": "none"})))
        .await;
    assert_eq!(none.json["plan"], "none");
    assert_eq!(none.json["plan_on"], serde_json::Value::Null);
    let stale = kit
        .post(&ana, &plan, Some(json!({"revision": 2, "plan": "none"})))
        .await;
    assert_eq!(stale.status, StatusCode::CONFLICT);
}
