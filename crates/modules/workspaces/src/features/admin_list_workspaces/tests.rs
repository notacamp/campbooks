use axum::http::StatusCode;
use sqlx::PgPool;

use crate::testkit::Kit;

#[sqlx::test(migrations = "../../../migrations")]
async fn administrators_see_every_workspace_with_members_and_owners(db: PgPool) {
    let kit = Kit::new(db).await;
    let ana = kit.user("ana");
    let shop = kit.create_workspace(&ana, "Shop").await;
    kit.join(&ana, shop, "bea", "admin").await;
    let bea = kit.user_id("bea").await;
    let promoted = kit
        .patch(
            &ana,
            &format!("/api/v1/w/{shop}/members/{bea}"),
            serde_json::json!({ "role": "owner" }),
        )
        .await;
    assert_eq!(promoted.status, StatusCode::OK, "{}", promoted.json);
    kit.join(&ana, shop, "cid", "member").await;
    let home = kit.create_workspace(&kit.user("eve"), "Home").await;
    let root = kit.admin("root");

    let response = kit
        .admin_get(&root, "/admin/v1/workspaces/workspaces")
        .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.json);
    let list = response.json["workspaces"].as_array().unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0]["id"], home.to_string());
    assert_eq!(list[0]["member_count"], 1);
    assert_eq!(list[1]["id"], shop.to_string());
    assert_eq!(list[1]["name"], "Shop");
    assert_eq!(list[1]["member_count"], 3);
    let owners = [kit.user_id("ana").await, bea].map(|id| serde_json::Value::from(id.to_string()));
    assert_eq!(list[1]["owner_ids"], serde_json::json!(owners));

    let found = kit
        .admin_get(&root, "/admin/v1/workspaces/workspaces?q=sh&limit=1")
        .await;
    let found = found.json["workspaces"].as_array().unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0]["id"], shop.to_string());

    // `users.read` is not `workspaces.read`.
    let response = kit
        .admin_get(&kit.admin("people"), "/admin/v1/workspaces/workspaces")
        .await;
    assert_eq!(response.status, StatusCode::NOT_FOUND);
}
