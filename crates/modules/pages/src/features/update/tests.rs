//! Renaming a page.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::json;
use sqlx::PgPool;

use crate::testkit::Kit;

#[sqlx::test(migrations = "../../../migrations")]
async fn a_title_and_icon_change_together_and_the_event_holds_neither(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let page = kit.add(&ana, workspace, json!({"title": "Draft"})).await;
    let uri = format!("{}/{}", Kit::pages(workspace), page["id"].as_str().unwrap());
    let renamed = kit
        .patch(
            &ana,
            &uri,
            json!({"meta_revision": 1, "title": "Weekly plan", "icon": "tasks"}),
        )
        .await;
    assert_eq!(renamed.status, StatusCode::OK, "{:?}", renamed.json);
    assert_eq!(renamed.json["title"], "Weekly plan");
    assert_eq!(renamed.json["icon"], "tasks");
    assert_eq!(renamed.json["meta_revision"], 2);

    let events = kit.events("pages.page_changed").await;
    assert_eq!(events.len(), 2);
    assert_eq!(events[1]["kinds"], json!(["meta"]));
    assert_eq!(events[1]["revision"], 2);
    assert!(!events[1].to_string().contains("Weekly"));

    // Sending what it already is changes nothing and announces nothing.
    let same = kit
        .patch(
            &ana,
            &uri,
            json!({"meta_revision": 2, "title": "Weekly plan", "icon": "tasks"}),
        )
        .await;
    assert_eq!(same.status, StatusCode::OK);
    assert_eq!(same.json["meta_revision"], 2);
    assert_eq!(kit.events("pages.page_changed").await.len(), 2);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_stale_revision_a_bad_icon_and_a_reader_are_refused(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (rui, rui_id) = kit.member("rui", workspace);
    let page = kit.add(&ana, workspace, json!({"title": "Draft"})).await;
    let id = page["id"].as_str().unwrap();
    let uri = format!("{}/{id}", Kit::pages(workspace));
    let body =
        |revision: i64, icon: &str| json!({"meta_revision": revision, "title": "x", "icon": icon});

    let stale = kit.patch(&ana, &uri, body(7, "star")).await;
    assert_eq!(stale.status, StatusCode::CONFLICT);
    assert_eq!(stale.json["code"], "page_changed");
    let icon = kit.patch(&ana, &uri, body(1, "rocket")).await;
    assert_eq!(icon.json["code"], "invalid_icon");

    // Not shared: not there. Shared as a reader: seen, not writable.
    assert_eq!(
        kit.patch(&rui, &uri, body(1, "star")).await.status,
        StatusCode::NOT_FOUND
    );
    kit.put(
        &ana,
        &format!("{uri}/grants/{rui_id}"),
        json!({"role": "reader"}),
    )
    .await;
    assert_eq!(
        kit.patch(&rui, &uri, body(1, "star")).await.status,
        StatusCode::FORBIDDEN
    );
    let still = kit.get(&ana, &uri).await;
    assert_eq!(still.json["title"], "Draft");
}
