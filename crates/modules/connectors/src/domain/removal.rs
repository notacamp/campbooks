//! Disconnecting: revoke the grant at the provider, then delete the
//! connection with its credentials, streams, index and batch holds.

use campbooks_kernel::{
    AppState, Result, blobs::BlobOwner, db::BeginFor as _, tenancy::WorkspaceId,
};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use super::{
    credentials,
    model::{ConnectionId, InboxId},
};
use crate::{deps::Deps, events::ConnectionRemoved};

/// The blob owner type of a connection (its account document).
pub const CONNECTION_OWNER: &str = "connectors.connection";

/// The blob owner type of a Gmail index row.
pub const GMAIL_MESSAGE_OWNER: &str = "connectors.gmail_message";

/// The blob owner type of a batch row.
pub const BATCH_OWNER: &str = "connectors.batch";

/// Revoke connection `id`'s grant at its provider. Best effort: a failure
/// is logged (never with the token) and the disconnect goes on, because the
/// member asked for the link to be cut and the credentials are deleted
/// either way.
pub async fn revoke(state: &AppState, deps: &Deps, workspace: WorkspaceId, id: ConnectionId) {
    let (Some(cipher), Some(google)) = (&deps.cipher, &deps.google) else {
        return;
    };
    let token = async {
        let mut tx = state.db.begin_for(workspace).await?;
        let token = credentials::refresh_token(&mut tx, cipher, id).await;
        tx.commit().await?;
        Ok::<_, sqlx::Error>(token)
    }
    .await;
    match token {
        Ok(Ok(Some(token))) => {
            if let Err(err) = google.revoke(&token).await {
                tracing::warn!(%workspace, connection = %id, error = %err, "could not revoke the grant at Google");
            }
        }
        Ok(Ok(None)) => {}
        Ok(Err(_)) => {
            tracing::warn!(%workspace, connection = %id, "stored credentials unreadable: nothing to revoke");
        }
        Err(err) => {
            tracing::warn!(%workspace, connection = %id, error = %err, "could not read credentials to revoke");
        }
    }
}

/// Delete connection `id` and everything it holds, in `tx`, and publish
/// [`ConnectionRemoved`]. Returns `false` if it was gone already.
pub async fn delete(
    tx: &mut Transaction<'_, Postgres>,
    state: &AppState,
    workspace: WorkspaceId,
    id: ConnectionId,
) -> Result<bool> {
    let workspace_key = workspace.to_string();
    let inbox_ids = sqlx::query_scalar!(
        r#"SELECT inbox_id AS "inbox_id!: InboxId" FROM connectors.streams
            WHERE connection_id = $1 AND inbox_id IS NOT NULL ORDER BY inbox_id"#,
        id as ConnectionId
    )
    .fetch_all(&mut **tx)
    .await?;
    let owners = sqlx::query!(
        r#"SELECT m.id AS "id!", 'm' AS "kind!" FROM connectors.gmail_messages m
             JOIN connectors.streams s ON s.id = m.stream_id WHERE s.connection_id = $1
           UNION ALL
           SELECT b.id AS "id!", 'b' AS "kind!" FROM connectors.batches b
             JOIN connectors.streams s ON s.id = b.stream_id WHERE s.connection_id = $1"#,
        id as ConnectionId
    )
    .fetch_all(&mut **tx)
    .await?;
    for owner in owners {
        let kind = if owner.kind == "m" {
            GMAIL_MESSAGE_OWNER
        } else {
            BATCH_OWNER
        };
        state
            .blobs
            .detach_owner(tx, &workspace_key, &BlobOwner::new(kind, owner.id))
            .await?;
    }
    state
        .blobs
        .detach_owner(
            tx,
            &workspace_key,
            &BlobOwner::new(CONNECTION_OWNER, Uuid::from(id)),
        )
        .await?;
    let deleted = sqlx::query!(
        "DELETE FROM connectors.connections WHERE id = $1",
        id as ConnectionId
    )
    .execute(&mut **tx)
    .await?
    .rows_affected();
    if deleted == 0 {
        return Ok(false);
    }
    state
        .events
        .publish(
            tx,
            &ConnectionRemoved {
                workspace: workspace_key,
                connection: id,
                inbox_ids,
            },
        )
        .await?;
    tracing::info!(%workspace, connection = %id, "connection removed");
    Ok(true)
}
