//! Views.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::json;
use sqlx::PgPool;

use crate::testkit::Kit;

#[sqlx::test(migrations = "../../../migrations")]
async fn views_are_table_or_board_and_the_reserved_types_are_refused(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let ws = WorkspaceId::new();
    let (marta, _) = kit.member("marta", ws);
    let table = kit.table(&marta, ws).await;
    let t = table["page"]["id"].as_str().unwrap();
    let uri = format!("{}/{t}/views", Kit::tables(ws));
    for ty in ["list", "calendar", "gallery"] {
        let refused = kit
            .post(&marta, &uri, Some(json!({"name": "V", "type": ty})))
            .await;
        assert_eq!(refused.status, StatusCode::BAD_REQUEST, "{ty}");
        assert_eq!(refused.json["code"], "view_type_reserved");
    }
    let text = kit
        .property(&marta, ws, t, json!({"name": "Notes", "type": "text"}))
        .await;
    let select = kit
        .property(
            &marta,
            ws,
            t,
            json!({"name": "Stage", "type": "select", "config": {"options": [{"name": "A"}]}}),
        )
        .await;
    // A board groups by a select or status property.
    for config in [
        json!({}),
        json!({"group_by": text["property"]["id"]}),
        json!({"group_by": uuid::Uuid::now_v7()}),
    ] {
        let refused = kit
            .post(
                &marta,
                &uri,
                Some(json!({"name": "B", "type": "board", "config": config})),
            )
            .await;
        assert_eq!(refused.status, StatusCode::BAD_REQUEST);
        assert_eq!(refused.json["code"], "invalid_view");
    }
    let board = kit
        .post(&marta, &uri, Some(json!({"name": "Pipeline", "type": "board",
            "config": {"group_by": select["property"]["id"], "card_properties": [text["property"]["id"]]}})))
        .await;
    assert_eq!(board.status, StatusCode::CREATED, "{:?}", board.json);
    let bad_filter = kit
        .post(&marta, &uri, Some(json!({"name": "F", "type": "table",
            "config": {"filter": {"property": text["property"]["id"], "operator": "gt", "value": "1"}}})))
        .await;
    assert_eq!(bad_filter.json["code"], "invalid_filter");
    let tracked = campbooks_kernel::testing::tracked_events(&kit.db, "pages.view_created").await;
    assert_eq!(tracked.len(), 1);
    assert!(!tracked[0].body.to_string().contains("Pipeline"));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_last_view_stays_and_a_removed_view_comes_back_with_its_id(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let (marta, _) = kit.member("marta", ws);
    let table = kit.table(&marta, ws).await;
    let t = table["page"]["id"].as_str().unwrap();
    let first = table["views"][0]["id"].as_str().unwrap();
    let base = format!("{}/{t}/views", Kit::tables(ws));
    let last = kit.delete(&marta, &format!("{base}/{first}")).await;
    assert_eq!(last.status, StatusCode::CONFLICT);
    assert_eq!(last.json["code"], "last_view");
    let second = kit
        .post(
            &marta,
            &base,
            Some(json!({"name": "Second", "type": "table"})),
        )
        .await;
    let id = second.json["id"].as_str().unwrap().to_owned();
    let removed = kit.delete(&marta, &format!("{base}/{id}")).await;
    assert_eq!(removed.status, StatusCode::OK);
    let mut again = removed.json.clone();
    again["id"] = json!(id);
    let back = kit.post(&marta, &base, Some(again)).await;
    assert_eq!(back.status, StatusCode::CREATED, "{:?}", back.json);
    assert_eq!(back.json["id"], id);
    let dup = kit
        .post(
            &marta,
            &base,
            Some(json!({"id": id, "name": "X", "type": "table"})),
        )
        .await;
    assert_eq!(dup.json["code"], "view_exists");
}
