//! The module's part of the access catalogue (ADR 0017): a connection is
//! owned by the member who connected it.

use std::sync::Arc;

use campbooks_kernel::{
    RecordRef,
    authz::{
        self, Action, Decision, Facts, FactsFuture, Grant, Grantee, Owner, OwnerFact,
        ResourceFacts, ResourceType, Resources, Tenancy,
    },
    tenancy::{Access, Role, UserId},
};
use sqlx::PgConnection;

use super::model::ConnectionId;
use crate::events::CONNECTION_RECORD;

use Action::{Disconnect, Read, Write};

/// The module's resource types.
static TYPES: [ResourceType; 1] = [ResourceType {
    owner: Owner::Member,
    owner_actions: &[Read, Write, Disconnect],
    grants: &[
        Grant::new(Grantee::WorkspaceRole(Role::Admin), &[Read, Write]),
        Grant::new(Grantee::Clients, &[Read, Write]),
    ],
    ..ResourceType::new(
        CONNECTION_RECORD,
        "A connection to an outside account (a mailbox), owned by the member who connected \
         it. Only they disconnect it (choosing whether its mail is deleted or kept read-only); \
         owners and admins see its health and may sync it now, never read its mail through it.",
        Tenancy::Workspace,
    )
}];

/// The module's resources, for its descriptor.
pub fn resources() -> Resources {
    Resources::new(&TYPES, Arc::new(ConnectionFacts))
}

/// The facts of a connection granted by `granted_by`.
pub fn facts(granted_by: UserId) -> Facts {
    Facts::owned_by(OwnerFact::User(granted_by))
}

/// Whether `access` may see a connection granted by `granted_by` (the
/// catalogue's `read`).
pub fn visible(
    catalogue: &authz::Catalogue,
    access: &Access,
    granted_by: UserId,
) -> bool {
    authz::decide(
        catalogue,
        access,
        Read,
        CONNECTION_RECORD,
        Some(&facts(granted_by)),
    ) == Decision::Allow
}

#[derive(Debug, Clone, Copy)]
struct ConnectionFacts;

impl ResourceFacts for ConnectionFacts {
    fn facts<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        _access: &'a Access,
        resource: &'a RecordRef,
    ) -> FactsFuture<'a> {
        Box::pin(async move {
            if resource.record_type != CONNECTION_RECORD {
                return Ok(None);
            }
            let Ok(id) = resource.id.parse::<ConnectionId>() else {
                return Ok(None);
            };
            let granted_by = sqlx::query_scalar!(
                r#"SELECT granted_by AS "granted_by: UserId" FROM connectors.connections
                    WHERE id = $1"#,
                id as ConnectionId,
            )
            .fetch_optional(conn)
            .await?;
            Ok(granted_by.map(facts))
        })
    }
}

#[cfg(test)]
mod tests {
    use campbooks_kernel::tenancy::WorkspaceId;

    use super::*;

    fn decide(access: &Access, action: Action, granted_by: UserId) -> Decision {
        let catalogue = authz::Catalogue::new([&resources()]).unwrap();
        authz::decide(
            &catalogue,
            access,
            action,
            CONNECTION_RECORD,
            Some(&facts(granted_by)),
        )
    }

    #[test]
    fn grantor_admins_and_clients_see_it_only_the_grantor_disconnects() {
        let grantor = UserId::new();
        let other = UserId::new();
        let access = |user: Option<UserId>, role: Option<Role>| Access {
            workspace: WorkspaceId::new(),
            user,
            role,
        };
        let member = |user| access(Some(user), Some(Role::Member));
        assert_eq!(decide(&member(grantor), Read, grantor), Decision::Allow);
        assert_eq!(decide(&member(grantor), Disconnect, grantor), Decision::Allow);
        assert_eq!(decide(&member(other), Read, grantor), Decision::NotFound);
        for role in [Role::Admin, Role::Owner] {
            let manager = access(Some(other), Some(role));
            assert_eq!(decide(&manager, Read, grantor), Decision::Allow);
            assert_eq!(decide(&manager, Write, grantor), Decision::Allow);
            assert_eq!(decide(&manager, Disconnect, grantor), Decision::Forbidden);
        }
        let client = access(None, None);
        assert_eq!(decide(&client, Read, grantor), Decision::Allow);
        assert_eq!(decide(&client, Disconnect, grantor), Decision::Forbidden);
    }
}
