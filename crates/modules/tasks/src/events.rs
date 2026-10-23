//! The events this module publishes (ADR 0012), and the other modules'
//! events it consumes, as their published contracts describe them (modules
//! never import each other).
//!
//! **No personal data.** `tasks.task_changed` holds ids, enums and dates:
//! never a title, an address or an amount.

use campbooks_kernel::{DomainEvent, RecordRef, changes, tenancy::UserId};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::domain::{
    TaskChange, TaskId, TaskOrigin, TaskPlan, TaskState, TaskVerb,
    resources::{ASSIGNEE_RECORD, TASK_RECORD},
};

/// `tasks.task_changed` v1 (`contracts/tasks.task_changed.v1.schema.json`):
/// a task was made or changed. A snapshot, so a consumer applies only newer
/// revisions and stays order-proof.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskChanged {
    /// The workspace.
    pub workspace: String,
    /// The task.
    pub task: TaskId,
    /// Grows with every change of this task.
    pub revision: i64,
    /// What changed.
    pub change: TaskChange,
    /// Open, done or not needed.
    pub state: TaskState,
    /// Do or Pay.
    pub verb: TaskVerb,
    /// How it came to be.
    pub origin: TaskOrigin,
    /// The day it is due.
    pub due_on: Option<NaiveDate>,
    /// The time it is due.
    pub due_at: Option<DateTime<Utc>>,
    /// The plan.
    pub plan: TaskPlan,
    /// The planned day.
    pub plan_on: Option<NaiveDate>,
    /// When Later ends.
    pub later_until: Option<DateTime<Utc>>,
    /// The member it is with.
    pub assignee: UserId,
    /// The member who handed it to the assignee.
    pub handed_by: Option<UserId>,
    /// For a hand-off or a take-back, the member who had it before.
    #[serde(default)]
    pub previous_assignee: Option<UserId>,
    /// The record it came from.
    pub source: Option<RecordRef>,
}

impl DomainEvent for TaskChanged {
    const NAME: &'static str = "tasks.task_changed";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }

    /// The task changed for its assignee, and for the member who handed it
    /// off (who sees it as "with <name>"); whoever had it before sees it go.
    fn changes(&self) -> Vec<changes::Change> {
        let task = || RecordRef::new(TASK_RECORD, &self.task);
        let container = |member: &UserId| RecordRef::new(ASSIGNEE_RECORD, member);
        let mut changed = vec![changes::Change::upsert(task()).within(container(&self.assignee))];
        if let Some(by) = self.handed_by.filter(|by| *by != self.assignee) {
            changed.push(changes::Change::upsert(task()).within(container(&by)));
        }
        if let Some(previous) = self
            .previous_assignee
            .filter(|previous| *previous != self.assignee && Some(*previous) != self.handed_by)
        {
            changed.push(changes::Change::delete(task()).within(container(&previous)));
        }
        changed
    }
}

/// `workspaces.workspace_deleting`: the workspace is gone at once and its
/// data is purged at `purge_after`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceDeleting {
    /// The workspace.
    pub workspace: String,
    /// When its data goes.
    pub purge_after: DateTime<Utc>,
}

impl DomainEvent for WorkspaceDeleting {
    const NAME: &'static str = "workspaces.workspace_deleting";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}
