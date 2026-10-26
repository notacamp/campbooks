//! Adding properties.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::json;
use sqlx::PgPool;

use crate::testkit::Kit;

#[sqlx::test(migrations = "../../../migrations")]
async fn every_type_a_member_may_make_can_be_made_and_the_reserved_ones_cannot(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let ws = WorkspaceId::new();
    let (marta, _) = kit.member("marta", ws);
    let table = kit.table(&marta, ws).await;
    let id = table["page"]["id"].as_str().unwrap();
    let other = kit.table(&marta, ws).await;
    let uri = format!("{}/{id}/properties", Kit::tables(ws));
    for ty in [
        "text",
        "number",
        "select",
        "multi_select",
        "status",
        "date",
        "checkbox",
        "person",
        "files",
        "url",
        "email",
        "phone",
        "campbooks_link",
        "created",
        "edited",
        "unique_id",
    ] {
        let made = kit
            .post(
                &marta,
                &uri,
                Some(json!({"name": format!("P {ty}"), "type": ty})),
            )
            .await;
        assert_eq!(made.status, StatusCode::CREATED, "{ty}: {:?}", made.json);
        assert_eq!(made.json["property"]["type"], ty);
    }
    let relation = kit
        .post(
            &marta,
            &uri,
            Some(json!({"name": "R", "type": "relation",
                "config": {"table": other["page"]["id"]}})),
        )
        .await;
    assert_eq!(relation.status, StatusCode::CREATED, "{:?}", relation.json);
    for ty in [
        "title",
        "notion_formula",
        "notion_rollup",
        "notion_people",
        "external",
    ] {
        let refused = kit
            .post(&marta, &uri, Some(json!({"name": "X", "type": ty})))
            .await;
        assert_eq!(refused.status, StatusCode::BAD_REQUEST, "{ty}");
        assert_eq!(refused.json["code"], "property_type_reserved");
    }
    for bad in [
        json!({"name": "", "type": "text"}),
        json!({"name": "x".repeat(101), "type": "text"}),
        json!({"name": "A", "type": "number", "config": {"format": "currency"}}),
        json!({"name": "A", "type": "text", "config": {"x": 1}}),
        json!({"name": "A", "type": "text", "paired_name": "B"}),
        json!({"name": "A", "type": "relation", "config": {"table": "nope"}}),
    ] {
        let refused = kit.post(&marta, &uri, Some(bad.clone())).await;
        assert_eq!(refused.status, StatusCode::BAD_REQUEST, "{bad}");
    }
    let tracked = campbooks_kernel::testing::tracked_events(&kit.db, "pages.property_added").await;
    assert_eq!(tracked.len(), 17);
    assert!(
        !tracked
            .iter()
            .any(|e| e.body.to_string().contains("P text"))
    );
    let events = kit.events("pages.table_changed").await;
    assert!(!events.is_empty());
    assert!(!events.iter().any(|e| e.to_string().contains("P text")));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_paired_relation_makes_its_other_side_and_both_are_written_together(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let (marta, _) = kit.member("marta", ws);
    let clients = kit.table(&marta, ws).await;
    let projects = kit.table(&marta, ws).await;
    let (c, p) = (
        clients["page"]["id"].as_str().unwrap(),
        projects["page"]["id"].as_str().unwrap(),
    );
    let made = kit
        .property(
            &marta,
            ws,
            c,
            json!({"name": "Projects", "type": "relation", "config": {"table": p},
                "paired_name": "Client"}),
        )
        .await;
    let forward = made["property"]["id"].as_str().unwrap();
    let back = made["paired"]["id"].as_str().unwrap();
    assert_eq!(made["paired"]["config"]["pair"], forward);
    assert_eq!(made["property"]["config"]["pair"], back);

    let client = kit.row(&marta, ws, c, json!({"title": "Northwind"})).await;
    let project = kit.row(&marta, ws, p, json!({"title": "Site"})).await;
    let (client_id, project_id) = (
        client["id"].as_str().unwrap(),
        project["id"].as_str().unwrap(),
    );
    let set = kit
        .cells(
            &marta,
            ws,
            c,
            client_id,
            json!([{"property": forward, "value": [project_id]}]),
        )
        .await;
    assert_eq!(set.status, StatusCode::OK, "{:?}", set.json);
    // The other side shows it.
    let rows = kit.query(&marta, ws, p, json!({})).await;
    assert_eq!(
        rows.json["rows"][0]["cells"][back]["value"],
        json!([client_id])
    );
    // And taking it away on this side takes it off the other.
    kit.cells(
        &marta,
        ws,
        c,
        client_id,
        json!([{"property": forward, "value": null}]),
    )
    .await;
    let rows = kit.query(&marta, ws, p, json!({})).await;
    assert!(rows.json["rows"][0]["cells"].get(back).is_none());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_table_has_at_most_200_properties(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let (marta, _) = kit.member("marta", ws);
    let table = kit.table(&marta, ws).await;
    let id = table["page"]["id"].as_str().unwrap();
    sqlx::query(
        "INSERT INTO pages.table_properties (id, table_id, workspace_id, name, type, position, created_by)
         SELECT gen_random_uuid(), $1::uuid, $2, 'p' || n, 'text', 'b' || lpad(n::text, 4, '0'), $3
           FROM generate_series(1, 199) n",
    )
    .bind(id.parse::<uuid::Uuid>().unwrap())
    .bind(ws.as_uuid())
    .bind(uuid::Uuid::now_v7())
    .execute(&kit.db)
    .await
    .unwrap();
    let over = kit
        .post(
            &marta,
            &format!("{}/{id}/properties", Kit::tables(ws)),
            Some(json!({"name": "one too many", "type": "text"})),
        )
        .await;
    assert_eq!(over.status, StatusCode::CONFLICT);
    assert_eq!(over.json["code"], "too_many_properties");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_reader_cannot_add_a_property_and_a_stranger_does_not_see_the_table(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let (marta, _) = kit.member("marta", ws);
    let (rui, rui_id) = kit.member("rui", ws);
    let made = kit
        .post(&marta, &Kit::tables(ws), Some(json!({"title": "Mine"})))
        .await;
    let id = made.json["page"]["id"].as_str().unwrap().to_owned();
    let uri = format!("{}/{id}/properties", Kit::tables(ws));
    let body = json!({"name": "X", "type": "text"});
    assert_eq!(
        kit.post(&rui, &uri, Some(body.clone())).await.status,
        StatusCode::NOT_FOUND
    );
    kit.share(&marta, ws, &id, rui_id, "reader").await;
    assert_eq!(
        kit.post(&rui, &uri, Some(body)).await.status,
        StatusCode::FORBIDDEN
    );
    let got = kit.get(&marta, &format!("{}/{id}", Kit::tables(ws))).await;
    assert_eq!(got.json["properties"].as_array().unwrap().len(), 1);
}
