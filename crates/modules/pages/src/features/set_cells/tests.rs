//! Writing cells.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::testkit::Kit;

struct Fixture {
    kit: Kit,
    ws: WorkspaceId,
    marta: String,
    rui: String,
    table: String,
    row: String,
    stage: String,
    value: String,
    options: Vec<String>,
}

async fn fixture(db: PgPool) -> Fixture {
    let kit = Kit::new(db).await.tracking();
    let ws = WorkspaceId::new();
    let (marta, _) = kit.member("marta", ws);
    let (rui, _) = kit.member("rui", ws);
    let table = kit.table(&marta, ws).await;
    let t = table["page"]["id"].as_str().unwrap().to_owned();
    let stage = kit
        .property(
            &marta,
            ws,
            &t,
            json!({"name": "Stage", "type": "status", "config": {"options": [
                {"name": "Proposal sent"}, {"name": "Kickoff"}, {"name": "Paused", "group": "done"}]}}),
        )
        .await;
    let value = kit
        .property(&marta, ws, &t, json!({"name": "Value", "type": "number"}))
        .await;
    let options = stage["property"]["config"]["options"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["id"].as_str().unwrap().to_owned())
        .collect();
    let row = kit
        .row(&marta, ws, &t, json!({"title": "Northwind Bakery"}))
        .await;
    Fixture {
        stage: stage["property"]["id"].as_str().unwrap().to_owned(),
        value: value["property"]["id"].as_str().unwrap().to_owned(),
        row: row["id"].as_str().unwrap().to_owned(),
        kit,
        ws,
        marta,
        rui,
        table: t,
        options,
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_bad_value_is_refused_and_nothing_is_stored(db: PgPool) {
    let f = fixture(db).await;
    let bad = f
        .kit
        .cells(
            &f.marta,
            f.ws,
            &f.table,
            &f.row,
            json!([
                {"property": f.stage, "value": f.options[0]},
                {"property": f.value, "value": "lots"},
            ]),
        )
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    assert_eq!(bad.json["code"], "invalid_value");
    let cells: i64 = sqlx::query_scalar("SELECT count(*) FROM pages.row_values")
        .fetch_one(&f.kit.db)
        .await
        .unwrap();
    assert_eq!(
        cells, 0,
        "the good cell of the same call was not stored either"
    );
    for body in [
        json!([{"property": uuid::Uuid::now_v7(), "value": "x"}]),
        json!([{"property": f.value, "value": "1"}, {"property": f.value, "value": "2"}]),
    ] {
        let refused = f.kit.cells(&f.marta, f.ws, &f.table, &f.row, body).await;
        assert_eq!(refused.status, StatusCode::BAD_REQUEST);
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn two_people_two_cells_both_are_stored_and_a_stale_write_answers_what_it_replaced(
    db: PgPool,
) {
    let f = fixture(db).await;
    let a = f
        .kit
        .cells(
            &f.marta,
            f.ws,
            &f.table,
            &f.row,
            json!([{"property": f.stage, "value": f.options[1]}]),
        )
        .await;
    let b = f
        .kit
        .cells(
            &f.rui,
            f.ws,
            &f.table,
            &f.row,
            json!([{"property": f.value, "value": 12000}]),
        )
        .await;
    assert_eq!((a.status, b.status), (StatusCode::OK, StatusCode::OK));
    let row = &b.json["row"]["cells"];
    assert_eq!(row[&f.stage]["value"], f.options[1]);
    assert_eq!(row[&f.value]["value"], "12000");

    // Rui read version 1 of Stage; Marta changed it meanwhile (version 2).
    let again = f
        .kit
        .cells(
            &f.marta,
            f.ws,
            &f.table,
            &f.row,
            json!([{"property": f.stage, "value": f.options[2]}]),
        )
        .await;
    assert_eq!(again.json["results"][0]["version"], 2);
    let stale = f
        .kit
        .cells(
            &f.rui,
            f.ws,
            &f.table,
            &f.row,
            json!([{"property": f.stage, "value": f.options[0], "version": 1}]),
        )
        .await;
    assert_eq!(stale.status, StatusCode::OK);
    let result = &stale.json["results"][0];
    assert_eq!(result["stale"], true);
    assert_eq!(result["previous"], f.options[2]);
    assert_eq!(result["version"], 3);
    assert_eq!(
        stale.json["row"]["cells"][&f.stage]["value"], f.options[0],
        "the later write wins"
    );

    // The undo is the same call with what it replaced.
    let undo = f
        .kit
        .cells(
            &f.rui,
            f.ws,
            &f.table,
            &f.row,
            json!([{"property": f.stage, "value": result["previous"]}]),
        )
        .await;
    assert_eq!(undo.json["row"]["cells"][&f.stage]["value"], f.options[2]);
    // The same value again changes nothing.
    let same = f
        .kit
        .cells(
            &f.rui,
            f.ws,
            &f.table,
            &f.row,
            json!([{"property": f.stage, "value": f.options[2]}]),
        )
        .await;
    assert_eq!(
        same.json["results"][0]["version"],
        undo.json["results"][0]["version"]
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn dragging_a_card_sets_the_group_value_and_events_carry_no_values(db: PgPool) {
    let f = fixture(db).await;
    f.kit
        .cells(
            &f.marta,
            f.ws,
            &f.table,
            &f.row,
            json!([{"property": f.stage, "value": f.options[0]}]),
        )
        .await;
    let board = f
        .kit
        .post(
            &f.marta,
            &format!("{}/{}/views", Kit::tables(f.ws), f.table),
            Some(json!({"name": "Pipeline", "type": "board", "config": {"group_by": f.stage}})),
        )
        .await;
    assert_eq!(board.status, StatusCode::CREATED, "{:?}", board.json);
    let moved = f
        .kit
        .cells(
            &f.marta,
            f.ws,
            &f.table,
            &f.row,
            json!([{"property": f.stage, "value": f.options[1]}]),
        )
        .await;
    assert_eq!(moved.json["results"][0]["previous"], f.options[0]);
    let q = f
        .kit
        .query(&f.marta, f.ws, &f.table, json!({"view": board.json["id"]}))
        .await;
    assert_eq!(
        q.json["groups"],
        json!([{"option": f.options[1], "rows": 1}])
    );

    for name in ["pages.page_changed", "pages.table_changed"] {
        for event in f.kit.events(name).await {
            let text = event.to_string();
            for secret in ["Northwind", "Kickoff", "12000", &f.options[1]] {
                assert!(!text.contains(secret), "{name}: {text}");
            }
        }
    }
    let kinds: Vec<Value> = f
        .kit
        .events("pages.page_changed")
        .await
        .iter()
        .map(|e| e["kinds"].clone())
        .collect();
    assert!(kinds.contains(&json!(["values"])));
    let tracked = campbooks_kernel::testing::tracked_events(&f.kit.db, "pages.cells_edited").await;
    assert_eq!(tracked.len(), 2);
    assert!(
        !tracked
            .iter()
            .any(|e| e.body.to_string().contains("Northwind"))
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_title_is_a_cell_and_computed_cells_are_not_writable(db: PgPool) {
    let f = fixture(db).await;
    let t = f
        .kit
        .get(&f.marta, &format!("{}/{}", Kit::tables(f.ws), f.table))
        .await;
    let title = t.json["properties"][0]["id"].as_str().unwrap().to_owned();
    let renamed = f
        .kit
        .cells(
            &f.marta,
            f.ws,
            &f.table,
            &f.row,
            json!([{"property": title, "value": "Northwind Ltd"}]),
        )
        .await;
    assert_eq!(renamed.status, StatusCode::OK);
    assert_eq!(renamed.json["row"]["title"], "Northwind Ltd");
    let id = f
        .kit
        .property(
            &f.marta,
            f.ws,
            &f.table,
            json!({"name": "ID", "type": "unique_id", "config": {"prefix": "CL-"}}),
        )
        .await;
    let refused = f
        .kit
        .cells(
            &f.marta,
            f.ws,
            &f.table,
            &f.row,
            json!([{"property": id["property"]["id"], "value": "5"}]),
        )
        .await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST);
    assert_eq!(refused.json["code"], "property_not_editable");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_reader_cannot_write_a_cell_and_a_stranger_does_not_see_the_row(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let (marta, _) = kit.member("marta", ws);
    let (rui, rui_id) = kit.member("rui", ws);
    let made = kit
        .post(&marta, &Kit::tables(ws), Some(json!({"title": "Private"})))
        .await;
    let t = made.json["page"]["id"].as_str().unwrap().to_owned();
    let value = kit
        .property(&marta, ws, &t, json!({"name": "Value", "type": "number"}))
        .await;
    let p = value["property"]["id"].as_str().unwrap().to_owned();
    let row = kit.row(&marta, ws, &t, json!({"title": "A"})).await;
    let r = row["id"].as_str().unwrap().to_owned();
    let body = json!([{"property": p, "value": "5"}]);
    assert_eq!(
        kit.cells(&rui, ws, &t, &r, body.clone()).await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        kit.query(&rui, ws, &t, json!({})).await.status,
        StatusCode::NOT_FOUND
    );
    kit.share(&marta, ws, &t, rui_id, "reader").await;
    let denied = kit.cells(&rui, ws, &t, &r, body.clone()).await;
    assert_eq!(denied.status, StatusCode::FORBIDDEN);
    assert_eq!(
        kit.row_cell_count(ws).await,
        0,
        "the reader's write stored nothing"
    );
    assert_eq!(
        kit.query(&rui, ws, &t, json!({})).await.status,
        StatusCode::OK
    );
    let add = kit
        .post(
            &rui,
            &format!("{}/{t}/rows", Kit::tables(ws)),
            Some(json!({"title": "B"})),
        )
        .await;
    assert_eq!(add.status, StatusCode::FORBIDDEN);
    // A write of the writer goes through.
    kit.share(&marta, ws, &t, rui_id, "writer").await;
    assert_eq!(
        kit.cells(&rui, ws, &t, &r, body).await.status,
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_relation_names_only_rows_of_its_table_that_the_caller_may_read(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let (marta, _) = kit.member("marta", ws);
    let (rui, _) = kit.member("rui", ws);
    let mine = kit.table(&marta, ws).await;
    let secret = kit
        .post(&marta, &Kit::tables(ws), Some(json!({"title": "Secret"})))
        .await
        .json;
    let (m, s) = (
        mine["page"]["id"].as_str().unwrap(),
        secret["page"]["id"].as_str().unwrap(),
    );
    let rel = kit
        .property(
            &marta,
            ws,
            m,
            json!({"name": "Link", "type": "relation", "config": {"table": s}}),
        )
        .await;
    let hidden = kit.row(&marta, ws, s, json!({"title": "Hidden"})).await;
    let row = kit.row(&rui, ws, m, json!({"title": "A"})).await;
    let set = kit
        .cells(
            &rui,
            ws,
            m,
            row["id"].as_str().unwrap(),
            json!([{"property": rel["property"]["id"], "value": [hidden["id"]]}]),
        )
        .await;
    assert_eq!(set.status, StatusCode::NOT_FOUND, "{:?}", set.json);
    // Nor can a row of another table be named.
    let other = kit
        .row(&marta, ws, m, json!({"title": "Wrong table"}))
        .await;
    let wrong = kit
        .cells(
            &rui,
            ws,
            m,
            row["id"].as_str().unwrap(),
            json!([{"property": rel["property"]["id"], "value": [other["id"]]}]),
        )
        .await;
    assert_ne!(wrong.status, StatusCode::OK);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn people_must_be_members_and_files_must_be_uploaded_to_the_row(db: PgPool) {
    let f = fixture(db).await;
    let person = f
        .kit
        .property(
            &f.marta,
            f.ws,
            &f.table,
            json!({"name": "Owner", "type": "person"}),
        )
        .await;
    let files = f
        .kit
        .property(
            &f.marta,
            f.ws,
            &f.table,
            json!({"name": "Docs", "type": "files"}),
        )
        .await;
    let outsider = uuid::Uuid::now_v7().to_string();
    let bad = f
        .kit
        .cells(
            &f.marta,
            f.ws,
            &f.table,
            &f.row,
            json!([{"property": person["property"]["id"], "value": [outsider]}]),
        )
        .await;
    assert_eq!(bad.json["code"], "not_a_member");
    let file =
        json!({"hash": "a".repeat(64), "name": "a.pdf", "size": 3, "type": "application/pdf"});
    let bad = f
        .kit
        .cells(
            &f.marta,
            f.ws,
            &f.table,
            &f.row,
            json!([{"property": files["property"]["id"], "value": [file]}]),
        )
        .await;
    assert_eq!(bad.json["code"], "foreign_blob");
}
