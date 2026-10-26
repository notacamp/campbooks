//! Renaming and converting properties.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::json;
use sqlx::PgPool;

use crate::testkit::Kit;

#[sqlx::test(migrations = "../../../migrations")]
async fn a_change_of_type_converts_what_fits_and_restoring_puts_everything_back(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let (marta, _) = kit.member("marta", ws);
    let t = kit.table(&marta, ws).await["page"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let p = kit
        .property(&marta, ws, &t, json!({"name": "Value", "type": "text"}))
        .await;
    let p = p["property"]["id"].as_str().unwrap().to_owned();
    let mut rows = Vec::new();
    for text in ["12.5", "lots", "7"] {
        let row = kit
            .row(
                &marta,
                ws,
                &t,
                json!({"cells": [{"property": p, "value": text}]}),
            )
            .await;
        rows.push(row["id"].as_str().unwrap().to_owned());
    }
    let uri = format!("{}/{t}/properties/{p}", Kit::tables(ws));
    let changed = kit.patch(&marta, &uri, json!({"type": "number"})).await;
    assert_eq!(changed.status, StatusCode::OK, "{:?}", changed.json);
    assert_eq!(changed.json["type"], "number");
    assert_eq!(changed.json["can_revert"], true);
    let q = kit.query(&marta, ws, &t, json!({})).await;
    let values: Vec<_> = q.json["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["cells"].get(&p).map(|c| c["value"].clone()))
        .collect();
    assert_eq!(values, [Some(json!("12.5")), None, Some(json!("7"))]);

    let back = kit.post(&marta, &format!("{uri}/restore"), None).await;
    assert_eq!(back.status, StatusCode::OK, "{:?}", back.json);
    assert_eq!(back.json["type"], "text");
    assert_eq!(back.json["can_revert"], false);
    let q = kit.query(&marta, ws, &t, json!({})).await;
    let texts: Vec<_> = q.json["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| crate::properties::value::runs_text(&r["cells"][&p]["value"]))
        .collect();
    assert_eq!(texts, ["12.5", "lots", "7"]);
    let again = kit.post(&marta, &format!("{uri}/restore"), None).await;
    assert_eq!(again.status, StatusCode::CONFLICT);
    assert_eq!(again.json["code"], "nothing_to_restore");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_title_only_renames_and_reserved_types_cannot_be_changed_to(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let (marta, _) = kit.member("marta", ws);
    let table = kit.table(&marta, ws).await;
    let t = table["page"]["id"].as_str().unwrap();
    let title = table["properties"][0]["id"].as_str().unwrap();
    let uri = format!("{}/{t}/properties/{title}", Kit::tables(ws));
    let renamed = kit.patch(&marta, &uri, json!({"name": "Client"})).await;
    assert_eq!(renamed.json["name"], "Client");
    let retyped = kit.patch(&marta, &uri, json!({"type": "text"})).await;
    assert_eq!(retyped.json["code"], "property_not_editable");
    let p = kit
        .property(&marta, ws, t, json!({"name": "X", "type": "text"}))
        .await;
    let uri = format!(
        "{}/{t}/properties/{}",
        Kit::tables(ws),
        p["property"]["id"].as_str().unwrap()
    );
    for ty in ["title", "notion_formula", "external"] {
        let refused = kit.patch(&marta, &uri, json!({"type": ty})).await;
        assert_eq!(refused.json["code"], "property_type_reserved", "{ty}");
    }
    assert_eq!(
        kit.patch(&marta, &uri, json!({"name": ""})).await.status,
        StatusCode::BAD_REQUEST
    );
}
