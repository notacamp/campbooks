//! Tenancy: which workspace a request works in, and whether its caller may.
//!
//! Workspace-owned resources live under `/api/v1/w/{workspace_id}/…`. The
//! [`Workspace<S>`] extractor validates the token and scope (as
//! [`Scoped<S>`]), reads the workspace from the path and asks the
//! [`Membership`] port whether the caller belongs to it. The port is
//! implemented by the `workspaces` module and registered in
//! [`AppState`] by the server, so the kernel never depends on that module
//! (ADR 0002, 0008).
//!
//! ```text
//! pub async fn handler(ws: Workspace<MailRead>, State(state): State<AppState>) -> Result<…> {
//!     let mut tx = state.db.begin_for(ws.id).await?;   // row-level security on
//!     …
//! }
//! ```
//!
//! A caller who may not see the workspace gets 404, whatever the reason.

use std::{future::Future, marker::PhantomData, pin::Pin, sync::Arc};

use axum::{
    extract::{FromRequestParts, RawPathParams},
    http::request::Parts,
};
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, PgPool};
use utoipa::ToSchema;

use crate::{
    AppState, Error,
    auth::{AuthFailure, Principal, PrincipalKind, ScopeName, Scoped},
};

crate::define_id!(
    /// A workspace: a household or business whose data is kept together and
    /// apart from everyone else's.
    WorkspaceId
);

crate::define_id!(
    /// A user: someone who signs in, known by their identity-provider subject.
    UserId
);

/// The name of the path parameter that carries the workspace.
pub const WORKSPACE_PATH_PARAM: &str = "workspace_id";

/// A member's role in a workspace.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Full control, including deleting the workspace. Every workspace keeps
    /// at least one.
    Owner,
    /// Manages members, invitations and settings.
    Admin,
    /// Uses the workspace.
    Member,
}

impl Role {
    /// The wire and database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Admin => "admin",
            Self::Member => "member",
        }
    }

    /// Whether this role manages members, invitations and settings.
    pub const fn can_manage(self) -> bool {
        matches!(self, Self::Owner | Self::Admin)
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Role {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "owner" => Ok(Self::Owner),
            "admin" => Ok(Self::Admin),
            "member" => Ok(Self::Member),
            other => Err(Error::internal(format!("unknown role `{other}`"))),
        }
    }
}

/// What the caller may do in a workspace, as the [`Membership`] port
/// resolved it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Access {
    /// The workspace.
    pub workspace: WorkspaceId,
    /// The calling user; `None` for a client token.
    pub user: Option<UserId>,
    /// The user's role; `None` for a client token, which acts through its
    /// scopes only.
    pub role: Option<Role>,
}

/// A future returned by a [`Membership`] port.
pub type MembershipFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Option<Access>, Error>> + Send + 'a>>;

/// A live workspace someone belongs to, as the [`Membership`] port lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSummary {
    /// The workspace.
    pub id: WorkspaceId,
    /// Its name.
    pub name: String,
    /// Their role there.
    pub role: Role,
}

/// A future returned by [`Membership::workspaces_of`].
pub type WorkspacesFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Vec<WorkspaceSummary>, Error>> + Send + 'a>>;

/// A future returned by [`Membership::role_held`].
pub type RoleFuture<'a> = Pin<Box<dyn Future<Output = Result<Option<Role>, Error>> + Send + 'a>>;

/// The port that answers "may this principal work in this workspace?".
/// Implemented by the `workspaces` module; the server registers it with
/// [`AppState::with_membership`].
pub trait Membership: std::fmt::Debug + Send + Sync + 'static {
    /// For a user, their membership of the live workspace `workspace`; for a
    /// client (whose token claim already matched), whether the workspace is
    /// live. `None` when the caller may not see it.
    fn resolve<'a>(
        &'a self,
        db: &'a PgPool,
        principal: &'a Principal,
        workspace: WorkspaceId,
    ) -> MembershipFuture<'a>;

    /// The live workspaces of the person signed in as `subject` (their
    /// identity-provider subject), oldest first; none when they are not
    /// known. For screens served outside any workspace (the consent screen
    /// asks which workspace an app may use).
    fn workspaces_of<'a>(&'a self, _db: &'a PgPool, _subject: &'a str) -> WorkspacesFuture<'a> {
        Box::pin(async { Ok(Vec::new()) })
    }

    /// The role of `user` in `workspace`, read in `conn` (a transaction
    /// working in that workspace) and held there until it ends: removing
    /// them waits for it. `None` when they are not a member. For work done
    /// later on a member's behalf (a scheduled send), which must stop once
    /// they are gone.
    fn role_held<'a>(
        &'a self,
        _conn: &'a mut PgConnection,
        _workspace: WorkspaceId,
        _user: UserId,
    ) -> RoleFuture<'a> {
        Box::pin(async { Ok(None) })
    }

    /// Record types (`<module>.<record>`) whose changes may change who
    /// belongs to a workspace. An open change stream that sees one resolves
    /// the caller's membership again and ends when it is gone.
    fn access_records(&self) -> &'static [&'static str] {
        &[]
    }
}

/// The port before any module provides one: nobody belongs anywhere.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoMembership;

impl Membership for NoMembership {
    fn resolve<'a>(
        &'a self,
        _db: &'a PgPool,
        _principal: &'a Principal,
        _workspace: WorkspaceId,
    ) -> MembershipFuture<'a> {
        Box::pin(async { Ok(None) })
    }
}

/// Shared handle to the registered [`Membership`] port.
pub type MembershipPort = Arc<dyn Membership>;

/// A future returned by a [`KeptMail`] port.
pub type KeptMailFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Vec<crate::RecordRef>, Error>> + Send + 'a>>;

/// The port that answers, as a member is removed with their mail kept:
/// which containers (inboxes) would be left with nobody to read them.
/// Implemented by the module that owns them; until then, none.
pub trait KeptMail: std::fmt::Debug + Send + Sync + 'static {
    /// The containers `user`'s departure would leave unread if their mail
    /// were kept, when `others` are the members who stay. Asked in `conn`,
    /// the departure's own transaction, which holds what the answer rests
    /// on until it ends.
    fn unread_if_kept<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        workspace: WorkspaceId,
        user: UserId,
        others: &'a [UserId],
    ) -> KeptMailFuture<'a>;
}

/// The port before a module provides one.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoKeptMail;

impl KeptMail for NoKeptMail {
    fn unread_if_kept<'a>(
        &'a self,
        _conn: &'a mut PgConnection,
        _workspace: WorkspaceId,
        _user: UserId,
        _others: &'a [UserId],
    ) -> KeptMailFuture<'a> {
        Box::pin(async { Ok(Vec::new()) })
    }
}

/// Shared handle to the registered [`KeptMail`] port.
pub type KeptMailPort = Arc<dyn KeptMail>;

/// Whom a client token acts for (ADR 0017): a client a member registered in
/// their workspace acts as that member, within their memberships (never
/// more), re-resolved on every request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientActor {
    /// A client the instance does not register (another identity
    /// provider's): it acts through its own, narrower catalogue grants.
    Unregistered,
    /// A registered client acting for the person with this subject.
    ActsFor(String),
    /// A registered client that no longer exists, or acts for nobody: its
    /// tokens are refused (401).
    Revoked,
}

/// A future returned by a [`ClientDirectory`].
pub type ClientsFuture<'a> = Pin<Box<dyn Future<Output = Result<ClientActor, Error>> + Send + 'a>>;

/// The port that answers "whom does this client act for?". Implemented by
/// the issuer (the `identity` module) when it runs in this process.
pub trait ClientDirectory: std::fmt::Debug + Send + Sync + 'static {
    /// Whom `client_id`, bound to `workspace`, acts for.
    fn acting_for<'a>(
        &'a self,
        db: &'a PgPool,
        client_id: &'a str,
        workspace: WorkspaceId,
    ) -> ClientsFuture<'a>;
}

/// The port before the issuer provides one: every client is unregistered.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoClientDirectory;

impl ClientDirectory for NoClientDirectory {
    fn acting_for<'a>(
        &'a self,
        _db: &'a PgPool,
        _client_id: &'a str,
        _workspace: WorkspaceId,
    ) -> ClientsFuture<'a> {
        Box::pin(async { Ok(ClientActor::Unregistered) })
    }
}

/// Shared handle to the registered [`ClientDirectory`].
pub type ClientDirectoryPort = Arc<dyn ClientDirectory>;

/// What `principal` may do in `workspace`, as every entry point resolves it
/// (the [`Workspace`] extractor, idempotent replays, the change feed): a
/// user's membership; a client token's binding, then, for a client a member
/// registered, that member's membership (401 once they are gone or the
/// client is removed). `None` when the caller may not see the workspace.
///
/// # Errors
/// [`Error::Unauthorized`] for a revoked client; the ports' errors.
pub async fn resolve_access(
    membership: &dyn Membership,
    clients: &dyn ClientDirectory,
    db: &PgPool,
    principal: &Principal,
    workspace: WorkspaceId,
) -> Result<Option<Access>, Error> {
    if !may_reach(principal, workspace) {
        return Ok(None);
    }
    let access = if principal.kind == PrincipalKind::Client {
        let client = principal.client_id.as_deref().unwrap_or(&principal.subject);
        match clients.acting_for(db, client, workspace).await? {
            ClientActor::Unregistered => membership.resolve(db, principal, workspace).await?,
            ClientActor::ActsFor(subject) => {
                let creator = Principal {
                    subject,
                    kind: PrincipalKind::User,
                    workspace: None,
                    ..principal.clone()
                };
                match membership.resolve(db, &creator, workspace).await? {
                    Some(access) => Some(access),
                    None => return Err(Error::Unauthorized(AuthFailure::InvalidToken)),
                }
            }
            ClientActor::Revoked => return Err(Error::Unauthorized(AuthFailure::InvalidToken)),
        }
    } else {
        membership.resolve(db, principal, workspace).await?
    };
    Ok(access.filter(|access| access.workspace == workspace))
}

/// Requires a valid token granting scope `S` (exactly like [`Scoped<S>`])
/// whose caller may work in the workspace named by the `{workspace_id}` path
/// parameter.
///
/// - token or scope problems → 401 / 403, before the workspace is looked at
/// - `workspace_id` is not a UUID → 404
/// - a client token whose `workspace_id` claim is missing or different → 404
/// - a user token bound to another workspace (`workspace_id`, set by the
///   in-process issuer when a person allowed an app in one workspace) → 404
/// - a user who is not a member, or a deleted or unknown workspace → 404
///
/// Operations using it document the path parameter and
/// `security(("bearer" = ["<S>"]))`.
pub struct Workspace<S: ScopeName> {
    /// The authenticated caller.
    pub principal: Principal,
    /// The workspace from the path.
    pub id: WorkspaceId,
    /// What the caller may do there.
    pub access: Access,
    scope: PhantomData<fn() -> S>,
}

impl<S: ScopeName> std::fmt::Debug for Workspace<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Workspace")
            .field("scope", &S::NAME)
            .field("id", &self.id)
            .field("access", &self.access)
            .field("principal", &self.principal)
            .finish()
    }
}

impl<S: ScopeName> FromRequestParts<AppState> for Workspace<S> {
    type Rejection = Error;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let Scoped { principal, .. } = Scoped::<S>::from_request_parts(parts, state).await?;
        let params = RawPathParams::from_request_parts(parts, state)
            .await
            .map_err(|e| Error::internal(format!("workspace route without path params: {e}")))?;
        let raw = params
            .iter()
            .find_map(|(name, value)| (name == WORKSPACE_PATH_PARAM).then_some(value))
            .ok_or_else(|| {
                Error::internal(format!(
                    "workspace route without a `{{{WORKSPACE_PATH_PARAM}}}` path parameter"
                ))
            })?;
        let id: WorkspaceId = raw.parse().map_err(|_| Error::NotFound)?;
        let access = resolve_access(
            state.membership.as_ref(),
            state.clients.as_ref(),
            &state.db,
            &principal,
            id,
        )
        .await?
        .ok_or(Error::NotFound)?;
        tracing::Span::current().record(
            crate::telemetry::field::WORKSPACE_ID,
            tracing::field::display(id),
        );
        Ok(Self {
            principal,
            id,
            access,
            scope: PhantomData,
        })
    }
}

/// Whether the token may reach `workspace` at all: a client token must name
/// it; a user token that names a workspace (a third-party app the person
/// allowed in one workspace) may reach that one only; any other user token
/// leaves the question to membership.
fn may_reach(principal: &Principal, workspace: WorkspaceId) -> bool {
    match principal.kind {
        PrincipalKind::Client => client_bound_to(principal, workspace),
        PrincipalKind::User => {
            principal.workspace.is_none() || client_bound_to(principal, workspace)
        }
    }
}

/// Whether a token's `workspace_id` claim names `workspace`.
fn client_bound_to(principal: &Principal, workspace: WorkspaceId) -> bool {
    principal
        .workspace
        .as_deref()
        .and_then(|claim| claim.parse::<WorkspaceId>().ok())
        == Some(workspace)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    fn principal(kind: PrincipalKind, workspace: Option<&str>) -> Principal {
        Principal {
            subject: "s".to_owned(),
            kind,
            client_id: None,
            scopes: BTreeSet::new(),
            workspace: workspace.map(ToOwned::to_owned),
            email: None,
            email_verified: false,
            name: None,
            expires_at: None,
            issued_at: None,
            auth_time: None,
        }
    }

    #[test]
    fn clients_are_bound_to_their_claim() {
        let id = WorkspaceId::new();
        let other = WorkspaceId::new();
        let client = principal(PrincipalKind::Client, Some(&id.to_string()));
        assert!(client_bound_to(&client, id));
        assert!(!client_bound_to(&client, other));
        assert!(!client_bound_to(
            &principal(PrincipalKind::Client, None),
            id
        ));
        assert!(!client_bound_to(
            &principal(PrincipalKind::Client, Some("not-a-uuid")),
            id
        ));
    }

    #[test]
    fn a_bound_user_token_reaches_its_workspace_only() {
        let id = WorkspaceId::new();
        let other = WorkspaceId::new();
        let unbound = principal(PrincipalKind::User, None);
        assert!(may_reach(&unbound, id) && may_reach(&unbound, other));
        let bound = principal(PrincipalKind::User, Some(&id.to_string()));
        assert!(may_reach(&bound, id));
        assert!(!may_reach(&bound, other));
        let garbled = principal(PrincipalKind::User, Some("not-a-uuid"));
        assert!(
            !may_reach(&garbled, id),
            "a claim that names nothing reaches nothing"
        );
        let client = principal(PrincipalKind::Client, None);
        assert!(!may_reach(&client, id));
    }

    #[test]
    fn roles_round_trip() {
        for role in [Role::Owner, Role::Admin, Role::Member] {
            assert_eq!(role.as_str().parse::<Role>().unwrap(), role);
            assert_eq!(
                serde_json::to_value(role).unwrap(),
                serde_json::json!(role.as_str())
            );
        }
        assert!("root".parse::<Role>().is_err());
        assert!(Role::Owner.can_manage() && Role::Admin.can_manage());
        assert!(!Role::Member.can_manage());
    }

    #[tokio::test]
    async fn no_membership_admits_nobody() {
        let p = principal(PrincipalKind::User, None);
        let resolved = NoMembership
            .resolve(&crate::testing::unreachable_pool(), &p, WorkspaceId::new())
            .await
            .unwrap();
        assert_eq!(resolved, None);
    }
}
