//! Listing the pages a Notion connection can see: only for the member whose
//! Notion it is, only what they shared, with the sub-page counts the import
//! dialog shows.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::{Role, WorkspaceId};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::testkit::Kit;

fn uri(ws: WorkspaceId, connection: &Value) -> String {
    format!(
        "/api/v1/w/{ws}/connections/{}/notion/pages",
        connection["id"].as_str().unwrap()
    )
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_owner_sees_the_shared_pages_with_their_sub_page_counts(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let ws = WorkspaceId::new();
    let marta = kit.user("marta");
    kit.join(&marta, ws, Role::Member);
    let connection = kit.connect_notion(&marta, ws).await;
    let roadmap = kit.notion.add_page("Product roadmap", None);
    let q4 = kit.notion.add_page("Q4 plan", Some(roadmap));
    let notes = kit.notion.add_page("Notes", Some(roadmap));
    let secret = kit.notion.add_page("Not shared", None);
    kit.notion.unshare(secret);
    let listed = kit.get(&marta, &uri(ws, &connection)).await;
    assert_eq!(listed.status, StatusCode::OK, "{:?}", listed.json);
    let pages = listed.json["pages"].as_array().unwrap();
    assert_eq!(pages.len(), 3);
    let by_id = |id: Uuid| {
        pages
            .iter()
            .find(|p| p["notion_page"] == id.to_string())
            .unwrap_or_else(|| panic!("{id} is listed"))
    };
    assert_eq!(by_id(roadmap)["sub_pages"], 2);
    assert_eq!(by_id(q4)["parent"], roadmap.to_string());
    assert_eq!(by_id(notes)["sub_pages"], 0);
    assert_eq!(by_id(roadmap)["parent"], Value::Null);
    assert_eq!(by_id(roadmap)["linked"], false);
    assert!(!pages.iter().any(|p| p["notion_page"] == secret.to_string()));
    assert_eq!(listed.json["truncated"], false);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_linked_page_says_so(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let ws = WorkspaceId::new();
    let marta = kit.user("marta");
    kit.join(&marta, ws, Role::Member);
    let connection = kit.connect_notion(&marta, ws).await;
    let roadmap = kit.notion.add_page("Product roadmap", None);
    kit.link_notion(ws, &connection, Uuid::now_v7(), roadmap).await;
    let listed = kit.get(&marta, &uri(ws, &connection)).await;
    assert_eq!(listed.json["pages"][0]["linked"], true);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_long_listing_is_followed_across_calls(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let ws = WorkspaceId::new();
    let marta = kit.user("marta");
    kit.join(&marta, ws, Role::Member);
    let connection = kit.connect_notion(&marta, ws).await;
    for n in 0..5 {
        kit.notion.add_page(&format!("Page {n}"), None);
    }
    kit.notion.page_size(2);
    let listed = kit.get(&marta, &uri(ws, &connection)).await;
    assert_eq!(listed.json["pages"].as_array().unwrap().len(), 5);
    assert_eq!(listed.json["requests"], 3);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn others_cannot_list_a_connection_that_is_not_theirs(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let ws = WorkspaceId::new();
    let marta = kit.user("marta");
    kit.join(&marta, ws, Role::Member);
    let rui = kit.user("rui");
    kit.join(&rui, ws, Role::Member);
    let admin = kit.user("admin");
    kit.join(&admin, ws, Role::Admin);
    let connection = kit.connect_notion(&marta, ws).await;
    kit.notion.add_page("Product roadmap", None);
    // A member who may not see it is told there is no such connection.
    let other = kit.get(&rui, &uri(ws, &connection)).await;
    assert_eq!(other.status, StatusCode::NOT_FOUND, "{:?}", other.json);
    assert!(!other.json.to_string().contains("Product roadmap"));
    // An admin, or a client token, who may see it, may not list its pages.
    let admin = kit.get(&admin, &uri(ws, &connection)).await;
    assert_eq!(admin.status, StatusCode::FORBIDDEN, "{:?}", admin.json);
    let client = kit.get(&kit.client(ws), &uri(ws, &connection)).await;
    assert!(
        matches!(client.status, StatusCode::FORBIDDEN | StatusCode::NOT_FOUND),
        "{:?}",
        client.json
    );
    // Not across workspaces.
    let elsewhere = WorkspaceId::new();
    kit.join(&rui, elsewhere, Role::Member);
    let crossed = kit.get(&rui, &uri(elsewhere, &connection)).await;
    assert_eq!(crossed.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_google_connection_has_no_notion_pages(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let ws = WorkspaceId::new();
    let marta = kit.user("marta");
    kit.join(&marta, ws, Role::Member);
    let google = kit.connect(&marta, ws).await;
    let listed = kit.get(&marta, &uri(ws, &google)).await;
    assert_eq!(listed.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_revoked_integration_asks_the_member_to_connect_again(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let ws = WorkspaceId::new();
    let marta = kit.user("marta");
    kit.join(&marta, ws, Role::Member);
    let connection = kit.connect_notion(&marta, ws).await;
    kit.notion.revoke_access();
    let listed = kit.get(&marta, &uri(ws, &connection)).await;
    assert_eq!(listed.status, StatusCode::CONFLICT, "{:?}", listed.json);
    assert_eq!(listed.json["code"], "needs_reauth");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn notion_trouble_reaches_the_member_as_a_short_code(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let ws = WorkspaceId::new();
    let marta = kit.user("marta");
    kit.join(&marta, ws, Role::Member);
    let connection = kit.connect_notion(&marta, ws).await;
    kit.notion.throttle_next(42);
    let listed = kit.get(&marta, &uri(ws, &connection)).await;
    assert_eq!(listed.status, StatusCode::TOO_MANY_REQUESTS, "{:?}", listed.json);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn it_is_conflict_when_notion_is_not_offered(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let marta = kit.user("marta");
    kit.join(&marta, ws, Role::Member);
    let missing = serde_json::json!({"id": Uuid::now_v7().to_string()});
    let listed = kit.get(&marta, &uri(ws, &missing)).await;
    assert!(
        matches!(listed.status, StatusCode::CONFLICT | StatusCode::NOT_FOUND),
        "{:?}",
        listed.json
    );
}
