use std::sync::{Arc, Mutex};

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
};
use campbooks_kernel::{
    Error,
    admin::{
        ACCESS_SCOPE, AdminAction, AdminAudit, AdminProfiles, AuditFuture, PermissionSet,
        ProfilesFuture,
    },
    testing::{self, TestIdp, TestResponse},
};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug)]
struct Profiles;

impl AdminProfiles for Profiles {
    fn permissions_of<'a>(&'a self, _db: &'a PgPool, account: &'a str) -> ProfilesFuture<'a> {
        let names: &[&str] = match account {
            "root" => &["super"],
            "viewer" => &["workspaces.read"],
            "people" => &["users.read"],
            _ => &[],
        };
        Box::pin(async move { Ok(PermissionSet::from_names(names)) })
    }
}

#[derive(Debug, Default)]
struct Trail(Mutex<Vec<AdminAction>>);

impl AdminAudit for Trail {
    fn record<'a>(&'a self, _db: &'a PgPool, action: &'a AdminAction) -> AuditFuture<'a> {
        self.0.lock().unwrap().push(action.clone());
        Box::pin(async { Ok::<(), Error>(()) })
    }
}

struct World {
    idp: TestIdp,
    app: Router,
    trail: Arc<Trail>,
}

impl World {
    async fn new(db: &PgPool) -> Self {
        let idp = TestIdp::start().await;
        let trail = Arc::new(Trail::default());
        let state = idp
            .state(testing::app_role_pool(db).await)
            .with_admin(Arc::new(Profiles), trail.clone());
        let app = testing::admin_app(crate::module(crate::Deps::default()), state);
        Self { idp, app, trail }
    }

    async fn get(&self, sub: &str, workspace: Uuid) -> TestResponse {
        let mut claims = self.idp.claims(ACCESS_SCOPE);
        claims["sub"] = sub.into();
        claims["auth_time"] = Utc::now().timestamp().into();
        let request = Request::get(format!(
            "/admin/v1/connectors/workspaces/{workspace}/connections"
        ))
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", self.idp.token(&claims)),
        )
        .body(Body::empty())
        .unwrap();
        testing::send(&self.app, request).await
    }
}

/// A connection of `workspace` (with its sealed credential, which must
/// never show) and a `messages` stream; its id.
async fn connect(db: &PgPool, workspace: Uuid, address: &str, health: &str) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO connectors.connections
             (id, workspace_id, provider, account_subject, address, granted_by, capabilities,
              scopes, health, health_reason)
         VALUES ($1, $2, 'google', $3, $4, $5, '{messages}', 'gmail.modify', $6,
                 CASE WHEN $6 = 'ok' THEN NULL ELSE 'grant_revoked' END)",
    )
    .bind(id)
    .bind(workspace)
    .bind(format!("sub-{address}"))
    .bind(address)
    .bind(Uuid::now_v7())
    .bind(health)
    .execute(db)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO connectors.credentials (connection_id, workspace_id, refresh_token)
         VALUES ($1, $2, 'sealed-refresh-token')",
    )
    .bind(id)
    .bind(workspace)
    .execute(db)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO connectors.streams
             (id, workspace_id, connection_id, capability, phase, cursor, interval_seconds,
              paused, last_synced_at, consecutive_failures)
         VALUES ($1, $2, $3, 'messages', 'incremental', '{\"history_id\": \"secret-cursor\"}',
                 60, $4, now(), $5)",
    )
    .bind(Uuid::now_v7())
    .bind(workspace)
    .bind(id)
    .bind(health != "ok")
    .bind(i32::from(health != "ok") * 3)
    .execute(db)
    .await
    .unwrap();
    id
}

#[sqlx::test(migrations = "../../../migrations")]
async fn administrators_see_a_workspaces_connection_health_and_nothing_else(db: PgPool) {
    let shop = Uuid::now_v7();
    let home = Uuid::now_v7();
    let healthy = connect(&db, shop, "sales@example.com", "ok").await;
    let broken = connect(&db, shop, "billing@example.com", "needs_reauth").await;
    connect(&db, home, "me@example.com", "ok").await;
    let w = World::new(&db).await;

    for sub in ["root", "viewer"] {
        let response = w.get(sub, shop).await;
        assert_eq!(response.status, StatusCode::OK, "{sub}: {}", response.json);
        let list = response.json["connections"].as_array().unwrap();
        assert_eq!(list.len(), 2, "only the workspace's own");
        assert_eq!(list[0]["id"], healthy.to_string());
        assert_eq!(list[0]["address"], "sales@example.com");
        assert_eq!(list[0]["capabilities"], serde_json::json!(["messages"]));
        assert_eq!(list[0]["health"], "ok");
        assert_eq!(list[0]["streams"][0]["phase"], "incremental");
        assert_eq!(list[0]["streams"][0]["consecutive_failures"], 0);
        assert_eq!(list[1]["id"], broken.to_string());
        assert_eq!(list[1]["health"], "needs_reauth");
        assert_eq!(list[1]["health_reason"], "grant_revoked");
        assert_eq!(list[1]["streams"][0]["paused"], true);
        assert_eq!(list[1]["streams"][0]["consecutive_failures"], 3);
        let text = response.json.to_string();
        for secret in [
            "sealed-refresh-token",
            "secret-cursor",
            "gmail.modify",
            "sub-",
        ] {
            assert!(!text.contains(secret), "{secret} leaked: {text}");
        }
    }

    let empty = w.get("root", Uuid::now_v7()).await;
    assert_eq!(empty.json["connections"], serde_json::json!([]));

    for sub in ["people", "member"] {
        assert_eq!(
            w.get(sub, shop).await.status,
            StatusCode::NOT_FOUND,
            "{sub}"
        );
    }
    let recorded = w.trail.0.lock().unwrap().clone();
    assert_eq!(recorded.len(), 3);
    assert_eq!(recorded[0].operation, "connectors.admin_list_connections");
    assert_eq!(
        recorded[0].target,
        serde_json::json!({ "workspace_id": shop.to_string() })
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn outside_an_admin_transaction_the_runtime_role_still_sees_nothing(db: PgPool) {
    let shop = Uuid::now_v7();
    connect(&db, shop, "sales@example.com", "ok").await;
    let pool = testing::app_role_pool(&db).await;
    for sql in [
        "SELECT count(*) FROM connectors.connections",
        "SELECT count(*) FROM connectors.streams",
        "SELECT count(*) FROM connectors.credentials",
    ] {
        let seen: i64 = sqlx::query_scalar(sql).fetch_one(&pool).await.unwrap();
        assert_eq!(seen, 0, "{sql}");
    }
}
