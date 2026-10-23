//! The one door to a task: ask the central check (ADR 0017), then load the
//! row, locked when the caller is about to change it.

use campbooks_kernel::{
    AppState, Error, RecordRef, Result,
    authz::{self, Action},
    tenancy::Access,
};
use sqlx::{Postgres, Transaction};

use super::{SELECT_LOCKED, SELECT_ONE, TaskId, TaskRow, resources::TASK_RECORD};

/// Task `id` as `access` may take `action` on it, or 404 when they may not
/// see it. `lock` holds the row for the rest of the transaction.
pub async fn require_task(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    access: &Access,
    action: Action,
    id: TaskId,
    lock: bool,
) -> Result<TaskRow> {
    authz::require(
        state,
        tx,
        access,
        action,
        &RecordRef::new(TASK_RECORD, &id),
        || Error::Denied(crate::MESSAGES.reason("task_not_yours")),
    )
    .await?;
    sqlx::query_as::<_, TaskRow>(if lock { SELECT_LOCKED } else { SELECT_ONE })
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(Error::NotFound)
}

/// 409 `task_changed` unless `seen` is the task's revision.
pub fn expect_revision(row: &TaskRow, seen: i64) -> Result<()> {
    if row.revision == seen {
        Ok(())
    } else {
        Err(Error::Conflict(crate::MESSAGES.reason("task_changed")))
    }
}

/// 409 `task_closed` unless the task is open.
pub fn expect_open(row: &TaskRow) -> Result<()> {
    if row.is_open() {
        Ok(())
    } else {
        Err(Error::Conflict(crate::MESSAGES.reason("task_closed")))
    }
}
