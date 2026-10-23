//! Listing tasks.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::WorkspaceId;
use chrono::{TimeDelta, Utc};
use serde_json::json;
use sqlx::PgPool;

use crate::testkit::Kit;

fn titles(json: &serde_json::Value) -> Vec<String> {
    json["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|task| task["title"].as_str().unwrap().to_owned())
        .collect()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn mine_lists_overdue_first_then_by_due_then_undated(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (joao, _) = kit.member("joao", workspace);
    let today = Utc::now().date_naive();
    let day = |n: i64| (today + TimeDelta::days(n)).to_string();
    kit.add(&ana, workspace, json!({"title": "Undated"})).await;
    kit.add(
        &ana,
        workspace,
        json!({"title": "Next week", "due_on": day(7)}),
    )
    .await;
    kit.add(
        &ana,
        workspace,
        json!({"title": "Late bill", "verb": "pay", "due_on": day(-5)}),
    )
    .await;
    kit.add(
        &ana,
        workspace,
        json!({"title": "Tomorrow", "due_on": day(1)}),
    )
    .await;
    kit.add(&joao, workspace, json!({"title": "João's"})).await;

    let uri = format!("/api/v1/w/{workspace}/tasks");
    let mine = kit.get(&ana, &uri).await;
    assert_eq!(mine.status, StatusCode::OK, "{:?}", mine.json);
    assert_eq!(
        titles(&mine.json),
        ["Late bill", "Tomorrow", "Next week", "Undated"]
    );
    assert_eq!(mine.json["tasks"][0]["overdue"], true);
    assert_eq!(mine.json["tasks"][1]["overdue"], false);

    let overdue = kit.get(&ana, &format!("{uri}?overdue=true")).await;
    assert_eq!(titles(&overdue.json), ["Late bill"]);
    let undated = kit.get(&ana, &format!("{uri}?undated=true")).await;
    assert_eq!(titles(&undated.json), ["Undated"]);
    let window = kit
        .get(
            &ana,
            &format!("{uri}?due_from={}&due_to={}", day(1), day(2)),
        )
        .await;
    assert_eq!(titles(&window.json), ["Tomorrow"]);
    assert_eq!(titles(&kit.get(&joao, &uri).await.json), ["João's"]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_offset_moves_the_callers_day(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let utc_today = Utc::now().date_naive();
    kit.add(
        &ana,
        workspace,
        json!({"title": "Today in UTC", "due_on": utc_today.to_string()}),
    )
    .await;
    let uri = format!("/api/v1/w/{workspace}/tasks");
    // 14 hours ahead, it is already the next day in some hours of the day;
    // 14 hours behind it can be the day before. Either way a wide offset is
    // accepted, a wider one is not.
    let ok = kit.get(&ana, &format!("{uri}?utc_offset=840")).await;
    assert_eq!(ok.status, StatusCode::OK);
    let wide = kit.get(&ana, &format!("{uri}?utc_offset=900")).await;
    assert_eq!(wide.status, StatusCode::BAD_REQUEST);
    assert_eq!(wide.json["code"], "invalid_offset");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn ids_read_a_batch_and_leave_out_what_you_cannot_see(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (joao, _) = kit.member("joao", workspace);
    let mine = kit.add(&ana, workspace, json!({"title": "Mine"})).await;
    let theirs = kit.add(&joao, workspace, json!({"title": "Theirs"})).await;
    let uri = format!("/api/v1/w/{workspace}/tasks");
    let ids = format!(
        "{},{}",
        mine["id"].as_str().unwrap(),
        theirs["id"].as_str().unwrap()
    );
    let batch = kit.get(&ana, &format!("{uri}?ids={ids}")).await;
    assert_eq!(titles(&batch.json), ["Mine"]);

    for bad in [
        String::new(),
        "not-an-id".to_owned(),
        vec![ids.clone(); 60].join(","),
    ] {
        let refused = kit.get(&ana, &format!("{uri}?ids={bad}")).await;
        assert_eq!(refused.status, StatusCode::BAD_REQUEST, "{bad}");
        assert_eq!(refused.json["code"], "invalid_ids");
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn handed_off_lists_the_open_tasks_you_gave_away(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (joao, joao_id) = kit.member("joao", workspace);
    let task = kit
        .add(&ana, workspace, json!({"title": "Book the venue"}))
        .await;
    kit.add(&ana, workspace, json!({"title": "Keep"})).await;
    let id = task["id"].as_str().unwrap();
    let handed = kit
        .post(
            &ana,
            &format!("/api/v1/w/{workspace}/tasks/{id}/hand_off"),
            Some(json!({"revision": 1, "to": joao_id})),
        )
        .await;
    assert_eq!(handed.status, StatusCode::OK, "{:?}", handed.json);

    let uri = format!("/api/v1/w/{workspace}/tasks");
    let away = kit.get(&ana, &format!("{uri}?whose=handed_off")).await;
    assert_eq!(titles(&away.json), ["Book the venue"]);
    assert_eq!(away.json["tasks"][0]["assignee"], joao_id.to_string());
    assert_eq!(titles(&kit.get(&ana, &uri).await.json), ["Keep"]);
    assert_eq!(titles(&kit.get(&joao, &uri).await.json), ["Book the venue"]);
}
