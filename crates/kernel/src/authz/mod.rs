//! Resource ownership and access: one catalogue, one check (ADR 0017).
//!
//! Every resource type a module serves is declared once, in the module's
//! descriptor ([`Module::resources`](crate::Module)), as a [`ResourceType`]:
//!
//! - its **owner** ([`Owner`]): the member who created or connected it, the
//!   workspace's owners, the account itself, or nobody;
//! - the **actions** the owner may take ([`Action`]);
//! - its **sharing** ([`Grant`]s): who else gets which actions, by
//!   workspace role, by role in the resource's container (an inbox's
//!   `reader`/`sender`/`manager`), as a client token bound to the workspace,
//!   or as an instance administrator;
//! - for a container, its **role ladder**, lowest first.
//!
//! [`authorize`] is the one check every handler (and every job acting for a
//! person) asks before it reads or changes a resource. It is deny by
//! default: an unknown type, a resource the owning module does not find, or
//! a caller none of the rules admits gets [`Decision::NotFound`]; a caller
//! who may see the resource but not take the action gets
//! [`Decision::Forbidden`]. Seeing is the `read` action: whoever may not read
//! a resource is never told it exists.
//!
//! The kernel knows no domain, so the owning module answers the *facts* of
//! one resource ([`ResourceFacts`]: does it exist in this workspace, who owns
//! it, which container it is in and the caller's role there, whether it is
//! frozen read-only) and the kernel applies the catalogue. Row-level
//! security stays underneath as the second layer (ADR 0011).
//!
//! ```text
//! let mut tx = state.db.begin_for(ws.id).await?;
//! authz::require(&state, &mut tx, &ws.access, Action::Write,
//!                &RecordRef::new("conversations.conversation", &id)).await?;
//! ```

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    future::Future,
    pin::Pin,
    sync::Arc,
};

use serde::{Deserialize, Serialize};
use sqlx::PgConnection;

use crate::{
    AppState, Error,
    records::RecordRef,
    tenancy::{Access, Role, UserId},
};

/// What a caller does to a resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// See it (and know it exists).
    Read,
    /// Change it.
    Write,
    /// Delete it.
    Delete,
    /// Send from it, or send it (mail).
    Send,
    /// Give others access to it.
    Share,
    /// Cut its link to the outside account (a connection).
    Disconnect,
    /// Make a new one, becoming its owner (asked of the type, with
    /// [`authorize_type`]).
    Create,
}

impl Action {
    /// Every action, in order.
    pub const ALL: [Self; 7] = [
        Self::Read,
        Self::Write,
        Self::Delete,
        Self::Send,
        Self::Share,
        Self::Disconnect,
        Self::Create,
    ];

    /// The wire name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Delete => "delete",
            Self::Send => "send",
            Self::Share => "share",
            Self::Disconnect => "disconnect",
            Self::Create => "create",
        }
    }

    /// The action named `name`.
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|action| action.as_str() == name)
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Where a resource lives, and so what a caller must be to reach it at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tenancy {
    /// Inside a workspace: callers are its members and the client tokens
    /// bound to it (`/api/v1/w/{workspace_id}/…`).
    Workspace,
    /// Belongs to one account, whatever its workspaces (ways in, passkeys).
    Account,
    /// The instance's own (administration views).
    Instance,
    /// Anyone may read it (signing keys, discovery, health).
    Public,
}

/// Who owns a resource of a type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Owner {
    /// The member who created or connected it, as the module records
    /// ([`OwnerFact::User`]), or the client tokens when a client wrote it
    /// ([`OwnerFact::Clients`]).
    Member,
    /// The workspace's owners.
    WorkspaceOwners,
    /// The account it belongs to ([`OwnerFact::Subject`]).
    Account,
    /// Nobody in particular: access comes from the grants only.
    Nobody,
}

/// Who a [`Grant`] is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "role")]
pub enum Grantee {
    /// Members whose workspace role is at least this one.
    WorkspaceRole(Role),
    /// Members whose role in the resource's container (or in the resource,
    /// for a container) is at least this one, on the container's ladder.
    ContainerRole(&'static str),
    /// The owner, while their role in the container is at least this one
    /// (a draft's author who may still send from its inbox).
    OwnerWithContainerRole(&'static str),
    /// Members who may see the resource's container, as the kernel's
    /// [`Visibility`](crate::changes::Visibility) port answers: for records
    /// of one module living in another module's container (an attention
    /// item in an inbox).
    ContainerViewers,
    /// Client tokens bound to the workspace: a narrower grant than a
    /// member's, listed explicitly.
    Clients,
    /// Client tokens, on a record a client wrote ([`OwnerFact::Clients`]).
    OwningClients,
    /// Instance administrators (the admin API; never the main API).
    InstanceAdmins,
    /// Anyone, signed in or not ([`Tenancy::Public`] resources only).
    Anyone,
}

/// Actions given to someone besides the owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Grant {
    /// Who.
    pub to: Grantee,
    /// What they may do.
    pub actions: &'static [Action],
}

impl Grant {
    /// `actions` for `to`.
    pub const fn new(to: Grantee, actions: &'static [Action]) -> Self {
        Self { to, actions }
    }
}

/// One resource type in the catalogue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ResourceType {
    /// `<module>.<record>`, the name operations and the change feed use.
    pub name: &'static str,
    /// What it is.
    pub description: &'static str,
    /// Where it lives.
    pub tenancy: Tenancy,
    /// Who owns it.
    pub owner: Owner,
    /// What its owner may do.
    pub owner_actions: &'static [Action],
    /// The container type its [`Grantee::ContainerRole`] grants refer to;
    /// its own name for a container.
    pub container: Option<&'static str>,
    /// For a container: its roles, lowest first.
    pub roles: &'static [&'static str],
    /// Who else may do what.
    pub grants: &'static [Grant],
}

impl ResourceType {
    /// A type named `name` owned by nobody, granting nothing: fill it in
    /// with struct update syntax.
    pub const fn new(name: &'static str, description: &'static str, tenancy: Tenancy) -> Self {
        Self {
            name,
            description,
            tenancy,
            owner: Owner::Nobody,
            owner_actions: &[],
            container: None,
            roles: &[],
            grants: &[],
        }
    }

    /// Every action anyone may take on it.
    pub fn actions(&self) -> BTreeSet<Action> {
        self.owner_actions
            .iter()
            .chain(self.grants.iter().flat_map(|grant| grant.actions))
            .copied()
            .collect()
    }
}

/// Who owns one resource, as its module records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnerFact {
    /// This member.
    User(UserId),
    /// The account with this subject.
    Subject(String),
    /// The workspace's client tokens (a client wrote it).
    Clients,
    /// Nobody (or nobody left).
    Nobody,
}

/// What the owning module knows about one resource, for [`authorize`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts {
    /// Its owner.
    pub owner: OwnerFact,
    /// The container it lives in (itself, for a container).
    pub container: Option<RecordRef>,
    /// The caller's role in that container, if any.
    pub container_role: Option<String>,
    /// Kept read-only (a disconnected mailbox kept as history): every
    /// action but `read` (and `delete`, of what one may delete) is refused.
    pub frozen: bool,
    /// Whether the caller may see the container, as the visibility port
    /// says; filled by [`authorize`] for types granting to
    /// [`Grantee::ContainerViewers`].
    pub container_visible: bool,
}

impl Facts {
    /// A resource owned as `owner`, in no container.
    pub fn owned_by(owner: OwnerFact) -> Self {
        Self {
            owner,
            container: None,
            container_role: None,
            frozen: false,
            container_visible: false,
        }
    }

    /// The same facts, inside `container` where the caller holds `role`.
    #[must_use]
    pub fn within(mut self, container: RecordRef, role: Option<String>) -> Self {
        self.container = Some(container);
        self.container_role = role;
        self
    }

    /// The same facts, with the container visible to the caller or not.
    #[must_use]
    pub fn visible(mut self, visible: bool) -> Self {
        self.container_visible = visible;
        self
    }

    /// The same facts, frozen read-only when `frozen`.
    #[must_use]
    pub fn frozen(mut self, frozen: bool) -> Self {
        self.frozen = frozen;
        self
    }
}

/// A future returned by a [`ResourceFacts`] port.
pub type FactsFuture<'a> = Pin<Box<dyn Future<Output = Result<Option<Facts>, Error>> + Send + 'a>>;

/// How a module answers the facts of its resources. `conn` is the caller's
/// transaction, with the workspace set (row-level security applies), so a
/// resource of another workspace is simply not found.
pub trait ResourceFacts: fmt::Debug + Send + Sync + 'static {
    /// The facts of `resource` (one of the module's types) as `access` sees
    /// them, or `None` when it does not exist in the workspace.
    fn facts<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        access: &'a Access,
        resource: &'a RecordRef,
    ) -> FactsFuture<'a>;
}

/// Shared handle to a module's [`ResourceFacts`].
pub type FactsPort = Arc<dyn ResourceFacts>;

/// A module's part of the catalogue: its resource types and the port that
/// answers their facts.
#[derive(Clone, Default)]
pub struct Resources {
    /// Its resource types.
    pub types: &'static [ResourceType],
    /// Its facts; `None` for a module whose types are not checked per
    /// instance (public or instance-level only).
    pub facts: Option<FactsPort>,
}

impl fmt::Debug for Resources {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Resources")
            .field(
                "types",
                &self.types.iter().map(|t| t.name).collect::<Vec<_>>(),
            )
            .field("facts", &self.facts.is_some())
            .finish()
    }
}

impl Resources {
    /// `types`, whose facts `facts` answers.
    pub fn new(types: &'static [ResourceType], facts: FactsPort) -> Self {
        Self {
            types,
            facts: Some(facts),
        }
    }

    /// `types`, with no per-instance facts.
    pub const fn declared(types: &'static [ResourceType]) -> Self {
        Self { types, facts: None }
    }
}

/// Every module's resource types and facts, as the server registers them
/// ([`AppState::with_resources`]). Empty until then: every check answers
/// [`Decision::NotFound`].
#[derive(Clone, Default)]
pub struct Catalogue {
    types: Arc<BTreeMap<&'static str, (ResourceType, Option<FactsPort>)>>,
}

impl fmt::Debug for Catalogue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.types.keys()).finish()
    }
}

impl Catalogue {
    /// The catalogue of `modules`' declarations.
    ///
    /// # Errors
    /// A type declared twice, or a grant naming a container role its
    /// container does not have.
    pub fn new<'a>(modules: impl IntoIterator<Item = &'a Resources>) -> Result<Self, String> {
        let mut types = BTreeMap::new();
        for resources in modules {
            for declared in resources.types {
                if types
                    .insert(declared.name, (*declared, resources.facts.clone()))
                    .is_some()
                {
                    return Err(format!(
                        "resource type `{}` is declared twice",
                        declared.name
                    ));
                }
            }
        }
        let catalogue = Self {
            types: Arc::new(types),
        };
        catalogue.check()?;
        Ok(catalogue)
    }

    fn check(&self) -> Result<(), String> {
        for (declared, _) in self.types.values() {
            for grant in declared.grants {
                let (Grantee::ContainerRole(role) | Grantee::OwnerWithContainerRole(role)) =
                    grant.to
                else {
                    continue;
                };
                // A container of a module not in this catalogue (a module's
                // own tests) is checked where the full catalogue is built.
                let Some(container) = declared.container.and_then(|c| self.get(c)) else {
                    continue;
                };
                let ladder = container.roles;
                if !ladder.contains(&role) {
                    return Err(format!(
                        "resource type `{}` grants to container role `{role}`, which its \
                         container does not have",
                        declared.name
                    ));
                }
            }
        }
        Ok(())
    }

    /// The type named `name`.
    pub fn get(&self, name: &str) -> Option<&ResourceType> {
        self.types.get(name).map(|(declared, _)| declared)
    }

    /// Containers some type names that no type declares: empty for a
    /// complete catalogue (the server's tests check it).
    pub fn missing_containers(&self) -> Vec<&'static str> {
        self.iter()
            .filter_map(|declared| declared.container)
            .filter(|container| self.get(container).is_none())
            .collect()
    }

    /// Whether it holds no type.
    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }

    /// Every type, by name.
    pub fn iter(&self) -> impl Iterator<Item = &ResourceType> {
        self.types.values().map(|(declared, _)| declared)
    }

    fn facts_of(&self, name: &str) -> Option<&FactsPort> {
        self.types.get(name).and_then(|(_, facts)| facts.as_ref())
    }

    /// The rank of `role` on `container`'s ladder (higher is more).
    fn rank(&self, container: &str, role: &str) -> Option<usize> {
        self.get(container)?.roles.iter().position(|r| *r == role)
    }
}

/// The outcome of [`authorize`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Go ahead.
    Allow,
    /// The caller may not know the resource exists: answer 404.
    NotFound,
    /// The caller sees it but may not take this action: answer 403.
    Forbidden,
}

impl Decision {
    /// `Ok` when allowed; else [`Error::NotFound`], or `forbidden()` for a
    /// visible resource (a module's own reason, e.g. `inbox_read_only`).
    ///
    /// # Errors
    /// When not allowed.
    pub fn or_else(self, forbidden: impl FnOnce() -> Error) -> Result<(), Error> {
        match self {
            Self::Allow => Ok(()),
            Self::NotFound => Err(Error::NotFound),
            Self::Forbidden => Err(forbidden()),
        }
    }
}

fn workspace_rank(role: Role) -> u8 {
    match role {
        Role::Member => 1,
        Role::Admin => 2,
        Role::Owner => 3,
    }
}

/// The actions `access` may take on a resource of `declared` with `facts`.
fn allowed(
    catalogue: &Catalogue,
    declared: &ResourceType,
    access: &Access,
    facts: &Facts,
) -> BTreeSet<Action> {
    let is_owner = match (&facts.owner, declared.owner) {
        (_, Owner::WorkspaceOwners) => access.role == Some(Role::Owner),
        (OwnerFact::User(owner), Owner::Member) => access.user == Some(*owner),
        (OwnerFact::Clients, Owner::Member) => access.user.is_none(),
        // Account resources are answered for the caller's own account only.
        (OwnerFact::Subject(_), Owner::Account) => true,
        // Nobody owns it, or the fact does not fit the declaration.
        _ => false,
    };
    let container_rank = facts
        .container
        .as_ref()
        .zip(facts.container_role.as_deref())
        .and_then(|(container, role)| catalogue.rank(&container.record_type, role));
    let at_least = |role: &str| {
        declared
            .container
            .and_then(|container| catalogue.rank(container, role))
            .zip(container_rank)
            .is_some_and(|(needed, held)| held >= needed)
    };
    let mut actions: BTreeSet<Action> = BTreeSet::new();
    if is_owner {
        actions.extend(declared.owner_actions);
    }
    for grant in declared.grants {
        let applies = match grant.to {
            Grantee::WorkspaceRole(min) => access
                .role
                .is_some_and(|role| workspace_rank(role) >= workspace_rank(min)),
            Grantee::ContainerRole(min) => access.user.is_some() && at_least(min),
            Grantee::OwnerWithContainerRole(min) => is_owner && at_least(min),
            Grantee::ContainerViewers => access.user.is_some() && facts.container_visible,
            Grantee::Clients => access.user.is_none(),
            Grantee::OwningClients => access.user.is_none() && facts.owner == OwnerFact::Clients,
            // The main API never admits an administrator as one.
            Grantee::InstanceAdmins => false,
            Grantee::Anyone => true,
        };
        if applies {
            actions.extend(grant.actions);
        }
    }
    if facts.frozen {
        // Kept read-only: seen, and what is one's own may still be deleted
        // (a draft), nothing else.
        actions.retain(|action| matches!(action, Action::Read | Action::Delete));
    }
    actions
}

/// Decide whether `access` may take `action` on `resource` with `facts`
/// (pure; [`authorize`] fetches the facts).
pub fn decide(
    catalogue: &Catalogue,
    access: &Access,
    action: Action,
    resource_type: &str,
    facts: Option<&Facts>,
) -> Decision {
    let (Some(declared), Some(facts)) = (catalogue.get(resource_type), facts) else {
        return Decision::NotFound;
    };
    let actions = allowed(catalogue, declared, access, facts);
    if !actions.contains(&Action::Read) {
        Decision::NotFound
    } else if actions.contains(&action) {
        Decision::Allow
    } else {
        Decision::Forbidden
    }
}

/// Whether `access` may take `action` on `resource`, asking the owning
/// module for its facts in `conn` (the caller's workspace transaction).
/// Deny by default. An allowed resource inside a container is noted for
/// the idempotency layer, so a replay is served only while the caller may
/// still see it.
///
/// # Errors
/// When the owning module cannot answer (the database).
pub async fn authorize(
    state: &AppState,
    conn: &mut PgConnection,
    access: &Access,
    action: Action,
    resource: &RecordRef,
) -> Result<Decision, Error> {
    let catalogue = &state.resources;
    let Some(port) = catalogue.facts_of(&resource.record_type) else {
        return Ok(Decision::NotFound);
    };
    let mut facts = port.facts(conn, access, resource).await?;
    // A record in another module's container (an attention item in an
    // inbox): the visibility port says whether the caller sees it.
    let viewers = catalogue
        .get(&resource.record_type)
        .is_some_and(|declared| {
            declared
                .grants
                .iter()
                .any(|grant| grant.to == Grantee::ContainerViewers)
        });
    if viewers
        && let Some(found) = facts.as_mut()
        && let Some(container) = found.container.clone()
    {
        found.container_visible = state
            .visibility
            .container_visible(&state.db, access, &container)
            .await?;
    }
    let decision = decide(
        catalogue,
        access,
        action,
        &resource.record_type,
        facts.as_ref(),
    );
    if decision == Decision::Allow
        && let Some(container) = facts.and_then(|facts| facts.container)
    {
        crate::idempotency::checked_container(container);
    }
    Ok(decision)
}

/// [`authorize`], as a `Result`: 404 when the caller may not see the
/// resource, `forbidden()` when they may see it but not take the action.
///
/// # Errors
/// When not allowed, or when the owning module cannot answer.
pub async fn require(
    state: &AppState,
    conn: &mut PgConnection,
    access: &Access,
    action: Action,
    resource: &RecordRef,
    forbidden: impl FnOnce() -> Error,
) -> Result<(), Error> {
    authorize(state, conn, access, action, resource)
        .await?
        .or_else(forbidden)
}

/// Whether `access` may take `action` on resources of `resource_type` at
/// all, before any instance exists or for a list filtered to what the
/// caller sees: a client token needs an explicit [`Grantee::Clients`] grant
/// of the action, a member a workspace role reaching one, or some owner or
/// container rule that could give it to them.
pub fn authorize_type(
    catalogue: &Catalogue,
    access: &Access,
    action: Action,
    resource_type: &str,
) -> Decision {
    let Some(declared) = catalogue.get(resource_type) else {
        return Decision::NotFound;
    };
    let possible = |grant: &Grant| grant.actions.contains(&action);
    let allowed = if access.user.is_none() {
        declared.grants.iter().any(|grant| {
            matches!(
                grant.to,
                Grantee::Clients | Grantee::OwningClients | Grantee::Anyone
            ) && possible(grant)
        })
    } else {
        {
            let by_owner = match declared.owner {
                Owner::Member | Owner::Account => declared.owner_actions.contains(&action),
                Owner::WorkspaceOwners => {
                    access.role == Some(Role::Owner) && declared.owner_actions.contains(&action)
                }
                Owner::Nobody => false,
            };
            by_owner
                || declared.grants.iter().any(|grant| {
                    possible(grant)
                        && match grant.to {
                            Grantee::WorkspaceRole(min) => access
                                .role
                                .is_some_and(|role| workspace_rank(role) >= workspace_rank(min)),
                            Grantee::ContainerRole(_)
                            | Grantee::OwnerWithContainerRole(_)
                            | Grantee::ContainerViewers
                            | Grantee::Anyone => true,
                            Grantee::Clients | Grantee::OwningClients | Grantee::InstanceAdmins => {
                                false
                            }
                        }
                })
        }
    };
    if allowed {
        Decision::Allow
    } else {
        Decision::Forbidden
    }
}

/// [`authorize_type`] as a `Result`.
///
/// # Errors
/// `forbidden()` when not allowed.
pub fn require_type(
    state: &AppState,
    access: &Access,
    action: Action,
    resource_type: &str,
    forbidden: impl FnOnce() -> Error,
) -> Result<(), Error> {
    match authorize_type(&state.resources, access, action, resource_type) {
        Decision::Allow => Ok(()),
        Decision::NotFound => Err(Error::NotFound),
        Decision::Forbidden => Err(forbidden()),
    }
}

/// Whether `principal` may take `action` on its own resources of
/// `resource_type`, an [`Tenancy::Account`] type (their ways in, the
/// workspaces they belong to): a user takes the owner's actions on what is
/// theirs (the owning module answers only the caller's own records); a
/// client token only what a [`Grantee::Clients`] grant lists.
pub fn authorize_account(
    catalogue: &Catalogue,
    principal: &crate::auth::Principal,
    action: Action,
    resource_type: &str,
) -> Decision {
    let Some(declared) = catalogue
        .get(resource_type)
        .filter(|declared| declared.tenancy == Tenancy::Account)
    else {
        return Decision::NotFound;
    };
    let allowed = match principal.kind {
        crate::auth::PrincipalKind::User => {
            declared.owner == Owner::Account && declared.owner_actions.contains(&action)
        }
        crate::auth::PrincipalKind::Client => declared.grants.iter().any(|grant| {
            matches!(grant.to, Grantee::Clients | Grantee::Anyone)
                && grant.actions.contains(&action)
        }),
    };
    if allowed {
        Decision::Allow
    } else {
        Decision::Forbidden
    }
}

/// [`authorize_account`] as a `Result`.
///
/// # Errors
/// [`Error::NotFound`] for an unknown type, `forbidden()` when not allowed.
pub fn require_account(
    state: &AppState,
    principal: &crate::auth::Principal,
    action: Action,
    resource_type: &str,
    forbidden: impl FnOnce() -> Error,
) -> Result<(), Error> {
    match authorize_account(&state.resources, principal, action, resource_type) {
        Decision::Allow => Ok(()),
        Decision::NotFound => Err(Error::NotFound),
        Decision::Forbidden => Err(forbidden()),
    }
}

/// The `access` an operation declares in `x-campbooks` (ADR 0017): the
/// resource type it acts on and the action. `cargo xtask arch` requires one
/// on every operation, naming a catalogued type and one of its actions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationAccess {
    /// The resource type, `<module>.<record>`.
    pub resource: String,
    /// The action.
    pub action: Action,
}

#[cfg(test)]
mod tests;
