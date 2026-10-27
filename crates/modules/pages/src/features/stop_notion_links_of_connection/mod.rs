//! `pages.stop_notion_links_of_connection` ← `connectors.connection_removed`:
//! a Notion connection was disconnected (or its member left, which removes
//! it): every page linked through it stops being kept in step. The pages stay
//! as they are, and nothing is deleted at Notion (campbooks-cloud#376).

use campbooks_kernel::{Event, EventHandler, JobContext, JobError, tenancy::WorkspaceId};

use crate::{
    domain::{
        PageId,
        notion::{self, StopReason},
    },
    events::ConnectionRemoved,
};

/// Ends the links of a connection that is gone.
pub struct StopNotionLinksOfConnection;

impl EventHandler for StopNotionLinksOfConnection {
    type Event = ConnectionRemoved;
    const NAME: &'static str = "pages.stop_notion_links_of_connection";
    const QUEUE: &'static str = "default";

    async fn handle(ctx: &mut JobContext, event: Event<ConnectionRemoved>) -> Result<(), JobError> {
        let removed = event.payload;
        let workspace: WorkspaceId = removed
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("connection_removed names no workspace"))?;
        let state = ctx.state().clone();
        let tx = ctx.tx().await?;
        let pages: Vec<PageId> = sqlx::query_scalar(
            "SELECT page_id FROM pages.external_links
              WHERE connection_id = $1 AND state <> 'stopped' ORDER BY page_id",
        )
        .bind(removed.connection)
        .fetch_all(&mut **tx)
        .await?;
        for page in pages {
            notion::stop(&state, tx, workspace, page, StopReason::Disconnected).await?;
        }
        // The tables kept in step with a database of that account.
        let tables: Vec<PageId> = sqlx::query_scalar(
            "SELECT table_id FROM pages.external_tables
              WHERE connection_id = $1 AND state <> 'stopped' ORDER BY table_id",
        )
        .bind(removed.connection)
        .fetch_all(&mut **tx)
        .await?;
        for table in tables {
            notion::stop(&state, tx, workspace, table, StopReason::Disconnected).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
