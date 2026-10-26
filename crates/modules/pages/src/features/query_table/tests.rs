//! Querying.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::json;
use sqlx::PgPool;

use crate::testkit::Kit;

#[sqlx::test(migrations = "../../../migrations")]
async fn a_view_that_filters_stage_and_sums_value_answers_the_rows_and_the_sum(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let ws = WorkspaceId::new();
    let (marta, _) = kit.member("marta", ws);
    let table = kit.table(&marta, ws).await;
    let t = table["page"]["id"].as_str().unwrap();
    let stage = kit
        .property(
            &marta,
            ws,
            t,
            json!({"name": "Stage", "type": "select",
            "config": {"options": [{"name": "Kickoff"}, {"name": "Paused"}]}}),
        )
        .await;
    let value = kit
        .property(&marta, ws, t, json!({"name": "Value", "type": "number"}))
        .await;
    let (s, v) = (
        stage["property"]["id"].as_str().unwrap().to_owned(),
        value["property"]["id"].as_str().unwrap().to_owned(),
    );
    let (kick, paused) = (
        stage["property"]["config"]["options"][0]["id"]
            .as_str()
            .unwrap()
            .to_owned(),
        stage["property"]["config"]["options"][1]["id"]
            .as_str()
            .unwrap()
            .to_owned(),
    );
    for (title, option, amount) in [
        ("A", &kick, "100"),
        ("B", &paused, "50"),
        ("C", &kick, "25.5"),
        ("D", &kick, "1"),
    ] {
        kit.row(
            &marta,
            ws,
            t,
            json!({"title": title, "cells": [
            {"property": s, "value": option}, {"property": v, "value": amount}]}),
        )
        .await;
    }
    kit.row(&marta, ws, t, json!({"title": "E (no stage)"}))
        .await;
    let view = kit
        .post(
            &marta,
            &format!("{}/{t}/views", Kit::tables(ws)),
            Some(json!({"name": "All clients", "type": "table", "config": {
                "filter": {"and": [{"property": s, "operator": "is_not", "value": paused}]},
                "sorts": [{"property": v, "direction": "desc"}],
                "summaries": [{"property": v, "kind": "sum"}, {"property": v, "kind": "max"},
                              {"property": s, "kind": "count"}]}})),
        )
        .await;
    assert_eq!(view.status, StatusCode::CREATED, "{:?}", view.json);
    let q = kit
        .query(&marta, ws, t, json!({"view": view.json["id"]}))
        .await;
    assert_eq!(q.status, StatusCode::OK, "{:?}", q.json);
    let titles: Vec<_> = q.json["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["title"].as_str().unwrap())
        .collect();
    assert_eq!(
        titles,
        ["A", "C", "D", "E (no stage)"],
        "Paused is out, empty value last"
    );
    assert_eq!(q.json["matching"], 4);
    let sums: Vec<_> = q.json["summaries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["value"].clone())
        .collect();
    assert_eq!(sums, [json!("126.5"), json!("100"), json!("4")]);
    assert_eq!(q.json["next_cursor"], json!(null));

    // An ad hoc filter on top of the view, and the tracked event holds a bucket.
    let own = kit
        .query(
            &marta,
            ws,
            t,
            json!({"view": view.json["id"],
            "filter": {"property": v, "operator": "gt", "value": 30}}),
        )
        .await;
    assert_eq!(own.json["matching"], 1);
    let tracked = campbooks_kernel::testing::tracked_events(&kit.db, "pages.rows_queried").await;
    assert_eq!(tracked.len(), 2);
    assert_eq!(tracked[0].body["rows"], "few");
    assert!(
        !tracked
            .iter()
            .any(|e| e.body.to_string().contains("Kickoff"))
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn rows_come_100_at_a_time_with_a_cursor(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let (marta, marta_id) = kit.member("marta", ws);
    let t = kit.table(&marta, ws).await["page"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    sqlx::query(
        "WITH made AS (
             INSERT INTO pages.pages (id, workspace_id, parent_id, place, position, title, created_by, edited_by, kind)
             SELECT gen_random_uuid(), $2, $1, 'workspace', 'a0', 'r' || n, $3, $3, 'row' FROM generate_series(1, 250) n
             RETURNING id, title)
         INSERT INTO pages.table_rows (row_id, table_id, workspace_id, number)
         SELECT id, $1, $2, substr(title, 2)::bigint FROM made",
    )
    .bind(t.parse::<uuid::Uuid>().unwrap())
    .bind(ws.as_uuid())
    .bind(marta_id.as_uuid())
    .execute(&kit.db)
    .await
    .unwrap();
    let mut cursor = json!(null);
    let mut seen = Vec::new();
    loop {
        let page = kit.query(&marta, ws, &t, json!({"cursor": cursor})).await;
        assert_eq!(page.status, StatusCode::OK, "{:?}", page.json);
        let rows = page.json["rows"].as_array().unwrap();
        assert!(rows.len() <= 100);
        seen.extend(rows.iter().map(|r| r["number"].as_i64().unwrap()));
        cursor = page.json["next_cursor"].clone();
        if cursor.is_null() {
            break;
        }
    }
    assert_eq!(seen, (1..=250).collect::<Vec<i64>>());
    for bad in [
        json!({"limit": 0}),
        json!({"limit": 101}),
        json!({"cursor": "x9"}),
        json!({"cursor": "o99999999"}),
    ] {
        assert_eq!(
            kit.query(&marta, ws, &t, bad).await.status,
            StatusCode::BAD_REQUEST
        );
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_filter_naming_nothing_real_is_refused_and_operands_are_never_sql(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let (marta, _) = kit.member("marta", ws);
    let table = kit.table(&marta, ws).await;
    let t = table["page"]["id"].as_str().unwrap();
    let title = table["properties"][0]["id"].as_str().unwrap();
    kit.row(
        &marta,
        ws,
        t,
        json!({"title": "O'Brien; DROP TABLE pages.pages; --"}),
    )
    .await;
    let q = kit
        .query(
            &marta,
            ws,
            t,
            json!({"filter": {"property": title, "operator": "contains", "value": "'Brien; DROP"}}),
        )
        .await;
    assert_eq!(q.status, StatusCode::OK);
    assert_eq!(q.json["matching"], 1);
    let bad = kit
        .query(
            &marta,
            ws,
            t,
            json!({"filter": {"property": uuid::Uuid::now_v7(), "operator": "is", "value": "x"}}),
        )
        .await;
    assert_eq!(bad.json["code"], "invalid_filter");
    assert_eq!(kit.page_count(ws).await, 2);
}
