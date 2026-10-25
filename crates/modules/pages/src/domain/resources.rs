//! The module's part of the access catalogue (ADR 0017), and the change
//! feed's and search's `Visibility` port for its containers.
//!
//! A page is its own container, with the roles `reader` < `writer` <
//! `manager`: the module's facts answer the highest role the caller holds
//! (`roles`), and the catalogue says what each role may do. A caller with no
//! role does not know the page exists.

use std::sync::Arc;

use campbooks_kernel::{
    RecordRef,
    authz::{
        Action, Facts, FactsFuture, Grant, Grantee, Owner, OwnerFact, ResourceFacts, ResourceType,
        Resources, Tenancy,
    },
    changes::{ContainersFuture, Visibility, VisibilityFuture, VisibilityPort},
    db::BeginFor as _,
    tenancy::Access,
};
use sqlx::{PgConnection, PgPool};

use super::{PageId, roles};

/// The record type of a page, which is also its container type.
pub const PAGE_RECORD: &str = "pages.page";

/// The module's resource types.
static TYPES: [ResourceType; 1] = [ResourceType {
    owner: Owner::Nobody,
    container: Some(PAGE_RECORD),
    roles: &["reader", "writer", "manager"],
    grants: &[
        Grant::new(Grantee::ContainerRole("reader"), &[Action::Read]),
        Grant::new(
            Grantee::ContainerRole("writer"),
            &[Action::Read, Action::Write, Action::Create],
        ),
        Grant::new(
            Grantee::ContainerRole("manager"),
            &[
                Action::Read,
                Action::Write,
                Action::Create,
                Action::Delete,
                Action::Share,
            ],
        ),
    ],
    ..ResourceType::new(
        PAGE_RECORD,
        "A page a member wrote. In the workspace place every member writes it and its creator \
         and the workspace's owners also share and delete it; a private page is its creator's \
         alone. A member it is shared with holds the role they were given (`reader`, `writer` \
         or `manager`) on it and its sub-pages. Workspace owners and admins get nothing on a \
         private page. A mention on a page grants nothing.",
        Tenancy::Workspace,
    )
}];

/// The module's resources, for its descriptor.
pub fn resources() -> Resources {
    Resources::new(&TYPES, Arc::new(PageFacts))
}

#[derive(Debug, Clone, Copy)]
struct PageFacts;

impl ResourceFacts for PageFacts {
    fn facts<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        access: &'a Access,
        resource: &'a RecordRef,
    ) -> FactsFuture<'a> {
        Box::pin(async move {
            if resource.record_type != PAGE_RECORD {
                return Ok(None);
            }
            let Ok(id) = resource.id.parse::<PageId>() else {
                return Ok(None);
            };
            let Some(role) = roles::standing(conn, access, id).await? else {
                return Ok(None);
            };
            Ok(Some(Facts::owned_by(OwnerFact::Nobody).within(
                RecordRef::new(PAGE_RECORD, &id),
                role.map(|r| r.as_str().to_owned()),
            )))
        })
    }
}

/// The port, for the server to register.
pub fn visibility() -> VisibilityPort {
    Arc::new(PageVisibility)
}

/// Who sees what lives in a page's container: whoever holds a role on it.
#[derive(Debug, Clone, Copy, Default)]
pub struct PageVisibility;

impl Visibility for PageVisibility {
    fn container_types(&self) -> &'static [&'static str] {
        &[PAGE_RECORD]
    }

    /// A change to any page may change who reads it (a share, a move, a
    /// trash), so streams ask again which pages they see.
    fn access_records(&self) -> &'static [&'static str] {
        &[PAGE_RECORD]
    }

    fn container_visible<'a>(
        &'a self,
        db: &'a PgPool,
        access: &'a Access,
        container: &'a RecordRef,
    ) -> VisibilityFuture<'a> {
        Box::pin(async move {
            if container.record_type != PAGE_RECORD {
                return Ok(false);
            }
            let Ok(id) = container.id.parse::<PageId>() else {
                return Ok(false);
            };
            let mut tx = db.begin_for(access.workspace).await?;
            let standing = roles::standing(&mut tx, access, id).await?;
            tx.rollback().await?;
            Ok(matches!(standing, Some(Some(_))))
        })
    }

    fn visible_containers<'a>(
        &'a self,
        db: &'a PgPool,
        access: &'a Access,
        container_type: &'a str,
    ) -> ContainersFuture<'a> {
        Box::pin(async move {
            if container_type != PAGE_RECORD {
                return Ok(Vec::new());
            }
            let mut tx = db.begin_for(access.workspace).await?;
            let readable = roles::readable(&mut tx, access).await?;
            tx.rollback().await?;
            // Pages in the trash too: the feed tells a member's open trash
            // view about them, and search holds none of them in its index.
            let mut ids: Vec<PageId> = readable.keys().copied().collect();
            ids.sort_unstable();
            Ok(ids
                .iter()
                .map(|id| RecordRef::new(PAGE_RECORD, id))
                .collect())
        })
    }
}
