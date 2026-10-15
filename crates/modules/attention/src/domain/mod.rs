//! The module's vocabulary and what its features share: an item as the API
//! shows it, and which containers a caller may see.

use std::str::FromStr;

pub mod resources;

use campbooks_kernel::{
    AppState, Error, RecordRef, Result, define_id,
    tenancy::{Access, UserId},
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use utoipa::ToSchema;

define_id!(
    /// One thing that needs the members who can see its container.
    ItemId
);

/// Why an item needs someone, in the order Home shows the groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    /// Look before acting: a warning, or a question.
    Review,
    /// Someone is waiting for an answer.
    Reply,
    /// A choice is needed.
    Decide,
    /// Something to do.
    Do,
    /// Money is owed.
    Pay,
}

impl Reason {
    /// Every reason, in display order.
    pub const ALL: [Self; 5] = [Self::Review, Self::Reply, Self::Decide, Self::Do, Self::Pay];

    /// The wire and database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Review => "review",
            Self::Reply => "reply",
            Self::Decide => "decide",
            Self::Do => "do",
            Self::Pay => "pay",
        }
    }
}

impl FromStr for Reason {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|r| r.as_str() == s)
            .ok_or_else(|| Error::internal(format!("unknown reason `{s}`")))
    }
}

/// How much the producing module stands behind an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Trust {
    /// It asserts it.
    Sure,
    /// It asks the member (a question).
    Check,
}

impl Trust {
    /// The wire and database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sure => "sure",
            Self::Check => "check",
        }
    }
}

impl FromStr for Trust {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "sure" => Ok(Self::Sure),
            "check" => Ok(Self::Check),
            other => Err(Error::internal(format!("unknown trust `{other}`"))),
        }
    }
}

/// Whether an item still needs someone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ItemState {
    /// It does.
    Open,
    /// It no longer does.
    Cleared,
}

/// Days past its due date after which an item leaves the list.
pub const OVERDUE_DAYS: i64 = 7;

/// An attention item, as the caller sees it. References and facts only: the
/// owning module renders who and what.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ItemView {
    /// The item.
    pub id: ItemId,
    /// The record it is about.
    pub record: RecordRef,
    /// Who may see it: its container.
    pub container: RecordRef,
    /// The record that raised it.
    pub cause: RecordRef,
    /// Why.
    pub reason: Reason,
    /// `sure`, or `check` for a question to the member.
    pub trust: Trust,
    /// When it is due.
    pub due_on: Option<NaiveDate>,
    /// Whether the due date has passed.
    pub overdue: bool,
    /// Whether it is for today.
    pub urgent: bool,
    /// When its cause arrived.
    pub since: DateTime<Utc>,
    /// Open or cleared.
    pub state: ItemState,
    /// Why it was cleared (`replied`, `archived`, `trashed`, `done`,
    /// `not_needed`), as the producing module said.
    pub cleared_because: Option<String>,
    /// The member who cleared it, when one did.
    pub cleared_by: Option<UserId>,
    /// When it was cleared.
    pub cleared_at: Option<DateTime<Utc>>,
    /// Until when the caller put it off ("Later"), when they did.
    pub snoozed_until: Option<DateTime<Utc>>,
}

/// An item row (`attention.items` with the caller's snooze).
pub struct ItemRow {
    pub id: ItemId,
    pub record_type: String,
    pub record_id: String,
    pub container_type: String,
    pub container_id: String,
    pub cause_type: String,
    pub cause_id: String,
    pub reason: String,
    pub trust: String,
    pub due_on: Option<NaiveDate>,
    pub urgent: bool,
    pub since: DateTime<Utc>,
    pub state: String,
    pub cleared_because: Option<String>,
    pub cleared_by: Option<UserId>,
    pub cleared_at: Option<DateTime<Utc>>,
    pub snoozed_until: Option<DateTime<Utc>>,
}

impl ItemRow {
    /// The view, with `today` deciding whether it is overdue.
    pub fn view(self, today: NaiveDate) -> Result<ItemView> {
        Ok(ItemView {
            id: self.id,
            record: RecordRef::new(self.record_type, &self.record_id),
            container: RecordRef::new(self.container_type, &self.container_id),
            cause: RecordRef::new(self.cause_type, &self.cause_id),
            reason: self.reason.parse()?,
            trust: self.trust.parse()?,
            overdue: self.due_on.is_some_and(|due| due < today),
            due_on: self.due_on,
            urgent: self.urgent,
            since: self.since,
            state: if self.state == "open" {
                ItemState::Open
            } else {
                ItemState::Cleared
            },
            cleared_because: self.cleared_because,
            cleared_by: self.cleared_by,
            cleared_at: self.cleared_at,
            snoozed_until: self.snoozed_until,
        })
    }
}

/// The containers of the workspace's items that `access` may see, as
/// `<type>:<id>` keys, asked of the kernel's `Visibility` port (implemented
/// by the module that owns each container).
pub async fn visible_containers(
    state: &AppState,
    conn: &mut PgConnection,
    access: &Access,
) -> Result<Vec<String>> {
    let containers = sqlx::query!(
        "SELECT DISTINCT container_type, container_id FROM attention.items WHERE workspace_id = $1",
        access.workspace.as_uuid(),
    )
    .fetch_all(conn)
    .await?;
    let mut visible = Vec::new();
    for container in containers {
        let reference = RecordRef::new(container.container_type, &container.container_id);
        if state
            .visibility
            .container_visible(&state.db, access, &reference)
            .await?
        {
            visible.push(key(&reference));
        }
    }
    Ok(visible)
}

/// `<type>:<id>`, the key [`visible_containers`] returns.
pub fn key(reference: &RecordRef) -> String {
    format!("{}:{}", reference.record_type, reference.id)
}

/// Forget every item inside `container` (and their snoozes) in `conn`'s
/// workspace: the hook for when the owning module deletes a container, such
/// as an inbox (campbooks-cloud#117). Returns how many items went.
pub async fn purge_container(conn: &mut PgConnection, container: &RecordRef) -> Result<u64> {
    let gone = sqlx::query!(
        "DELETE FROM attention.items WHERE container_type = $1 AND container_id = $2",
        container.record_type,
        container.id,
    )
    .execute(conn)
    .await?
    .rows_affected();
    Ok(gone)
}
