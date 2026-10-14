//! Disconnecting: revoke the grant at the provider, then delete the
//! connection with its credentials, streams, index and batch holds.

use campbooks_kernel::{
    AppState, Result,
    blobs::BlobOwner,
    db::BeginFor as _,
    tenancy::{UserId, WorkspaceId},
};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use super::{
    credentials,
    model::{ConnectionId, InboxId, MailChoice, Provider},
    schedule,
};
use crate::{deps::Deps, events::ConnectionRemoved};

/// The blob owner type of a connection (its account document).
pub const CONNECTION_OWNER: &str = "connectors.connection";

/// The blob owner type of an index row (`connectors.messages`). The name
/// predates the second adapter; stored holds use it, so it never changes.
pub const MESSAGE_OWNER: &str = "connectors.gmail_message";

/// The blob owner type of a batch row.
pub const BATCH_OWNER: &str = "connectors.batch";

/// Revoke connection `id`'s grant at its provider. Best effort: a failure
/// is logged (never with the token) and the disconnect goes on, because the
/// member asked for the link to be cut and the credentials are deleted
/// either way.
pub async fn revoke(state: &AppState, deps: &Deps, workspace: WorkspaceId, id: ConnectionId) {
    let Some(cipher) = &deps.cipher else {
        return;
    };
    let found = async {
        let mut tx = state.db.begin_for(workspace).await?;
        let token = credentials::refresh_token(&mut tx, cipher, id).await;
        let target = sqlx::query!(
            "SELECT provider, provider_server FROM connectors.connections WHERE id = $1",
            id as ConnectionId
        )
        .fetch_optional(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok::<_, sqlx::Error>((token, target))
    }
    .await;
    let (token, target) = match found {
        Ok((Ok(Some(token)), Some(target))) => (token, target),
        Ok((Ok(_), _)) => return,
        Ok((Err(_), _)) => {
            tracing::warn!(%workspace, connection = %id, "stored credentials unreadable: nothing to revoke");
            return;
        }
        Err(err) => {
            tracing::warn!(%workspace, connection = %id, error = %err, "could not read credentials to revoke");
            return;
        }
    };
    let revoked = match Provider::parse(&target.provider) {
        Ok(Provider::Google) => match &deps.google {
            Some(google) => google.revoke(&token).await,
            None => return,
        },
        Ok(Provider::Zoho) => {
            let centre = deps.zoho.as_ref().and_then(|zoho| {
                let server = target.provider_server.as_deref()?;
                Some((zoho, zoho.centre(server)?))
            });
            match centre {
                Some((zoho, centre)) => zoho.revoke(&centre, &token).await,
                None => return,
            }
        }
        Err(_) => return,
    };
    if let Err(err) = revoked {
        tracing::warn!(%workspace, connection = %id, provider = target.provider, error = %err, "could not revoke the grant at the provider");
    }
}

/// Delete connection `id` and everything it holds, in `tx`, and publish
/// [`ConnectionRemoved`] with `mail`: the conversations module then deletes
/// the inboxes it fed, or keeps them read-only. Returns `false` if it was
/// gone already.
pub async fn delete(
    tx: &mut Transaction<'_, Postgres>,
    state: &AppState,
    workspace: WorkspaceId,
    id: ConnectionId,
    mail: MailChoice,
) -> Result<bool> {
    let workspace_key = workspace.to_string();
    // Lock the connection and its streams first: a sync writing to a stream
    // holds it (its rows reference the stream), so this waits for it and then
    // sees every index row and batch it wrote; a later sync finds the stream
    // gone. Reading first and deleting later could miss their files.
    let removed = sqlx::query!(
        r#"SELECT granted_by AS "granted_by: UserId", provider, account_subject
             FROM connectors.connections WHERE id = $1 FOR UPDATE"#,
        id as ConnectionId
    )
    .fetch_optional(&mut **tx)
    .await?;
    sqlx::query_scalar!(
        "SELECT id FROM connectors.streams WHERE connection_id = $1 FOR UPDATE",
        id as ConnectionId
    )
    .fetch_all(&mut **tx)
    .await?;
    let inbox_ids = sqlx::query_scalar!(
        r#"SELECT inbox_id AS "inbox_id!: InboxId" FROM connectors.streams
            WHERE connection_id = $1 AND inbox_id IS NOT NULL ORDER BY inbox_id"#,
        id as ConnectionId
    )
    .fetch_all(&mut **tx)
    .await?;
    let owners = sqlx::query!(
        r#"SELECT m.id AS "id!", 'm' AS "kind!" FROM connectors.messages m
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
            MESSAGE_OWNER
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
    schedule::forget_budget(tx, &schedule::connection_budget(id)).await?;
    state
        .events
        .publish(
            tx,
            &ConnectionRemoved {
                workspace: workspace_key,
                connection: id,
                inbox_ids,
                granted_by: removed.as_ref().map(|r| r.granted_by),
                provider: removed
                    .as_ref()
                    .and_then(|r| Provider::parse(&r.provider).ok()),
                account_subject: removed.map(|r| r.account_subject),
                mail,
            },
        )
        .await?;
    tracing::info!(%workspace, connection = %id, "connection removed");
    Ok(true)
}
