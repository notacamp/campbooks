//! `pages.push_table_to_notion`: what a member changed in a table that is
//! kept in step with Notion is looked at ten seconds after the change,
//! compared with the bases the last sync agreed, and what differs is
//! recorded as a request for the connectors module to make
//! (campbooks-cloud#377).
//!
//! Nothing is sent from here: a request is stored bound (MAC) and announced
//! (`pages.notion_table_changes_requested`, ids and a blob reference), and
//! the connector asks this module whether it still stands before it writes.
//! One request waits at a time; its end
//! (`connectors.notion_table_changes_completed`) moves the bases. A column
//! or cell in conflict is not sent until the member chooses.

use campbooks_kernel::{Job, JobContext, JobError, tenancy::WorkspaceId};
use chrono::{TimeDelta, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::{
    PageId, PageRow, SELECT_LOCKED,
    notion::tables::{self, diff, requests},
};

/// How long a push waits for a request still with the connector.
const WAITING_SECONDS: i64 = 15;

/// Looks at one table's changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PushTableToNotion {
    /// The table.
    pub table: PageId,
}

impl Job for PushTableToNotion {
    const NAME: &'static str = "pages.push_table_to_notion";
    const QUEUE: &'static str = "default";

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        let workspace: WorkspaceId = ctx
            .workspace()
            .and_then(|w| w.parse().ok())
            .ok_or_else(|| JobError::fatal("a push to Notion was enqueued outside a workspace"))?;
        let state = ctx.state().clone();
        let tx = ctx.tx().await?;
        let row: Option<PageRow> = sqlx::query_as(SELECT_LOCKED)
            .bind(self.table)
            .fetch_optional(&mut **tx)
            .await?;
        let Some(row) = row.filter(|row| !row.is_trashed() && row.kind == "table") else {
            return Ok(());
        };
        let _ = row;
        let Some(ext) = tables::lock_external(tx, self.table).await? else {
            return Ok(());
        };
        if !ext.syncing() {
            return Ok(());
        }
        let waiting: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM pages.notion_table_requests
                             WHERE table_id = $1 AND state = 'waiting')",
        )
        .bind(self.table)
        .fetch_one(&mut **tx)
        .await?;
        if waiting {
            // The connector has not made the last request: look again.
            let again = Utc::now() + TimeDelta::seconds(WAITING_SECONDS);
            campbooks_kernel::jobs::enqueue(
                tx,
                &self,
                campbooks_kernel::jobs::EnqueueOptions::for_workspace(workspace.to_string())
                    .unique(format!(
                        "pages.push_table_to_notion:{}:{}",
                        self.table,
                        again.timestamp_millis()
                    ))
                    .run_at(again),
            )
            .await?;
            return Ok(());
        }
        let plan = diff::plan(tx, &ext).await?;
        if plan.items.is_empty() {
            return Ok(());
        }
        requests::create(
            &state,
            tx,
            workspace,
            &ext,
            ext.created_by,
            &plan.items,
            &plan.document,
        )
        .await?;
        tracing::info!(%workspace, changes = plan.items.len(), "table changes were recorded for Notion");
        Ok(())
    }
}
