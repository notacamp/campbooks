//! `connectors.record_notion_table_changes` ←
//! `pages.notion_table_changes_requested`: table changes a member's edits
//! asked for are recorded for the Notion database's stream, whose next run
//! makes them before it reads
//! (`contracts/pages.notion_table_changes_requested.v1.schema.json`).
//!
//! The event is not taken as proof (campbooks-cloud#227): before anything is
//! recorded the pages module is asked, through the kernel's page-requests
//! port, whether the request still stands (waiting for the connector, its
//! binding verifying, its table's link syncing), and what is recorded is
//! what it stored, never what the event names. Each change recorded is bound
//! to the request and to the stream, connection and data source that make it
//! with a MAC under a key the database never holds. A request that does not
//! stand records nothing. One that cannot be made at all (databases are off,
//! the stream is gone, or waits for the member) ends at once, every change
//! skipped, for the pages module to know.

use campbooks_kernel::{
    AppState, Event, EventConsumer, JobContext, JobError, outbound::StoredTableItem,
    tenancy::WorkspaceId,
};
use sqlx::{PgConnection, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    deps::Deps,
    domain::{
        model::{ConnectionId, StreamId},
        schedule,
    },
    events::{
        NotionOutcome, NotionReason, NotionTableChange, NotionTableChangesCompleted,
        NotionTableChangesRequested, NotionTableItemResult,
    },
    notion::table_changes::{self, At, Request, kind_name},
};

/// Records a member's table changes for the connector to make.
#[derive(Debug, Clone)]
pub struct RecordNotionTableChanges {
    deps: Deps,
}

impl RecordNotionTableChanges {
    /// The consumer, composed with the module's dependencies.
    pub fn new(deps: Deps) -> Self {
        Self { deps }
    }
}

/// The stream that reads a linked Notion database.
struct Target {
    stream: StreamId,
    paused: bool,
}

impl EventConsumer for RecordNotionTableChanges {
    type Event = NotionTableChangesRequested;
    const NAME: &'static str = "connectors.record_notion_table_changes";
    const QUEUE: &'static str = "events";

    async fn handle(
        &self,
        ctx: &mut JobContext,
        event: Event<NotionTableChangesRequested>,
    ) -> Result<(), JobError> {
        let asked = event.payload;
        let workspace: WorkspaceId = asked
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("notion_table_changes_requested names no workspace"))?;
        let now = self.deps.clock.now();
        let state = ctx.state().clone();
        let tx = ctx.tx().await?;
        let stored = state
            .page_requests
            .stored_table_changes(&state, tx, workspace, asked.request)
            .await?;
        let Some(stored) = stored else {
            tracing::warn!(%workspace, request = %asked.request, "Notion table changes that do not stand are not recorded");
            return Ok(());
        };
        if stored.table != asked.table
            || stored.connection != Uuid::from(asked.connection)
            || stored.external != asked.data_source
        {
            tracing::warn!(%workspace, request = %asked.request, "notion_table_changes_requested names another table, connection or database than the request it cites; nothing is recorded");
            return Ok(());
        }
        let skip = |reason: NotionReason| {
            stored
                .items
                .iter()
                .map(|item| result_of(item, reason))
                .collect::<Vec<_>>()
        };
        let target = if self.deps.notion.is_some() && self.deps.notion_databases {
            target_of(tx, asked.connection, asked.data_source, asked.table).await?
        } else {
            None
        };
        let Some(target) = target else {
            end_at_once(
                &state,
                tx,
                workspace,
                &asked,
                skip(NotionReason::NotConnected),
            )
            .await?;
            return Ok(());
        };
        if target.paused {
            end_at_once(
                &state,
                tx,
                workspace,
                &asked,
                skip(NotionReason::NeedsReauth),
            )
            .await?;
            return Ok(());
        }
        // Serialise with the stream's run, which looks for changes recorded
        // while it ran under the same lock.
        sqlx::query("SELECT id FROM connectors.streams WHERE id = $1 FOR UPDATE")
            .bind(target.stream)
            .fetch_one(&mut **tx)
            .await?;
        let recorded = table_changes::record(
            tx,
            &state.binding,
            workspace,
            target.stream,
            At {
                connection: asked.connection,
                data_source: asked.data_source,
            },
            &Request {
                request: asked.request,
                table: asked.table,
                items: &stored.items,
                body: &stored.body,
            },
        )
        .await?;
        if recorded > 0 {
            schedule::run_now(tx, workspace, target.stream, now).await?;
            tracing::info!(%workspace, stream = %target.stream, changes = recorded, "Notion table changes were recorded");
        }
        Ok(())
    }
}

/// An item that ended at once, for `reason`.
fn result_of(item: &StoredTableItem, reason: NotionReason) -> NotionTableItemResult {
    NotionTableItemResult {
        subject: item.subject,
        kind: NotionTableChange::of(kind_name(item.kind))
            .unwrap_or(NotionTableChange::SchemaUpdate),
        outcome: NotionOutcome::Skipped,
        external: None,
        edited_at: None,
        hash: None,
        reason: Some(reason),
    }
}

/// End the request `asked` names at once, with `results`.
async fn end_at_once(
    state: &AppState,
    tx: &mut Transaction<'static, Postgres>,
    workspace: WorkspaceId,
    asked: &NotionTableChangesRequested,
    results: Vec<NotionTableItemResult>,
) -> Result<(), JobError> {
    state
        .events
        .publish(
            tx,
            &NotionTableChangesCompleted {
                workspace: workspace.to_string(),
                request: asked.request,
                table: asked.table,
                results,
            },
        )
        .await?;
    Ok(())
}

/// The stream that reads `data_source` through `connection` for `table`.
async fn target_of(
    conn: &mut PgConnection,
    connection: ConnectionId,
    data_source: Uuid,
    table: Uuid,
) -> Result<Option<Target>, JobError> {
    let row: Option<(StreamId, bool, String)> = sqlx::query_as(
        "SELECT s.id, s.paused, c.health
           FROM connectors.notion_links l
           JOIN connectors.streams s ON s.id = l.stream_id
           JOIN connectors.connections c ON c.id = l.connection_id
          WHERE l.connection_id = $1 AND l.notion_page_id = $2 AND l.page_id = $3
            AND l.kind = 'data_source'",
    )
    .bind(connection)
    .bind(data_source)
    .bind(table)
    .fetch_optional(conn)
    .await?;
    Ok(row.map(|(stream, paused, health)| Target {
        stream,
        paused: paused || health == "needs_reauth",
    }))
}
