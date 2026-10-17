//! `identity.remove_workspace_clients` ← `workspaces.workspace_deleting`: the
//! workspace's own apps are removed, and every app allowed in it loses its
//! access (consents and refresh tokens).
//!
//! The event alone is no proof that the workspace is going: any
//! transaction working in it could write it (campbooks-cloud#211), and
//! revoking an app's refresh tokens reaches past the workspace. Before
//! acting, the handler asks the workspaces module (the membership port)
//! whether the workspace is still live; while it is, nothing happens.

use campbooks_kernel::{Event, EventConsumer, JobContext, JobError, tenancy::WorkspaceId};
use chrono::Utc;
use uuid::Uuid;

use super::member_left::publish;
use crate::{
    domain::{clients::as_server, consents, workspace_clients},
    events::WorkspaceDeleting,
};

/// Removes a deleted workspace's clients and consents.
#[derive(Debug, Clone, Copy, Default)]
pub struct RemoveWorkspaceClients;

impl EventConsumer for RemoveWorkspaceClients {
    type Event = WorkspaceDeleting;
    const NAME: &'static str = "identity.remove_workspace_clients";
    const QUEUE: &'static str = "events";

    async fn handle(
        &self,
        ctx: &mut JobContext,
        event: Event<WorkspaceDeleting>,
    ) -> Result<(), JobError> {
        let workspace: Uuid = event
            .payload
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("workspace_deleting without a workspace id"))?;
        let now = Utc::now();
        let retry = |e: campbooks_kernel::Error| JobError::retry(e.to_string());
        let state = ctx.state().clone();
        let live = state
            .membership
            .workspace_live(&state.db, WorkspaceId::from_uuid(workspace))
            .await
            .map_err(retry)?;
        if live {
            tracing::warn!("workspace_deleting names a live workspace; not acted on");
            return Ok(());
        }
        let tx = ctx.tx().await?;
        as_server(tx).await.map_err(retry)?;
        let removed: Vec<String> = sqlx::query_scalar(
            "DELETE FROM identity.clients WHERE kind = 'workspace' AND workspace_id = $1
             RETURNING client_id",
        )
        .bind(workspace)
        .fetch_all(&mut **tx)
        .await?;
        let mut ended = Vec::new();
        for client in &removed {
            for (account, bound) in
                workspace_clients::revoke_everything(tx, client, "workspace_deleting", now)
                    .await
                    .map_err(retry)?
            {
                ended.push(consents::Revoked {
                    account_id: account,
                    client_id: client.clone(),
                    bound_workspace_id: bound,
                });
            }
        }
        ended.extend(
            consents::revoke_in_workspace(tx, workspace, None, "workspace_deleting", now)
                .await
                .map_err(retry)?,
        );
        for consent in ended {
            publish(ctx, consent, "workspace_deleting").await?;
        }
        Ok(())
    }
}
