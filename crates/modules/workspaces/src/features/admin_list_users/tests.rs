use axum::http::StatusCode;
use serde_json::Value;
use sqlx::PgPool;

use crate::testkit::Kit;

/// `ana` owns Shop and Home, `bea` is a member of Shop, `cid` belongs
/// nowhere; created in that order.
async fn people(kit: &Kit) {
    let ana = kit.user("ana");
    let shop = kit.create_workspace(&ana, "Shop").await;
    kit.create_workspace(&ana, "Home").await;
    kit.join(&ana, shop, "bea", "member").await;
    kit.get(&kit.user("cid"), "/api/v1/workspaces").await;
}

fn summary(page: &Value) -> Vec<(String, i64)> {
    page["users"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| {
            (
                u["account_id"].as_str().unwrap().to_owned(),
                u["workspace_count"].as_i64().unwrap(),
            )
        })
        .collect()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn administrators_page_through_everyone_newest_first(db: PgPool) {
    let kit = Kit::new(db).await;
    people(&kit).await;
    let root = kit.admin("root");

    let all = kit.admin_get(&root, "/admin/v1/workspaces/users").await;
    assert_eq!(all.status, StatusCode::OK, "{}", all.json);
    assert_eq!(
        summary(&all.json),
        [("cid".into(), 0), ("bea".into(), 1), ("ana".into(), 2)]
    );
    assert_eq!(all.json["users"][2]["email"], "ana@example.com");
    assert_eq!(all.json["users"][2]["display_name"], "ana");
    assert!(all.json["next"].is_null());

    let first = kit
        .admin_get(&root, "/admin/v1/workspaces/users?limit=2")
        .await;
    assert_eq!(summary(&first.json).len(), 2);
    let next = first.json["next"].as_str().unwrap();
    let second = kit
        .admin_get(
            &root,
            &format!("/admin/v1/workspaces/users?limit=2&after={next}"),
        )
        .await;
    assert_eq!(summary(&second.json), [("ana".into(), 2)]);
    assert!(second.json["next"].is_null());

    // `users.read` is enough.
    let people = kit
        .admin_get(&kit.admin("people"), "/admin/v1/workspaces/users")
        .await;
    assert_eq!(people.status, StatusCode::OK);

    let recorded = kit.trail.entries();
    assert_eq!(recorded.len(), 4);
    assert_eq!(
        recorded[0],
        ("root".into(), "workspaces.admin_list_users".into(), 200)
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn search_matches_a_name_or_address_prefix_literally(db: PgPool) {
    let kit = Kit::new(db).await;
    people(&kit).await;
    let root = kit.admin("root");
    for (q, expected) in [
        ("be", vec!["bea"]),
        ("BEA%40EX", vec!["bea"]),
        ("a", vec!["ana"]),
        ("ea", vec![]),
        ("%25", vec![]),
        ("_", vec![]),
    ] {
        let page = kit
            .admin_get(&root, &format!("/admin/v1/workspaces/users?q={q}"))
            .await;
        assert_eq!(page.status, StatusCode::OK, "{q}");
        let found: Vec<String> = summary(&page.json).into_iter().map(|(a, _)| a).collect();
        assert_eq!(found, expected, "{q}");
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn anyone_else_finds_nothing_here_even_with_bad_parameters(db: PgPool) {
    let kit = Kit::new(db).await;
    people(&kit).await;
    for sub in ["ana", "ops"] {
        for uri in [
            "/admin/v1/workspaces/users",
            "/admin/v1/workspaces/users?limit=0",
        ] {
            let response = kit.admin_get(&kit.admin(sub), uri).await;
            assert_eq!(response.status, StatusCode::NOT_FOUND, "{sub} {uri}");
        }
    }
    // A member's ordinary token, without `admin:access`.
    let response = kit
        .admin_get(&kit.user("root"), "/admin/v1/workspaces/users")
        .await;
    assert_eq!(response.status, StatusCode::NOT_FOUND);
    assert!(kit.trail.entries().is_empty());

    let root = kit.admin("root");
    for (uri, code) in [
        ("/admin/v1/workspaces/users?limit=201", "invalid_limit"),
        ("/admin/v1/workspaces/users?limit=x", "invalid_query"),
        ("/admin/v1/workspaces/users?after=nope", "invalid_cursor"),
    ] {
        let response = kit.admin_get(&root, uri).await;
        assert_eq!(response.status, StatusCode::BAD_REQUEST, "{uri}");
        assert_eq!(response.json["code"], code, "{uri}");
    }
}
