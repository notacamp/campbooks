use axum::http::{Method, StatusCode};
use sqlx::PgPool;
use uuid::Uuid;

use crate::admin_testkit::AdminKit;

#[sqlx::test(migrations = "../../../migrations")]
async fn a_page_of_people_learns_who_is_suspended_in_one_request(db: PgPool) {
    let kit = AdminKit::new(db, &[]).await;
    let ana = kit.account("ana").await;
    let bea = kit.account("bea").await;
    kit.account("cai").await;
    let suspended = kit
        .call(
            "root",
            Method::POST,
            &format!("/admin/v1/identity/accounts/{bea}/suspend"),
            "en",
        )
        .await;
    assert_eq!(suspended.status, StatusCode::OK, "{}", suspended.json);

    // Two accounts, a repeat, an unknown id and another provider's subject.
    let uri = format!(
        "/admin/v1/identity/accounts?ids={ana},{bea},%20{ana},{},google-ana,",
        Uuid::now_v7()
    );
    for sub in ["root", "viewer"] {
        let response = kit.call(sub, Method::GET, &uri, "en").await;
        assert_eq!(response.status, StatusCode::OK, "{sub}: {}", response.json);
        let mut accounts = response.json["accounts"].as_array().unwrap().clone();
        accounts.sort_by_key(|account| account["id"].as_str().unwrap().to_owned());
        let mut expected = [(ana, false), (bea, true)];
        expected.sort_by_key(|(id, _)| id.to_string());
        assert_eq!(accounts.len(), 2, "{accounts:?}");
        for (account, (id, is_suspended)) in accounts.iter().zip(expected) {
            assert_eq!(account["id"], id.to_string());
            assert_eq!(
                account["suspended_at"].is_string(),
                is_suspended,
                "{account}"
            );
            // Its id and suspension time: nothing else.
            assert_eq!(account.as_object().unwrap().len(), 2, "{account}");
        }
    }

    let none = kit
        .call("root", Method::GET, "/admin/v1/identity/accounts", "en")
        .await;
    assert_eq!(none.status, StatusCode::OK);
    assert_eq!(none.json["accounts"], serde_json::json!([]));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn at_most_a_page_and_administrators_only(db: PgPool) {
    let kit = AdminKit::new(db, &[]).await;
    let ana = kit.account("ana").await;
    let ids = (0..=super::MAX_IDS)
        .map(|_| Uuid::now_v7().to_string())
        .collect::<Vec<_>>()
        .join(",");
    let many = format!("/admin/v1/identity/accounts?ids={ids}");

    let refused = kit.call("root", Method::GET, &many, "pt-PT").await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST, "{}", refused.json);
    assert_eq!(refused.json["code"], "too_many_accounts");
    assert!(
        refused.json["detail"].as_str().unwrap().contains("200"),
        "{}",
        refused.json
    );

    // Not an administrator: 404, whatever the parameters.
    for (sub, uri) in [
        ("member", format!("/admin/v1/identity/accounts?ids={ana}")),
        ("member", many.clone()),
        (
            &ana.to_string(),
            format!("/admin/v1/identity/accounts?ids={ana}"),
        ),
    ] {
        let response = kit.call(sub, Method::GET, &uri, "en").await;
        assert_eq!(response.status, StatusCode::NOT_FOUND, "{sub} {uri}");
    }
}
