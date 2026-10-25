//! The admin API as the server serves it (ADR 0015): the real `admin`
//! module's profiles and audit trail, the composed router, tokens from a
//! test identity provider.

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
};
use campbooks_kernel::{
    admin::{ACCESS_SCOPE, Permission},
    operations,
    testing::{self, TestIdp, TestResponse},
};
use chrono::Utc;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{admin_openapi, openapi, router, with_ports};

const OVERVIEW: &str = "/admin/v1/system/overview";

struct World {
    idp: TestIdp,
    app: Router,
    db: PgPool,
}

async fn world(db: PgPool) -> World {
    let idp = TestIdp::start().await;
    let state = with_ports(idp.state(testing::app_role_pool(&db).await));
    World {
        app: router(state, None),
        idp,
        db,
    }
}

impl World {
    fn token(&self, edit: impl FnOnce(&mut Value)) -> String {
        let mut claims = self.idp.claims(&format!("{ACCESS_SCOPE} system:read"));
        claims["sub"] = json!("ada");
        claims["auth_time"] = json!(Utc::now().timestamp() - 60);
        edit(&mut claims);
        self.idp.token(&claims)
    }

    async fn get(&self, uri: &str, token: Option<&str>) -> TestResponse {
        let mut request = Request::get(uri);
        if let Some(token) = token {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        testing::send(&self.app, request.body(Body::empty()).unwrap()).await
    }

    async fn grant(&self, account: &str, permissions: &[Permission]) {
        let mut conn = self.db.acquire().await.unwrap();
        campbooks_admin::cli::grant(
            &mut conn,
            &campbooks_kernel::tracking::Tracker::default(),
            account,
            permissions,
            None,
            Utc::now(),
        )
        .await
        .unwrap();
    }

    async fn trail(&self) -> Vec<campbooks_admin::cli::Action> {
        let mut conn = self.db.acquire().await.unwrap();
        let mut trail = campbooks_admin::cli::log(&mut conn, 100).await.unwrap();
        trail.retain(|action| action.via == "api");
        trail
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn only_an_administrator_reaches_the_admin_api(db: PgPool) {
    let w = world(db).await;
    w.grant("ops", &[Permission::JobsRead]).await;
    w.grant("acme-sync", &[Permission::Super]).await;
    let unknown = w.get("/admin/v1/system/nothing-here", None).await;
    let cases = [
        ("anonymous", None),
        ("a member", Some(w.token(|_| {}))),
        (
            "an administrator without system.read",
            Some(w.token(|c| c["sub"] = json!("ops"))),
        ),
        (
            "a client token, even one with a profile",
            Some(w.token(|c| {
                c["sub"] = json!("acme-sync");
                c["azp"] = json!("acme-sync");
            })),
        ),
        (
            "a token without admin:access",
            Some(w.token(|c| c["scope"] = json!("openid system:read"))),
        ),
    ];
    for (case, token) in cases {
        let response = w.get(OVERVIEW, token.as_deref()).await;
        assert_eq!(response.status, StatusCode::NOT_FOUND, "{case}");
        assert_eq!(
            (response.content_type.as_deref(), &response.json),
            (unknown.content_type.as_deref(), &unknown.json),
            "{case}: indistinguishable from a route that does not exist"
        );
    }
    assert!(
        w.trail().await.is_empty(),
        "nobody passed the profile check"
    );

    w.grant("ada", &[Permission::Super]).await;
    let response = w.get(OVERVIEW, Some(&w.token(|_| {}))).await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.json);
    assert_eq!(response.json["database"]["reachable"], true);

    let stale = w
        .get(
            OVERVIEW,
            Some(&w.token(|c| c["auth_time"] = json!(Utc::now().timestamp() - 13 * 3600))),
        )
        .await;
    assert_eq!(stale.status, StatusCode::UNAUTHORIZED);
    assert_eq!(stale.json["code"], "admin_reauth_required");

    let trail = w.trail().await;
    let seen: Vec<(&str, &str, Option<i16>)> = trail
        .iter()
        .map(|a| (a.actor.as_str(), a.operation.as_str(), a.status))
        .collect();
    assert_eq!(
        seen,
        [
            ("ada", "system.admin_overview", Some(401)),
            ("ada", "system.admin_overview", Some(200)),
        ]
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn admin_operations_stay_out_of_the_catalogue(db: PgPool) {
    let admin = serde_json::to_value(admin_openapi()).unwrap();
    let main = serde_json::to_value(openapi()).unwrap();
    let ids = |doc: &Value| -> Vec<String> {
        doc["paths"]
            .as_object()
            .unwrap()
            .values()
            .flat_map(|item| item.as_object().unwrap().values())
            .filter_map(|op| op["operationId"].as_str().map(ToOwned::to_owned))
            .collect()
    };
    let admin_ids = ids(&admin);
    assert!(admin_ids.contains(&"system.admin_overview".to_owned()));
    let main_ids = ids(&main);
    for id in &admin_ids {
        assert!(!main_ids.contains(id), "{id} is in openapi.json");
    }
    assert!(
        main["paths"]
            .as_object()
            .unwrap()
            .keys()
            .all(|path| !path.starts_with("/admin/")),
        "no admin path in openapi.json"
    );

    // Neither automation nor the assistant can name one.
    let composed = operations::compose(
        crate::api(),
        with_ports(testing::state(testing::app_role_pool(&db).await)),
    );
    for caller in [
        operations::Caller::Assistant,
        operations::Caller::Automation,
    ] {
        let refused = operations::invoke(
            &composed.state,
            operations::Invocation::new(
                caller,
                campbooks_kernel::auth::Principal {
                    subject: "ada".to_owned(),
                    kind: campbooks_kernel::auth::PrincipalKind::User,
                    client_id: None,
                    scopes: [campbooks_kernel::auth::Scope::new(ACCESS_SCOPE)].into(),
                    workspace: None,
                    email: None,
                    email_verified: false,
                    name: None,
                    expires_at: None,
                    issued_at: None,
                    auth_time: Some(Utc::now()),
                    sign_in: None,
                },
                "system.admin_overview",
                Value::Null,
            ),
        )
        .await;
        assert!(
            matches!(
                refused,
                Err(operations::InvokeError::UnknownOperation(ref id)) if id == "system.admin_overview"
            ),
            "{refused:?}"
        );
    }
}

#[test]
fn the_admin_document_describes_its_security_and_permissions() {
    let admin = serde_json::to_value(admin_openapi()).unwrap();
    assert_eq!(admin["info"]["title"], "Campbooks admin API");
    let scheme = &admin["components"]["securitySchemes"]["admin"];
    assert_eq!(scheme["scheme"], "bearer");
    let overview = &admin["paths"][OVERVIEW]["get"];
    assert_eq!(overview["operationId"], "system.admin_overview");
    assert_eq!(overview["security"], json!([{ "admin": ["admin:access"] }]));
    assert_eq!(
        overview["x-campbooks-admin"],
        json!({ "permission": "system.read", "mutates": false })
    );
    assert!(overview.get("x-campbooks").is_none());
}

/// Tables whose rows an administrator may read across workspaces through
/// `kernel::db::begin_admin` (an admin-read policy). Grows with the slices
/// that add admin views; reviewed like an ADR (ADR 0015). Metadata only:
/// Admin → Signups (slice S1) reads signup requests; Admin → People and
/// Workspaces (slice S2) read workspaces, memberships, invitations (counted)
/// and connections' health and sync state; the AI usage read
/// (campbooks-cloud#152) reads counts of model calls and tokens.
const ADMIN_READ_TABLES: &[&str] = &[
    "connectors.connections",
    "connectors.streams",
    "system.ai_usage",
    "system.ai_usage_days",
    "workspaces.invitations",
    "workspaces.members",
    "workspaces.signup_requests",
    "workspaces.workspaces",
];

/// Tables holding people's content (or secrets). They never get an
/// admin-read policy.
const CONTENT_TABLES: &[&str] = &[
    "conversations.messages",
    "conversations.attachments",
    "conversations.conversations",
    "conversations.decisions",
    "conversations.corrections",
    "connectors.messages",
    "connectors.oversized",
    "connectors.credentials",
    "kernel.blobs",
    "kernel.blob_refs",
    "identity.held_grants",
    "search.entries",
    "search.groups",
];

#[sqlx::test(migrations = "../../migrations")]
async fn admin_reads_reach_only_allowlisted_metadata_tables(db: PgPool) {
    let with_policy: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT n.nspname || '.' || c.relname
           FROM pg_policy p
           JOIN pg_class c ON c.oid = p.polrelid
           JOIN pg_namespace n ON n.oid = c.relnamespace
          WHERE coalesce(pg_get_expr(p.polqual, p.polrelid), '')
                || coalesce(pg_get_expr(p.polwithcheck, p.polrelid), '') LIKE '%' || $1 || '%'
          ORDER BY 1",
    )
    .bind(campbooks_kernel::db::ADMIN_READS_SETTING)
    .fetch_all(&db)
    .await
    .unwrap();
    for table in &with_policy {
        assert!(
            !CONTENT_TABLES.contains(&table.as_str()),
            "{table} holds content and must never be readable across workspaces"
        );
    }
    assert_eq!(
        with_policy, ADMIN_READ_TABLES,
        "tables with an admin-read policy changed: update ADMIN_READ_TABLES (reviewed like an ADR)"
    );
    for table in ADMIN_READ_TABLES {
        assert!(!CONTENT_TABLES.contains(table), "{table}");
    }
}
