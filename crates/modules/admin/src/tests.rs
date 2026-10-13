use campbooks_kernel::{
    admin::{AdminAction, Permission},
    testing,
};
use chrono::Utc;
use sqlx::PgPool;

use crate::cli::{self, Revoked};

#[test]
fn the_module_serves_nothing() {
    let module = super::module();
    assert_eq!(module.name, "admin");
    assert!(module.admin_router.is_none());
    let spec = module.router.into_openapi();
    assert!(spec.paths.paths.is_empty());
    assert_eq!(module.scopes.len(), 1);
    assert_eq!(module.scopes[0].name, "admin:access");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn grants_add_up_and_are_recorded(db: PgPool) {
    let mut conn = db.acquire().await.unwrap();
    let now = Utc::now();
    let first = cli::grant(
        &mut conn,
        "acc-1",
        &[Permission::SystemRead, Permission::SystemRead],
        Some("on call"),
        now,
    )
    .await
    .unwrap();
    assert_eq!(first.permissions, ["system.read"]);
    assert_eq!(first.note.as_deref(), Some("on call"));
    let second = cli::grant(&mut conn, "acc-1", &[Permission::Super], None, now)
        .await
        .unwrap();
    assert_eq!(second.permissions, ["super", "system.read"]);
    assert_eq!(second.note.as_deref(), Some("on call"), "kept");
    assert_eq!(cli::list(&mut conn).await.unwrap(), [second]);

    let actions = cli::log(&mut conn, 10).await.unwrap();
    assert_eq!(actions.len(), 2);
    let login: String = sqlx::query_scalar("SELECT session_user::text")
        .fetch_one(&db)
        .await
        .unwrap();
    for action in &actions {
        assert_eq!(
            (
                &action.actor,
                action.via.as_str(),
                action.operation.as_str(),
                action.status
            ),
            (&login, "cli", "admin.grant", None)
        );
    }
    assert_eq!(
        actions[0].target,
        serde_json::json!({ "account": "acc-1", "permissions": ["super"] }),
        "newest first"
    );
    assert_eq!(cli::log(&mut conn, 1).await.unwrap().len(), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn revoking_removes_permissions_then_the_profile(db: PgPool) {
    let mut conn = db.acquire().await.unwrap();
    let now = Utc::now();
    assert_eq!(
        cli::revoke(&mut conn, "acc-1", &[], now).await.unwrap(),
        Revoked::NotAnAdmin
    );
    cli::grant(
        &mut conn,
        "acc-1",
        &[Permission::JobsRead, Permission::JobsRetry],
        None,
        now,
    )
    .await
    .unwrap();
    let Revoked::Remaining(left) = cli::revoke(&mut conn, "acc-1", &[Permission::JobsRetry], now)
        .await
        .unwrap()
    else {
        panic!("jobs.read remains");
    };
    assert_eq!(left.permissions, ["jobs.read"]);
    assert_eq!(
        cli::revoke(&mut conn, "acc-1", &[Permission::JobsRead], now)
            .await
            .unwrap(),
        Revoked::Removed,
        "nothing left: no profile"
    );
    cli::grant(&mut conn, "acc-1", &[Permission::Super], None, now)
        .await
        .unwrap();
    assert_eq!(
        cli::revoke(&mut conn, "acc-1", &[], now).await.unwrap(),
        Revoked::Removed
    );
    assert!(cli::list(&mut conn).await.unwrap().is_empty());
    let revokes: i64 =
        sqlx::query_scalar("SELECT count(*) FROM admin.actions WHERE operation = 'admin.revoke'")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(revokes, 3, "a revoke that changed nothing is not recorded");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_runtime_role_reads_profiles_and_appends_actions(db: PgPool) {
    let mut conn = db.acquire().await.unwrap();
    cli::grant(&mut conn, "acc-1", &[Permission::Super], None, Utc::now())
        .await
        .unwrap();
    let app = testing::app_role_pool(&db).await;
    let permissions = super::profiles()
        .permissions_of(&app, "acc-1")
        .await
        .unwrap();
    assert!(permissions.allows(Permission::JobsRetry));
    assert!(
        super::profiles()
            .permissions_of(&app, "acc-2")
            .await
            .unwrap()
            .is_empty()
    );
    let action = AdminAction {
        at: Utc::now(),
        actor: "acc-1".into(),
        operation: "system.admin_overview".into(),
        target: serde_json::json!({}),
        status: 200,
    };
    super::audit().record(&app, &action).await.unwrap();
    let row: (String, String, String, i16) =
        sqlx::query_as("SELECT actor, via, operation, status FROM admin.actions WHERE via = 'api'")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(
        row,
        (
            "acc-1".to_owned(),
            "api".to_owned(),
            "system.admin_overview".to_owned(),
            200
        )
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_runtime_role_cannot_write_a_profile_or_rewrite_the_trail(db: PgPool) {
    let mut conn = db.acquire().await.unwrap();
    cli::grant(
        &mut conn,
        "acc-1",
        &[Permission::SystemRead],
        None,
        Utc::now(),
    )
    .await
    .unwrap();
    for pool in [
        testing::app_role_pool(&db).await,
        testing::runtime_login_pool(&db).await,
    ] {
        for sql in [
            "INSERT INTO admin.profiles (account_id, permissions, granted_at, updated_at) \
             VALUES ('intruder', '{super}', now(), now())",
            "UPDATE admin.profiles SET permissions = '{super}'",
            "DELETE FROM admin.profiles",
            "UPDATE admin.actions SET status = 200",
            "DELETE FROM admin.actions",
            "SELECT count(*) FROM admin.actions",
            "TRUNCATE admin.profiles",
        ] {
            let err = sqlx::query(sql).execute(&pool).await.unwrap_err();
            let code = err
                .as_database_error()
                .and_then(sqlx::error::DatabaseError::code)
                .map(std::borrow::Cow::into_owned);
            assert_eq!(
                code.as_deref(),
                Some("42501"),
                "{sql}: expected a permission error, got {err}"
            );
        }
        let mut conn = pool.acquire().await.unwrap();
        assert!(
            cli::grant(
                &mut conn,
                "intruder",
                &[Permission::Super],
                None,
                Utc::now()
            )
            .await
            .is_err(),
            "the CLI's functions need the migration login"
        );
        drop(conn);
        pool.close().await;
    }
    testing::drop_runtime_login(&db).await;
    assert_eq!(
        cli::list(&mut conn).await.unwrap()[0].permissions,
        ["system.read"]
    );
}
