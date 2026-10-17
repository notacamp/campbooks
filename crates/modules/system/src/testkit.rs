//! Test fixtures shared by the admin operations: an in-memory admin
//! profile directory, an audit trail, and an app wired like the server's
//! admin router.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use axum::{
    Router,
    body::Body,
    http::{Method, Request, header},
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
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::Deps;

/// Accounts and their permission names, in memory.
#[derive(Debug, Default)]
pub struct Profiles(pub HashMap<&'static str, Vec<&'static str>>);

impl AdminProfiles for Profiles {
    fn permissions_of<'a>(&'a self, _db: &'a PgPool, account: &'a str) -> ProfilesFuture<'a> {
        let names = self.0.get(account).cloned().unwrap_or_default();
        Box::pin(async move { Ok(PermissionSet::from_names(names)) })
    }
}

#[derive(Debug, Default)]
pub struct Trail(pub Mutex<Vec<AdminAction>>);

impl AdminAudit for Trail {
    fn record<'a>(&'a self, _db: &'a PgPool, action: &'a AdminAction) -> AuditFuture<'a> {
        self.0.lock().unwrap().push(action.clone());
        Box::pin(async { Ok::<(), Error>(()) })
    }
}

pub struct World {
    pub idp: TestIdp,
    pub app: Router,
    pub trail: Arc<Trail>,
}

/// `ada` is a super admin, `ops` may only see jobs, `fixer` may only retry
/// them; anyone else is not an administrator.
pub async fn world(db: PgPool, deps: Deps) -> World {
    let idp = TestIdp::start().await;
    let trail = Arc::new(Trail::default());
    let profiles = Profiles(HashMap::from([
        ("ada", vec!["super"]),
        ("ops", vec!["jobs.read"]),
        ("fixer", vec!["jobs.retry"]),
    ]));
    let pool = testing::app_role_pool(&db).await;
    // The kernel's own schedules, for the schedule actions.
    let registry = campbooks_kernel::jobs::Registry::build(&[crate::module()])
        .unwrap_or_else(|e| unreachable!("the system module registers: {e}"));
    let mut state = idp
        .state(pool)
        .with_registry(registry)
        .with_admin(Arc::new(profiles), trail.clone());
    let config = Arc::make_mut(&mut state.config);
    config.database_url = "postgres://app:db-password@db.example.com/campbooks".into();
    config.signup.codes = Some("signup-code-1".into());
    let app = testing::admin_app(crate::module_with(deps), state);
    World { idp, app, trail }
}

impl World {
    /// Send a request as `sub`, signed in `auth_age_seconds` ago.
    pub async fn send(
        &self,
        method: Method,
        uri: &str,
        sub: &str,
        auth_age_seconds: i64,
        language: &str,
        body: Option<Value>,
    ) -> TestResponse {
        let mut claims = self.idp.claims(ACCESS_SCOPE);
        claims["sub"] = json!(sub);
        claims["auth_time"] = json!(Utc::now().timestamp() - auth_age_seconds);
        let builder = Request::builder()
            .method(method)
            .uri(uri)
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", self.idp.token(&claims)),
            )
            .header(header::ACCEPT_LANGUAGE, language);
        let request = match body {
            Some(body) => builder
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string())),
            None => builder.body(Body::empty()),
        }
        .unwrap();
        testing::send(&self.app, request).await
    }

    /// `GET`/`POST`/`DELETE` as a freshly signed-in `sub`, in English.
    pub async fn call(
        &self,
        method: Method,
        uri: &str,
        sub: &str,
        body: Option<Value>,
    ) -> TestResponse {
        self.send(method, uri, sub, 0, "en", body).await
    }
}
