//! Making a table.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::json;
use sqlx::PgPool;

use crate::testkit::Kit;

#[sqlx::test(migrations = "../../../migrations")]
async fn a_new_table_has_a_title_property_one_view_and_no_rows(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let ws = WorkspaceId::new();
    let (marta, _) = kit.member("marta", ws);
    let table = kit.table(&marta, ws).await;
    assert_eq!(table["page"]["title"], "Clients");
    assert_eq!(table["rows"], 0);
    assert_eq!(table["properties"].as_array().unwrap().len(), 1);
    assert_eq!(table["properties"][0]["type"], "title");
    assert_eq!(table["views"].as_array().unwrap().len(), 1);
    assert_eq!(table["views"][0]["type"], "table");

    // Its events carry ids and enums, never the title or a name.
    for name in ["pages.page_changed"] {
        let events = kit.events(name).await;
        assert!(!events.is_empty(), "{name}");
        assert!(!events.iter().any(|e| e.to_string().contains("Clients")));
    }
    let kinds = kit.events("pages.page_changed").await[0]["kinds"].clone();
    assert_eq!(kinds, json!(["meta", "schema"]));
    let tracked = campbooks_kernel::testing::tracked_events(&kit.db, "pages.table_created").await;
    assert_eq!(tracked.len(), 1);
    assert!(!tracked[0].body.to_string().contains("Clients"));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn tables_off_means_no_routes_and_no_tables_in_the_list(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let (marta, _) = kit.member("marta", ws);
    let table = kit.table(&marta, ws).await;
    let kit = kit.with_tables(false);
    let made = kit
        .post(&marta, &Kit::tables(ws), Some(json!({"title": "No"})))
        .await;
    assert_eq!(made.status, StatusCode::NOT_FOUND);
    let id = table["page"]["id"].as_str().unwrap();
    let got = kit.get(&marta, &format!("{}/{id}", Kit::tables(ws))).await;
    assert_eq!(got.status, StatusCode::NOT_FOUND);
    let listed = kit.get(&marta, &Kit::pages(ws)).await;
    assert_eq!(listed.status, StatusCode::OK);
    assert_eq!(listed.json["pages"], json!([]));
    assert_eq!(listed.json["tables"], false, "a client offers no tables");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_table_goes_under_a_page_and_a_page_never_under_a_table(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let (marta, _) = kit.member("marta", ws);
    let page = kit.add(&marta, ws, json!({"title": "Handbook"})).await;
    let under = kit
        .post(
            &marta,
            &Kit::tables(ws),
            Some(json!({"title": "T", "parent": page["id"]})),
        )
        .await;
    assert_eq!(under.status, StatusCode::CREATED, "{:?}", under.json);
    let table = under.json["page"]["id"].as_str().unwrap().to_owned();
    let nested = kit
        .post(&marta, &Kit::tables(ws), Some(json!({"parent": table})))
        .await;
    assert_eq!(nested.status, StatusCode::CONFLICT);
    assert_eq!(nested.json["code"], "table_parent");
    let page_in = kit
        .post(&marta, &Kit::pages(ws), Some(json!({"parent": table})))
        .await;
    assert_eq!(page_in.status, StatusCode::CONFLICT);
    assert_eq!(page_in.json["code"], "table_parent");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_stranger_does_not_see_the_table_and_a_reader_reads_it(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let (marta, _) = kit.member("marta", ws);
    let (rui, rui_id) = kit.member("rui", ws);
    let made = kit
        .post(&marta, &Kit::tables(ws), Some(json!({"title": "Private"})))
        .await;
    let id = made.json["page"]["id"].as_str().unwrap().to_owned();
    let uri = format!("{}/{id}", Kit::tables(ws));
    assert_eq!(kit.get(&rui, &uri).await.status, StatusCode::NOT_FOUND);
    kit.share(&marta, ws, &id, rui_id, "reader").await;
    let read = kit.get(&rui, &uri).await;
    assert_eq!(read.status, StatusCode::OK);
    assert_eq!(read.json["page"]["role"], "reader");
    // An ordinary page is not a table.
    let page = kit.add(&marta, ws, json!({"title": "P"})).await;
    let not = kit
        .get(
            &marta,
            &format!("{}/{}", Kit::tables(ws), page["id"].as_str().unwrap()),
        )
        .await;
    assert_eq!(not.status, StatusCode::CONFLICT);
    assert_eq!(not.json["code"], "not_a_table");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn pages_say_whether_they_are_tables_and_rows_are_never_listed(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let (marta, _) = kit.member("marta", ws);
    let table = kit.table(&marta, ws).await;
    let t = table["page"]["id"].as_str().unwrap();
    let plain = kit
        .add(&marta, ws, json!({"title": "Notes", "place": "workspace"}))
        .await;
    let row = kit.row(&marta, ws, t, json!({"title": "Acme"})).await;

    let listed = kit.get(&marta, &Kit::pages(ws)).await.json;
    let kind_of = |id: &str| {
        listed["pages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == id)
            .map(|p| p["kind"].clone())
    };
    assert_eq!(kind_of(t), Some(json!("table")));
    assert_eq!(kind_of(plain["id"].as_str().unwrap()), Some(json!("page")));
    assert_eq!(listed["pages"].as_array().unwrap().len(), 2);
    assert_eq!(listed["tables"], true);

    let got = kit
        .get(&marta, &format!("{}/{t}", Kit::pages(ws)))
        .await
        .json;
    assert_eq!(got["kind"], "table");
    let row_id = row["page"]["id"].as_str().or(row["id"].as_str()).unwrap();
    let got = kit
        .get(&marta, &format!("{}/{row_id}", Kit::pages(ws)))
        .await
        .json;
    assert_eq!(got["kind"], "row");
}
