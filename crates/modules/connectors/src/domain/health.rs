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

/// The grant stopped working: `needs_reauth`, every stream paused and off
/// the schedule, and [`ConnectionNeedsReauth`] published once.
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
        "SELECT health = 'needs_reauth' AS \"already!\" FROM connectors.connections WHERE id = $1",
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
