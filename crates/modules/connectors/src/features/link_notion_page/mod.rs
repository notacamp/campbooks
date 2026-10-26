//! `connectors.link_notion_page` ← `pages.notion_link_requested`: a page was
//! linked to a page in a member's Notion account, which now needs a stream
//! to be read by (campbooks-cloud#376).
//!
//! The event is not taken as proof (campbooks-cloud#227): the pages module
//! is asked, through the kernel's page-requests port, whether the page is
//! linked to that Notion page through that connection and the link is
//! syncing. The connection must be a Notion connection of this workspace.
//! Then the stream (capability `pages`, keyed by the Notion page) and its
//! link are made, and it runs now: the first run reads the page.

use campbooks_kernel::{Event, EventConsumer, JobContext, JobError, tenancy::WorkspaceId};

use crate::{
    deps::Deps,
    domain::{
        model::{Phase, StreamId},
        schedule,
    },
    events::NotionLinkRequested,
};

/// Makes the stream of a linked Notion page.
#[derive(Debug, Clone)]
pub struct LinkNotionPage {
    deps: Deps,
}

impl LinkNotionPage {
    /// The consumer, composed with the module's dependencies.
    pub fn new(deps: Deps) -> Self {
        Self { deps }
    }
}

impl EventConsumer for LinkNotionPage {
    type Event = NotionLinkRequested;
    const NAME: &'static str = "connectors.link_notion_page";
    const QUEUE: &'static str = "events";

    async fn handle(
        &self,
        ctx: &mut JobContext,
        event: Event<NotionLinkRequested>,
    ) -> Result<(), JobError> {
        let asked = event.payload;
        let workspace: WorkspaceId = asked
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("notion_link_requested names no workspace"))?;
        let now = self.deps.clock.now();
        let state = ctx.state().clone();
        let tx = ctx.tx().await?;
        let stands = state
            .page_requests
            .link_standing(
                tx,
                workspace,
                asked.page,
                asked.connection.into(),
                asked.notion_page,
            )
            .await?;
        if !stands {
            tracing::warn!(%workspace, "a Notion link that does not stand is not made");
            return Ok(());
        }
        let connection: Option<(String, bool)> = sqlx::query_as(
            "SELECT provider, health = 'needs_reauth'
               FROM connectors.connections WHERE id = $1",
        )
        .bind(asked.connection)
        .fetch_optional(&mut **tx)
        .await?;
        let Some((provider, needs_reauth)) = connection else {
            return Ok(());
        };
        if provider != "notion" {
            tracing::warn!(%workspace, "a Notion link names a connection that is not Notion's");
            return Ok(());
        }
        let existing: Option<StreamId> = sqlx::query_scalar(
            "SELECT stream_id FROM connectors.notion_links
              WHERE connection_id = $1 AND notion_page_id = $2",
        )
        .bind(asked.connection)
        .bind(asked.notion_page)
        .fetch_optional(&mut **tx)
        .await?;
        if existing.is_some() {
            return Ok(());
        }
        let stream = StreamId::new();
        let interval = i32::try_from(self.deps.config.poll.min_seconds).unwrap_or(i32::MAX);
        sqlx::query(
            "INSERT INTO connectors.streams
                 (id, workspace_id, connection_id, capability, inbox_id, phase,
                  interval_seconds, created_at, mailbox_key, paused)
             VALUES ($1, $2, $3, 'pages', NULL, $4, $5, $6, $7, $8)",
        )
        .bind(stream)
        .bind(workspace)
        .bind(asked.connection)
        .bind(Phase::Backfill.as_str())
        .bind(interval)
        .bind(now)
        .bind(asked.notion_page.to_string())
        .bind(needs_reauth)
        .execute(&mut **tx)
        .await?;
        sqlx::query(
            "INSERT INTO connectors.notion_links
                 (stream_id, workspace_id, connection_id, notion_page_id, page_id)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(stream)
        .bind(workspace)
        .bind(asked.connection)
        .bind(asked.notion_page)
        .bind(asked.page)
        .execute(&mut **tx)
        .await?;
        // A connection waiting for the member holds its streams paused: they
        // run when the member connects again.
        if !needs_reauth {
            schedule::run_now(tx, workspace, stream, now).await?;
        }
        tracing::info!(%workspace, connection = %asked.connection, "a Notion page was linked");
        Ok(())
    }
}
