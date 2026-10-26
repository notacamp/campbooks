//! `pages.stop_notion_link` ← `connectors.notion_link_stopped`: Notion no
//! longer shares a page with the integration, or the page went to the trash
//! in Notion. The link ends with that reason, the page stays as it is, and
//! nothing is deleted at Notion (campbooks-cloud#376).

use campbooks_kernel::{Event, EventHandler, JobContext, JobError, tenancy::WorkspaceId};

use crate::{
    domain::notion::{self, StopReason},
    events::{NotionLinkStopped, NotionStop},
};

/// Ends a link the connector stopped reading.
pub struct StopNotionLink;

impl EventHandler for StopNotionLink {
    type Event = NotionLinkStopped;
    const NAME: &'static str = "pages.stop_notion_link";
    const QUEUE: &'static str = "default";

    async fn handle(ctx: &mut JobContext, event: Event<NotionLinkStopped>) -> Result<(), JobError> {
        let stopped = event.payload;
        let workspace: WorkspaceId = stopped
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("notion_link_stopped names no workspace"))?;
        let state = ctx.state().clone();
        let tx = ctx.tx().await?;
        let Some(link) = notion::lock_link(tx, stopped.page).await? else {
            return Ok(());
        };
        // Only the link the event is about.
        if link.connection_id != stopped.connection || link.notion_page_id != stopped.notion_page {
            return Ok(());
        }
        let reason = match stopped.reason {
            NotionStop::NotShared => StopReason::NotShared,
            NotionStop::TrashedInNotion => StopReason::TrashedInNotion,
            NotionStop::NotionUnavailable => StopReason::NotionUnavailable,
        };
        notion::stop(&state, tx, workspace, stopped.page, reason).await?;
        Ok(())
    }
}
