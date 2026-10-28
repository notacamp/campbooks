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
    /// For a hand-off or a take-back, the member who had handed it off
    /// before, so a re-hand-off removes it from their list.
    #[serde(default)]
    pub previous_handed_by: Option<UserId>,
    /// The record it came from.
    pub source: Option<RecordRef>,
    /// Whether the member is asked "You replied. Is this done?"
    /// (campbooks-cloud#396). Absent from events written before.
    #[serde(default)]
    pub asks_done: bool,
    /// When the member's newest reply in the task's conversation was sent,
    /// while the task is an open Do with one. Absent from events written
    /// before.
    #[serde(default)]
    pub replied_at: Option<DateTime<Utc>>,
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
        // The giver keeps seeing it ("with <name>") only while it is open; a
        // closed task is theirs to read no more.
        if let Some(by) = self.handed_by.filter(|by| *by != self.assignee) {
            if self.state == TaskState::Open {
                changed.push(changes::Change::upsert(task()).within(container(&by)));
            } else {
                changed.push(changes::Change::delete(task()).within(container(&by)));
            }
        }
        // Whoever had it before sees it go: the previous assignee and the
        // previous giver, unless they are in it again.
        let kept = |member: &UserId| {
            *member == self.assignee
                || (self.state == TaskState::Open && Some(*member) == self.handed_by)
        };
        let mut gone: Vec<UserId> = Vec::new();
        for member in [self.previous_assignee, self.previous_handed_by]
            .into_iter()
            .flatten()
        {
            if !kept(&member) && !gone.contains(&member) && Some(member) != self.handed_by {
                gone.push(member);
            }
        }
        for member in gone {
            changed.push(changes::Change::delete(task()).within(container(&member)));
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

#[cfg(test)]
mod tests {
    use campbooks_kernel::changes::ChangeOp;
    use serde_json::json;

    use super::*;

    const ANA: &str = "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9a01";
    const JOAO: &str = "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9a02";
    const MARIA: &str = "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9a03";

    fn event(
        state: &str,
        assignee: &str,
        handed_by: Option<&str>,
        previous: [Option<&str>; 2],
    ) -> TaskChanged {
        serde_json::from_value(json!({
            "workspace": "w", "task": "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9d01",
            "revision": 3, "change": "handed_off", "state": state, "verb": "do",
            "origin": "own", "plan": "none", "assignee": assignee,
            "handed_by": handed_by, "previous_assignee": previous[0],
            "previous_handed_by": previous[1],
        }))
        .unwrap()
    }

    /// Which members' lists get what, as `(member, upsert?)`.
    fn effects(event: &TaskChanged) -> Vec<(String, bool)> {
        let mut effects: Vec<_> = event
            .changes()
            .into_iter()
            .map(|change| (change.container.unwrap().id, change.op == ChangeOp::Upsert))
            .collect();
        effects.sort();
        effects
    }

    #[test]
    fn a_re_hand_off_removes_it_from_the_previous_giver() {
        // Ana gave it to Joao; Joao hands it to Maria.
        let moved = event("open", MARIA, Some(JOAO), [Some(JOAO), Some(ANA)]);
        let mut expected = vec![
            (MARIA.to_owned(), true),
            (JOAO.to_owned(), true),
            (ANA.to_owned(), false),
        ];
        expected.sort();
        assert_eq!(effects(&moved), expected);
    }

    #[test]
    fn closing_a_handed_off_task_removes_it_from_the_giver() {
        let done = event("done", JOAO, Some(ANA), [None, None]);
        let mut expected = vec![(JOAO.to_owned(), true), (ANA.to_owned(), false)];
        expected.sort();
        assert_eq!(effects(&done), expected);
    }
}
