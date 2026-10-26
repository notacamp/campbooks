//! Options by id.

use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::json;
use sqlx::PgPool;

use crate::testkit::Kit;

#[sqlx::test(migrations = "../../../migrations")]
async fn renaming_an_option_changes_every_row_and_no_value(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let (marta, _) = kit.member("marta", ws);
    let t = kit.table(&marta, ws).await["page"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let made = kit
        .property(
            &marta,
            ws,
            &t,
            json!({"name": "Stage", "type": "select",
            "config": {"options": [{"name": "Kickoff"}, {"name": "Done"}]}}),
        )
        .await;
    let p = made["property"]["id"].as_str().unwrap().to_owned();
    let options = made["property"]["config"]["options"].clone();
    let kick = options[0]["id"].as_str().unwrap().to_owned();
    let row = kit
        .row(
            &marta,
            ws,
            &t,
            json!({"cells": [{"property": p, "value": kick}]}),
        )
        .await;
    let before = kit.query(&marta, ws, &t, json!({})).await.json["rows"][0]["cells"][&p].clone();

    let uri = format!("{}/{t}/properties/{p}/options", Kit::tables(ws));
    let renamed = kit
        .put(
            &marta,
            &uri,
            json!({"options": [
            {"id": kick, "name": "Onboarding", "color": "green"},
            {"id": options[1]["id"], "name": "Done"},
            {"name": "Paused"}]}),
        )
        .await;
    assert_eq!(
        renamed.status,
        axum::http::StatusCode::OK,
        "{:?}",
        renamed.json
    );
    assert_eq!(renamed.json["config"]["options"][0]["name"], "Onboarding");
    assert_eq!(
        renamed.json["config"]["options"].as_array().unwrap().len(),
        3
    );
    let after = kit.query(&marta, ws, &t, json!({})).await.json["rows"][0]["cells"][&p].clone();
    assert_eq!(before, after, "no row value changed, version included");
    assert_eq!(after["value"], kick);
    assert_eq!(row["cells"][&p]["version"], 1);

    // Taking an option out keeps the ids in the cells; sending it back (the
    // undo) shows them again.
    let without = kit
        .put(
            &marta,
            &uri,
            json!({"options": [{"id": options[1]["id"], "name": "Done"}]}),
        )
        .await;
    assert_eq!(
        without.json["config"]["options"].as_array().unwrap().len(),
        1
    );
    let kept = kit.query(&marta, ws, &t, json!({})).await.json["rows"][0]["cells"][&p].clone();
    assert_eq!(kept, after);
    for bad in [
        json!({"options": [{"name": "A"}, {"name": "a"}]}),
        json!({"options": [{"name": "A", "color": "mauve"}]}),
    ] {
        let refused = kit.put(&marta, &uri, bad).await;
        assert_eq!(refused.status, axum::http::StatusCode::BAD_REQUEST);
        assert_eq!(refused.json["code"], "invalid_config");
    }
}
