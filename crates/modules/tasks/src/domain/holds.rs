//! Time held for a task on its assignee's calendar (campbooks-cloud#362):
//! the block itself is the schedule module's, made and taken off through
//! the kernel's `Holds` port in the task's own transaction; this module keeps
//! the event's id, its calendar and its times on the task.

use campbooks_kernel::{
    AppState, Error, Result,
    schedule::DEFAULT_TIME_ZONE,
    tenancy::{UserId, WorkspaceId},
};
use chrono::{DateTime, NaiveDate, Utc};
use chrono_tz::Tz;
use sqlx::{PgConnection, Postgres, Transaction};

use super::TaskRow;

/// The day `at` falls on where `member` lives (their time zone, from the
/// schedule module; UTC while they have none).
pub async fn local_day(
    state: &AppState,
    conn: &mut PgConnection,
    workspace: WorkspaceId,
    member: UserId,
    at: DateTime<Utc>,
) -> Result<NaiveDate> {
    let zone = state
        .time_zones
        .time_zone(conn, workspace, member)
        .await?
        .unwrap_or_else(|| DEFAULT_TIME_ZONE.to_owned());
    let zone: Tz = zone.parse().unwrap_or(Tz::UTC);
    Ok(at.with_timezone(&zone).date_naive())
}

/// Release the hold of task `row` when it has not started by `now`, as a
/// task is closed or handed over: the block goes from the assignee's
/// calendar and the task keeps no hold and no plan. A hold that has started
/// stays as a record, and so does one whose block has a change of the
/// member's still on its way to the provider. Whether it was released.
pub async fn free_ahead(
    state: &AppState,
    tx: &mut Transaction<'static, Postgres>,
    workspace: WorkspaceId,
    row: &TaskRow,
    now: DateTime<Utc>,
) -> Result<bool> {
    let (Some(event), Some(from)) = (row.hold_event, row.held_from) else {
        return Ok(false);
    };
    if from <= now {
        return Ok(false);
    }
    match state
        .holds
        .release(state, tx, workspace, row.assignee, event)
        .await
    {
        Ok(()) => {}
        Err(Error::Conflict(_)) => return Ok(false),
        Err(error) => return Err(error),
    }
    clear(tx, row).await?;
    Ok(true)
}

/// Clear the hold of task `row` and its plan, without bumping the revision
/// (the change that releases it does).
pub async fn clear(conn: &mut PgConnection, row: &TaskRow) -> Result<()> {
    sqlx::query(
        "UPDATE tasks.tasks
            SET hold_event = NULL, hold_calendar = NULL, held_from = NULL, held_to = NULL,
                plan = 'none', plan_on = NULL, later_until = NULL
          WHERE id = $1",
    )
    .bind(row.id)
    .execute(conn)
    .await?;
    Ok(())
}
