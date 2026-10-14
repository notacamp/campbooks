use axum::http::StatusCode;
use sqlx::PgPool;

use crate::testkit::Kit;

#[sqlx::test(migrations = "../../../migrations")]
async fn shows_a_person_and_every_workspace_with_their_role(db: PgPool) {
    let kit = Kit::new(db).await;
    let ana = kit.user("ana");
    let shop = kit.create_workspace(&ana, "Shop").await;
    let gone = kit.create_workspace(&ana, "Gone").await;
    kit.delete(&ana, &format!("/api/v1/w/{gone}")).await;
    let bea = kit.user("bea");
    let home = kit.create_workspace(&bea, "Home").await;
    kit.join(&bea, home, "ana", "admin").await;
    let id = kit.user_id("ana").await;

    let response = kit
        .admin_get(
            &kit.admin("root"),
            &format!("/admin/v1/workspaces/users/{id}"),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.json);
    let body = &response.json;
    assert_eq!(body["user"]["account_id"], "ana");
    assert_eq!(body["user"]["workspace_count"], 2);
    let workspaces: Vec<_> = body["workspaces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| {
            (
                w["id"].as_str().unwrap().to_owned(),
                w["role"].as_str().unwrap().to_owned(),
                w["deleted_at"].is_null(),
            )
        })
        .collect();
    assert_eq!(
        workspaces,
        [
            (shop.to_string(), "owner".to_owned(), true),
            (gone.to_string(), "owner".to_owned(), false),
            (home.to_string(), "admin".to_owned(), true),
        ]
    );
    assert_eq!(body["workspaces"][2]["name"], "Home");
    assert_eq!(body["workspaces"][2]["kind"], "business");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_unknown_person_or_a_non_administrator_gets_404(db: PgPool) {
    let kit = Kit::new(db).await;
    kit.create_workspace(&kit.user("ana"), "Shop").await;
    let id = kit.user_id("ana").await;
    let root = kit.admin("root");
    for uri in [
        format!("/admin/v1/workspaces/users/{}", uuid::Uuid::now_v7()),
        "/admin/v1/workspaces/users/not-a-uuid".to_owned(),
    ] {
        let response = kit.admin_get(&root, &uri).await;
        assert_eq!(response.status, StatusCode::NOT_FOUND, "{uri}");
    }
    let response = kit
        .admin_get(
            &kit.admin("ops"),
            &format!("/admin/v1/workspaces/users/{id}"),
        )
        .await;
    assert_eq!(response.status, StatusCode::NOT_FOUND);
}
