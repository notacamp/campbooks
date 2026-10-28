//! The tracked events of a task (ADR 0020): which ones a change makes,
//! comparing the row before with the row after, and the snapshot they carry,
//! read from the module's own row in the transaction of the change.
//!
//! Ids, enums, booleans, dates and times only: never the title, the sender's
//! address, the amount or its currency, the ask's key or the inbox.

use campbooks_kernel::{AppState, RecordRef, Result, tenancy::WorkspaceId};
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{PgConnection, Postgres, Transaction, types::Uuid};

use super::{Outcome, TaskChange, TaskHistoryWhat, TaskId, TaskRow};
use crate::tracking;

fn verb_of(verb: &str) -> tracking::TaskVerb {
    if verb == "pay" {
        tracking::TaskVerb::Pay
    } else {
        tracking::TaskVerb::Do
    }
}

fn origin_of(origin: &str) -> tracking::TaskOrigin {
    match origin {
        "ask" => tracking::TaskOrigin::Ask,
        "deadline" => tracking::TaskOrigin::Deadline,
        _ => tracking::TaskOrigin::Own,
    }
}

fn state_of(state: &str) -> tracking::TaskState {
    match state {
        "done" => tracking::TaskState::Done,
        "not_needed" => tracking::TaskState::NotNeeded,
        _ => tracking::TaskState::Open,
    }
}

fn plan_of(plan: &str) -> tracking::PlanKind {
    match plan {
        "date" => tracking::PlanKind::Date,
        "later" => tracking::PlanKind::Later,
        _ => tracking::PlanKind::None,
    }
}

/// A task, as the snapshot reads it.
#[derive(sqlx::FromRow)]
struct TaskFacts {
    id: Uuid,
    verb: String,
    origin: String,
    state: String,
    due_on: Option<NaiveDate>,
    due_at: Option<DateTime<Utc>>,
    plan: String,
    plan_on: Option<NaiveDate>,
    later_until: Option<DateTime<Utc>>,
    assignee: Uuid,
    created_by: Uuid,
    handed_by: Option<Uuid>,
    source_type: Option<String>,
    source_id: Option<String>,
    has_amount: bool,
    revision: i64,
    closed_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

/// The snapshot of task `id` as it is now in `conn`'s transaction, if it
/// exists.
async fn snapshot(conn: &mut PgConnection, id: TaskId) -> Result<Option<tracking::TaskSnapshot>> {
    let facts = sqlx::query_as::<_, TaskFacts>(
        "SELECT id, verb, origin, state, due_on, due_at, plan, plan_on, later_until, assignee,
                created_by, handed_by, source_type, source_id,
                (amount_minor IS NOT NULL) AS has_amount, revision, closed_at, created_at,
                updated_at
           FROM tasks.tasks
          WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(conn)
    .await?;
    Ok(facts.map(|row| tracking::TaskSnapshot {
        id: row.id,
        verb: verb_of(&row.verb),
        origin: origin_of(&row.origin),
        state: state_of(&row.state),
        due_on: row.due_on,
        due_at: row.due_at,
        plan: plan_of(&row.plan),
        plan_on: row.plan_on,
        later_until: row.later_until,
        assignee: row.assignee,
        created_by: row.created_by,
        handed_by: row.handed_by,
        source: match (row.source_type, row.source_id) {
            (Some(record_type), Some(id)) => Some(RecordRef::new(record_type, &id)),
            _ => None,
        },
        has_amount: row.has_amount,
        revision: row.revision,
        closed_at: row.closed_at,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }))
}

/// The fields other than the due date that differ between `before` and
/// `after`.
fn edited(before: &TaskRow, after: &TaskRow) -> Vec<tracking::TaskField> {
    let mut fields = Vec::new();
    if before.title != after.title {
        fields.push(tracking::TaskField::Title);
    }
    if before.verb != after.verb {
        fields.push(tracking::TaskField::Verb);
    }
    fields
}

/// Whether the due date differs between `before` and `after`.
fn due_moved(before: &TaskRow, after: &TaskRow) -> bool {
    (before.due_on, before.due_at) != (after.due_on, after.due_at)
}

/// Whether the plan differs between `before` and `after`.
fn replanned(before: &TaskRow, after: &TaskRow) -> bool {
    (&before.plan, before.plan_on, before.later_until)
        != (&after.plan, after.plan_on, after.later_until)
}

/// `tasks.task_reassigned` of a hand-off or a take-back of the task that was
/// `before`.
fn reassigned(
    workspace: WorkspaceId,
    task: tracking::TaskSnapshot,
    change: TaskChange,
    before: &TaskRow,
) -> tracking::TaskReassigned {
    tracking::TaskReassigned {
        workspace,
        task,
        reassignment: if change == TaskChange::HandedOff {
            tracking::Reassignment::HandedOff
        } else {
            tracking::Reassignment::TakenBack
        },
        previous_assignee: *before.assignee.as_uuid(),
        previous_handed_by: before.handed_by.map(|by| *by.as_uuid()),
    }
}

async fn track_done(
    tracker: &campbooks_kernel::tracking::Tracker,
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    task: tracking::TaskSnapshot,
) -> Result<()> {
    tracker
        .track(tx, &tracking::TaskDone { workspace, task })
        .await?;
    Ok(())
}

/// Track a change about the question "You replied. Is this done?": the reply
/// noticed, or Not yet. The undo of Not yet tracks nothing, so the answer on
/// record stays the one given. Whether `change` is one of the three.
async fn reply(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    change: TaskChange,
    task: &tracking::TaskSnapshot,
) -> Result<bool> {
    let task = task.clone();
    match change {
        TaskChange::Replied => {
            let noticed = tracking::TaskReplyNoticed { workspace, task };
            state.tracker.track(tx, &noticed).await?;
        }
        TaskChange::ReplyAnswered => {
            let answered = tracking::TaskReplyAnswered { workspace, task };
            state.tracker.track(tx, &answered).await?;
        }
        TaskChange::ReplyUnanswered => {}
        _ => return Ok(false),
    }
    Ok(true)
}

/// Track, in `tx`, the change `outcome` of the task that was `before` (none
/// for a new one) and is `after` now. What did not change tracks nothing: a
/// plan set again to the one the task had, an update whose values it had.
/// Time held or released is tracked by the feature that knows where and by
/// whom ([`held`], [`hold_released`]).
pub async fn track(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    outcome: &Outcome,
    before: Option<&TaskRow>,
    after: &TaskRow,
) -> Result<()> {
    let change = outcome.change;
    if matches!(
        outcome.history,
        Some(TaskHistoryWhat::Held | TaskHistoryWhat::Released)
    ) {
        return Ok(());
    }
    // Every change but a new task's has the row before.
    if before.is_none() && change != TaskChange::Created {
        return Ok(());
    }
    let fields = before
        .map(|before| edited(before, after))
        .unwrap_or_default();
    let moved = before.is_some_and(|before| due_moved(before, after));
    let unchanged = match change {
        TaskChange::Edited | TaskChange::DueChanged => fields.is_empty() && !moved,
        TaskChange::Planned => !before.is_some_and(|before| replanned(before, after)),
        _ => false,
    };
    if unchanged {
        return Ok(());
    }
    let Some(task) = snapshot(tx, after.id).await? else {
        return Ok(());
    };
    let tracker = &state.tracker;
    if reply(state, tx, workspace, change, &task).await? {
        return Ok(());
    }
    let Some(before) = before.filter(|_| change != TaskChange::Created) else {
        tracker
            .track(tx, &tracking::TaskCreated { workspace, task })
            .await?;
        return Ok(());
    };
    match change {
        TaskChange::Edited | TaskChange::DueChanged => {
            if !fields.is_empty() {
                let before_verb = fields
                    .contains(&tracking::TaskField::Verb)
                    .then(|| verb_of(&before.verb));
                let edited = tracking::TaskEdited {
                    workspace,
                    task: task.clone(),
                    fields,
                    before_verb,
                };
                tracker.track(tx, &edited).await?;
            }
            if moved {
                let before = tracking::TaskDueSnapshot {
                    due_on: before.due_on,
                    due_at: before.due_at,
                };
                let due = tracking::TaskDueChanged {
                    workspace,
                    task,
                    before,
                };
                tracker.track(tx, &due).await?;
            }
        }
        TaskChange::Planned => {
            let before = tracking::TaskPlanSnapshot {
                plan: plan_of(&before.plan),
                plan_on: before.plan_on,
                later_until: before.later_until,
            };
            let planned = tracking::TaskPlanned {
                workspace,
                task,
                before,
            };
            tracker.track(tx, &planned).await?;
        }
        TaskChange::Done => track_done(tracker, tx, workspace, task).await?,
        TaskChange::NotNeeded => {
            let closed = tracking::TaskNotNeeded { workspace, task };
            tracker.track(tx, &closed).await?;
        }
        TaskChange::Reopened => {
            let reopened = tracking::TaskReopened {
                workspace,
                task,
                before: state_of(&before.state),
            };
            tracker.track(tx, &reopened).await?;
        }
        TaskChange::HandedOff | TaskChange::TakenBack => {
            let reassigned = reassigned(workspace, task, change, before);
            tracker.track(tx, &reassigned).await?;
        }
        // Handled above, or the undo of Not yet (see [`reply`]).
        TaskChange::Created
        | TaskChange::Replied
        | TaskChange::ReplyAnswered
        | TaskChange::ReplyUnanswered => {}
    }
    Ok(())
}

/// What a tracked hold says besides the task.
#[derive(Debug, Clone, Copy)]
pub struct HoldFacts {
    /// How long the held time is, in minutes.
    pub minutes: i64,
    /// Whether it replaced or moved an earlier hold.
    pub moved: bool,
    /// The member, or the calendar.
    pub by: tracking::HoldActor,
    /// Where the block is, when the member held it.
    pub calendar: Option<tracking::HoldCalendar>,
}

/// The length of a hold from `from` to `to`, in minutes.
pub fn minutes(from: DateTime<Utc>, to: DateTime<Utc>) -> i64 {
    (to - from).num_minutes()
}

/// Track `tasks.task_held` for task `id`, in `tx`, after the change.
pub async fn held(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    id: TaskId,
    facts: HoldFacts,
) -> Result<()> {
    let Some(task) = snapshot(tx, id).await? else {
        return Ok(());
    };
    let held = tracking::TaskHeld {
        workspace,
        by: facts.by,
        calendar: facts.calendar,
        minutes: facts.minutes,
        moved: facts.moved,
        task,
    };
    state.tracker.track(tx, &held).await?;
    Ok(())
}

/// Track `tasks.task_hold_released` for task `id`, in `tx`, after the
/// change: a hold of `minutes`, released by `by`.
pub async fn hold_released(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    id: TaskId,
    minutes: i64,
    by: tracking::HoldActor,
) -> Result<()> {
    let Some(task) = snapshot(tx, id).await? else {
        return Ok(());
    };
    let released = tracking::TaskHoldReleased {
        workspace,
        by,
        minutes,
        task,
    };
    state.tracker.track(tx, &released).await?;
    Ok(())
}
