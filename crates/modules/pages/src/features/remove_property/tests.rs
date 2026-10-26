//! Removing properties.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::json;
use sqlx::PgPool;

use crate::testkit::Kit;

#[sqlx::test(migrations = "../../../migrations")]
async fn a_removed_property_keeps_its_values_and_comes_back(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let (marta, _) = kit.member("marta", ws);
    let table = kit.table(&marta, ws).await;
    let t = table["page"]["id"].as_str().unwrap();
    let title = table["properties"][0]["id"].as_str().unwrap();
    let p = kit
        .property(&marta, ws, t, json!({"name": "Value", "type": "number"}))
        .await;
    let p = p["property"]["id"].as_str().unwrap().to_owned();
    let row = kit
        .row(
            &marta,
            ws,
            t,
            json!({"cells": [{"property": p, "value": "9"}]}),
        )
        .await;
    let uri = format!("{}/{t}/properties/{p}", Kit::tables(ws));
    assert_eq!(kit.delete(&marta, &uri).await.status, StatusCode::OK);
    let gone = kit.get(&marta, &format!("{}/{t}", Kit::tables(ws))).await;
    assert_eq!(gone.json["properties"].as_array().unwrap().len(), 1);
    let q = kit.query(&marta, ws, t, json!({})).await;
    assert!(q.json["rows"][0]["cells"].get(&p).is_none());
    // A removed property is not writable or filterable.
    let write = kit
        .cells(
            &marta,
            ws,
            t,
            row["id"].as_str().unwrap(),
            json!([{"property": p, "value": "1"}]),
        )
        .await;
    assert_eq!(write.json["code"], "unknown_property");
    assert_eq!(kit.delete(&marta, &uri).await.status, StatusCode::NOT_FOUND);
    let back = kit.post(&marta, &format!("{uri}/restore"), None).await;
    assert_eq!(back.status, StatusCode::OK);
    let q = kit.query(&marta, ws, t, json!({})).await;
    assert_eq!(q.json["rows"][0]["cells"][&p]["value"], "9");
    // The title stays.
    let t_uri = format!("{}/{t}/properties/{title}", Kit::tables(ws));
    let refused = kit.delete(&marta, &t_uri).await;
    assert_eq!(refused.status, StatusCode::CONFLICT);
    assert_eq!(refused.json["code"], "title_property");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_view_that_names_a_removed_property_still_answers(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let (marta, _) = kit.member("marta", ws);
    let table = kit.table(&marta, ws).await;
    let t = table["page"]["id"].as_str().unwrap();
    let view = table["views"][0]["id"].as_str().unwrap();
    let p = kit
        .property(&marta, ws, t, json!({"name": "Value", "type": "number"}))
        .await;
    let p = p["property"]["id"].as_str().unwrap().to_owned();
    kit.row(
        &marta,
        ws,
        t,
        json!({"cells": [{"property": p, "value": "9"}]}),
    )
    .await;
    let set = kit
        .patch(
            &marta,
            &format!("{}/{t}/views/{view}", Kit::tables(ws)),
            json!({"config": {"filter": {"and": [{"property": p, "operator": "gt", "value": "5"}]},
                "sorts": [{"property": p, "direction": "desc"}],
                "summaries": [{"property": p, "kind": "sum"}]}}),
        )
        .await;
    assert_eq!(set.status, StatusCode::OK, "{:?}", set.json);
    kit.delete(&marta, &format!("{}/{t}/properties/{p}", Kit::tables(ws)))
        .await;
    let q = kit.query(&marta, ws, t, json!({"view": view})).await;
    assert_eq!(q.status, StatusCode::OK, "{:?}", q.json);
    assert_eq!(q.json["matching"], 1);
    assert_eq!(q.json["summaries"], json!([]));
}
