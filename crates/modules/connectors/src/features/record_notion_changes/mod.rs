//! `connectors.record_notion_changes` ← `pages.notion_changes_requested`:
//! block changes a member's edits asked for are recorded for the Notion
//! page's stream, whose next run makes them before it reads
//! (`contracts/pages.notion_changes_requested.v1.schema.json`).
//!
//! The event is not taken as proof (campbooks-cloud#227): before anything is
//! recorded the pages module is asked, through the kernel's page-requests
//! port, whether the request still stands (waiting for the connector, its
//! binding verifying, its page's link syncing), and what is recorded is what
//! it stored, never what the event names. Each change recorded is bound to
//! the request and to the stream, connection and Notion page that make it
//! with a MAC under a key the database never holds. A request that does not
//! stand records nothing. One that cannot be made at all (the stream is
//! gone, or waits for the member) ends at once, every change skipped, for
//! the pages module to know.

use campbooks_kernel::{
    AppState, Event, EventConsumer, JobContext, JobError, tenancy::WorkspaceId,
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
        NotionChangesCompleted, NotionChangesRequested, NotionItemResult, NotionOutcome,
        NotionReason,
    },
    notion::changes::{self, At, Request},
};

/// Records a member's block changes for the connector to make.
#[derive(Debug, Clone)]
pub struct RecordNotionChanges {
    deps: Deps,
}

impl RecordNotionChanges {
    /// The consumer, composed with the module's dependencies.
    pub fn new(deps: Deps) -> Self {
        Self { deps }
    }
}

/// The stream that reads a linked Notion page.
struct Target {
    stream: StreamId,
    paused: bool,
}

impl EventConsumer for RecordNotionChanges {
    type Event = NotionChangesRequested;
    const NAME: &'static str = "connectors.record_notion_changes";
    const QUEUE: &'static str = "events";

    async fn handle(
        &self,
        ctx: &mut JobContext,
        event: Event<NotionChangesRequested>,
    ) -> Result<(), JobError> {
        let asked = event.payload;
        let workspace: WorkspaceId = asked
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("notion_changes_requested names no workspace"))?;
        let now = self.deps.clock.now();
        let state = ctx.state().clone();
        let tx = ctx.tx().await?;
        let stored = state
            .page_requests
            .stored_changes(&state, tx, workspace, asked.request)
            .await?;
        let Some(stored) = stored else {
            // It does not stand: nothing is made.
            tracing::warn!(%workspace, request = %asked.request, "Notion block changes that do not stand are not recorded");
            return Ok(());
        };
        if stored.page != asked.page
            || stored.connection != Uuid::from(asked.connection)
            || stored.external_page != asked.notion_page
        {
            tracing::warn!(%workspace, request = %asked.request, "notion_changes_requested names another page, connection or Notion page than the request it cites; nothing is recorded");
            return Ok(());
        }
        let skip = |reason: NotionReason| {
            stored
                .items
                .iter()
                .map(|item| NotionItemResult {
                    block: item.block,
                    outcome: NotionOutcome::Skipped,
                    notion_block: None,
                    edited_at: None,
                    reason: Some(reason),
                })
                .collect::<Vec<_>>()
        };
        let target = if self.deps.notion.is_some() {
            target_of(tx, asked.connection, asked.notion_page, asked.page).await?
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
            // The member must connect again first.
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
        let recorded = changes::record(
            tx,
            &state.binding,
            workspace,
            target.stream,
            At {
                connection: asked.connection,
                notion_page: asked.notion_page,
            },
            &Request {
                request: asked.request,
                page: asked.page,
                items: &stored.items,
                body: &stored.body,
            },
        )
        .await?;
        if recorded > 0 {
            schedule::run_now(tx, workspace, target.stream, now).await?;
            tracing::info!(%workspace, stream = %target.stream, changes = recorded, "Notion block changes were recorded");
        }
        Ok(())
    }
}

/// End the request `asked` names at once, with `results`: the pages module
/// is told so its request does not wait.
async fn end_at_once(
    state: &AppState,
    tx: &mut Transaction<'static, Postgres>,
    workspace: WorkspaceId,
    asked: &NotionChangesRequested,
    results: Vec<NotionItemResult>,
) -> Result<(), JobError> {
    state
        .events
        .publish(
            tx,
            &NotionChangesCompleted {
                workspace: workspace.to_string(),
                request: asked.request,
                page: asked.page,
                results,
            },
        )
        .await?;
    Ok(())
}

/// The stream that reads `notion_page` through `connection` for `page`.
async fn target_of(
    conn: &mut PgConnection,
    connection: ConnectionId,
    notion_page: Uuid,
    page: Uuid,
) -> Result<Option<Target>, JobError> {
    let row: Option<(StreamId, bool, String)> = sqlx::query_as(
        "SELECT s.id, s.paused, c.health
           FROM connectors.notion_links l
           JOIN connectors.streams s ON s.id = l.stream_id
           JOIN connectors.connections c ON c.id = l.connection_id
          WHERE l.connection_id = $1 AND l.notion_page_id = $2 AND l.page_id = $3",
    )
    .bind(connection)
    .bind(notion_page)
    .bind(page)
    .fetch_optional(conn)
    .await?;
    Ok(row.map(|(stream, paused, health)| Target {
        stream,
        paused: paused || health == "needs_reauth",
    }))
}
