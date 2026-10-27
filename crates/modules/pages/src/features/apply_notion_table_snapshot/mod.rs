//! `pages.apply_notion_table_snapshot` ← `connectors.notion_data_source_read`:
//! a Notion database was read, and is applied to the table linked to it,
//! column by column and cell by cell (campbooks-cloud#377).
//!
//! In one transaction that holds the table's row like a sync does:
//!
//! 1. The table and its link must be as the event says (syncing, the same
//!    connection and data source), and the snapshot newer than the last one
//!    applied; anything else is ignored.
//! 2. The snapshot is merged (`domain::notion::tables::apply`): what changed
//!    only at Notion is written in, what changed only here stays, what
//!    changed on both sides is kept apart as a conflict.
//! 3. The link's counters are kept, the change is tracked, and the table is
//!    looked at again for what is now different from the new bases.
//!
//! No name, option or value reaches an event, a log or a job error: they
//! carry ids and counts.

use campbooks_kernel::{
    AppState, Event, EventHandler, JobContext, JobError,
    blobs::BlobRef,
    tenancy::{Access, WorkspaceId},
};
use futures_util::StreamExt as _;
use sqlx::{Postgres, Transaction};

use crate::{
    domain::{
        PageRow, SELECT_LOCKED, announce, clean_title,
        notion::tables::{
            self, apply,
            model::{MAX_SNAPSHOT_BYTES, Snapshot},
        },
        tracked::{self, ConflictKind},
    },
    events::{NotionDataSourceRead, PageChangeKind},
};

/// Applies a Notion database snapshot to the table linked to it.
pub struct ApplyNotionTableSnapshot;

impl EventHandler for ApplyNotionTableSnapshot {
    type Event = NotionDataSourceRead;
    const NAME: &'static str = "pages.apply_notion_table_snapshot";
    const QUEUE: &'static str = "default";

    async fn handle(
        ctx: &mut JobContext,
        event: Event<NotionDataSourceRead>,
    ) -> Result<(), JobError> {
        let read = event.payload;
        let workspace: WorkspaceId = read
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("notion_data_source_read names no workspace"))?;
        let state = ctx.state().clone();
        let tx = ctx.tx().await?;
        let row: Option<PageRow> = sqlx::query_as(SELECT_LOCKED)
            .bind(read.table)
            .fetch_optional(&mut **tx)
            .await?;
        let Some(row) = row.filter(|row| !row.is_trashed() && row.kind == "table") else {
            return Ok(());
        };
        let Some(ext) = tables::lock_external(tx, read.table).await? else {
            return Ok(());
        };
        if !ext.syncing()
            || ext.connection_id != read.connection
            || ext.notion_data_source_id != read.data_source
            || ext.last_read_at.is_some_and(|at| at >= read.read_at)
        {
            return Ok(());
        }
        let Some(snapshot) = read_snapshot(&state, workspace, &read.snapshot).await? else {
            tracing::warn!(%workspace, "a Notion database snapshot could not be read; it is not applied");
            return Ok(());
        };
        if snapshot.version != 1 || snapshot.data_source != read.data_source {
            tracing::warn!(%workspace, "a Notion database snapshot is not the database's; it is not applied");
            return Ok(());
        }
        let counts = apply::apply(&state, tx, workspace, &row, &ext, &snapshot).await?;
        let title = keep_title(tx, &state, workspace, &row, &ext, &snapshot).await?;
        let open = tables::open_conflicts(tx, read.table).await?;
        sqlx::query(
            "UPDATE pages.external_tables
                SET last_read_at = $2, conflicts = $3, last_title = $4,
                    last_inbound_at = CASE WHEN $5 THEN now() ELSE last_inbound_at END,
                    schema_hash = COALESCE($6, schema_hash)
              WHERE table_id = $1",
        )
        .bind(read.table)
        .bind(read.read_at)
        .bind(open)
        .bind(title)
        .bind(counts.moved() > 0)
        .bind(snapshot.schema.as_ref().map(|schema| schema.hash.clone()))
        .execute(&mut **tx)
        .await?;
        if counts.moved() > 0 {
            tracked::notion_table_synced(
                &state,
                tx,
                workspace,
                read.table,
                tracked::Direction::Inbound,
                counts.moved(),
            )
            .await?;
        }
        for (kind, n) in [
            (ConflictKind::Column, counts.column_conflicts),
            (ConflictKind::Cell, counts.cell_conflicts),
        ] {
            if n > 0 {
                tracked::notion_table_conflict_raised(&state, tx, workspace, read.table, kind, n)
                    .await?;
            }
        }
        // What is different here from the new bases goes out.
        tables::schedule_push(tx, workspace, read.table).await?;
        tracing::info!(
            %workspace,
            columns = counts.columns,
            rows = counts.rows,
            cells = counts.cells,
            conflicts = counts.column_conflicts + counts.cell_conflicts,
            "a Notion database snapshot was applied"
        );
        Ok(())
    }
}

/// The snapshot `blob` holds in `workspace`, when it is at most
/// [`MAX_SNAPSHOT_BYTES`] and parses.
async fn read_snapshot(
    state: &AppState,
    workspace: WorkspaceId,
    blob: &BlobRef,
) -> Result<Option<Snapshot>, JobError> {
    if usize::try_from(blob.size).unwrap_or(usize::MAX) > MAX_SNAPSHOT_BYTES {
        return Ok(None);
    }
    let mut stream = state.blobs.get(&workspace.to_string(), blob).await?;
    let mut bytes: Vec<u8> = Vec::new();
    while let Some(chunk) = stream.next().await {
        bytes.extend_from_slice(&chunk?);
        if bytes.len() > MAX_SNAPSHOT_BYTES {
            return Ok(None);
        }
    }
    Ok(serde_json::from_slice(&bytes).ok())
}

/// Keep Notion's title: the table takes it when it has not been renamed here
/// since the last read. The title kept as Notion's now.
async fn keep_title(
    tx: &mut Transaction<'static, Postgres>,
    state: &AppState,
    workspace: WorkspaceId,
    row: &PageRow,
    ext: &tables::ExternalTable,
    snapshot: &Snapshot,
) -> Result<String, JobError> {
    let theirs =
        clean_title(&snapshot.title.chars().take(200).collect::<String>()).unwrap_or_default();
    let seen = ext.last_title.clone().unwrap_or_default();
    if theirs != seen && row.title == seen && theirs != row.title {
        sqlx::query(
            "UPDATE pages.pages
                SET title = $2, meta_revision = meta_revision + 1, revision = revision + 1
              WHERE id = $1",
        )
        .bind(row.id)
        .bind(&theirs)
        .execute(&mut **tx)
        .await?;
        let access = Access {
            workspace,
            user: Some(ext.created_by),
            role: None,
        };
        announce(state, tx, &access, row.id, vec![PageChangeKind::Meta]).await?;
    }
    Ok(theirs)
}
