//! Building a workspace's index from its sources, a page at a time, in the
//! background (`search.backfill`, its own queue so live refreshes never wait
//! behind it). Started by `domain::index::ensure_backfill`, resumable: each
//! page records how far it got and enqueues the next one.

use campbooks_kernel::{
    Job, JobContext, JobError,
    jobs::EnqueueOptions,
    search::SourceContext,
    tenancy::WorkspaceId,
};
use serde::{Deserialize, Serialize};

use crate::domain::index::{self, INDEX_VERSION};

/// Groups refreshed per job.
pub const PAGE: u32 = 100;

/// One page of a workspace's backfill of one source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Backfill {
    /// The source, by group type.
    pub group_type: String,
    /// The last group id of the previous page; `None` for the first.
    pub after: Option<String>,
}

impl Backfill {
    /// Its enqueue options in `workspace`: one pending job per page, behind
    /// live work.
    pub fn options(&self, workspace: WorkspaceId) -> EnqueueOptions {
        let page = self.after.as_deref().unwrap_or("^");
        EnqueueOptions::for_workspace(workspace.to_string())
            .unique(format!("search.backfill:{workspace}:{}:{page}", self.group_type))
            .priority(-10)
    }
}

impl Job for Backfill {
    const NAME: &'static str = "search.backfill";
    const QUEUE: &'static str = "search";

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        let workspace: WorkspaceId = ctx
            .workspace()
            .and_then(|w| w.parse().ok())
            .ok_or_else(|| JobError::fatal("backfill enqueued outside a workspace"))?;
        let state = ctx.state().clone();
        let Some(source) = state.search_sources.get(&self.group_type).cloned() else {
            return Ok(());
        };
        let tx = ctx.tx().await?;
        let current = sqlx::query!(
            "SELECT state, after_id, index_version FROM search.backfills
              WHERE workspace_id = $1 AND group_type = $2 FOR UPDATE",
            workspace.as_uuid(),
            self.group_type,
        )
        .fetch_optional(&mut **tx)
        .await?;
        let Some(current) = current else {
            return Ok(());
        };
        // A newer backfill replaced this one, or this page was done already.
        if current.state != "running"
            || current.index_version != INDEX_VERSION
            || current.after_id != self.after
        {
            return Ok(());
        }
        let cx = SourceContext {
            conn: &mut **tx,
            blobs: &state.blobs,
            workspace,
        };
        let page = source.page(cx, self.after.as_deref(), PAGE).await?;
        for id in &page.groups {
            index::refresh(&mut **tx, &state.blobs, workspace, source.as_ref(), id).await?;
        }
        if let Some(next) = page.next {
            sqlx::query!(
                "UPDATE search.backfills SET after_id = $3
                  WHERE workspace_id = $1 AND group_type = $2",
                workspace.as_uuid(),
                self.group_type,
                next,
            )
            .execute(&mut **tx)
            .await?;
            let job = Self {
                group_type: self.group_type.clone(),
                after: Some(next),
            };
            campbooks_kernel::jobs::enqueue(&mut *tx, &job, job.options(workspace)).await?;
        } else {
            sqlx::query!(
                "UPDATE search.backfills SET state = 'done', finished_at = now()
                  WHERE workspace_id = $1 AND group_type = $2",
                workspace.as_uuid(),
                self.group_type,
            )
            .execute(&mut **tx)
            .await?;
            tracing::info!(
                groups = page.groups.len(),
                "search backfill finished for a workspace"
            );
        }
        Ok(())
    }
}
