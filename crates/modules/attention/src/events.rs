//! The events this module publishes (ADR 0012), and the other modules'
//! events it consumes, as their published contracts describe them (modules
//! never import each other).
//!
//! **No personal data.** The module holds ids, enums and dates only; its
//! events carry ids.

use campbooks_kernel::{DomainEvent, RecordRef, changes::Change, tenancy::UserId};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::domain::{ItemId, Reason, Trust};

/// Record type of an attention item.
pub const ITEM_RECORD: &str = "attention.item";

/// An item opened, changed, cleared, or was snoozed or unsnoozed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ItemChanged {
    /// The workspace.
    pub workspace: String,
    /// The item.
    pub item: ItemId,
    /// Who may see it: its container (`conversations.inbox`).
    pub container: RecordRef,
}

impl DomainEvent for ItemChanged {
    const NAME: &'static str = "attention.item_changed";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }

    fn changes(&self) -> Vec<Change> {
        vec![Change::upsert(RecordRef::new(ITEM_RECORD, &self.item)).within(self.container.clone())]
    }
}

/// `conversations.need_changed` v1
/// (`contracts/conversations.need_changed.v1.schema.json`): what a
/// conversation needs from its inbox's members.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationNeedChanged {
    /// The workspace.
    pub workspace: String,
    /// The inbox.
    pub inbox: String,
    /// The conversation.
    pub conversation: String,
    /// The message the need comes from.
    pub message: String,
    /// Grows with every change of this conversation's need.
    pub revision: i64,
    /// When that message was sent.
    pub sent_at: DateTime<Utc>,
    /// The need, or `None` once cleared.
    pub need: Option<NeedFacts>,
    /// Why it was cleared.
    #[serde(default)]
    pub cleared_because: Option<String>,
    /// The member who cleared it.
    #[serde(default)]
    pub cleared_by: Option<UserId>,
    /// The conversation's newest inbound message (the sender writing again).
    #[serde(default)]
    pub latest_message: Option<String>,
}

/// The need as `conversations.need_changed` carries it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NeedFacts {
    /// Why.
    pub reason: Reason,
    /// How much the producer stands behind it.
    pub trust: Trust,
    /// When it is due.
    #[serde(default)]
    pub due_on: Option<NaiveDate>,
    /// When what it asks stops being possible (campbooks-cloud#275); absent
    /// from events written before.
    #[serde(default)]
    pub lapses_at: Option<DateTime<Utc>>,
    /// Whether it is for today.
    pub urgent: bool,
    /// Whether it is the warning about suspicious mail (campbooks-cloud#358);
    /// false in events written before.
    #[serde(default)]
    pub warning: bool,
    /// Whether a person sent its message rather than a machine, from the
    /// headers alone; `None` when unknown, and in events written before.
    #[serde(default)]
    pub from_person: Option<bool>,
}

impl DomainEvent for ConversationNeedChanged {
    const NAME: &'static str = "conversations.need_changed";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// `tasks.task_changed` v1 (`contracts/tasks.task_changed.v1.schema.json`):
/// a task was made or changed. Only what the projection reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskChanged {
    /// The workspace.
    pub workspace: String,
    /// The task.
    pub task: String,
    /// Grows with every change of this task.
    pub revision: i64,
    /// Open, done or not needed.
    pub state: TaskState,
    /// Do or Pay.
    pub verb: TaskVerb,
    /// The day it is due.
    #[serde(default)]
    pub due_on: Option<NaiveDate>,
    /// The plan: `none`, `date` or `later`.
    pub plan: String,
    /// The day it is planned for, when its plan is a day.
    #[serde(default)]
    pub plan_on: Option<NaiveDate>,
    /// When Later ends.
    #[serde(default)]
    pub later_until: Option<DateTime<Utc>>,
    /// The member it is with.
    pub assignee: UserId,
    /// The record it came from.
    #[serde(default)]
    pub source: Option<RecordRef>,
}

/// A task's state, as `tasks.task_changed` carries it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    /// It still needs doing.
    Open,
    /// It was done (or paid).
    Done,
    /// It is not needed after all.
    NotNeeded,
}

/// A task's verb, as `tasks.task_changed` carries it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskVerb {
    /// Something to do.
    Do,
    /// A bill to pay.
    Pay,
}

impl DomainEvent for TaskChanged {
    const NAME: &'static str = "tasks.task_changed";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
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

/// `workspaces.member_left`: a user is no longer a member.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberLeft {
    /// The workspace.
    pub workspace: String,
    /// The former member.
    pub user: UserId,
}

impl DomainEvent for MemberLeft {
    const NAME: &'static str = "workspaces.member_left";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// `conversations.inbox_removed`: an inbox was deleted with everything in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InboxRemoved {
    /// The workspace.
    pub workspace: String,
    /// The inbox (a container of items).
    pub inbox: String,
}

impl DomainEvent for InboxRemoved {
    const NAME: &'static str = "conversations.inbox_removed";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}
