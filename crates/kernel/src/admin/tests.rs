//! `Admin<P>` and the audit middleware end to end: an admin route served
//! like the server serves one, tokens signed by an in-process [`TestIdp`].

use std::sync::{Arc, Mutex};

use axum::{
    Json, Router,
    body::Body,
    extract::Path,
    http::{Request, StatusCode, header},
};
use chrono::{TimeDelta, Utc};
use serde_json::{Value, json};
use tower::ServiceExt as _;
use utoipa_axum::{router::OpenApiRouter, routes};

use super::{permission::SystemRead, *};
use crate::testing::{self, TestIdp};

/// Profiles held in memory: account → permission names.
#[derive(Debug, Default)]
struct FakeProfiles(Mutex<BTreeMap<String, Vec<&'static str>>>);

impl FakeProfiles {
    fn grant(&self, account: &str, permissions: &[&'static str]) {
        self.0
            .lock()
            .unwrap()
            .insert(account.to_owned(), permissions.to_vec());
    }
}

impl AdminProfiles for FakeProfiles {
    fn permissions_of<'a>(&'a self, _db: &'a PgPool, account: &'a str) -> ProfilesFuture<'a> {
        let names = self.0.lock().unwrap().get(account).cloned();
        Box::pin(async move { Ok(PermissionSet::from_names(names.unwrap_or_default())) })
    }
}

/// An audit trail in memory, which can be made to fail.
#[derive(Debug, Default)]
struct FakeAudit {
    rows: Mutex<Vec<AdminAction>>,
    broken: std::sync::atomic::AtomicBool,
}

impl AdminAudit for FakeAudit {
    fn record<'a>(&'a self, _db: &'a PgPool, action: &'a AdminAction) -> AuditFuture<'a> {
        Box::pin(async move {
            if self.broken.load(std::sync::atomic::Ordering::SeqCst) {
                return Err(Error::internal("disk full"));
            }
            self.rows.lock().unwrap().push(action.clone());
            Ok(())
        })
    }
}

#[utoipa::path(
    get,
    path = "/admin/v1/test/things/{id}",
    operation_id = "test.get_thing",
    params(("id" = String, Path)),
    responses((status = 200, description = "A thing.")),
)]
async fn get_thing(admin: Admin<SystemRead>, Path(id): Path<String>) -> Json<Value> {
    Json(json!({ "id": id, "account": admin.grant.account() }))
}

struct Fixture {
    idp: TestIdp,
    profiles: Arc<FakeProfiles>,
    audit: Arc<FakeAudit>,
    app: Router,
}

async fn fixture() -> Fixture {
    let idp = TestIdp::start().await;
    let profiles = Arc::new(FakeProfiles::default());
    let trail = Arc::new(FakeAudit::default());
    let state = idp
        .state(testing::unreachable_pool())
        .with_admin(profiles.clone(), trail.clone());
    let (routes, spec) = OpenApiRouter::<AppState>::new()
        .routes(routes!(get_thing))
        .split_for_parts();
    let app = routes
        .route_layer(axum::middleware::from_fn_with_state(
            AuditLayer::new(&state, &spec),
            audit,
        ))
        .with_state(state);
    Fixture {
        idp,
        profiles,
        audit: trail,
        app,
    }
}

impl Fixture {
    fn token(&self, edit: impl FnOnce(&mut Value)) -> String {
        let mut claims = self.idp.claims(ACCESS_SCOPE);
        claims["auth_time"] = json!(Utc::now().timestamp() - 60);
        edit(&mut claims);
        self.idp.token(&claims)
    }

    async fn get(&self, token: Option<&str>) -> testing::TestResponse {
        let mut request = Request::get("/admin/v1/test/things/t-1");
        if let Some(token) = token {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        testing::send(&self.app, request.body(Body::empty()).unwrap()).await
    }

    fn recorded(&self) -> Vec<AdminAction> {
        self.audit.rows.lock().unwrap().clone()
    }
}

#[test]
fn permissions_have_stable_wire_names() {
    for permission in Permission::ALL {
        assert_eq!(Permission::parse(permission.as_str()), Some(permission));
        assert_eq!(
            serde_json::to_value(permission).unwrap(),
            json!(permission.as_str())
        );
    }
    assert_eq!(Permission::SystemRead.to_string(), "system.read");
    assert_eq!(Permission::parse("root"), None);
}

#[test]
fn super_implies_every_permission() {
    let super_admin = PermissionSet::from_names(["super"]);
    for permission in Permission::ALL {
        assert!(super_admin.allows(permission), "{permission}");
    }
    let reader = PermissionSet::from_names(["system.read", "from.the.future"]);
    assert_eq!(reader.names(), ["system.read"]);
    assert!(reader.allows(Permission::SystemRead));
    assert!(!reader.allows(Permission::JobsRetry));
    assert!(!reader.allows(Permission::Super));
    assert!(PermissionSet::default().is_empty());
}

#[test]
fn admin_operations_carry_their_permission() {
    assert_eq!(
        serde_json::to_value(AdminOperation::query::<SystemRead>()).unwrap(),
        json!({ "permission": "system.read", "mutates": false })
    );
    assert!(AdminOperation::command::<permission::JobsRetry>().mutates);
}

#[test]
fn recent_means_within_an_hour() {
    let now = Utc::now();
    assert!(recently_authenticated(
        Some(now - TimeDelta::hours(11)),
        now
    ));
    assert!(recently_authenticated(Some(now - REAUTH_AFTER), now));
    assert!(!recently_authenticated(
        Some(now - REAUTH_AFTER - TimeDelta::seconds(1)),
        now
    ));
    assert!(!recently_authenticated(None, now));
}

#[tokio::test]
async fn an_administrator_is_served_and_audited() {
    let f = fixture().await;
    f.profiles.grant("test-user", &["super"]);
    let response = f.get(Some(&f.token(|_| {}))).await;
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(
        response.json,
        json!({ "id": "t-1", "account": "test-user" })
    );
    let rows = f.recorded();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].actor, "test-user");
    assert_eq!(rows[0].operation, "test.get_thing");
    assert_eq!(rows[0].target, json!({ "id": "t-1" }));
    assert_eq!(rows[0].status, 200);
}

#[tokio::test]
async fn everyone_else_gets_404_and_leaves_no_trace() {
    let f = fixture().await;
    f.profiles.grant("test-user", &["jobs.read"]);
    f.profiles.grant("acme-sync", &["super"]);
    let cases = [
        ("anonymous", None),
        ("garbage token", Some("not-a-token".to_owned())),
        ("lacking the permission", Some(f.token(|_| {}))),
        (
            "without admin:access",
            Some(f.token(|c| c["scope"] = json!("openid system:read"))),
        ),
        (
            "a client-credentials client",
            Some(f.token(|c| {
                c["sub"] = json!("acme-sync");
                c["azp"] = json!("acme-sync");
            })),
        ),
        (
            "no profile",
            Some(f.token(|c| c["sub"] = json!("someone-else"))),
        ),
    ];
    for (case, token) in cases {
        let response = f.get(token.as_deref()).await;
        assert_eq!(response.status, StatusCode::NOT_FOUND, "{case}");
        assert_eq!(response.json["code"], "not_found", "{case}");
    }
    assert!(f.recorded().is_empty());
}

#[tokio::test]
async fn a_stale_sign_in_must_authenticate_again() {
    let f = fixture().await;
    f.profiles.grant("test-user", &["system.read"]);
    for token in [
        f.token(|c| c["auth_time"] = json!(Utc::now().timestamp() - 13 * 3600)),
        f.token(|c| {
            c.as_object_mut().unwrap().remove("auth_time");
        }),
    ] {
        let mut request = Request::get("/admin/v1/test/things/t-1")
            .header(header::AUTHORIZATION, format!("Bearer {token}"));
        request = request.header(header::ACCEPT_LANGUAGE, "en");
        let response = f
            .app
            .clone()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            response.headers()[header::WWW_AUTHENTICATE],
            r#"Bearer error="insufficient_user_authentication", max_age=43200"#
        );
    }
    let rows = f.recorded();
    assert_eq!(rows.len(), 2, "past the profile check, so recorded");
    assert!(rows.iter().all(|row| row.status == 401));
}

#[tokio::test]
async fn revoking_takes_effect_on_the_next_request() {
    let f = fixture().await;
    f.profiles.grant("test-user", &["super"]);
    let token = f.token(|_| {});
    assert_eq!(f.get(Some(&token)).await.status, StatusCode::OK);
    f.profiles.0.lock().unwrap().clear();
    assert_eq!(f.get(Some(&token)).await.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn no_answer_leaves_unrecorded() {
    let f = fixture().await;
    f.profiles.grant("test-user", &["super"]);
    f.audit
        .broken
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let response = f.get(Some(&f.token(|_| {}))).await;
    assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(response.json["code"], "internal_error");
    assert!(response.json.get("id").is_none());
}

#[tokio::test]
async fn admin_handlers_answer_only_behind_the_audit_layer() {
    let idp = TestIdp::start().await;
    let profiles = Arc::new(FakeProfiles::default());
    profiles.grant("test-user", &["super"]);
    let state = idp
        .state(testing::unreachable_pool())
        .with_admin(profiles, Arc::new(FakeAudit::default()));
    let router: Router<AppState> = OpenApiRouter::new().routes(routes!(get_thing)).into();
    let app = router.with_state(state);
    let mut claims = idp.claims(ACCESS_SCOPE);
    claims["auth_time"] = json!(Utc::now().timestamp());
    let request = Request::get("/admin/v1/test/things/t-1")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", idp.token(&claims)),
        )
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        testing::send(&app, request).await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn nobody_is_an_administrator_and_nothing_is_recorded_by_default() {
    let state = testing::state(testing::unreachable_pool());
    assert!(
        state
            .admin_profiles
            .permissions_of(&state.db, "anyone")
            .await
            .unwrap()
            .is_empty()
    );
    let action = AdminAction {
        at: Utc::now(),
        actor: "a".into(),
        operation: "x.y".into(),
        target: json!({}),
        status: 200,
    };
    assert!(state.admin_audit.record(&state.db, &action).await.is_err());
}
