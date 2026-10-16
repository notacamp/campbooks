//! `conversations.fill_templates` — work out the subject templates of an
//! inbox's older mail (stored before templates existed), a page per job, so
//! template conditions can use their index. Enqueued once per inbox while
//! some are missing (by `apply_changes` and when a rule is created); a page
//! that comes back full enqueues the next.

use campbooks_kernel::{
    JobContext, JobError, JobHandler,
    jobs::{EnqueueOptions, Enqueued},
};
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Transaction};

use crate::{
    domain::rules::{self, TEMPLATE_PAGE},
    model::InboxId,
};

/// One page of an inbox's templates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FillTemplatesPayload {
    /// The inbox.
    pub inbox: InboxId,
    /// Which page (0 for the first), so a page can enqueue the next while
    /// its own unique key is held.
    #[serde(default)]
    pub page: u32,
}

/// Works out older mail's subject templates.
pub struct FillTemplates;

impl JobHandler for FillTemplates {
    type Payload = FillTemplatesPayload;
    const NAME: &'static str = "conversations.fill_templates";
    const QUEUE: &'static str = "rules";

    async fn run(
        &self,
        ctx: &mut JobContext,
        payload: FillTemplatesPayload,
    ) -> Result<(), JobError> {
        let workspace = ctx
            .workspace()
            .map(ToOwned::to_owned)
            .ok_or_else(|| JobError::fatal("fill_templates enqueued outside a workspace"))?;
        let tx = ctx.tx().await?;
        let done = rules::fill_templates(tx, payload.inbox).await?;
        if i64::try_from(done).unwrap_or(0) >= TEMPLATE_PAGE {
            enqueue_page(
                tx,
                &workspace,
                payload.inbox,
                payload.page.saturating_add(1),
            )
            .await?;
        }
        Ok(())
    }
}

/// Enqueue the template backfill of `inbox` in `workspace`, unless its
/// first page is pending.
pub async fn enqueue(
    tx: &mut Transaction<'_, Postgres>,
    workspace: &str,
    inbox: InboxId,
) -> Result<Enqueued, sqlx::Error> {
    enqueue_page(tx, workspace, inbox, 0).await
}

async fn enqueue_page(
    tx: &mut Transaction<'_, Postgres>,
    workspace: &str,
    inbox: InboxId,
    page: u32,
) -> Result<Enqueued, sqlx::Error> {
    campbooks_kernel::jobs::enqueue_to::<FillTemplates>(
        tx,
        &FillTemplatesPayload { inbox, page },
        EnqueueOptions::for_workspace(workspace.to_owned())
            .unique(format!("conversations.fill_templates:{inbox}:{page}"))
            .priority(-10),
    )
    .await
}
