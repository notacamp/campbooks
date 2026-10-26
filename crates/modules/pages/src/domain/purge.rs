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
    // The documents of the block changes asked of Notion for them.
    let requests: Vec<sqlx::types::Uuid> =
        sqlx::query_scalar("SELECT id FROM pages.notion_requests WHERE page_id = ANY($1)")
            .bind(&uuids)
            .fetch_all(&mut **tx)
            .await?;
    state
        .blobs
        .detach_owners(
            tx,
            &workspace.to_string(),
            super::notion::REQUEST_OWNER,
            &requests,
        )
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
            row: false,
        };
        state.events.publish(tx, &event).await?;
    }
    // Rows deleted for good leave the relation cells of other rows that held
    // them (a cell left empty reads as cleared); the rest goes with the
    // pages, by their foreign keys.
    let gone: Vec<String> = ids.iter().map(ToString::to_string).collect();
    sqlx::query(
        "UPDATE pages.row_values v
            SET value = trimmed.value, version = v.version + 1, previous = NULL, updated_at = now()
           FROM (
               SELECT rv.row_id, rv.property_id, COALESCE(keep.items, 'null'::jsonb) AS value
                 FROM pages.row_values rv
                 JOIN pages.table_properties tp ON tp.id = rv.property_id AND tp.type = 'relation'
                CROSS JOIN LATERAL (
                    SELECT jsonb_agg(item) AS items
                      FROM jsonb_array_elements(rv.value) item
                     WHERE item #>> '{}' <> ALL($1::text[])
                ) keep
                WHERE jsonb_typeof(rv.value) = 'array' AND rv.value ?| $1::text[]
           ) trimmed
          WHERE v.row_id = trimmed.row_id AND v.property_id = trimmed.property_id",
    )
    .bind(&gone)
    .execute(&mut **tx)
    .await?;
    sqlx::query("DELETE FROM pages.pages WHERE trash_root = $1")
        .bind(row.id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
