//! Notion's consents and links in the database: a consent started and not
//! finished (single use, ten minutes, bound to the browser that goes to
//! Notion and back), and which Notion page each `pages` stream reads.

use campbooks_kernel::{
    AppState, Result,
    blobs::BlobOwner,
    tenancy::{UserId, WorkspaceId},
};
use chrono::{DateTime, TimeDelta, Utc};
use sqlx::{PgConnection, Postgres, Transaction};
use uuid::Uuid;

use super::{
    model::{AuthorizationId, ConnectionId, StreamId},
    removal::BATCH_OWNER,
};

/// How long a consent started may take to be finished.
pub const AUTHORIZATION_LIFETIME: TimeDelta = TimeDelta::minutes(10);

/// A consent started.
#[derive(Debug, Clone, Copy)]
pub struct Started {
    /// The consent.
    pub id: AuthorizationId,
    /// When it must be finished by.
    pub expires_at: DateTime<Utc>,
}

/// Record a consent `user` started in `workspace`, to connect Notion anew or
/// to connect `connection` again, and forget the ones that have expired.
pub async fn start(
    conn: &mut PgConnection,
    workspace: WorkspaceId,
    user: UserId,
    connection: Option<ConnectionId>,
    return_to: Option<&str>,
    now: DateTime<Utc>,
) -> Result<Started> {
    sqlx::query("DELETE FROM connectors.notion_authorizations WHERE expires_at < $1")
        .bind(now)
        .execute(&mut *conn)
        .await?;
    let id = AuthorizationId::new();
    let expires_at = now + AUTHORIZATION_LIFETIME;
    sqlx::query(
        "INSERT INTO connectors.notion_authorizations
             (id, workspace_id, user_id, connection_id, return_to, expires_at, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(id)
    .bind(workspace)
    .bind(user)
    .bind(connection)
    .bind(return_to)
    .bind(expires_at)
    .bind(now)
    .execute(conn)
    .await?;
    Ok(Started { id, expires_at })
}

/// The browser is going to Notion with `state_hash`, bound to `browser_hash`,
/// and `verifier` (sealed). Only once, and only before the consent expires:
/// `false` otherwise.
pub async fn arm(
    conn: &mut PgConnection,
    id: AuthorizationId,
    (state_hash, browser_hash): (&[u8], &[u8]),
    verifier: &[u8],
    now: DateTime<Utc>,
) -> Result<bool> {
    let armed = sqlx::query(
        "UPDATE connectors.notion_authorizations
            SET state_hash = $2, browser_hash = $3, pkce_verifier = $4
          WHERE id = $1 AND state_hash IS NULL AND expires_at > $5",
    )
    .bind(id)
    .bind(state_hash)
    .bind(browser_hash)
    .bind(verifier)
    .bind(now)
    .execute(conn)
    .await?
    .rows_affected();
    Ok(armed == 1)
}

/// A consent coming back.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Finished {
    /// The member who started it.
    pub user_id: UserId,
    /// The connection of theirs it connects again.
    pub connection_id: Option<ConnectionId>,
    /// Where to send the browser.
    pub return_to: Option<String>,
    /// The PKCE verifier, sealed.
    pub pkce_verifier: Vec<u8>,
}

/// Take the consent `id` back: it is deleted (single use) when `state_hash`
/// and `browser_hash` are the ones it was armed with and it has not
/// expired. `None` otherwise, and the consent is left as it was.
pub async fn finish(
    conn: &mut PgConnection,
    id: AuthorizationId,
    (state_hash, browser_hash): (&[u8], &[u8]),
    now: DateTime<Utc>,
) -> Result<Option<Finished>> {
    Ok(sqlx::query_as(
        "DELETE FROM connectors.notion_authorizations
          WHERE id = $1 AND state_hash = $2 AND browser_hash = $3 AND expires_at > $4
      RETURNING user_id, connection_id, return_to, pkce_verifier",
    )
    .bind(id)
    .bind(state_hash)
    .bind(browser_hash)
    .bind(now)
    .fetch_optional(conn)
    .await?)
}

/// The Notion page a `pages` stream reads, and the page it feeds.
#[derive(Debug, Clone, Copy, sqlx::FromRow)]
pub struct Link {
    /// The Notion page.
    pub notion_page_id: Uuid,
    /// The pages module's page.
    pub page_id: Uuid,
    /// The link is of a Notion database (a data source) and the table it
    /// feeds, not of a page: `notion_page_id` is then the data source.
    pub data_source: bool,
}

/// The link of `stream`, when it has one.
pub async fn link_of(conn: &mut PgConnection, stream: StreamId) -> Result<Option<Link>> {
    Ok(sqlx::query_as(
        "SELECT notion_page_id, page_id, kind = 'data_source' AS data_source
           FROM connectors.notion_links WHERE stream_id = $1",
    )
    .bind(stream)
    .fetch_optional(conn)
    .await?)
}

/// Delete `stream` (a `pages` stream) with its link, its recorded changes
/// and its schedule, and let go of the snapshots it kept. Nothing is
/// deleted at Notion.
pub async fn drop_stream(
    tx: &mut Transaction<'_, Postgres>,
    state: &AppState,
    workspace: WorkspaceId,
    stream: StreamId,
) -> Result<()> {
    let key = workspace.to_string();
    let batches: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM connectors.batches WHERE stream_id = $1")
            .bind(stream)
            .fetch_all(&mut **tx)
            .await?;
    for id in batches {
        state
            .blobs
            .detach_owner(tx, &key, &BlobOwner::new(BATCH_OWNER, id))
            .await?;
    }
    sqlx::query("DELETE FROM connectors.streams WHERE id = $1")
        .bind(stream)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
