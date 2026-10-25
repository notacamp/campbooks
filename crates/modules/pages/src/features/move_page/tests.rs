//! Moving pages.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::testkit::Kit;

fn summary(json: &Value) -> (Option<String>, String, String) {
    (
        json["parent"].as_str().map(ToOwned::to_owned),
        json["place"].as_str().unwrap().to_owned(),
        json["position"].as_str().unwrap().to_owned(),
    )
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_page_moves_under_another_and_back_with_the_undo(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let a = kit.add(&ana, workspace, json!({"title": "A"})).await;
    let b = kit.add(&ana, workspace, json!({"title": "B"})).await;
    let uri = Kit::pages(workspace);
    let b_uri = format!("{uri}/{}", b["id"].as_str().unwrap());

    let moved = kit
        .post(
            &ana,
            &format!("{b_uri}/move"),
            Some(json!({"meta_revision": 1, "parent": a["id"]})),
        )
        .await;
    assert_eq!(moved.status, StatusCode::OK, "{:?}", moved.json);
    let (parent, place, _) = summary(&moved.json);
    assert_eq!(parent.as_deref(), a["id"].as_str());
    assert_eq!(place, "private");
    assert_eq!(moved.json["meta_revision"], 2);

    // The undo is the same call with where it was.
    let back = kit
        .post(
            &ana,
            &format!("{b_uri}/move"),
            Some(json!({
                "meta_revision": 2, "parent": null, "place": "private",
                "position": b["position"],
            })),
        )
        .await;
    assert_eq!(back.status, StatusCode::OK, "{:?}", back.json);
    let (parent, _, position) = summary(&back.json);
    assert_eq!(parent, None);
    assert_eq!(position, b["position"].as_str().unwrap());

    // And the move was announced, with no title.
    let events = kit.events("pages.page_changed").await;
    assert!(events.iter().any(|e| e["kinds"] == json!(["move"])));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_page_never_goes_inside_itself(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let top = kit
        .add(&ana, workspace, json!({"title": "Team handbook"}))
        .await;
    let sub = kit
        .add(
            &ana,
            workspace,
            json!({"title": "Holidays", "parent": top["id"]}),
        )
        .await;
    let uri = Kit::pages(workspace);
    let top_uri = format!("{uri}/{}", top["id"].as_str().unwrap());
    for target in [&top["id"], &sub["id"]] {
        let refused = kit
            .post(
                &ana,
                &format!("{top_uri}/move"),
                Some(json!({"meta_revision": 1, "parent": target})),
            )
            .await;
        assert_eq!(refused.status, StatusCode::CONFLICT, "{:?}", refused.json);
        assert_eq!(refused.json["code"], "page_cycle");
    }
    // Nothing moved.
    let list = kit.get(&ana, &uri).await.json;
    let holidays = list["pages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|page| page["title"] == "Holidays")
        .unwrap();
    assert_eq!(holidays["parent"], top["id"]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_move_cannot_make_the_tree_deeper_than_twelve_levels(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let mut parent: Option<Value> = None;
    for level in 1..=11 {
        let body = match &parent {
            Some(parent) => json!({"title": format!("L{level}"), "parent": parent}),
            None => json!({"title": "L1"}),
        };
        parent = Some(kit.add(&ana, workspace, body).await["id"].clone());
    }
    // A page with one sub-page: two levels, which cannot go under level 11.
    let pair = kit.add(&ana, workspace, json!({"title": "Pair"})).await;
    kit.add(
        &ana,
        workspace,
        json!({"title": "Half", "parent": pair["id"]}),
    )
    .await;
    let uri = Kit::pages(workspace);
    let refused = kit
        .post(
            &ana,
            &format!("{uri}/{}/move", pair["id"].as_str().unwrap()),
            Some(json!({"meta_revision": 1, "parent": parent})),
        )
        .await;
    assert_eq!(refused.status, StatusCode::CONFLICT);
    assert_eq!(refused.json["code"], "page_too_deep");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn moving_between_places_needs_the_right_to_share_and_takes_sub_pages_along(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (rui, _) = kit.member("rui", workspace);
    let page = kit
        .add(
            &ana,
            workspace,
            json!({"title": "Plan", "place": "workspace"}),
        )
        .await;
    let child = kit
        .add(
            &ana,
            workspace,
            json!({"title": "Plan notes", "parent": page["id"]}),
        )
        .await;
    let uri = Kit::pages(workspace);
    let move_uri = format!("{uri}/{}/move", page["id"].as_str().unwrap());
    let to_private = json!({"meta_revision": 1, "parent": null, "place": "private"});

    // Rui writes the page but did not make it: he may reorder, not make it
    // private.
    let denied = kit.post(&rui, &move_uri, Some(to_private.clone())).await;
    assert_eq!(denied.status, StatusCode::FORBIDDEN);

    let moved = kit.post(&ana, &move_uri, Some(to_private)).await;
    assert_eq!(moved.status, StatusCode::OK, "{:?}", moved.json);
    assert_eq!(moved.json["place"], "private");
    // The sub-page went with it, and Rui no longer sees either.
    let seen = kit.get(&rui, &uri).await.json;
    assert_eq!(seen["pages"], json!([]));
    let child_now = kit
        .get(&ana, &format!("{uri}/{}", child["id"].as_str().unwrap()))
        .await;
    assert_eq!(child_now.json["place"], "private");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_writer_cannot_take_a_private_page_away_from_its_owner(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (rui, rui_id) = kit.member("rui", workspace);
    let uri = Kit::pages(workspace);
    let top = kit.add(&ana, workspace, json!({"title": "P"})).await;
    let sub = kit
        .add(&ana, workspace, json!({"title": "S", "parent": top["id"]}))
        .await;
    let mine = kit.add(&rui, workspace, json!({"title": "R"})).await;
    kit.put(
        &ana,
        &format!("{uri}/{}/grants/{rui_id}", top["id"].as_str().unwrap()),
        json!({"role": "writer"}),
    )
    .await;
    let refused = kit
        .post(
            &rui,
            &format!("{uri}/{}/move", sub["id"].as_str().unwrap()),
            Some(json!({"meta_revision": 1, "parent": mine["id"]})),
        )
        .await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN, "{:?}", refused.json);
    let still = kit
        .get(&ana, &format!("{uri}/{}", sub["id"].as_str().unwrap()))
        .await;
    assert_eq!(still.json["parent"], top["id"]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_writer_cannot_share_a_page_by_moving_it_under_a_shared_one(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (rui, rui_id) = kit.member("rui", workspace);
    let (carol, carol_id) = kit.member("carol", workspace);
    let uri = Kit::pages(workspace);
    let x = kit.add(&ana, workspace, json!({"title": "X"})).await;
    let q = kit.add(&rui, workspace, json!({"title": "Q"})).await;
    let x_uri = format!("{uri}/{}", x["id"].as_str().unwrap());
    kit.put(
        &ana,
        &format!("{x_uri}/grants/{rui_id}"),
        json!({"role": "writer"}),
    )
    .await;
    kit.put(
        &rui,
        &format!("{uri}/{}/grants/{carol_id}", q["id"].as_str().unwrap()),
        json!({"role": "reader"}),
    )
    .await;
    let refused = kit
        .post(
            &rui,
            &format!("{x_uri}/move"),
            Some(json!({"meta_revision": 1, "parent": q["id"]})),
        )
        .await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN, "{:?}", refused.json);
    assert_eq!(kit.get(&carol, &x_uri).await.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_writer_may_reorder_among_the_same_siblings(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (rui, rui_id) = kit.member("rui", workspace);
    let uri = Kit::pages(workspace);
    let top = kit.add(&ana, workspace, json!({"title": "P"})).await;
    let first = kit
        .add(&ana, workspace, json!({"title": "A", "parent": top["id"]}))
        .await;
    kit.add(&ana, workspace, json!({"title": "B", "parent": top["id"]}))
        .await;
    kit.put(
        &ana,
        &format!("{uri}/{}/grants/{rui_id}", top["id"].as_str().unwrap()),
        json!({"role": "writer"}),
    )
    .await;
    let moved = kit
        .post(
            &rui,
            &format!("{uri}/{}/move", first["id"].as_str().unwrap()),
            Some(json!({"meta_revision": 1, "parent": top["id"], "position": "a9"})),
        )
        .await;
    assert_eq!(moved.status, StatusCode::OK, "{:?}", moved.json);
    assert_eq!(moved.json["position"], "a9");
}
