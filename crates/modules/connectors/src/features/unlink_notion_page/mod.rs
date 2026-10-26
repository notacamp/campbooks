//! `connectors.unlink_notion_page` ← `pages.notion_link_ended`: a page
//! stopped being kept in step with its Notion page, so the stream that read
//! it goes. Nothing is deleted at Notion (campbooks-cloud#376).

use campbooks_kernel::{Event, EventConsumer, JobContext, JobError, tenancy::WorkspaceId};

use crate::{
    domain::{model::StreamId, notion as notion_db},
    events::NotionLinkEnded,
};

/// Deletes the stream of a Notion page that is no longer linked.
#[derive(Debug, Clone, Copy)]
pub struct UnlinkNotionPage;

impl EventConsumer for UnlinkNotionPage {
    type Event = NotionLinkEnded;
    const NAME: &'static str = "connectors.unlink_notion_page";
    const QUEUE: &'static str = "events";

    async fn handle(
        &self,
        ctx: &mut JobContext,
        event: Event<NotionLinkEnded>,
    ) -> Result<(), JobError> {
        let ended = event.payload;
        let workspace: WorkspaceId = ended
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("notion_link_ended names no workspace"))?;
        let state = ctx.state().clone();
        let tx = ctx.tx().await?;
        // Only the stream of that link: the page, the connection and the
        // Notion page must all be the ones the event names.
        let stream: Option<StreamId> = sqlx::query_scalar(
            "SELECT stream_id FROM connectors.notion_links
              WHERE connection_id = $1 AND notion_page_id = $2 AND page_id = $3
                FOR UPDATE",
        )
        .bind(ended.connection)
        .bind(ended.notion_page)
        .bind(ended.page)
        .fetch_optional(&mut **tx)
        .await?;
        if let Some(stream) = stream {
            notion_db::drop_stream(tx, &state, workspace, stream).await?;
            tracing::info!(%workspace, "a Notion page is no longer read");
        }
        Ok(())
    }
}
