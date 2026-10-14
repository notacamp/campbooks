use axum::http::{Method, StatusCode};
use sqlx::PgPool;
use uuid::Uuid;

use crate::admin_testkit::AdminKit;

const LIVE_SESSIONS: &str =
    "SELECT count(*) FROM identity.sessions WHERE account_id = $1 AND ended_at IS NULL";
const LIVE_FAMILIES: &str =
    "SELECT count(*) FROM identity.refresh_families WHERE account_id = $1 AND revoked_at IS NULL";
const SUSPENSIONS: &str = "SELECT count(*) FROM identity.audit_events
    WHERE account_id = $1 AND kind = 'account_suspended' AND metadata->>'by' = 'root'";
const EVENTS: &str = "SELECT count(*) FROM kernel.events
    WHERE name = 'identity.account_suspended' AND (payload->>'account')::uuid = $1";

#[sqlx::test(migrations = "../../../migrations")]
async fn suspending_signs_a_person_out_everywhere_once(db: PgPool) {
    let kit = AdminKit::new(db, &[]).await;
    let ana = kit.account("ana").await;
    let bea = kit.account("bea").await;
    let suspend = format!("/admin/v1/identity/accounts/{ana}/suspend");

    let first = kit.call("root", Method::POST, &suspend, "en").await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.json);
    assert!(first.json["suspended_at"].is_string());
    assert_eq!(first.json["sessions"]["active"], 0);
    assert_eq!(kit.count(LIVE_SESSIONS, ana).await, 0);
    assert_eq!(kit.count(LIVE_FAMILIES, ana).await, 0);
    assert_eq!(kit.count(SUSPENSIONS, ana).await, 1);
    assert_eq!(kit.count(EVENTS, ana).await, 1);
    // Nobody else is touched.
    assert_eq!(kit.count(LIVE_SESSIONS, bea).await, 1);
    assert_eq!(kit.count(LIVE_FAMILIES, bea).await, 2);

    // A retry is a no-op success.
    let again = kit.call("root", Method::POST, &suspend, "en").await;
    assert_eq!(again.status, StatusCode::OK);
    assert_eq!(again.json["suspended_at"], first.json["suspended_at"]);
    assert_eq!(kit.count(SUSPENSIONS, ana).await, 1);
    assert_eq!(kit.count(EVENTS, ana).await, 1);

    let unsuspend = format!("/admin/v1/identity/accounts/{ana}/unsuspend");
    for _ in 0..2 {
        let lifted = kit.call("root", Method::POST, &unsuspend, "en").await;
        assert_eq!(lifted.status, StatusCode::OK, "{}", lifted.json);
        assert!(lifted.json["suspended_at"].is_null());
    }
    assert_eq!(
        kit.count(
            "SELECT count(*) FROM identity.audit_events
              WHERE account_id = $1 AND kind = 'account_unsuspended'",
            ana
        )
        .await,
        1
    );
    let recorded = kit.trail.entries();
    assert_eq!(
        recorded
            .iter()
            .map(|(_, operation, status)| (operation.as_str(), *status))
            .collect::<Vec<_>>(),
        [
            ("identity.admin_suspend_account", 200),
            ("identity.admin_suspend_account", 200),
            ("identity.admin_unsuspend_account", 200),
            ("identity.admin_unsuspend_account", 200),
        ]
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_administrators_account_is_refused_until_its_profile_is_revoked(db: PgPool) {
    let admin = Uuid::now_v7();
    let kit = AdminKit::new(db, &[admin]).await;
    sqlx::query(
        "INSERT INTO identity.accounts (id, primary_email_verified, created_at)
         VALUES ($1, false, now())",
    )
    .bind(admin)
    .execute(&kit.db)
    .await
    .unwrap();
    let uri = format!("/admin/v1/identity/accounts/{admin}/suspend");
    for (language, says) in [
        ("en", "campbooks-server admin revoke"),
        ("pt-PT", "Revogue primeiro"),
        ("es-ES", "Revoca primero"),
        ("fr", "Révoquez d’abord"),
    ] {
        let refused = kit.call("root", Method::POST, &uri, language).await;
        assert_eq!(refused.status, StatusCode::CONFLICT, "{}", refused.json);
        assert_eq!(refused.json["code"], "account_is_admin");
        assert!(
            refused.json["detail"].as_str().unwrap().contains(says),
            "{language}: {}",
            refused.json
        );
    }
    assert_eq!(
        kit.count(
            "SELECT count(*) FROM identity.accounts WHERE id = $1 AND disabled_at IS NULL",
            admin
        )
        .await,
        1
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn only_users_suspend_may_suspend(db: PgPool) {
    let kit = AdminKit::new(db, &[]).await;
    let ana = kit.account("ana").await;
    for (sub, uri) in [
        (
            "viewer",
            format!("/admin/v1/identity/accounts/{ana}/suspend"),
        ),
        (
            "viewer",
            format!("/admin/v1/identity/accounts/{ana}/unsuspend"),
        ),
        (
            "member",
            format!("/admin/v1/identity/accounts/{ana}/suspend"),
        ),
        (
            "root",
            format!("/admin/v1/identity/accounts/{}/suspend", Uuid::now_v7()),
        ),
        (
            "root",
            format!("/admin/v1/identity/accounts/{}/unsuspend", Uuid::now_v7()),
        ),
    ] {
        let response = kit.call(sub, Method::POST, &uri, "en").await;
        assert_eq!(response.status, StatusCode::NOT_FOUND, "{sub} {uri}");
    }
    assert_eq!(kit.count(LIVE_SESSIONS, ana).await, 1);
}
