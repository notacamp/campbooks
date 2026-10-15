//! The mailbox connections a way in brought (`identity.mailbox_links`): a
//! way in that came only from connecting a mailbox goes with its last
//! connection (campbooks-cloud#117).
//!
//! A connection is recorded when the connect that created the way in hands
//! its grant over, and for every later connection of that way in. A way in
//! linked before this record existed has none, so it is never removed here:
//! some of its connections would be unknown.

use campbooks_kernel::Result;
use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use uuid::Uuid;

/// Record that `connection` came with way in `identity_id`: always when the
/// round trip created the way in (`created`), otherwise only when the way in
/// is already tracked.
pub async fn record(
    conn: &mut PgConnection,
    identity_id: Uuid,
    connection: &str,
    created: bool,
    now: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO identity.mailbox_links (connection_id, provider_identity_id, created_at)
         SELECT $1, $2, $3
          WHERE $4 OR EXISTS (SELECT 1 FROM identity.mailbox_links
                               WHERE provider_identity_id = $2)
         ON CONFLICT (connection_id) DO NOTHING",
    )
    .bind(connection)
    .bind(identity_id)
    .bind(now)
    .bind(created)
    .execute(conn)
    .await?;
    Ok(())
}

/// Forget `connection`; the way in it came with, when it has no other
/// recorded connection left.
pub async fn forget(conn: &mut PgConnection, connection: &str) -> Result<Option<Uuid>> {
    let identity: Option<Uuid> = sqlx::query_scalar(
        "DELETE FROM identity.mailbox_links WHERE connection_id = $1
         RETURNING provider_identity_id",
    )
    .bind(connection)
    .fetch_optional(&mut *conn)
    .await?;
    let Some(identity) = identity else {
        return Ok(None);
    };
    let others: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM identity.mailbox_links WHERE provider_identity_id = $1)",
    )
    .bind(identity)
    .fetch_one(conn)
    .await?;
    Ok((!others).then_some(identity))
}
