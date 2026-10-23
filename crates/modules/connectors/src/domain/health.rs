//! A connection's health and its transitions.

use campbooks_kernel::{EventBus, Result, tenancy::UserId};
use chrono::{DateTime, Utc};
use sqlx::{PgConnection, Postgres, Transaction};

use super::{
    model::{ConnectionId, Health, StreamId},
    schedule,
};
use crate::events::ConnectionNeedsReauth;

/// Set `connection`'s health to `health` (with `reason`) if it differs.
/// Returns whether it changed.
pub async fn set(
    conn: &mut PgConnection,
    connection: ConnectionId,
    health: Health,
    reason: Option<&str>,
    now: DateTime<Utc>,
) -> Result<bool> {
    let changed = sqlx::query!(
        r#"UPDATE connectors.connections
              SET health = $2, health_reason = $3, health_changed_at = $4, updated_at = $4
            WHERE id = $1 AND (health <> $2 OR health_reason IS DISTINCT FROM $3)"#,
        connection as ConnectionId,
        health.as_str(),
        reason,
        now,
    )
    .execute(conn)
    .await?
    .rows_affected();
    Ok(changed > 0)
}

/// Lock `connection` for a sync's writes, before any of its streams: the
/// order removal takes too (connection, then streams), so a sync and a
/// disconnect never wait on each other in a cycle.
pub async fn lock(conn: &mut PgConnection, connection: ConnectionId) -> Result<()> {
    sqlx::query_scalar!(
        "SELECT id FROM connectors.connections WHERE id = $1 FOR NO KEY UPDATE",
        connection as ConnectionId
    )
    .fetch_optional(conn)
    .await?;
    Ok(())
}

/// Sign-ins an IMAP server refuses in a row, by sync runs and IDLE sessions
/// alike, before the connection needs the member.
pub const LOGIN_REFUSALS: u32 = 3;

/// Count one more refused IMAP sign-in on `stream`'s cursor
/// (`login_refusals`, which a run that signs in resets), after locking
/// `connection` (removal's order: the connection, then its streams). The
/// count now; zero when the stream is gone.
pub async fn count_login_refusal(
    conn: &mut PgConnection,
    connection: ConnectionId,
    stream: StreamId,
) -> Result<u32> {
    lock(&mut *conn, connection).await?;
    let refusals: Option<i32> = sqlx::query_scalar(
        "UPDATE connectors.streams
            SET cursor = jsonb_set(
                    cursor, '{login_refusals}',
                    to_jsonb(coalesce((cursor->>'login_refusals')::int, 0) + 1))
          WHERE id = $1
        RETURNING (cursor->>'login_refusals')::int",
    )
    .bind(stream)
    .fetch_optional(&mut *conn)
    .await?;
    Ok(refusals.map_or(0, |n| u32::try_from(n).unwrap_or(0)))
}

/// The grant stopped working: `needs_reauth`, every stream paused and off
/// the schedule, its IDLE lease withdrawn, and [`ConnectionNeedsReauth`]
/// published once.
pub async fn needs_reauth(
    tx: &mut Transaction<'_, Postgres>,
    events: &EventBus,
    workspace: &str,
    connection: ConnectionId,
    granted_by: UserId,
    reason: &'static str,
    now: DateTime<Utc>,
) -> Result<()> {
    let already = sqlx::query_scalar!(
        "SELECT health = 'needs_reauth' AS \"already!\" FROM connectors.connections WHERE id = $1 FOR NO KEY UPDATE",
        connection as ConnectionId
    )
    .fetch_optional(&mut **tx)
    .await?;
    let Some(already) = already else {
        return Ok(());
    };
    set(tx, connection, Health::NeedsReauth, Some(reason), now).await?;
    let streams = sqlx::query_scalar!(
        r#"UPDATE connectors.streams SET paused = true WHERE connection_id = $1
        RETURNING id AS "id: StreamId""#,
        connection as ConnectionId
    )
    .fetch_all(&mut **tx)
    .await?;
    for stream in streams {
        schedule::unschedule(tx, stream).await?;
    }
    // An IDLE session on it ends at its holder's next renewal.
    crate::features::imap_idle::lease::withdraw(tx, connection).await?;
    if !already {
        events
            .publish(
                tx,
                &ConnectionNeedsReauth {
                    workspace: workspace.to_owned(),
                    connection,
                    granted_by,
                    reason: reason.to_owned(),
                },
            )
            .await?;
        tracing::info!(workspace, %connection, reason, "connection needs reauthorisation");
    }
    Ok(())
}
