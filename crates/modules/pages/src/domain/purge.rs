//! Deleting pages for good.

use campbooks_kernel::{AppState, Result, tenancy::WorkspaceId};
use sqlx::{Postgres, Transaction};

use super::{PageId, PageRow};
use crate::events::{PageChangeKind, PageChanged};

/// Delete trash root `row` and everything deleted with it, in `tx`: its
/// bodies, grants, links and files go with the rows, the blobs they held are
/// let go, and each page is announced as purged.
pub async fn root(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    row: &PageRow,
) -> Result<()> {
    let ids: Vec<PageId> = sqlx::query_scalar("SELECT id FROM pages.pages WHERE trash_root = $1")
        .bind(row.id)
        .fetch_all(&mut **tx)
        .await?;
    let uuids: Vec<sqlx::types::Uuid> = ids.iter().map(|id| *id.as_uuid()).collect();
    state
        .blobs
        .detach_owners(tx, &workspace.to_string(), "pages.page", &uuids)
        .await?;
    for id in &ids {
        let event = PageChanged {
            workspace: workspace.to_string(),
            page: *id,
            revision: row.revision + 1,
            parent: None,
            place: row.place()?,
            trashed: true,
            kinds: vec![PageChangeKind::Purge],
        };
        state.events.publish(tx, &event).await?;
    }
    sqlx::query("DELETE FROM pages.pages WHERE trash_root = $1")
        .bind(row.id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
