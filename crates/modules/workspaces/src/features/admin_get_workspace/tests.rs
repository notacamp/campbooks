use axum::http::StatusCode;
use sqlx::PgPool;

use crate::testkit::Kit;

#[sqlx::test(migrations = "../../../migrations")]
async fn shows_members_and_counts_pending_invitations_without_addresses(db: PgPool) {
    let kit = Kit::new(db).await;
    let ana = kit.user("ana");
    let shop = kit.create_workspace(&ana, "Shop").await;
    kit.join(&ana, shop, "bea", "admin").await;
    kit.invite(&ana, shop, "dan@example.com", "member").await;
    let root = kit.admin("root");

    let response = kit
        .admin_get(&root, &format!("/admin/v1/workspaces/workspaces/{shop}"))
        .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.json);
    let body = &response.json;
    assert_eq!(body["workspace"]["name"], "Shop");
    assert_eq!(body["workspace"]["member_count"], 2);
    assert_eq!(body["pending_invitations"], 1);
    let members: Vec<_> = body["members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| (m["email"].as_str().unwrap(), m["role"].as_str().unwrap()))
        .collect();
    assert_eq!(
        members,
        [("ana@example.com", "owner"), ("bea@example.com", "admin")]
    );
    assert!(!body.to_string().contains("dan@example.com"));

    let unknown = kit
        .admin_get(
            &root,
            &format!("/admin/v1/workspaces/workspaces/{}", uuid::Uuid::now_v7()),
        )
        .await;
    assert_eq!(unknown.status, StatusCode::NOT_FOUND);
    let member = kit
        .admin_get(
            &kit.admin("ana"),
            &format!("/admin/v1/workspaces/workspaces/{shop}"),
        )
        .await;
    assert_eq!(member.status, StatusCode::NOT_FOUND);
}
