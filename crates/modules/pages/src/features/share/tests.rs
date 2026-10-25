//! Sharing a page.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::{UserId, WorkspaceId};
use serde_json::json;
use sqlx::PgPool;

use crate::testkit::Kit;

#[sqlx::test(migrations = "../../../migrations")]
async fn a_grant_reaches_sub_pages_and_gives_the_role_named(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (rui, rui_id) = kit.member("rui", workspace);
    let plan = kit
        .add(&ana, workspace, json!({"title": "Weekly plan"}))
        .await;
    let notes = kit
        .add(
            &ana,
            workspace,
            json!({"title": "Notes", "parent": plan["id"]}),
        )
        .await;
    let uri = Kit::pages(workspace);
    let plan_uri = format!("{uri}/{}", plan["id"].as_str().unwrap());
    let notes_uri = format!("{uri}/{}", notes["id"].as_str().unwrap());

    // Before: not there.
    assert_eq!(kit.get(&rui, &plan_uri).await.status, StatusCode::NOT_FOUND);

    let shared = kit
        .put(
            &ana,
            &format!("{plan_uri}/grants/{rui_id}"),
            json!({"role": "reader"}),
        )
        .await;
    assert_eq!(shared.status, StatusCode::OK, "{:?}", shared.json);
    assert_eq!(
        shared.json,
        json!({"member": rui_id.to_string(), "role": "reader"})
    );

    // After: he reads the page and its sub-page, and cannot write.
    for page in [&plan_uri, &notes_uri] {
        let got = kit.get(&rui, page).await;
        assert_eq!(got.status, StatusCode::OK, "{page}");
        assert_eq!(got.json["role"], "reader");
    }
    let rename = kit
        .patch(
            &rui,
            &notes_uri,
            json!({"meta_revision": 1, "title": "Mine now"}),
        )
        .await;
    assert_eq!(rename.status, StatusCode::FORBIDDEN);

    // The list of members on the page.
    let got = kit.get(&ana, &plan_uri).await;
    assert_eq!(
        got.json["grants"],
        json!([{"member": rui_id.to_string(), "role": "reader"}])
    );

    // Raised to writer, then taken away.
    kit.put(
        &ana,
        &format!("{plan_uri}/grants/{rui_id}"),
        json!({"role": "writer"}),
    )
    .await;
    let rename = kit
        .patch(
            &rui,
            &notes_uri,
            json!({"meta_revision": 1, "title": "Notes v2"}),
        )
        .await;
    assert_eq!(rename.status, StatusCode::OK, "{:?}", rename.json);
    let gone = kit
        .delete(&ana, &format!("{plan_uri}/grants/{rui_id}"))
        .await;
    assert_eq!(gone.status, StatusCode::NO_CONTENT);
    assert_eq!(kit.get(&rui, &plan_uri).await.status, StatusCode::NOT_FOUND);
    // Taking away what is not there is a 404.
    assert_eq!(
        kit.delete(&ana, &format!("{plan_uri}/grants/{rui_id}"))
            .await
            .status,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn only_members_and_only_by_someone_who_may_share(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let other = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    let (rui, rui_id) = kit.member("rui", workspace);
    let (_, outsider) = kit.member("jonas", other);
    let page = kit
        .add(
            &ana,
            workspace,
            json!({"title": "Plan", "place": "workspace"}),
        )
        .await;
    let uri = format!("{}/{}", Kit::pages(workspace), page["id"].as_str().unwrap());

    // Not a member of this workspace.
    let refused = kit
        .put(
            &ana,
            &format!("{uri}/grants/{outsider}"),
            json!({"role": "reader"}),
        )
        .await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST);
    assert_eq!(refused.json["code"], "not_a_member");
    let nobody = kit
        .put(
            &ana,
            &format!("{uri}/grants/{}", UserId::new()),
            json!({"role": "reader"}),
        )
        .await;
    assert_eq!(nobody.json["code"], "not_a_member");
    // Yourself.
    let myself = kit
        .put(
            &ana,
            &format!("{uri}/grants/{ana_id}"),
            json!({"role": "manager"}),
        )
        .await;
    assert_eq!(myself.json["code"], "share_yourself");
    // Rui writes the workspace page but does not share it.
    let denied = kit
        .put(
            &rui,
            &format!("{uri}/grants/{ana_id}"),
            json!({"role": "reader"}),
        )
        .await;
    assert_eq!(denied.status, StatusCode::FORBIDDEN);
    // And an unknown role is a bad body.
    let bad = kit
        .put(
            &ana,
            &format!("{uri}/grants/{rui_id}"),
            json!({"role": "owner"}),
        )
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM pages.grants")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert_eq!(rows, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_share_is_announced_and_its_events_hold_no_names(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (_, rui_id) = kit.member("rui", workspace);
    let page = kit
        .add(&ana, workspace, json!({"title": "Secret title"}))
        .await;
    let uri = format!("{}/{}", Kit::pages(workspace), page["id"].as_str().unwrap());
    kit.put(
        &ana,
        &format!("{uri}/grants/{rui_id}"),
        json!({"role": "reader"}),
    )
    .await;
    let events = kit.events("pages.page_changed").await;
    assert!(events.iter().any(|e| e["kinds"] == json!(["share"])));
    for event in events {
        assert!(!event.to_string().contains("Secret"));
        assert!(!event.to_string().contains(&rui_id.to_string()));
    }
}
