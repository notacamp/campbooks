//! `tasks.follow_hold_created`, `tasks.follow_hold_changed` and
//! `tasks.follow_hold_cancelled` ← `schedule.event_*`: a hold follows its
//! block on the calendar (campbooks-cloud#362). Only events made for a task
//! (`source = tasks.task:<id>`) are read, and only for an open task assigned
//! to the calendar's owner.
//!
//! - **Cancelled** (removed in Campbooks or at the provider): the task whose
//!   hold it is keeps no hold and no plan.
//! - **Changed** (moved): the hold takes the block's new times and the plan
//!   its new day. A block that became all-day no longer holds time: it is
//!   released.
//! - **Created** (the calendar's undo of a removal puts the same event back):
//!   it becomes the hold again when the task holds none.
//!
//! Each is recorded with the actor `calendar` and changes nothing when the
//! task already says what the block says, so a redelivery, or the echo of
//! the task's own hold and release, does nothing.

use campbooks_kernel::{
    AppState, Event, EventHandler, JobContext, JobError,
    tenancy::{Access, WorkspaceId},
};
use chrono::{DateTime, Utc};
use sqlx::{Postgres, Transaction};

use crate::{
    domain::{
        Outcome, SELECT_LOCKED, TaskActor, TaskChange, TaskHistoryDetail, TaskHistoryWhat, TaskId,
        TaskPlan, TaskRow, TaskVerb, finish,
        holds::{clear, local_day},
        resources::TASK_RECORD,
        tracked::{self, HoldFacts},
    },
    inbound::{CalendarEventCancelled, CalendarEventChanged, CalendarEventCreated},
    tracking,
};

/// What happened to the block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Happened {
    Created,
    Changed,
    Cancelled,
}

/// The facts of a block that this module reads.
struct Block<'a> {
    workspace: &'a str,
    event: sqlx::types::Uuid,
    calendar: sqlx::types::Uuid,
    owner: campbooks_kernel::tenancy::UserId,
    kind: &'a str,
    starts: &'a str,
    ends: Option<&'a str>,
    all_day: bool,
    task: Option<TaskId>,
}

/// The task a block was made for, when it was made for one.
fn task_of(source: Option<&campbooks_kernel::RecordRef>) -> Option<TaskId> {
    source
        .filter(|source| source.record_type == TASK_RECORD)
        .and_then(|source| source.id.parse().ok())
}

macro_rules! block {
    ($event:expr) => {
        Block {
            workspace: &$event.workspace,
            event: $event.event,
            calendar: $event.calendar,
            owner: $event.owner,
            kind: &$event.kind,
            starts: &$event.starts,
            ends: $event.ends.as_deref(),
            all_day: $event.all_day,
            task: task_of($event.source.as_ref()),
        }
    };
}

/// A held block put back on the calendar.
pub struct FollowHoldCreated;

impl EventHandler for FollowHoldCreated {
    type Event = CalendarEventCreated;
    const NAME: &'static str = "tasks.follow_hold_created";
    const QUEUE: &'static str = "default";

    async fn handle(
        ctx: &mut JobContext,
        event: Event<CalendarEventCreated>,
    ) -> Result<(), JobError> {
        let payload = event.payload;
        follow(ctx, &block!(payload), Happened::Created).await
    }
}

/// A held block moved.
pub struct FollowHoldChanged;

impl EventHandler for FollowHoldChanged {
    type Event = CalendarEventChanged;
    const NAME: &'static str = "tasks.follow_hold_changed";
    const QUEUE: &'static str = "default";

    async fn handle(
        ctx: &mut JobContext,
        event: Event<CalendarEventChanged>,
    ) -> Result<(), JobError> {
        let payload = event.payload;
        follow(ctx, &block!(payload), Happened::Changed).await
    }
}

/// A held block removed.
pub struct FollowHoldCancelled;

impl EventHandler for FollowHoldCancelled {
    type Event = CalendarEventCancelled;
    const NAME: &'static str = "tasks.follow_hold_cancelled";
    const QUEUE: &'static str = "default";

    async fn handle(
        ctx: &mut JobContext,
        event: Event<CalendarEventCancelled>,
    ) -> Result<(), JobError> {
        let payload = event.payload;
        follow(ctx, &block!(payload), Happened::Cancelled).await
    }
}

/// The block's times, when it holds time (a timed block with an end).
fn times(block: &Block<'_>) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    if block.all_day {
        return None;
    }
    let parse = |text: &str| {
        DateTime::parse_from_rfc3339(text)
            .ok()
            .map(|at| at.with_timezone(&Utc))
    };
    let starts = parse(block.starts)?;
    let ends = parse(block.ends?)?;
    (ends > starts).then_some((starts, ends))
}

async fn follow(
    ctx: &mut JobContext,
    block: &Block<'_>,
    happened: Happened,
) -> Result<(), JobError> {
    // Only blocks made for a task, of kind hold.
    let Some(task) = block.task else {
        return Ok(());
    };
    if block.kind != "hold" {
        return Ok(());
    }
    let workspace: WorkspaceId = block
        .workspace
        .parse()
        .map_err(|_| JobError::fatal("a schedule event names an invalid workspace"))?;
    let state = ctx.state().clone();
    let tx = ctx.tx().await?;
    let Some(row) = sqlx::query_as::<_, TaskRow>(SELECT_LOCKED)
        .bind(task)
        .fetch_optional(&mut **tx)
        .await?
    else {
        return Ok(());
    };
    // Gone, closed, or not the owner's: nothing to follow.
    if !row.is_open() || row.assignee != block.owner || row.verb != TaskVerb::Do.as_str() {
        return Ok(());
    }
    let access = Access {
        workspace,
        user: Some(block.owner),
        role: None,
    };
    let ours = row.hold_event == Some(block.event);
    match (happened, times(block)) {
        (Happened::Cancelled, _) | (Happened::Changed, None) if ours => {
            released(&state, tx, &access, row).await?;
        }
        (Happened::Changed, Some((starts, ends))) if ours => {
            if (row.held_from, row.held_to) != (Some(starts), Some(ends)) {
                held(&state, tx, &access, row, block, (starts, ends), true).await?;
            }
        }
        (Happened::Created, Some(span)) if row.hold_event.is_none() => {
            held(&state, tx, &access, row, block, span, false).await?;
        }
        _ => {}
    }
    Ok(())
}

/// The block is gone: the task keeps no hold and no plan.
async fn released(
    state: &AppState,
    tx: &mut Transaction<'static, Postgres>,
    access: &Access,
    row: TaskRow,
) -> Result<(), JobError> {
    let (Some(event), Some(from), Some(to)) = (row.hold_event, row.held_from, row.held_to) else {
        return Ok(());
    };
    clear(tx, &row).await?;
    sqlx::query("UPDATE tasks.tasks SET revision = revision + 1, updated_at = now() WHERE id = $1")
        .bind(row.id)
        .execute(&mut **tx)
        .await?;
    let id = row.id;
    finish(
        state,
        tx,
        access,
        id,
        &Outcome {
            actor: TaskActor::Calendar,
            change: TaskChange::Planned,
            history: Some(TaskHistoryWhat::Released),
            detail: TaskHistoryDetail {
                plan: Some(TaskPlan::None),
                hold_event: Some(event),
                ..TaskHistoryDetail::default()
            },
            previous_assignee: None,
            previous_handed_by: None,
            before: Some(row),
        },
        Utc::now().date_naive(),
    )
    .await?;
    tracked::hold_released(
        state,
        tx,
        access.workspace,
        id,
        tracked::minutes(from, to),
        tracking::HoldActor::Calendar,
    )
    .await?;
    Ok(())
}

/// The block holds `(starts, ends)` for the task: moved (`moved`), or back.
async fn held(
    state: &AppState,
    tx: &mut Transaction<'static, Postgres>,
    access: &Access,
    row: TaskRow,
    block: &Block<'_>,
    (starts, ends): (DateTime<Utc>, DateTime<Utc>),
    moved: bool,
) -> Result<(), JobError> {
    let day = local_day(state, tx, access.workspace, block.owner, starts).await?;
    sqlx::query(
        "UPDATE tasks.tasks SET plan = 'date', plan_on = $2, later_until = NULL,
                hold_event = $3, hold_calendar = $4, held_from = $5, held_to = $6,
                revision = revision + 1, updated_at = now()
          WHERE id = $1",
    )
    .bind(row.id)
    .bind(day)
    .bind(block.event)
    .bind(block.calendar)
    .bind(starts)
    .bind(ends)
    .execute(&mut **tx)
    .await?;
    let id = row.id;
    finish(
        state,
        tx,
        access,
        id,
        &Outcome {
            actor: TaskActor::Calendar,
            change: TaskChange::Planned,
            history: Some(TaskHistoryWhat::Held),
            detail: TaskHistoryDetail {
                plan: Some(TaskPlan::Date),
                plan_on: Some(day),
                hold_event: Some(block.event),
                held_from: Some(starts),
                held_to: Some(ends),
                ..TaskHistoryDetail::default()
            },
            previous_assignee: None,
            previous_handed_by: None,
            before: Some(row),
        },
        Utc::now().date_naive(),
    )
    .await?;
    tracked::held(
        state,
        tx,
        access.workspace,
        id,
        HoldFacts {
            minutes: tracked::minutes(starts, ends),
            moved,
            by: tracking::HoldActor::Calendar,
            calendar: None,
        },
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests;
