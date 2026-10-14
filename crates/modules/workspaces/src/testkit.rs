//! Test fixtures: the module's routes over a database whose connections run
//! as the runtime role (so row-level security applies, as in production),
//! a test identity provider, and shortcuts for the common steps.

use std::sync::{Arc, Mutex};

use axum::{
    Router,
    body::Body,
    http::{Method, Request, header},
};
use campbooks_kernel::{
    AppState, Error,
    admin::{
        ACCESS_SCOPE, AdminAction, AdminAudit, AdminProfiles, AuditFuture, PermissionSet,
        ProfilesFuture,
    },
    auth::{Principal, PrincipalKind},
    blobs::BlobStore,
    config::{BlobsConfig, SignupPolicy},
    jobs::Registry,
    tenancy::{Membership as _, UserId, WorkspaceId},
    testing::{self, TestIdp, TestResponse},
};
use secrecy::SecretString;
use sqlx::PgPool;

use crate::features::{invite_member, membership::WorkspaceMembership};

/// Every scope of this module.
const ALL_SCOPES: &str = "workspaces:read workspaces:write workspaces:admin";

/// A caller: their bearer token and who they are.
#[derive(Debug, Clone)]
pub struct Caller {
    token: String,
    subject: String,
    email: Option<String>,
    kind: PrincipalKind,
}

impl std::ops::Deref for Caller {
    type Target = str;

    fn deref(&self) -> &str {
        &self.token
    }
}

/// The module over a fresh database.
pub struct Kit {
    /// The test database as its owner (a superuser: no row-level security),
    /// for setting up and inspecting rows.
    pub db: PgPool,
    /// The state handlers run with: connections as the runtime role.
    pub state: AppState,
    app: Router,
    admin_app: Router,
    /// What the admin API recorded.
    pub trail: Arc<Trail>,
    idp: TestIdp,
}

/// The administrators of the test world: `root` holds `super`, `people`
/// only `users.read`, `ops` only `jobs.read`; everyone else none.
#[derive(Debug, Default)]
struct Profiles;

impl AdminProfiles for Profiles {
    fn permissions_of<'a>(&'a self, _db: &'a PgPool, account: &'a str) -> ProfilesFuture<'a> {
        let names: &[&str] = match account {
            "root" => &["super"],
            "people" => &["users.read"],
            "ops" => &["jobs.read"],
            _ => &[],
        };
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

impl Kit {
    /// The module with the default (`open`) signup policy.
    pub async fn new(db: PgPool) -> Self {
        Self::with_signup(db, SignupPolicy::Open, None).await
    }

    /// The module under a signup policy (and codes, for `code`).
    pub async fn with_signup(db: PgPool, policy: SignupPolicy, codes: Option<&str>) -> Self {
        let idp = TestIdp::start().await;
        let pool = testing::app_role_pool(&db).await;
        let blobs = BlobStore::memory(&BlobsConfig::default(), pool.clone());
        let mut state = idp
            .state(pool)
            .with_blobs(blobs)
            .with_membership(crate::membership())
            .with_locales(crate::locale_preference())
            .with_registry(Registry::build(&[crate::module()]).unwrap());
        let config = Arc::make_mut(&mut state.config);
        config.signup.policy = policy;
        config.signup.codes = codes.map(SecretString::from);
        let app = testing::app(crate::module(), state.clone());
        let trail = Arc::new(Trail::default());
        let admin_app = testing::admin_app(
            crate::module(),
            state.clone().with_admin(Arc::new(Profiles), trail.clone()),
        );
        Self {
            db,
            state,
            app,
            admin_app,
            trail,
            idp,
        }
    }

    /// A user `sub` with every scope and a verified `<sub>@example.com`.
    pub fn user(&self, sub: &str) -> Caller {
        self.user_with(sub, &format!("{sub}@example.com"), true)
    }

    /// A user `sub` with every scope and the given email.
    pub fn user_with(&self, sub: &str, email: &str, verified: bool) -> Caller {
        let mut claims = self.idp.claims(ALL_SCOPES);
        claims["sub"] = sub.into();
        claims["email"] = email.into();
        claims["email_verified"] = verified.into();
        claims["preferred_username"] = sub.into();
        Caller {
            token: self.idp.token(&claims),
            subject: sub.to_owned(),
            email: Some(email.to_owned()),
            kind: PrincipalKind::User,
        }
    }

    /// A user `sub` granted only `scopes`.
    pub fn user_scopes(&self, sub: &str, scopes: &str) -> Caller {
        let mut claims = self.idp.claims(scopes);
        claims["sub"] = sub.into();
        Caller {
            token: self.idp.token(&claims),
            subject: sub.to_owned(),
            email: None,
            kind: PrincipalKind::User,
        }
    }

    /// A client-credentials token, bound to `workspace` when given.
    pub fn client(&self, workspace: Option<WorkspaceId>, scopes: &str) -> Caller {
        let mut claims = self.idp.claims(scopes);
        let object = claims.as_object_mut().unwrap();
        object.remove("sub");
        object.insert("azp".into(), "acme-sync".into());
        if let Some(workspace) = workspace {
            object.insert("workspace_id".into(), workspace.to_string().into());
        }
        Caller {
            token: self.idp.token(&claims),
            subject: "acme-sync".to_owned(),
            email: None,
            kind: PrincipalKind::Client,
        }
    }

    /// A token of `sub` for the admin API: `admin:access`, signed in just
    /// now. Whether it gets past the profile check depends on `sub`.
    pub fn admin(&self, sub: &str) -> String {
        let mut claims = self.idp.claims(&format!("{ALL_SCOPES} {ACCESS_SCOPE}"));
        claims["sub"] = sub.into();
        claims["auth_time"] = chrono::Utc::now().timestamp().into();
        self.idp.token(&claims)
    }

    /// `GET uri` on the admin API with `token`.
    pub async fn admin_get(&self, token: &str, uri: &str) -> TestResponse {
        let request = Request::get(uri)
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap();
        testing::send(&self.admin_app, request).await
    }

    /// `GET uri` with `token` (none when empty).
    pub async fn get(&self, token: &str, uri: &str) -> TestResponse {
        self.call(Method::GET, token, uri, None).await
    }

    /// `POST uri` with a JSON body.
    pub async fn post(&self, token: &str, uri: &str, body: serde_json::Value) -> TestResponse {
        self.call(Method::POST, token, uri, Some(body)).await
    }

    /// `PATCH uri` with a JSON body.
    pub async fn patch(&self, token: &str, uri: &str, body: serde_json::Value) -> TestResponse {
        self.call(Method::PATCH, token, uri, Some(body)).await
    }

    /// `DELETE uri`.
    pub async fn delete(&self, token: &str, uri: &str) -> TestResponse {
        self.call(Method::DELETE, token, uri, None).await
    }

    async fn call(
        &self,
        method: Method,
        token: &str,
        uri: &str,
        body: Option<serde_json::Value>,
    ) -> TestResponse {
        let mut request = Request::builder().method(method).uri(uri);
        if !token.is_empty() {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        let request = match body {
            Some(body) => request
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string())),
            None => request.body(Body::empty()),
        };
        testing::send(&self.app, request.unwrap()).await
    }

    /// Create a workspace through the API; returns its id.
    pub async fn create_workspace(&self, caller: &str, name: &str) -> WorkspaceId {
        let response = self
            .post(
                caller,
                "/api/v1/workspaces",
                serde_json::json!({ "name": name, "kind": "business" }),
            )
            .await;
        assert_eq!(response.status.as_u16(), 201, "{:?}", response.json);
        response.json["id"].as_str().unwrap().parse().unwrap()
    }

    /// Invite `email` as `role` on behalf of `inviter`; returns the token
    /// (which the API never shows).
    /// `caller` as a person's token bound to `workspace` (a third-party app
    /// they allowed there; only the in-process issuer mints these).
    pub fn bound(caller: &Caller, workspace: WorkspaceId) -> Principal {
        Principal {
            subject: caller.subject.clone(),
            kind: PrincipalKind::User,
            client_id: Some("dcr_agent".to_owned()),
            scopes: std::collections::BTreeSet::new(),
            workspace: Some(workspace.to_string()),
            email: caller.email.clone(),
            email_verified: true,
            name: None,
            expires_at: None,
            auth_time: None,
        }
    }

    pub async fn invite(
        &self,
        inviter: &Caller,
        workspace: WorkspaceId,
        email: &str,
        role: &str,
    ) -> String {
        let principal = Principal {
            subject: inviter.subject.clone(),
            kind: inviter.kind,
            client_id: None,
            scopes: std::collections::BTreeSet::new(),
            workspace: None,
            email: inviter.email.clone(),
            email_verified: true,
            name: None,
            expires_at: None,
            auth_time: None,
        };
        let access = WorkspaceMembership
            .resolve(&self.state.db, &principal, workspace)
            .await
            .unwrap()
            .expect("the inviter is a member");
        let input = invite_member::InviteMember {
            email: email.to_owned(),
            role: role.parse().unwrap(),
        };
        invite_member::execute(&self.state, &access, input)
            .await
            .unwrap()
            .token
    }

    /// Invite user `sub` as `role` and accept; returns their caller.
    pub async fn join(
        &self,
        inviter: &Caller,
        workspace: WorkspaceId,
        sub: &str,
        role: &str,
    ) -> Caller {
        let caller = self.user(sub);
        let token = self
            .invite(inviter, workspace, &format!("{sub}@example.com"), role)
            .await;
        let response = self
            .post(
                &caller,
                "/api/v1/workspaces/invitations/accept",
                serde_json::json!({ "token": token }),
            )
            .await;
        assert_eq!(response.status.as_u16(), 200, "{:?}", response.json);
        caller
    }

    /// The id of user `sub`.
    pub async fn user_id(&self, sub: &str) -> UserId {
        sqlx::query_scalar!(
            r#"SELECT id AS "id: UserId" FROM workspaces.users WHERE idp_subject = $1"#,
            sub
        )
        .fetch_one(&self.db)
        .await
        .unwrap()
    }

    /// The stored email of user `sub`.
    pub async fn user_email(&self, sub: &str) -> Option<String> {
        sqlx::query_scalar!(
            "SELECT email FROM workspaces.users WHERE idp_subject = $1",
            sub
        )
        .fetch_one(&self.db)
        .await
        .unwrap()
    }

    /// How many users exist, and how many `signed_in` events.
    pub async fn count_users_and_sign_ins(&self) -> (i64, i64) {
        let row = sqlx::query!(
            r#"SELECT (SELECT count(*) FROM workspaces.users) AS "users!",
                      (SELECT count(*) FROM workspaces.security_events
                        WHERE kind = 'signed_in') AS "sign_ins!""#
        )
        .fetch_one(&self.db)
        .await
        .unwrap();
        (row.users, row.sign_ins)
    }

    /// The names of the logged events of `workspace`, oldest first.
    pub async fn events_of(&self, workspace: &str) -> Vec<String> {
        sqlx::query_scalar!(
            "SELECT name FROM kernel.events WHERE workspace_id = $1 ORDER BY id",
            workspace
        )
        .fetch_all(&self.db)
        .await
        .unwrap()
    }

    /// The payload of the newest logged event of `workspace`.
    pub async fn last_event_payload(&self, workspace: &str) -> serde_json::Value {
        sqlx::query_scalar!(
            "SELECT payload FROM kernel.events WHERE workspace_id = $1 ORDER BY id DESC LIMIT 1",
            workspace
        )
        .fetch_one(&self.db)
        .await
        .unwrap()
    }

    /// The security-event kinds of `workspace`, oldest first.
    pub async fn security_kinds(&self, workspace: &str) -> Vec<String> {
        let workspace: WorkspaceId = workspace.parse().unwrap();
        sqlx::query_scalar!(
            "SELECT kind FROM workspaces.security_events WHERE workspace_id = $1 ORDER BY at, id",
            workspace as WorkspaceId
        )
        .fetch_all(&self.db)
        .await
        .unwrap()
    }

    /// The security-event kinds of user `sub` that belong to no workspace.
    pub async fn instance_security_kinds(&self, sub: &str) -> Vec<String> {
        sqlx::query_scalar!(
            "SELECT e.kind FROM workspaces.security_events e
               JOIN workspaces.users u ON u.id = e.user_id
              WHERE u.idp_subject = $1 AND e.workspace_id IS NULL
              ORDER BY e.at, e.id",
            sub
        )
        .fetch_all(&self.db)
        .await
        .unwrap()
    }

    /// Rows of `workspace`: (workspace rows, members, invitations).
    pub async fn rows_of(&self, workspace: WorkspaceId) -> (i64, i64, i64) {
        let row = sqlx::query!(
            r#"SELECT (SELECT count(*) FROM workspaces.workspaces WHERE id = $1) AS "w!",
                      (SELECT count(*) FROM workspaces.members WHERE workspace_id = $1) AS "m!",
                      (SELECT count(*) FROM workspaces.invitations WHERE workspace_id = $1) AS "i!""#,
            workspace as WorkspaceId
        )
        .fetch_one(&self.db)
        .await
        .unwrap();
        (row.w, row.m, row.i)
    }

    /// Blobs stored for `workspace`.
    pub async fn blobs_of(&self, workspace: WorkspaceId) -> i64 {
        sqlx::query_scalar!(
            r#"SELECT count(*) AS "n!" FROM kernel.blobs WHERE workspace_id = $1"#,
            workspace.to_string()
        )
        .fetch_one(&self.db)
        .await
        .unwrap()
    }
}
