//! Adding rows.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::json;
use sqlx::PgPool;

use crate::testkit::Kit;

#[sqlx::test(migrations = "../../../migrations")]
async fn rows_are_pages_under_the_table_with_numbers_and_leave_the_page_list_alone(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let ws = WorkspaceId::new();
    let (marta, _) = kit.member("marta", ws);
    let t = kit.table(&marta, ws).await["page"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let a = kit.row(&marta, ws, &t, json!({"title": "Northwind"})).await;
    let b = kit.row(&marta, ws, &t, json!({"title": "Acme"})).await;
    assert_eq!(
        (a["number"].as_i64(), b["number"].as_i64()),
        (Some(1), Some(2))
    );
    // A row is a page: it opens, and trashes and restores like one.
    let page = kit
        .get(
            &marta,
            &format!("{}/{}", Kit::pages(ws), a["id"].as_str().unwrap()),
        )
        .await;
    assert_eq!(page.status, StatusCode::OK);
    assert_eq!(page.json["parent"], t);
    // But it is not in the tree.
    let listed = kit.get(&marta, &Kit::pages(ws)).await;
    assert_eq!(listed.json["pages"].as_array().unwrap().len(), 1);
    assert_eq!(listed.json["pages"][0]["has_children"], false);

    let id = a["id"].as_str().unwrap();
    let trashed = kit
        .post(&marta, &format!("{}/{id}/trash", Kit::pages(ws)), None)
        .await;
    assert_eq!(trashed.status, StatusCode::OK);
    assert_eq!(
        kit.query(&marta, ws, &t, json!({})).await.json["matching"],
        1
    );
    let restored = kit
        .post(&marta, &format!("{}/{id}/restore", Kit::pages(ws)), None)
        .await;
    assert_eq!(restored.status, StatusCode::OK);
    assert_eq!(
        kit.query(&marta, ws, &t, json!({})).await.json["matching"],
        2
    );
    // A row stays in its table.
    let moved = kit
        .post(
            &marta,
            &format!("{}/{id}/move", Kit::pages(ws)),
            Some(json!({"meta_revision": 1, "parent": null, "place": "workspace"})),
        )
        .await;
    assert_eq!(moved.status, StatusCode::CONFLICT);
    assert_eq!(moved.json["code"], "table_row_fixed");
    let created = campbooks_kernel::testing::tracked_events(&kit.db, "pages.page_created").await;
    assert!(created.len() >= 3);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_table_holds_at_most_20000_rows(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let (marta, marta_id) = kit.member("marta", ws);
    let t = kit.table(&marta, ws).await["page"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let table: uuid::Uuid = t.parse().unwrap();
    sqlx::query(
        "WITH made AS (
             INSERT INTO pages.pages (id, workspace_id, parent_id, place, position, title, created_by, edited_by, kind)
             SELECT gen_random_uuid(), $2, $1, 'workspace', 'a0', 'r' || n, $3, $3, 'row'
               FROM generate_series(1, 20000) n RETURNING id)
         INSERT INTO pages.table_rows (row_id, table_id, workspace_id, number)
         SELECT id, $1, $2, row_number() OVER () FROM made",
    )
    .bind(table)
    .bind(ws.as_uuid())
    .bind(marta_id.as_uuid())
    .execute(&kit.db)
    .await
    .unwrap();
    let full = kit
        .post(
            &marta,
            &format!("{}/{t}/rows", Kit::tables(ws)),
            Some(json!({})),
        )
        .await;
    assert_eq!(full.status, StatusCode::CONFLICT);
    assert_eq!(full.json["code"], "table_full");
}
