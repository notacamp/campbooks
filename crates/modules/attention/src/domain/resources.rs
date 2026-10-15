//! The module's part of the access catalogue (ADR 0017): an attention item
//! lives in another module's container (an inbox). Whoever the visibility
//! port lets see that container sees the item and puts it off for
//! themselves; workspace owners and admins get nothing without the
//! container, and a client only what the member it acts for gets.

use std::sync::Arc;

use campbooks_kernel::{
    RecordRef,
    authz::{
        Action, Facts, FactsFuture, Grant, Grantee, OwnerFact, ResourceFacts, ResourceType,
        Resources, Tenancy,
    },
    tenancy::Access,
};
use sqlx::PgConnection;

use super::ItemId;

/// The record type of an attention item.
pub const ITEM_RECORD: &str = "attention.item";

/// The container type items live in today.
const INBOX_RECORD: &str = "conversations.inbox";

/// The module's resource types.
static TYPES: [ResourceType; 1] = [ResourceType {
    container: Some(INBOX_RECORD),
    grants: &[Grant::new(
        Grantee::ContainerViewers,
        &[Action::Read, Action::Write],
    )],
    ..ResourceType::new(
        ITEM_RECORD,
        "Something that needs the members who can read its container (an inbox): they see \
         it and put it off for themselves. A client sees what the member it acts for sees.",
        Tenancy::Workspace,
    )
}];

/// The module's resources, for its descriptor.
pub fn resources() -> Resources {
    Resources::new(&TYPES, Arc::new(ItemFacts))
}

#[derive(Debug, Clone, Copy)]
struct ItemFacts;

impl ResourceFacts for ItemFacts {
    fn facts<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        _access: &'a Access,
        resource: &'a RecordRef,
    ) -> FactsFuture<'a> {
        Box::pin(async move {
            if resource.record_type != ITEM_RECORD {
                return Ok(None);
            }
            let Ok(id) = resource.id.parse::<ItemId>() else {
                return Ok(None);
            };
            let found = sqlx::query!(
                "SELECT container_type, container_id FROM attention.items WHERE id = $1",
                id as ItemId
            )
            .fetch_optional(conn)
            .await?;
            // The kernel asks the visibility port about the container.
            Ok(found.map(|item| {
                Facts::owned_by(OwnerFact::Nobody).within(
                    RecordRef::new(item.container_type, &item.container_id),
                    None,
                )
            }))
        })
    }
}
