//! Building a workspace's index from its sources, a page at a time, in the
//! background (`search.backfill`, its own queue so live refreshes never wait
//! behind it). Started by `domain::index::ensure_backfill`, resumable: each
//! page records how far it got and enqueues the next one.

use campbooks_kernel::{
    AppState, Job, JobContext, JobError, Result,
    db::BeginFor as _,
    jobs::{self, EnqueueOptions},
    search::SourceContext,
    tenancy::WorkspaceId,
};
use serde::{Deserialize, Serialize};

use crate::domain::index::{self, INDEX_VERSION};

/// Groups refreshed per job.
pub const PAGE: u32 = 50;

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
            .unique(format!(
                "search.backfill:{workspace}:{}:{page}",
                self.group_type
            ))
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
              WHERE workspace_id = $1 AND group_type = $2",
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
            conn: tx,
            blobs: &state.blobs,
            workspace,
        };
        let page = source.page(cx, self.after.as_deref(), PAGE).await?;
        for id in &page.groups {
            index::refresh(tx, &state.blobs, workspace, source.as_ref(), id).await?;
        }
        // Compare and set: move the backfill on only if nothing replaced it
        // while the page ran (no row lock is held across the page, so
        // searches and live deliveries never wait on it).
        let moved = sqlx::query_scalar!(
            r#"UPDATE search.backfills
                  SET after_id = $3,
                      state = CASE WHEN $3::text IS NULL THEN 'done' ELSE state END,
                      finished_at = CASE WHEN $3::text IS NULL THEN now() END
                WHERE workspace_id = $1 AND group_type = $2 AND state = 'running'
                  AND index_version = $4 AND after_id IS NOT DISTINCT FROM $5
               RETURNING group_type"#,
            workspace.as_uuid(),
            self.group_type,
            page.next.as_deref(),
            INDEX_VERSION,
            self.after.as_deref(),
        )
        .fetch_optional(&mut **tx)
        .await?
        .is_some();
        if !moved {
            return Ok(());
        }
        if let Some(next) = page.next {
            let job = Self {
                group_type: self.group_type.clone(),
                after: Some(next),
            };
            jobs::enqueue(&mut *tx, &job, job.options(workspace)).await?;
        } else {
            tracing::info!(
                groups = page.groups.len(),
                "search backfill finished for a workspace"
            );
        }
        Ok(())
    }
}

/// Rebuild `workspace`'s index from every registered source, from the first
/// page (`campbooks-server search reindex --workspace <id>`). Queries keep
/// answering from the current rows while it runs. Returns the sources
/// restarted.
pub async fn reindex(state: &AppState, workspace: WorkspaceId) -> Result<Vec<String>> {
    let mut tx = state.db.begin_for(workspace).await?;
    let mut restarted = Vec::new();
    for source in state.search_sources.iter() {
        let group_type = source.group_type();
        sqlx::query!(
            r#"INSERT INTO search.backfills (workspace_id, group_type, state, index_version)
               VALUES ($1, $2, 'running', $3)
               ON CONFLICT (workspace_id, group_type) DO UPDATE
                  SET state = 'running', after_id = NULL, index_version = EXCLUDED.index_version,
                      started_at = now(), finished_at = NULL
                WHERE search.backfills.state <> 'deleting'"#,
            workspace.as_uuid(),
            group_type,
            INDEX_VERSION,
        )
        .execute(&mut *tx)
        .await?;
        let job = Backfill {
            group_type: group_type.to_owned(),
            after: None,
        };
        jobs::enqueue(&mut tx, &job, job.options(workspace)).await?;
        restarted.push(group_type.to_owned());
    }
    tx.commit().await?;
    Ok(restarted)
}
