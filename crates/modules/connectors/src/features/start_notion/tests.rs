//! Starting a Notion connection: members only, their own connection only,
//! and a `return_to` that stays inside the app.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::{Role, WorkspaceId};
use serde_json::json;
use sqlx::PgPool;

use crate::testkit::Kit;

#[sqlx::test(migrations = "../../../migrations")]
async fn a_member_starts_a_consent_and_gets_the_instances_own_start_address(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    let started = kit.notion_start(&ana, ws, json!({})).await;
    assert_eq!(started.status, StatusCode::CREATED, "{:?}", started.json);
    let url = started.json["authorize_url"].as_str().unwrap();
    assert!(url.starts_with("/auth/start/notion?ticket="), "{url}");
    assert!(started.json["expires_at"].is_string());
    // The consent is recorded, unarmed, with no secret in it yet.
    let (armed, count): (bool, i64) = sqlx::query_as(
        "SELECT bool_or(state_hash IS NOT NULL), count(*) FROM connectors.notion_authorizations",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!((armed, count), (false, 1));
    assert!(!url.contains(&ws.to_string()), "the ticket names nothing: {url}");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_client_token_never_starts_one(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let ws = WorkspaceId::new();
    let started = kit.notion_start(&kit.client(ws), ws, json!({})).await;
    assert_eq!(started.status, StatusCode::FORBIDDEN, "{:?}", started.json);
    assert_eq!(started.json["code"], "connection_needs_user");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn it_is_unavailable_when_notion_is_not_offered(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    let started = kit.notion_start(&ana, ws, json!({})).await;
    assert_eq!(started.status, StatusCode::CONFLICT, "{:?}", started.json);
    assert_eq!(started.json["code"], "provider_not_configured");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_return_to_outside_the_app_is_refused(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    for bad in [
        "https://evil.example.com/",
        "//evil.example.com/x",
        "/\\evil.example.com",
        "javascript:alert(1)",
        "settings",
        "/ok\r\nSet-Cookie: x=1",
    ] {
        let started = kit.notion_start(&ana, ws, json!({"return_to": bad})).await;
        assert_eq!(started.status, StatusCode::BAD_REQUEST, "{bad}: {:?}", started.json);
        assert_eq!(started.json["code"], "invalid_return_to");
    }
    let ok = kit
        .notion_start(&ana, ws, json!({"return_to": "/w/x/settings/mailboxes"}))
        .await;
    assert_eq!(ok.status, StatusCode::CREATED);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn only_the_grantor_connects_their_notion_again(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    let bea = kit.user("bea");
    kit.join(&bea, ws, Role::Member);
    let connection = kit.connect_notion(&ana, ws).await;
    let id = connection["id"].clone();
    let theirs = kit
        .notion_start(&bea, ws, json!({"connection": id}))
        .await;
    assert_eq!(theirs.status, StatusCode::NOT_FOUND, "{:?}", theirs.json);
    let own = kit.notion_start(&ana, ws, json!({"connection": id})).await;
    assert_eq!(own.status, StatusCode::CREATED, "{:?}", own.json);
    // Not a connection that is not Notion's either.
    let google = kit.connect(&ana, ws).await;
    let wrong = kit
        .notion_start(&ana, ws, json!({"connection": google["id"]}))
        .await;
    assert_eq!(wrong.status, StatusCode::NOT_FOUND);
}
