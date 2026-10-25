//! `connectors.revoke_workspace_grants` ← `workspaces.workspace_deleting`:
//! access to outside accounts stops when the workspace is deleted, not at
//! the end of its grace period. Every grant is revoked and every connection
//! deleted with what it holds; nothing the module keeps is useful without
//! the grant.
//!
//! The event alone is no proof that the workspace is going: any
//! transaction working in it could write it (campbooks-cloud#211), and
//! revoking a grant at its provider reaches past the workspace. Before
//! acting, the handler asks the workspaces module (the membership port)
//! whether the workspace is still live; while it is, nothing happens.

use campbooks_kernel::{
    Event, EventConsumer, JobContext, JobError, blobs::BlobOwner, db::BeginFor as _,
    tenancy::WorkspaceId,
};

use crate::{
    deps::Deps,
    domain::{
        model::{ConnectionId, MailChoice},
        removal,
    },
    events::WorkspaceDeleting,
    features::send_message::{SEND_OWNER, SENT_COPY_OWNER},
};

/// Revokes and deletes a deleted workspace's connections.
#[derive(Debug, Clone)]
pub struct RevokeWorkspaceGrants {
    deps: Deps,
}

impl RevokeWorkspaceGrants {
    /// The consumer, composed with the module's dependencies.
    pub fn new(deps: Deps) -> Self {
        Self { deps }
    }
}

impl EventConsumer for RevokeWorkspaceGrants {
    type Event = WorkspaceDeleting;
    const NAME: &'static str = "connectors.revoke_workspace_grants";
    const QUEUE: &'static str = "events";

    async fn handle(
        &self,
        ctx: &mut JobContext,
        event: Event<WorkspaceDeleting>,
    ) -> Result<(), JobError> {
        let workspace: WorkspaceId = event
            .payload
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("workspace_deleting without a workspace id"))?;
        let state = ctx.state().clone();
        let live = state
            .membership
            .workspace_live(&state.db, workspace)
            .await
            .map_err(|e| JobError::retry(e.to_string()))?;
        if live {
            tracing::warn!("workspace_deleting names a live workspace; not acted on");
            return Ok(());
        }
        let mut tx = ctx.pool().begin_for(workspace).await?;
        let all =
            sqlx::query_scalar!(r#"SELECT id AS "id: ConnectionId" FROM connectors.connections"#)
                .fetch_all(&mut *tx)
                .await?;
        tx.commit().await?;
        for &id in &all {
            removal::revoke(&state, &self.deps, workspace, id).await;
        }
        let tx = ctx.tx().await?;
        for id in all {
            removal::delete(tx, &state, workspace, id, MailChoice::Delete).await?;
        }
        // Sends still open will never go out.
        let sends = sqlx::query_scalar!(
            "DELETE FROM connectors.sends WHERE workspace_id = $1 RETURNING id",
            workspace as WorkspaceId
        )
        .fetch_all(&mut **tx)
        .await?;
        let key = workspace.to_string();
        for send in sends {
            state
                .blobs
                .detach_owner(tx, &key, &BlobOwner::new(SEND_OWNER, send))
                .await?;
            state
                .blobs
                .detach_owner(tx, &key, &BlobOwner::new(SENT_COPY_OWNER, send))
                .await?;
        }
        tracing::info!(%workspace, "connections of a deleted workspace revoked");
        Ok(())
    }
}
