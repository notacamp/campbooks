//! Locks that keep a connection's calendars from being made while they are
//! being deleted.
//!
//! A delete that is already running cannot see a row inserted after its
//! snapshot, so the row would survive it. Everything that makes or deletes the
//! calendars of a connection takes this lock first, for the rest of its
//! transaction, so the two never overlap.

use sqlx::{Postgres, Transaction};

use crate::model::ConnectionId;

/// Wait for, and hold until the transaction ends, the lock of `connection`.
/// The key is a namespaced hash of the connection's id (unique across
/// workspaces), so it cannot meet another module's advisory locks. Taken
/// first by a list read again and by `connection_removed` (one connection
/// each), and by a member leaving for each of their connections, in
/// ascending id order, so two of these never wait on each other in a circle.
pub async fn connection(
    tx: &mut Transaction<'static, Postgres>,
    connection: ConnectionId,
) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('schedule.calendar_list:' || $1::uuid::text, 0))")
        .bind(connection)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
