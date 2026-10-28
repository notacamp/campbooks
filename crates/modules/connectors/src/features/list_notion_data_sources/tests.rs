//! Listing the Notion databases a connection can see: only for the member
//! whose Notion it is, only what they shared, only with the flag on.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::{Role, WorkspaceId};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::testkit::Kit;

fn uri(ws: WorkspaceId, connection: &Value) -> String {
    format!(
        "/api/v1/w/{ws}/connections/{}/notion/data-sources",
        connection["id"].as_str().unwrap()
    )
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_owner_sees_the_shared_databases_and_not_the_rest(db: PgPool) {
    let kit = Kit::with_notion_databases(db).await;
    let ws = WorkspaceId::new();
    let marta = kit.user("marta");
    kit.join(&marta, ws, Role::Member);
    let connection = kit.connect_notion(&marta, ws).await;
    let clients = kit.notion.add_data_source("Clients");
    kit.notion
        .add_property(clients, "Stage", "select", &json!({}));
    let projects = kit.notion.add_data_source("Projects");
    let private = kit.notion.add_data_source("Not shared");
    kit.notion.unshare_data_source(private);
    let trashed = kit.notion.add_data_source("Trashed");
    kit.notion.trash_data_source(trashed);
    // A page is not a database.
    kit.notion.add_page("Product roadmap", None);
    let listed = kit.get(&marta, &uri(ws, &connection)).await;
    assert_eq!(listed.status, StatusCode::OK, "{:?}", listed.json);
    let sources = listed.json["data_sources"].as_array().unwrap();
    assert_eq!(sources.len(), 2, "{sources:?}");
    let by_id = |id: Uuid| {
        sources
            .iter()
            .find(|s| s["data_source"] == id.to_string())
            .unwrap_or_else(|| panic!("{id} is listed"))
    };
    assert_eq!(by_id(clients)["title"], "Clients");
    assert_eq!(by_id(clients)["columns"], 2);
    assert_eq!(by_id(projects)["columns"], 1);
    assert_eq!(by_id(clients)["linked"], false);
    assert_eq!(listed.json["truncated"], false);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_linked_database_says_so(db: PgPool) {
    let kit = Kit::with_notion_databases(db).await;
    let ws = WorkspaceId::new();
    let marta = kit.user("marta");
    kit.join(&marta, ws, Role::Member);
    let connection = kit.connect_notion(&marta, ws).await;
    let clients = kit.notion.add_data_source("Clients");
    kit.notion.add_data_source("Projects");
    kit.link_notion_table(ws, &connection, Uuid::now_v7(), clients)
        .await;
    let listed = kit.get(&marta, &uri(ws, &connection)).await;
    assert_eq!(listed.status, StatusCode::OK, "{:?}", listed.json);
    let sources = listed.json["data_sources"].as_array().unwrap();
    let linked: Vec<_> = sources
        .iter()
        .filter(|s| s["linked"] == true)
        .map(|s| s["data_source"].clone())
        .collect();
    assert_eq!(linked, vec![json!(clients.to_string())]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_long_listing_is_followed_across_calls_and_bounded(db: PgPool) {
    let kit = Kit::with_notion_databases(db).await;
    let ws = WorkspaceId::new();
    let marta = kit.user("marta");
    kit.join(&marta, ws, Role::Member);
    let connection = kit.connect_notion(&marta, ws).await;
    for n in 0..5 {
        kit.notion.add_data_source(&format!("Database {n}"));
    }
    kit.notion.page_size(2);
    let listed = kit.get(&marta, &uri(ws, &connection)).await;
    assert_eq!(listed.json["data_sources"].as_array().unwrap().len(), 5);
    assert_eq!(listed.json["requests"], 3);
    assert_eq!(listed.json["truncated"], false);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn others_cannot_list_a_connection_that_is_not_theirs(db: PgPool) {
    let kit = Kit::with_notion_databases(db).await;
    let ws = WorkspaceId::new();
    let marta = kit.user("marta");
    kit.join(&marta, ws, Role::Member);
    let rui = kit.user("rui");
    kit.join(&rui, ws, Role::Member);
    let admin = kit.user("admin");
    kit.join(&admin, ws, Role::Admin);
    let connection = kit.connect_notion(&marta, ws).await;
    kit.notion.add_data_source("Clients");
    let other = kit.get(&rui, &uri(ws, &connection)).await;
    assert_eq!(other.status, StatusCode::NOT_FOUND, "{:?}", other.json);
    assert!(!other.json.to_string().contains("Clients"));
    let admin = kit.get(&admin, &uri(ws, &connection)).await;
    assert_eq!(admin.status, StatusCode::FORBIDDEN, "{:?}", admin.json);
    let client = kit.get(&kit.client(ws), &uri(ws, &connection)).await;
    assert!(
        matches!(client.status, StatusCode::FORBIDDEN | StatusCode::NOT_FOUND),
        "{:?}",
        client.json
    );
    let elsewhere = WorkspaceId::new();
    kit.join(&rui, elsewhere, Role::Member);
    let crossed = kit.get(&rui, &uri(elsewhere, &connection)).await;
    assert_eq!(crossed.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn with_databases_off_there_is_nothing_to_offer(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let ws = WorkspaceId::new();
    let marta = kit.user("marta");
    kit.join(&marta, ws, Role::Member);
    let connection = kit.connect_notion(&marta, ws).await;
    kit.notion.add_data_source("Clients");
    let listed = kit.get(&marta, &uri(ws, &connection)).await;
    assert_eq!(listed.status, StatusCode::OK, "{:?}", listed.json);
    assert_eq!(listed.json["data_sources"], json!([]));
    assert_eq!(listed.json["requests"], 0);
    // Still the owner's alone.
    let rui = kit.user("rui");
    kit.join(&rui, ws, Role::Member);
    let other = kit.get(&rui, &uri(ws, &connection)).await;
    assert_eq!(other.status, StatusCode::NOT_FOUND, "{:?}", other.json);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_revoked_integration_asks_the_member_to_connect_again(db: PgPool) {
    let kit = Kit::with_notion_databases(db).await;
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
async fn a_throttled_notion_is_a_wait(db: PgPool) {
    let kit = Kit::with_notion_databases(db).await;
    let ws = WorkspaceId::new();
    let marta = kit.user("marta");
    kit.join(&marta, ws, Role::Member);
    let connection = kit.connect_notion(&marta, ws).await;
    kit.notion.throttle_next(42);
    let listed = kit.get(&marta, &uri(ws, &connection)).await;
    assert_eq!(
        listed.status,
        StatusCode::TOO_MANY_REQUESTS,
        "{:?}",
        listed.json
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_google_connection_has_no_notion_databases(db: PgPool) {
    let kit = Kit::with_notion_databases(db).await;
    let ws = WorkspaceId::new();
    let marta = kit.user("marta");
    kit.join(&marta, ws, Role::Member);
    let google = kit.connect(&marta, ws).await;
    let listed = kit.get(&marta, &uri(ws, &google)).await;
    assert_eq!(listed.status, StatusCode::NOT_FOUND);
}
