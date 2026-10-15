use axum::http::{Method, StatusCode};
use sqlx::PgPool;
use uuid::Uuid;

use crate::admin_testkit::AdminKit;

#[sqlx::test(migrations = "../../../migrations")]
async fn administrators_see_how_a_person_gets_in_without_any_secret(db: PgPool) {
    let kit = AdminKit::new(db, &[]).await;
    let ana = kit.account("ana").await;
    kit.account("bea").await;
    let uri = format!("/admin/v1/identity/accounts/{ana}");

    for sub in ["root", "viewer"] {
        let response = kit.call(sub, Method::GET, &uri, "en").await;
        assert_eq!(response.status, StatusCode::OK, "{sub}: {}", response.json);
        let body = &response.json;
        assert_eq!(body["id"], ana.to_string());
        assert_eq!(body["display_name"], "ana");
        assert_eq!(body["primary_email"], "ana@example.com");
        assert_eq!(body["primary_email_verified"], true);
        assert!(body["suspended_at"].is_null());
        assert_eq!(body["passkeys"], 1);
        assert_eq!(body["sessions"]["active"], 1);
        assert!(body["sessions"]["last_sign_in_at"].is_string());
        let ways = body["ways_in"].as_array().unwrap();
        assert_eq!(ways.len(), 1);
        assert_eq!(ways[0]["provider"], "google");
        assert_eq!(ways[0]["email"], "ana@example.com");
        assert_eq!(ways[0]["linked_via"], "sign_up");
        let text = body.to_string();
        for secret in ["session-secret", "public-key-bytes", "google-ana", "bea"] {
            assert!(!text.contains(secret), "{secret} leaked: {text}");
        }
    }
    assert_eq!(
        kit.trail.entries()[0],
        ("root".into(), "identity.admin_get_account".into(), 200)
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn unknown_accounts_and_non_administrators_get_404(db: PgPool) {
    let kit = AdminKit::new(db, &[]).await;
    let ana = kit.account("ana").await;
    for (sub, uri) in [
        (
            "root",
            format!("/admin/v1/identity/accounts/{}", Uuid::now_v7()),
        ),
        ("root", "/admin/v1/identity/accounts/not-an-id".to_owned()),
        ("member", format!("/admin/v1/identity/accounts/{ana}")),
        (
            &ana.to_string(),
            format!("/admin/v1/identity/accounts/{ana}"),
        ),
    ] {
        let response = kit.call(sub, Method::GET, &uri, "en").await;
        assert_eq!(response.status, StatusCode::NOT_FOUND, "{sub} {uri}");
    }
}
