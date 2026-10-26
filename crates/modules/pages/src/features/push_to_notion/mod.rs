//! `pages.push_to_notion`: the blocks a member changed on a page that is
//! kept in step with Notion are looked at ten seconds after the last
//! keystroke, compared with the base the last sync agreed, and the ones
//! that differ are recorded as a request for the connectors module to make
//! (campbooks-cloud#376).
//!
//! Nothing is sent from here: a request is stored bound (MAC) and announced
//! (`pages.notion_changes_requested`, ids and a blob reference), and the
//! connector asks this module whether it still stands before it writes.
//! One request waits at a time; its end (`connectors.notion_changes_completed`)
//! moves the bases. A page with a conflict waiting does not send that block
//! until the member chooses.

use campbooks_kernel::{Job, JobContext, JobError, tenancy::WorkspaceId};
use chrono::{TimeDelta, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::{
    PageId, PageRow, SELECT_LOCKED, announce, body,
    notion::{self, DEBOUNCE_SECONDS, push, requests},
    store,
};
use crate::events::PageChangeKind;

/// How long a push waits for a request still with the connector.
const WAITING_SECONDS: i64 = 15;

/// Looks at one page's changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PushToNotion {
    /// The page.
    pub page: PageId,
}

impl Job for PushToNotion {
    const NAME: &'static str = "pages.push_to_notion";
    const QUEUE: &'static str = "default";

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        let workspace: WorkspaceId = ctx
            .workspace()
            .and_then(|w| w.parse().ok())
            .ok_or_else(|| JobError::fatal("a push to Notion was enqueued outside a workspace"))?;
        let state = ctx.state().clone();
        let tx = ctx.tx().await?;
        let row: Option<PageRow> = sqlx::query_as(SELECT_LOCKED)
            .bind(self.page)
            .fetch_optional(&mut **tx)
            .await?;
        let Some(row) = row.filter(|row| !row.is_trashed()) else {
            return Ok(());
        };
        let Some(link) = notion::lock_link(tx, self.page).await? else {
            return Ok(());
        };
        if !link.syncing() {
            return Ok(());
        }
        let waiting: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM pages.notion_requests
                             WHERE page_id = $1 AND state = 'waiting')",
        )
        .bind(self.page)
        .fetch_one(&mut **tx)
        .await?;
        let now = Utc::now();
        let quiet = row.edited_at + TimeDelta::seconds(DEBOUNCE_SECONDS);
        if waiting || now < quiet {
            // Not yet: the last edit was moments ago, or the connector has
            // not made the last request. Look again.
            let again = if waiting {
                now + TimeDelta::seconds(WAITING_SECONDS)
            } else {
                quiet
            };
            campbooks_kernel::jobs::enqueue(
                tx,
                &self,
                campbooks_kernel::jobs::EnqueueOptions::for_workspace(workspace.to_string())
                    .unique(format!(
                        "pages.push_to_notion:{}:{}",
                        self.page,
                        again.timestamp_millis()
                    ))
                    .run_at(again),
            )
            .await?;
            return Ok(());
        }
        let loaded = store::load(tx, self.page).await?;
        let resolver = notion::resolver(tx, &state, workspace, self.page).await?;
        let mappings = notion::mappings(tx, self.page).await?;
        let prepared = push::prepare(&loaded.doc, &resolver)
            .map_err(|_| JobError::fatal("a page body could not be read for Notion"))?;
        if let Some(update) = &prepared.update {
            let access = campbooks_kernel::tenancy::Access {
                workspace,
                user: Some(link.created_by),
                role: None,
            };
            body::store(
                tx,
                &access,
                self.page,
                &loaded,
                update,
                &prepared.inspection,
            )
            .await?;
            announce(&state, tx, &access, self.page, vec![PageChangeKind::Body]).await?;
        }
        let plan = push::plan(&prepared.tree, &mappings, resolver.page_link());
        if plan.items.is_empty() {
            return Ok(());
        }
        requests::create(
            &state,
            tx,
            workspace,
            self.page,
            &link,
            link.created_by,
            &plan,
        )
        .await?;
        sqlx::query("UPDATE pages.external_links SET push_requested_at = now() WHERE page_id = $1")
            .bind(self.page)
            .execute(&mut **tx)
            .await?;
        tracing::info!(%workspace, changes = plan.items.len(), "block changes were recorded for Notion");
        Ok(())
    }
}

#[cfg(test)]
mod tests;
