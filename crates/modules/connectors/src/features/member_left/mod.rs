//! `connectors.revoke_departed_grants` ← `workspaces.member_left`: a grant
//! is personal, so the connections a departing member granted in that
//! workspace are disconnected and revoked. Their mail is deleted, unless the
//! owner or admin who removed them chose to keep it read-only (`mail`).

use campbooks_kernel::{
    Event, EventConsumer, JobContext, JobError,
    db::BeginFor as _,
    tenancy::{UserId, WorkspaceId},
};

use crate::{
    deps::Deps,
    domain::{model::ConnectionId, removal},
    events::MemberLeft,
};

/// Disconnects a departed member's connections.
#[derive(Debug, Clone)]
pub struct RevokeDepartedGrants {
    deps: Deps,
}

impl RevokeDepartedGrants {
    /// The consumer, composed with the module's dependencies.
    pub fn new(deps: Deps) -> Self {
        Self { deps }
    }
}

impl EventConsumer for RevokeDepartedGrants {
    type Event = MemberLeft;
    const NAME: &'static str = "connectors.revoke_departed_grants";
    const QUEUE: &'static str = "events";

    async fn handle(&self, ctx: &mut JobContext, event: Event<MemberLeft>) -> Result<(), JobError> {
        let workspace: WorkspaceId = event
            .payload
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("member_left without a workspace id"))?;
        let state = ctx.state().clone();
        let mut tx = ctx.pool().begin_for(workspace).await?;
        // Only the connections that existed when they left: a member who
        // came back since keeps what they connected after.
        let granted = sqlx::query_scalar!(
            r#"SELECT id AS "id: ConnectionId" FROM connectors.connections
                WHERE granted_by = $1 AND created_at <= $2"#,
            event.payload.user as UserId,
            event.occurred_at,
        )
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        for &id in &granted {
            removal::revoke(&state, &self.deps, workspace, id).await;
        }
        let tx = ctx.tx().await?;
        for id in granted {
            removal::delete(tx, &state, workspace, id, event.payload.mail).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
