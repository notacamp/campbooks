//! The tracked events of a page (ADR 0020, campbooks-cloud#363): one per
//! change, with a snapshot read from the module's own row in the transaction
//! of the change.
//!
//! Ids, enums and numbers only: never the title, the icon, the body, a
//! mention or a file name.

use campbooks_kernel::{
    AppState, Result,
    tenancy::{UserId, WorkspaceId},
};
use sqlx::{Postgres, Transaction, types::Uuid};

use super::{PageId, Place, Role};
use crate::tracking;

/// A page, as the snapshot reads it.
#[derive(sqlx::FromRow)]
struct PageFacts {
    id: Uuid,
    place: String,
    parent_id: Option<Uuid>,
    created_by: Uuid,
    revision: i64,
}

const fn place_of(place: Place) -> tracking::Place {
    match place {
        Place::Private => tracking::Place::Private,
        Place::Workspace => tracking::Place::Workspace,
    }
}

const fn role_of(role: Role) -> tracking::PageRole {
    match role {
        Role::Reader => tracking::PageRole::Reader,
        Role::Writer => tracking::PageRole::Writer,
        Role::Manager => tracking::PageRole::Manager,
    }
}

/// The snapshot of page `id` as it is now in `tx`, if it exists.
async fn snapshot(
    tx: &mut Transaction<'_, Postgres>,
    id: PageId,
) -> Result<Option<tracking::PageSnapshot>> {
    let facts: Option<PageFacts> = sqlx::query_as(
        "SELECT id, place, parent_id, created_by, revision FROM pages.pages WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(facts.map(|row| tracking::PageSnapshot {
        id: row.id,
        place: place_of(row.place.parse::<Place>().unwrap_or(Place::Private)),
        parent: row.parent_id,
        created_by: row.created_by,
        revision: row.revision,
    }))
}

/// What happened to a page that carries nothing but the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Created,
    Trashed,
    Restored,
    Purged,
}

/// Track that `page` was made, trashed, restored or deleted for good. Call it
/// after the change, except for a purge, which reads the page before the
/// rows go.
pub async fn simple(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    page: PageId,
    change: Change,
) -> Result<()> {
    let Some(page) = snapshot(tx, page).await? else {
        return Ok(());
    };
    let tracker = &state.tracker;
    match change {
        Change::Created => {
            tracker
                .track(tx, &tracking::PageCreated { workspace, page })
                .await?;
        }
        Change::Trashed => {
            tracker
                .track(tx, &tracking::PageTrashed { workspace, page })
                .await?;
        }
        Change::Restored => {
            tracker
                .track(tx, &tracking::PageRestored { workspace, page })
                .await?;
        }
        Change::Purged => {
            tracker
                .track(tx, &tracking::PagePurged { workspace, page })
                .await?;
        }
    }
    Ok(())
}

/// A title or icon change: the fields that differ.
pub async fn edited(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    page: PageId,
    title: bool,
    icon: bool,
) -> Result<()> {
    let mut fields = Vec::new();
    if title {
        fields.push(tracking::PageField::Title);
    }
    if icon {
        fields.push(tracking::PageField::Icon);
    }
    if fields.is_empty() {
        return Ok(());
    }
    let Some(page) = snapshot(tx, page).await? else {
        return Ok(());
    };
    state
        .tracker
        .track(
            tx,
            &tracking::PageEdited {
                workspace,
                page,
                fields,
            },
        )
        .await?;
    Ok(())
}

/// A move: `before` is where the page was.
pub async fn moved(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    page: PageId,
    before_parent: Option<PageId>,
    before_place: Place,
) -> Result<()> {
    let Some(page) = snapshot(tx, page).await? else {
        return Ok(());
    };
    let before = tracking::PageLocationSnapshot {
        place: place_of(before_place),
        parent: before_parent.map(|parent| *parent.as_uuid()),
    };
    state
        .tracker
        .track(
            tx,
            &tracking::PageMoved {
                workspace,
                page,
                before,
            },
        )
        .await?;
    Ok(())
}

/// A grant: `previous` is the role the member had on the page, if any.
pub async fn shared(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    page: PageId,
    member: UserId,
    role: Role,
    previous: Option<Role>,
) -> Result<()> {
    let Some(page) = snapshot(tx, page).await? else {
        return Ok(());
    };
    state
        .tracker
        .track(
            tx,
            &tracking::PageShared {
                workspace,
                page,
                member: *member.as_uuid(),
                role: role_of(role),
                previous_role: previous.map(role_of),
            },
        )
        .await?;
    Ok(())
}

/// A grant removed.
pub async fn unshared(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    page: PageId,
    member: UserId,
) -> Result<()> {
    let Some(page) = snapshot(tx, page).await? else {
        return Ok(());
    };
    state
        .tracker
        .track(
            tx,
            &tracking::PageUnshared {
                workspace,
                page,
                member: *member.as_uuid(),
            },
        )
        .await?;
    Ok(())
}
