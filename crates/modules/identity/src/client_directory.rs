//! The kernel's `ClientDirectory` port (ADR 0017): a workspace client acts
//! for the member who registered it, within that member's access,
//! re-resolved on every request. Registered by the server when this
//! module's issuer runs in the process, so every client token the API sees
//! was issued here: an unknown client (removed, or never a workspace's) or
//! one with no registering member is refused.

use std::sync::Arc;

use campbooks_kernel::{
    db::BeginFor as _,
    tenancy::{ClientActor, ClientDirectory, ClientDirectoryPort, ClientsFuture, WorkspaceId},
};
use sqlx::PgPool;
use uuid::Uuid;

/// The port, for the server to register.
pub fn client_directory() -> ClientDirectoryPort {
    Arc::new(WorkspaceClients)
}

#[derive(Debug, Clone, Copy)]
struct WorkspaceClients;

impl ClientDirectory for WorkspaceClients {
    fn acting_for<'a>(
        &'a self,
        db: &'a PgPool,
        client_id: &'a str,
        workspace: WorkspaceId,
    ) -> ClientsFuture<'a> {
        Box::pin(async move {
            // In the client's workspace: row-level security admits only
            // that workspace's clients.
            let mut tx = db.begin_for(workspace).await?;
            let creator: Option<Option<Uuid>> = sqlx::query_scalar(
                "SELECT created_by FROM identity.clients
                  WHERE client_id = $1 AND kind = 'workspace' AND workspace_id = $2",
            )
            .bind(client_id)
            .bind(workspace.as_uuid())
            .fetch_optional(&mut *tx)
            .await?;
            tx.commit().await?;
            Ok(match creator {
                Some(Some(account)) => ClientActor::ActsFor(account.to_string()),
                _ => ClientActor::Revoked,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use campbooks_kernel::tenancy::{ClientActor, WorkspaceId};
    use sqlx::PgPool;
    use uuid::Uuid;

    #[sqlx::test(migrations = "../../../migrations")]
    async fn a_client_acts_for_whoever_registered_it_until_it_is_removed(db: PgPool) {
        let pool = campbooks_kernel::testing::app_role_pool(&db).await;
        let workspace = WorkspaceId::new();
        let account = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO identity.clients
                 (client_id, kind, workspace_id, name, redirect_uris, grant_types, auth_method,
                  secret_hash, allowed_scopes, created_by, created_at)
             VALUES ('cb_test', 'workspace', $1, 'Sync', '{}', '{client_credentials}',
                     'client_secret_basic', '\\x00'::bytea, '{}', $2, now())",
        )
        .bind(workspace.as_uuid())
        .bind(account)
        .execute(&db)
        .await
        .unwrap();
        let directory = super::client_directory();
        let ask = |id: &'static str, ws: WorkspaceId| {
            let directory = directory.clone();
            let pool = pool.clone();
            async move { directory.acting_for(&pool, id, ws).await.unwrap() }
        };
        assert_eq!(
            ask("cb_test", workspace).await,
            ClientActor::ActsFor(account.to_string())
        );
        assert_eq!(
            ask("cb_test", WorkspaceId::new()).await,
            ClientActor::Revoked
        );
        assert_eq!(ask("cb_other", workspace).await, ClientActor::Revoked);
        sqlx::query("DELETE FROM identity.clients WHERE client_id = 'cb_test'")
            .execute(&db)
            .await
            .unwrap();
        assert_eq!(ask("cb_test", workspace).await, ClientActor::Revoked);
    }
}
