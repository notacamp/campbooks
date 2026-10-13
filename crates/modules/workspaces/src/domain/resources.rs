//! The module's part of the access catalogue (ADR 0017): its resource
//! types, who owns them, who else may do what, and the facts of one
//! instance for `authz::authorize`.

use std::sync::Arc;

use campbooks_kernel::{
    RecordRef,
    authz::{
        Action, Facts, FactsFuture, Grant, Grantee, Owner, OwnerFact, ResourceFacts, ResourceType,
        Resources, Tenancy,
    },
    tenancy::{Access, Role, UserId, WorkspaceId},
};
use sqlx::PgConnection;

use crate::{
    domain::model::{InvitationId, SecurityEventId},
    events::{MEMBER_RECORD, WORKSPACE_RECORD},
};

/// The record type of an invitation.
pub const INVITATION_RECORD: &str = "workspaces.invitation";
/// The record type of a security event.
pub const SECURITY_EVENT_RECORD: &str = "workspaces.security_event";
/// The caller's own place in the instance: the workspaces they belong to,
/// creating one, accepting an invitation.
pub const ACCOUNT_RECORD: &str = "workspaces.account";

use Action::{Delete, Read, Share, Write};

/// The module's resource types.
pub static TYPES: [ResourceType; 5] = [
    ResourceType {
        owner: Owner::WorkspaceOwners,
        owner_actions: &[Read, Write, Share, Delete],
        grants: &[
            Grant::new(Grantee::WorkspaceRole(Role::Admin), &[Read, Write, Share]),
            Grant::new(Grantee::WorkspaceRole(Role::Member), &[Read]),
            Grant::new(Grantee::Clients, &[Read]),
        ],
        ..ResourceType::new(
            WORKSPACE_RECORD,
            "A workspace: its name and settings. Owners may delete it; owners and admins \
             change its settings and invite; members read it.",
            Tenancy::Workspace,
        )
    },
    ResourceType {
        owner: Owner::Member,
        owner_actions: &[Read, Delete],
        grants: &[
            Grant::new(Grantee::WorkspaceRole(Role::Admin), &[Read, Write, Delete]),
            Grant::new(Grantee::WorkspaceRole(Role::Member), &[Read]),
            Grant::new(Grantee::Clients, &[Read]),
        ],
        ..ResourceType::new(
            MEMBER_RECORD,
            "A membership, owned by the member (who may leave). Owners and admins change roles \
             and remove members (an admin never an owner); members see each other.",
            Tenancy::Workspace,
        )
    },
    ResourceType {
        grants: &[Grant::new(
            Grantee::WorkspaceRole(Role::Admin),
            &[Read, Write, Delete],
        )],
        ..ResourceType::new(
            INVITATION_RECORD,
            "An invitation to join a workspace: owners and admins only.",
            Tenancy::Workspace,
        )
    },
    ResourceType {
        grants: &[Grant::new(Grantee::WorkspaceRole(Role::Admin), &[Read])],
        ..ResourceType::new(
            SECURITY_EVENT_RECORD,
            "An entry of the workspace's security log: owners and admins read it.",
            Tenancy::Workspace,
        )
    },
    ResourceType {
        owner: Owner::Account,
        owner_actions: &[Read, Write],
        ..ResourceType::new(
            ACCOUNT_RECORD,
            "A person's own place in the instance: the workspaces they belong to, creating \
             one, accepting an invitation. Never a client token's.",
            Tenancy::Account,
        )
    },
];

/// The module's resources, for its descriptor.
pub fn resources() -> Resources {
    Resources::new(&TYPES, Arc::new(WorkspaceFacts))
}

/// The facts of the module's resources.
#[derive(Debug, Clone, Copy)]
struct WorkspaceFacts;

impl ResourceFacts for WorkspaceFacts {
    fn facts<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        access: &'a Access,
        resource: &'a RecordRef,
    ) -> FactsFuture<'a> {
        Box::pin(async move {
            // Every id of the module is a UUID; the workspace id type parses one.
            let Ok(id) = resource.id.parse::<WorkspaceId>() else {
                return Ok(None);
            };
            let id = *id.as_uuid();
            let found = match resource.record_type.as_str() {
                WORKSPACE_RECORD => sqlx::query_scalar!(
                    r#"SELECT true AS "found!" FROM workspaces.workspaces
                        WHERE id = $1 AND id = $2 AND deleted_at IS NULL"#,
                    id,
                    access.workspace as WorkspaceId,
                )
                .fetch_optional(&mut *conn)
                .await?
                .map(|_| Facts::owned_by(OwnerFact::Nobody)),
                MEMBER_RECORD => sqlx::query_scalar!(
                    r#"SELECT user_id AS "user_id: UserId" FROM workspaces.members
                        WHERE workspace_id = $1 AND user_id = $2"#,
                    access.workspace as WorkspaceId,
                    id,
                )
                .fetch_optional(&mut *conn)
                .await?
                .map(|user| Facts::owned_by(OwnerFact::User(user))),
                INVITATION_RECORD => sqlx::query_scalar!(
                    r#"SELECT true AS "found!" FROM workspaces.invitations
                        WHERE workspace_id = $1 AND id = $2"#,
                    access.workspace as WorkspaceId,
                    InvitationId::from_uuid(id) as InvitationId,
                )
                .fetch_optional(&mut *conn)
                .await?
                .map(|_| Facts::owned_by(OwnerFact::Nobody)),
                SECURITY_EVENT_RECORD => sqlx::query_scalar!(
                    r#"SELECT true AS "found!" FROM workspaces.security_events
                        WHERE workspace_id = $1 AND id = $2"#,
                    access.workspace as WorkspaceId,
                    SecurityEventId::from_uuid(id) as SecurityEventId,
                )
                .fetch_optional(&mut *conn)
                .await?
                .map(|_| Facts::owned_by(OwnerFact::Nobody)),
                _ => None,
            };
            Ok(found)
        })
    }
}
