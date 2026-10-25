//! Folding a page's updates into its snapshot (`pages.compact`).
//!
//! After 200 updates since the snapshot, or 10 minutes after the first one,
//! the page's document is written as one snapshot and the updates it covers
//! are deleted, in one transaction that holds the page's row like a sync
//! does. A reader takes the snapshot and the tail in one statement, so it
//! never sees one without the other; a sync that arrives meanwhile waits for
//! the lock and then finds the new snapshot.

use campbooks_kernel::{Job, JobContext, JobError};
use serde::{Deserialize, Serialize};

use crate::domain::{PageId, document, store};

/// Compacts one page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Compact {
    /// The page.
    pub page: PageId,
}

impl Job for Compact {
    const NAME: &'static str = "pages.compact";
    const QUEUE: &'static str = "default";

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        if ctx.workspace().is_none() {
            return Err(JobError::fatal("compaction enqueued outside a workspace"));
        }
        let tx = ctx.tx().await?;
        let locked: Option<PageId> =
            sqlx::query_scalar("SELECT id FROM pages.pages WHERE id = $1 FOR UPDATE")
                .bind(self.page)
                .fetch_optional(&mut **tx)
                .await?;
        if locked.is_none() {
            return Ok(());
        }
        let loaded = store::load(&mut **tx, self.page).await?;
        if loaded.tail == 0 {
            return Ok(());
        }
        let state = document::full_state(&loaded.doc);
        sqlx::query(
            "INSERT INTO pages.snapshots (page_id, workspace_id, state, upto_seq)
             SELECT id, workspace_id, $2, $3 FROM pages.pages WHERE id = $1
             ON CONFLICT (page_id)
             DO UPDATE SET state = EXCLUDED.state, upto_seq = EXCLUDED.upto_seq, at = now()",
        )
        .bind(self.page)
        .bind(&state)
        .bind(loaded.last_seq)
        .execute(&mut **tx)
        .await?;
        let folded = sqlx::query("DELETE FROM pages.updates WHERE page_id = $1 AND seq <= $2")
            .bind(self.page)
            .bind(loaded.last_seq)
            .execute(&mut **tx)
            .await?
            .rows_affected();
        tracing::info!(
            updates = folded,
            "a page's updates were folded into its snapshot"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests;
