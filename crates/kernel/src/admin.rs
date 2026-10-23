//! Instance administration (ADR 0015): who an administrator is, how an
//! admin request is authorised and audited, and how it may read across
//! workspaces.
//!
//! - An administrator is a user whose account has an **admin profile**: a
//!   set of [`Permission`]s, `super` implying all of them. Profiles are read
//!   through the [`AdminProfiles`] port (implemented by the `admin` module)
//!   and written only by the server's CLI: no HTTP endpoint touches them.
//! - Admin operations live in a separate API, `/admin/v1/<module>/…`, built
//!   from each module's [`Module::admin_router`](crate::Module::admin_router)
//!   and described by its own document (`openapi/admin.json`), never by the
//!   operation catalogue: [`invoke`](crate::operations::invoke), automation
//!   and the assistant cannot reach them.
//! - Every admin handler takes [`Admin<P>`], which answers **404** (as for a
//!   route that does not exist) unless the caller is a user whose token
//!   carries [`ACCESS_SCOPE`] and whose profile holds `P`, then **401**
//!   `admin_reauth_required` when they signed in more than
//!   [`REAUTH_AFTER`] ago.
//! - The [`audit`] middleware records every request that passed the profile
//!   check through the [`AdminAudit`] port.
//! - [`begin_admin`](crate::db::begin_admin) opens a transaction that sees
//!   other workspaces' rows on tables carrying an admin-read policy; only an
//!   [`AdminGrant`] (which only [`Admin<P>`] builds) opens one.
//!
//! ```text
//! #[utoipa::path(
//!     get,
//!     path = "/admin/v1/system/overview",
//!     operation_id = "system.admin_overview",
//!     security(("admin" = ["admin:access"])),
//!     extensions(("x-campbooks-admin" = json!(AdminOperation::query::<SystemRead>()))),
//!     responses(…),
//! )]
//! pub async fn handler(admin: Admin<SystemRead>, …) -> Result<Json<…>> { … }
//! ```

use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    marker::PhantomData,
    pin::Pin,
    sync::{Arc, Mutex, PoisonError},
};

use axum::{
    extract::{FromRequestParts, MatchedPath, RawPathParams, Request, State},
    http::{Method, request::Parts},
    middleware::Next,
    response::{IntoResponse as _, Response},
};
use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;

use crate::{
    AppState, Error,
    auth::{AuthFailure, Principal, PrincipalKind, bearer_token},
};

/// The OAuth scope every admin request's token must carry. Declared by the
/// `admin` module; the built-in issuer grants it only to first-party
/// clients.
pub const ACCESS_SCOPE: &str = "admin:access";

/// Every admin operation's path starts with this, then its module's name.
pub const PATH_PREFIX: &str = "/admin/v1/";

/// Name of the admin document's security scheme:
/// `security(("admin" = ["admin:access"]))`.
pub const SECURITY_SCHEME: &str = "admin";

/// Name of the OpenAPI extension carrying an [`AdminOperation`].
pub const EXTENSION: &str = "x-campbooks-admin";

/// How long after signing in a person may still use the admin API.
pub const REAUTH_AFTER: TimeDelta = TimeDelta::hours(1);

/// What an administrator may do. A closed set with stable wire names; a
/// permission is never an OAuth scope and never appears in a token.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, ToSchema,
)]
pub enum Permission {
    /// Everything, including permissions added later.
    #[serde(rename = "super")]
    Super,
    /// See the system overview (version, configuration, health).
    #[serde(rename = "system.read")]
    SystemRead,
    /// Approve or refuse signup requests.
    #[serde(rename = "signups.review")]
    SignupsReview,
    /// See accounts.
    #[serde(rename = "users.read")]
    UsersRead,
    /// Suspend and restore accounts.
    #[serde(rename = "users.suspend")]
    UsersSuspend,
    /// See workspaces (metadata only).
    #[serde(rename = "workspaces.read")]
    WorkspacesRead,
    /// See background jobs.
    #[serde(rename = "jobs.read")]
    JobsRead,
    /// Retry failed background jobs.
    #[serde(rename = "jobs.retry")]
    JobsRetry,
}

impl Permission {
    /// Every permission, `super` first.
    pub const ALL: [Self; 8] = [
        Self::Super,
        Self::SystemRead,
        Self::SignupsReview,
        Self::UsersRead,
        Self::UsersSuspend,
        Self::WorkspacesRead,
        Self::JobsRead,
        Self::JobsRetry,
    ];

    /// The wire name (`super`, `system.read`, …).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Super => "super",
            Self::SystemRead => "system.read",
            Self::SignupsReview => "signups.review",
            Self::UsersRead => "users.read",
            Self::UsersSuspend => "users.suspend",
            Self::WorkspacesRead => "workspaces.read",
            Self::JobsRead => "jobs.read",
            Self::JobsRetry => "jobs.retry",
        }
    }

    /// The permission named `name`, if any.
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.as_str() == name)
    }
}

impl std::fmt::Display for Permission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The permissions of one admin profile.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PermissionSet(BTreeSet<Permission>);

impl PermissionSet {
    /// Whether the set allows `permission`: it holds it, or `super`.
    pub fn allows(&self, permission: Permission) -> bool {
        self.0.contains(&Permission::Super) || self.0.contains(&permission)
    }

    /// Whether the set is empty (no profile).
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The permissions, in order.
    pub fn iter(&self) -> impl Iterator<Item = Permission> + '_ {
        self.0.iter().copied()
    }

    /// The wire names, in order.
    pub fn names(&self) -> Vec<&'static str> {
        self.iter().map(Permission::as_str).collect()
    }

    /// Parse wire names, ignoring unknown ones (a profile written by a newer
    /// server keeps working with the permissions this one knows).
    pub fn from_names<I, S>(names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        names
            .into_iter()
            .filter_map(|name| Permission::parse(name.as_ref()))
            .collect()
    }
}

impl FromIterator<Permission> for PermissionSet {
    fn from_iter<T: IntoIterator<Item = Permission>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}

/// A permission an admin handler requires, declared once as a type (like
/// [`ScopeName`](crate::auth::ScopeName)): the `P` of [`Admin<P>`].
pub trait AdminPermission: Send + Sync + 'static {
    /// The permission.
    const PERMISSION: Permission;
}

/// One marker type per [`Permission`], for [`Admin<P>`].
pub mod permission {
    use super::{AdminPermission, Permission};

    macro_rules! marker {
        ($($(#[$doc:meta])* $name:ident),* $(,)?) => {$(
            $(#[$doc])*
            #[derive(Debug, Clone, Copy)]
            pub struct $name;
            impl AdminPermission for $name {
                const PERMISSION: Permission = Permission::$name;
            }
        )*};
    }

    marker!(
        /// `super`.
        Super,
        /// `system.read`.
        SystemRead,
        /// `signups.review`.
        SignupsReview,
        /// `users.read`.
        UsersRead,
        /// `users.suspend`.
        UsersSuspend,
        /// `workspaces.read`.
        WorkspacesRead,
        /// `jobs.read`.
        JobsRead,
        /// `jobs.retry`.
        JobsRetry,
    );
}

/// The `x-campbooks-admin` metadata of one admin operation:
/// `extensions(("x-campbooks-admin" = json!(AdminOperation::query::<SystemRead>())))`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdminOperation {
    /// The permission its [`Admin<P>`] extractor requires.
    pub permission: Permission,
    /// Whether it changes state (`true` only for POST/PUT/PATCH/DELETE).
    pub mutates: bool,
}

impl AdminOperation {
    /// A read requiring `P`.
    pub fn query<P: AdminPermission>() -> Self {
        Self {
            permission: P::PERMISSION,
            mutates: false,
        }
    }

    /// A change requiring `P`.
    pub fn command<P: AdminPermission>() -> Self {
        Self {
            permission: P::PERMISSION,
            mutates: true,
        }
    }
}

// --- ports ---------------------------------------------------------------------

/// A future returned by [`AdminProfiles::permissions_of`].
pub type ProfilesFuture<'a> =
    Pin<Box<dyn Future<Output = Result<PermissionSet, Error>> + Send + 'a>>;

/// Reads admin profiles. Implemented by the `admin` module; read only.
pub trait AdminProfiles: std::fmt::Debug + Send + Sync + 'static {
    /// The permissions of `account` (an identity account id, the token's
    /// `sub`); empty when it has no profile.
    fn permissions_of<'a>(&'a self, db: &'a PgPool, account: &'a str) -> ProfilesFuture<'a>;
}

/// The port before a module provides one: nobody is an administrator.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoAdmins;

impl AdminProfiles for NoAdmins {
    fn permissions_of<'a>(&'a self, _db: &'a PgPool, _account: &'a str) -> ProfilesFuture<'a> {
        Box::pin(async { Ok(PermissionSet::default()) })
    }
}

/// The registered admin-profile port.
pub type AdminProfilesPort = Arc<dyn AdminProfiles>;

/// One admin request, as the audit trail keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminAction {
    /// When it was answered.
    pub at: DateTime<Utc>,
    /// The administrator's account id.
    pub actor: String,
    /// The operation's `operationId`.
    pub operation: String,
    /// Its path parameters (ids only), as a JSON object.
    pub target: serde_json::Value,
    /// The response status.
    pub status: u16,
}

/// A future returned by [`AdminAudit::record`].
pub type AuditFuture<'a> = Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'a>>;

/// Appends to the admin audit trail. Implemented by the `admin` module.
pub trait AdminAudit: std::fmt::Debug + Send + Sync + 'static {
    /// Record `action`.
    fn record<'a>(&'a self, db: &'a PgPool, action: &'a AdminAction) -> AuditFuture<'a>;
}

/// The port before a module provides one: nothing can be recorded, so no
/// admin request is answered (fail closed).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoAudit;

impl AdminAudit for NoAudit {
    fn record<'a>(&'a self, _db: &'a PgPool, _action: &'a AdminAction) -> AuditFuture<'a> {
        Box::pin(async { Err(Error::internal("no admin audit trail is registered")) })
    }
}

/// The registered audit port.
pub type AdminAuditPort = Arc<dyn AdminAudit>;

// --- the extractor ---------------------------------------------------------------

/// Proof that the request comes from an administrator: their account and
/// permissions. Only [`Admin<P>`] builds one; it opens
/// [`begin_admin`](crate::db::begin_admin) transactions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminGrant {
    account: String,
    permissions: PermissionSet,
}

impl AdminGrant {
    pub(crate) fn new(account: String, permissions: PermissionSet) -> Self {
        Self {
            account,
            permissions,
        }
    }

    /// The administrator's identity account id.
    pub fn account(&self) -> &str {
        &self.account
    }

    /// Everything their profile allows.
    pub fn permissions(&self) -> &PermissionSet {
        &self.permissions
    }
}

/// Requires an administrator holding `P` (or `super`):
///
/// 1. a valid bearer token for a **user** (not a client-credentials client),
/// 2. carrying [`ACCESS_SCOPE`],
/// 3. whose account's admin profile allows `P`,
///
/// otherwise **404**, the answer to a route that does not exist, so the API
/// tells nobody who is an administrator. Then
///
/// 4. the token's `auth_time` at most [`REAUTH_AFTER`] ago (or absent),
///    otherwise **401** `admin_reauth_required`.
///
/// It works only behind the [`audit`] middleware (the server's admin
/// router); anywhere else it answers 404.
pub struct Admin<P: AdminPermission> {
    /// The proof of the grant, for [`begin_admin`](crate::db::begin_admin).
    pub grant: AdminGrant,
    /// The authenticated caller.
    pub principal: Principal,
    permission: PhantomData<fn() -> P>,
}

impl<P: AdminPermission> std::fmt::Debug for Admin<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Admin")
            .field("permission", &P::PERMISSION)
            .field("grant", &self.grant)
            .finish_non_exhaustive()
    }
}

impl<P: AdminPermission> FromRequestParts<AppState> for Admin<P> {
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Error> {
        // Only on the audited admin router.
        let Some(slot) = parts.extensions.get::<AuditSlot>().cloned() else {
            return Err(Error::NotFound);
        };
        // A token, never a principal authenticated in-process: nothing
        // invokes admin operations on someone's behalf.
        let token = bearer_token(&parts.headers).map_err(|_| Error::NotFound)?;
        let principal = state
            .auth
            .verify(token)
            .await
            .map_err(|_| Error::NotFound)?;
        // A person's own token only: never a client's, nor one an app holds
        // for one workspace.
        if principal.kind != PrincipalKind::User
            || principal.bound_workspace().is_some()
            || !principal.has_scope(ACCESS_SCOPE)
        {
            return Err(Error::NotFound);
        }
        let permissions = state
            .admin_profiles
            .permissions_of(&state.db, &principal.subject)
            .await?;
        if !permissions.allows(P::PERMISSION) {
            return Err(Error::NotFound);
        }
        let grant = AdminGrant::new(principal.subject.clone(), permissions);
        slot.fill(grant.clone());
        // What the request tracks is the administrator's, by account id.
        crate::tracking::administrator(&principal.subject);
        if !recently_authenticated(principal.auth_time, Utc::now()) {
            return Err(Error::Unauthorized(AuthFailure::AdminReauthRequired));
        }
        Ok(Self {
            grant,
            principal,
            permission: PhantomData,
        })
    }
}

/// Whether a person who authenticated at `auth_time` may use the admin API
/// at `now`. Unknown means no.
fn recently_authenticated(auth_time: Option<DateTime<Utc>>, now: DateTime<Utc>) -> bool {
    auth_time.is_some_and(|at| now - at <= REAUTH_AFTER)
}

// --- audit ---------------------------------------------------------------------

/// Where [`Admin<P>`] leaves its grant for the [`audit`] middleware. Private
/// to the kernel, so only the middleware can make a route an admin route.
#[derive(Debug, Clone, Default)]
struct AuditSlot(Arc<Mutex<Option<AdminGrant>>>);

impl AuditSlot {
    fn fill(&self, grant: AdminGrant) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = Some(grant);
    }

    fn take(&self) -> Option<AdminGrant> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).take()
    }
}

/// What the [`audit`] middleware needs: the state (its database and audit
/// port) and each admin route's `operationId`.
#[derive(Debug, Clone)]
pub struct AuditLayer {
    state: AppState,
    operations: Arc<BTreeMap<(Method, String), String>>,
}

impl AuditLayer {
    /// The layer for the admin API described by `spec` (paths →
    /// `operationId`s), recording through `state`'s audit port.
    pub fn new(state: &AppState, spec: &utoipa::openapi::OpenApi) -> Self {
        let mut operations = BTreeMap::new();
        if let Ok(serde_json::Value::Object(paths)) = serde_json::to_value(&spec.paths.paths) {
            for (path, item) in paths {
                let Some(item) = item.as_object() else {
                    continue;
                };
                for (method, operation) in item {
                    let (Ok(method), Some(id)) = (
                        method.to_uppercase().parse::<Method>(),
                        operation
                            .get("operationId")
                            .and_then(serde_json::Value::as_str),
                    ) else {
                        continue;
                    };
                    operations.insert((method, path.clone()), id.to_owned());
                }
            }
        }
        Self {
            state: state.clone(),
            operations: Arc::new(operations),
        }
    }
}

/// Records every admin request that passed the profile check of
/// [`Admin<P>`]: actor, `operationId`, path parameters, status and time.
/// Requests turned away before it (404) leave no row: they have no actor.
/// When the row cannot be written the answer is replaced by a 500, so no
/// admin answer leaves unrecorded. Install it with `route_layer` on the
/// admin router (it reads the matched route).
pub async fn audit(State(layer): State<AuditLayer>, request: Request, next: Next) -> Response {
    let (mut parts, body) = request.into_parts();
    let operation = parts
        .extensions
        .get::<MatchedPath>()
        .and_then(|matched| {
            layer
                .operations
                .get(&(parts.method.clone(), matched.as_str().to_owned()))
        })
        .cloned();
    let target = match RawPathParams::from_request_parts(&mut parts, &()).await {
        Ok(params) => params
            .iter()
            .map(|(name, value)| (name.to_owned(), serde_json::Value::from(value)))
            .collect::<serde_json::Map<_, _>>(),
        Err(_) => serde_json::Map::new(),
    };
    let slot = AuditSlot::default();
    parts.extensions.insert(slot.clone());
    let response = next.run(Request::from_parts(parts, body)).await;
    let Some(grant) = slot.take() else {
        return response;
    };
    let action = AdminAction {
        at: Utc::now(),
        actor: grant.account,
        operation: operation.unwrap_or_else(|| "unknown".to_owned()),
        target: serde_json::Value::Object(target),
        status: response.status().as_u16(),
    };
    match layer
        .state
        .admin_audit
        .record(&layer.state.db, &action)
        .await
    {
        Ok(()) => response,
        Err(err) => {
            tracing::error!(error = %err, operation = %action.operation, "could not record an admin request; refusing its answer");
            Error::internal("admin audit failed").into_response()
        }
    }
}

#[cfg(test)]
mod tests;
