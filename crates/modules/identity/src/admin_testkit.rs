//! A world for the admin operations (ADR 0015): the module's admin API over
//! a database whose connections run as the runtime role, a test identity
//! provider for administrators' tokens, admin profiles and an audit trail in
//! memory, and accounts written straight to the database.

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
use chrono::{TimeDelta, Utc};
use sqlx::PgPool;
use uuid::Uuid;

/// Admin profiles by account id.
#[derive(Debug, Default)]
struct Profiles(HashMap<String, Vec<&'static str>>);

impl AdminProfiles for Profiles {
    fn permissions_of<'a>(&'a self, _db: &'a PgPool, account: &'a str) -> ProfilesFuture<'a> {
        let names = self.0.get(account).cloned().unwrap_or_default();
        Box::pin(async move { Ok(PermissionSet::from_names(names)) })
    }
}

/// The admin audit trail, in memory.
#[derive(Debug, Default)]
pub struct Trail(Mutex<Vec<AdminAction>>);

impl Trail {
    /// `(actor, operation, status)` of every recorded request.
    pub fn entries(&self) -> Vec<(String, String, u16)> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .map(|a| (a.actor.clone(), a.operation.clone(), a.status))
            .collect()
    }
}

impl AdminAudit for Trail {
    fn record<'a>(&'a self, _db: &'a PgPool, action: &'a AdminAction) -> AuditFuture<'a> {
        self.0.lock().unwrap().push(action.clone());
        Box::pin(async { Ok::<(), Error>(()) })
    }
}

/// The world: `root` holds `super`, `viewer` only `users.read`; accounts
/// given to [`AdminKit::new`] as administrators hold `system.read`.
pub struct AdminKit {
    /// The test database as its owner, for setting up and inspecting rows.
    pub db: PgPool,
    /// What the admin API recorded.
    pub trail: Arc<Trail>,
    idp: TestIdp,
    app: Router,
}

impl AdminKit {
    /// The world over `db`, where `administrators` (account ids) have an
    /// admin profile.
    pub async fn new(db: PgPool, administrators: &[Uuid]) -> Self {
        let idp = TestIdp::start().await;
        let trail = Arc::new(Trail::default());
        let mut profiles = HashMap::from([
            ("root".to_owned(), vec!["super"]),
            ("viewer".to_owned(), vec!["users.read"]),
        ]);
        for account in administrators {
            profiles.insert(account.to_string(), vec!["system.read"]);
        }
        let state = idp
            .state(testing::app_role_pool(&db).await)
            .with_admin(Arc::new(Profiles(profiles)), trail.clone());
        let app = testing::admin_app(crate::module(crate::Deps::default()), state);
        Self {
            db,
            trail,
            idp,
            app,
        }
    }

    /// `method uri` on the admin API as `sub` (an `admin:access` token,
    /// signed in just now).
    pub async fn call(&self, sub: &str, method: Method, uri: &str, language: &str) -> TestResponse {
        let mut claims = self.idp.claims(ACCESS_SCOPE);
        claims["sub"] = sub.into();
        claims["auth_time"] = Utc::now().timestamp().into();
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", self.idp.token(&claims)),
            )
            .header(header::ACCEPT_LANGUAGE, language)
            .body(Body::empty())
            .unwrap();
        testing::send(&self.app, request).await
    }

    /// An account named `name` with a Google way in, a passkey, a live
    /// issuer session and an ended one, and a refresh-token family.
    pub async fn account(&self, name: &str) -> Uuid {
        let account = Uuid::now_v7();
        let now = Utc::now();
        let email = format!("{name}@example.com");
        sqlx::query(
            "INSERT INTO identity.accounts
                 (id, display_name, primary_email, primary_email_verified, created_at)
             VALUES ($1, $2, $3, true, $4)",
        )
        .bind(account)
        .bind(name)
        .bind(&email)
        .bind(now - TimeDelta::days(30))
        .execute(&self.db)
        .await
        .unwrap();
        let identity = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO identity.provider_identities
                 (id, account_id, provider, issuer, subject, email_at_link,
                  email_verified_at_link, linked_via, linked_at, last_used_at)
             VALUES ($1, $2, 'google', 'https://accounts.google.com', $3, $4, true,
                     'sign_up', $5, $6)",
        )
        .bind(identity)
        .bind(account)
        .bind(format!("google-{name}"))
        .bind(&email)
        .bind(now - TimeDelta::days(30))
        .bind(now - TimeDelta::hours(1))
        .execute(&self.db)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO identity.passkeys
                 (id, account_id, credential_id, public_key, alg, aaguid, backup_eligible,
                  backup_state, name, created_at)
             VALUES ($1, $2, $3, 'public-key-bytes', -7, $1, true, true, 'Laptop', $4)",
        )
        .bind(Uuid::now_v7())
        .bind(account)
        .bind(Uuid::now_v7().as_bytes().to_vec())
        .bind(now)
        .execute(&self.db)
        .await
        .unwrap();
        for (ended, created) in [(true, TimeDelta::days(3)), (false, TimeDelta::hours(1))] {
            let session = Uuid::now_v7();
            sqlx::query(
                "INSERT INTO identity.sessions
                     (id, token_hash, account_id, method, provider_identity_id, auth_time,
                      created_at, last_seen_at, idle_expires_at, expires_at, ended_at)
                 VALUES ($1, $2, $3, 'provider', $4, $5, $5, $5, $6, $6,
                         CASE WHEN $7 THEN $5 END)",
            )
            .bind(session)
            .bind(format!("session-secret-{session}").into_bytes())
            .bind(account)
            .bind(identity)
            .bind(now - created)
            .bind(now + TimeDelta::days(7))
            .bind(ended)
            .execute(&self.db)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO identity.refresh_families
                     (id, client_id, account_id, session_id, scope, auth_time, created_at,
                      expires_at)
                 VALUES ($1, 'campbooks-web', $2, $3, 'openid', $4, $4, $5)",
            )
            .bind(Uuid::now_v7())
            .bind(account)
            .bind(session)
            .bind(now - created)
            .bind(now + TimeDelta::days(30))
            .execute(&self.db)
            .await
            .unwrap();
        }
        account
    }

    /// How many rows `sql` (with `$1` = `account`) counts.
    pub async fn count(&self, sql: &'static str, account: Uuid) -> i64 {
        sqlx::query_scalar(sql)
            .bind(account)
            .fetch_one(&self.db)
            .await
            .unwrap()
    }
}
