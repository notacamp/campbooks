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

/// Which way blocks crossed between a page and its Notion page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// From Notion into the page.
    Inbound,
    /// From the page to Notion.
    Outbound,
}

/// What a member chose for a conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    /// Keep the block as it is here.
    Mine,
    /// Use Notion's version.
    Notion,
    /// Keep both.
    Both,
}

const fn stop_reason_of(reason: super::notion::StopReason) -> tracking::SyncStopReason {
    use super::notion::StopReason;
    match reason {
        StopReason::StoppedByMember => tracking::SyncStopReason::StoppedByMember,
        StopReason::Disconnected => tracking::SyncStopReason::Disconnected,
        StopReason::TrashedHere => tracking::SyncStopReason::TrashedHere,
        StopReason::TrashedInNotion => tracking::SyncStopReason::TrashedInNotion,
        StopReason::NotShared => tracking::SyncStopReason::NotShared,
        StopReason::MemberLeft => tracking::SyncStopReason::MemberLeft,
        StopReason::NotionUnavailable => tracking::SyncStopReason::NotionUnavailable,
    }
}

/// `count` as the plan's whole number.
fn counted(count: usize) -> i64 {
    i64::try_from(count).unwrap_or(i64::MAX)
}

/// Pages were imported from Notion through `connection`.
pub async fn notion_imported(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    connection: Uuid,
    pages: usize,
) -> Result<()> {
    state
        .tracker
        .track(
            tx,
            &tracking::NotionPagesImported {
                workspace,
                connection,
                pages: counted(pages),
            },
        )
        .await?;
    Ok(())
}

/// `blocks` crossed between `page` and its Notion page.
pub async fn notion_applied(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    page: PageId,
    direction: Direction,
    blocks: usize,
) -> Result<()> {
    let Some(page) = snapshot(tx, page).await? else {
        return Ok(());
    };
    let direction = match direction {
        Direction::Inbound => tracking::SyncDirection::Inbound,
        Direction::Outbound => tracking::SyncDirection::Outbound,
    };
    state
        .tracker
        .track(
            tx,
            &tracking::NotionSyncApplied {
                workspace,
                page,
                direction,
                blocks: counted(blocks),
            },
        )
        .await?;
    Ok(())
}

/// `blocks` of `page` are in conflict with Notion's version, newly.
pub async fn notion_conflict_raised(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    page: PageId,
    blocks: usize,
) -> Result<()> {
    let Some(page) = snapshot(tx, page).await? else {
        return Ok(());
    };
    state
        .tracker
        .track(
            tx,
            &tracking::NotionConflictRaised {
                workspace,
                page,
                blocks: counted(blocks),
            },
        )
        .await?;
    Ok(())
}

/// A member settled a conflict of `page`.
pub async fn notion_conflict_resolved(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    page: PageId,
    choice: Choice,
) -> Result<()> {
    let Some(page) = snapshot(tx, page).await? else {
        return Ok(());
    };
    let choice = match choice {
        Choice::Mine => tracking::ConflictChoice::Mine,
        Choice::Notion => tracking::ConflictChoice::Notion,
        Choice::Both => tracking::ConflictChoice::Both,
    };
    state
        .tracker
        .track(
            tx,
            &tracking::NotionConflictResolved {
                workspace,
                page,
                choice,
            },
        )
        .await?;
    Ok(())
}

/// `page` stopped being kept in step with its Notion page.
pub async fn notion_stopped(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    page: PageId,
    reason: super::notion::StopReason,
) -> Result<()> {
    let Some(page) = snapshot(tx, page).await? else {
        return Ok(());
    };
    state
        .tracker
        .track(
            tx,
            &tracking::NotionSyncStopped {
                workspace,
                page,
                reason: stop_reason_of(reason),
            },
        )
        .await?;
    Ok(())
}

/// How many, in the buckets the plan counts in.
pub const fn bucket(count: usize) -> tracking::CountBucket {
    match count {
        0 => tracking::CountBucket::None,
        1 => tracking::CountBucket::One,
        2..=9 => tracking::CountBucket::Few,
        10..=99 => tracking::CountBucket::Some,
        _ => tracking::CountBucket::Many,
    }
}

const fn property_type_of(ty: crate::properties::PropertyType) -> Option<tracking::PropertyType> {
    use crate::properties::PropertyType as T;
    Some(match ty {
        T::Text => tracking::PropertyType::Text,
        T::Number => tracking::PropertyType::Number,
        T::Select => tracking::PropertyType::Select,
        T::MultiSelect => tracking::PropertyType::MultiSelect,
        T::Status => tracking::PropertyType::Status,
        T::Date => tracking::PropertyType::Date,
        T::Checkbox => tracking::PropertyType::Checkbox,
        T::Person => tracking::PropertyType::Person,
        T::Files => tracking::PropertyType::Files,
        T::Url => tracking::PropertyType::Url,
        T::Email => tracking::PropertyType::Email,
        T::Phone => tracking::PropertyType::Phone,
        T::Relation => tracking::PropertyType::Relation,
        T::CampbooksLink => tracking::PropertyType::CampbooksLink,
        T::Created => tracking::PropertyType::Created,
        T::Edited => tracking::PropertyType::Edited,
        T::UniqueId => tracking::PropertyType::UniqueId,
        _ => return None,
    })
}

/// The plan's name for a view type, if it has one (only table and board are
/// made).
pub const fn view_type_of(ty: super::table_views::ViewType) -> Option<tracking::ViewType> {
    match ty {
        super::table_views::ViewType::Table => Some(tracking::ViewType::Table),
        super::table_views::ViewType::Board => Some(tracking::ViewType::Board),
        _ => None,
    }
}

/// Track that table `table` was made.
pub async fn table_created(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    table: PageId,
) -> Result<()> {
    if let Some(table) = snapshot(tx, table).await? {
        state
            .tracker
            .track(tx, &tracking::TableCreated { workspace, table })
            .await?;
    }
    Ok(())
}

/// Track that a property of type `ty` was added to `table`.
pub async fn property_added(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    table: PageId,
    ty: crate::properties::PropertyType,
) -> Result<()> {
    let (Some(property_type), Some(table)) = (property_type_of(ty), snapshot(tx, table).await?)
    else {
        return Ok(());
    };
    state
        .tracker
        .track(
            tx,
            &tracking::PropertyAdded {
                workspace,
                property_type,
                table,
            },
        )
        .await?;
    Ok(())
}

/// Track that a view of type `view_type` was made on `table`.
pub async fn view_created(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    table: PageId,
    view_type: super::table_views::ViewType,
) -> Result<()> {
    let (Some(view_type), Some(table)) = (view_type_of(view_type), snapshot(tx, table).await?)
    else {
        return Ok(());
    };
    state
        .tracker
        .track(
            tx,
            &tracking::ViewCreated {
                workspace,
                table,
                view_type,
            },
        )
        .await?;
    Ok(())
}

/// Track that `table` was queried and answered `rows` rows.
pub async fn rows_queried(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    table: PageId,
    view_type: Option<super::table_views::ViewType>,
    rows: usize,
) -> Result<()> {
    let Some(table) = snapshot(tx, table).await? else {
        return Ok(());
    };
    state
        .tracker
        .track(
            tx,
            &tracking::RowsQueried {
                workspace,
                rows: bucket(rows),
                table,
                view_type: view_type.and_then(view_type_of),
            },
        )
        .await?;
    Ok(())
}

/// Track that `cells` cells of `row` changed.
pub async fn cells_edited(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    row: PageId,
    cells: usize,
) -> Result<()> {
    if cells == 0 {
        return Ok(());
    }
    let Some(row) = snapshot(tx, row).await? else {
        return Ok(());
    };
    state
        .tracker
        .track(
            tx,
            &tracking::CellsEdited {
                workspace,
                cells: bucket(cells),
                row,
            },
        )
        .await?;
    Ok(())
}
