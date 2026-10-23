//! The module's vocabulary and what its features share: a task as the API
//! shows it, its history, and the one way a change is finished (history, then
//! `tasks.task_changed`, in the change's own transaction).

use std::str::FromStr;

pub mod access;
pub mod resources;

use campbooks_kernel::{
    AppState, Error, RecordRef, Result, define_id,
    tenancy::{Access, UserId},
    text,
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Transaction};
use utoipa::ToSchema;

use crate::events::TaskChanged;

define_id!(
    /// Something a member has to do.
    TaskId
);

define_id!(
    /// One entry of a task's history.
    HistoryId
);

/// A closed set of words kept as text in the database and as snake_case on
/// the wire.
macro_rules! words {
    ($(#[$meta:meta])* $name:ident { $($(#[$vmeta:meta])* $variant:ident => $text:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            $($(#[$vmeta])* $variant),+
        }

        impl $name {
            /// The wire and database name.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $text),+
                }
            }
        }

        impl FromStr for $name {
            type Err = Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($text => Ok(Self::$variant),)+
                    other => Err(Error::internal(format!(
                        "unknown {} `{other}`",
                        stringify!($name)
                    ))),
                }
            }
        }
    };
}

words!(
    /// What kind of thing it is: something to do, or a bill to pay.
    Verb {
        /// Something to do.
        Do => "do",
        /// Money is owed. Campbooks never pays: this marks it paid.
        Pay => "pay",
    }
);

words!(
    /// How the task came to be.
    Origin {
        /// The member wrote it.
        Own => "own",
        /// Asked of the member in an email.
        Ask => "ask",
        /// A deadline found in an email.
        Deadline => "deadline",
    }
);

words!(
    /// When the member will do it.
    PlanKind {
        /// No plan.
        None => "none",
        /// A day.
        Date => "date",
        /// Later: put off until a time.
        Later => "later",
    }
);

words!(
    /// Whether it still needs doing.
    State {
        /// It does.
        Open => "open",
        /// It was done (or paid).
        Done => "done",
        /// It is not needed after all.
        NotNeeded => "not_needed",
    }
);

words!(
    /// Who made a change.
    Actor {
        /// The member.
        Member => "member",
        /// Scout, for the member.
        Scout => "scout",
        /// The calendar.
        Calendar => "calendar",
    }
);

words!(
    /// What changed: the event's `change` and the history's `what`.
    Change {
        /// The task was made.
        Created => "created",
        /// Its title or verb changed.
        Edited => "edited",
        /// Its due date changed.
        DueChanged => "due_changed",
        /// Its plan changed.
        Planned => "planned",
        /// It was done (or paid).
        Done => "done",
        /// It was marked not needed.
        NotNeeded => "not_needed",
        /// It was opened again.
        Reopened => "reopened",
        /// It was handed to a teammate.
        HandedOff => "handed_off",
        /// The member who handed it off took it back.
        TakenBack => "taken_back",
    }
);

words!(
    /// A field `tasks.update` changed.
    EditedField {
        /// The title.
        Title => "title",
        /// The verb.
        Verb => "verb",
        /// The due date or time.
        Due => "due",
    }
);

/// What a history entry holds besides its kind: ids, enums and dates only,
/// never a title.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct HistoryDetail {
    /// The fields an edit changed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<EditedField>,
    /// The due day afterwards.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_on: Option<NaiveDate>,
    /// The plan afterwards.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<PlanKind>,
    /// The planned day.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_on: Option<NaiveDate>,
    /// When Later ends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub later_until: Option<DateTime<Utc>>,
    /// The other member of a hand-off or a take-back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member: Option<UserId>,
}

/// A task, as the caller sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskView {
    /// The task.
    pub id: TaskId,
    /// What to do, in the member's words.
    pub title: String,
    /// Do or Pay.
    pub verb: Verb,
    /// How it came to be.
    pub origin: Origin,
    /// The day it is due.
    pub due_on: Option<NaiveDate>,
    /// The time it is due, when it has one (`due_on` is its local day).
    pub due_at: Option<DateTime<Utc>>,
    /// Whether it is open and its due day has passed (the caller's today).
    pub overdue: bool,
    /// The plan. A Later that has ended reads as `none`.
    pub plan: PlanKind,
    /// The planned day, for `date`.
    pub plan_on: Option<NaiveDate>,
    /// When Later ends, for `later`.
    pub later_until: Option<DateTime<Utc>>,
    /// Open, done or not needed.
    pub state: State,
    /// When it was closed.
    pub closed_at: Option<DateTime<Utc>>,
    /// The member it is with.
    pub assignee: UserId,
    /// The member who made it.
    pub created_by: UserId,
    /// The member who handed it to the assignee, while it is handed off.
    pub handed_by: Option<UserId>,
    /// The record it came from, when it came from one.
    pub source: Option<RecordRef>,
    /// Grows with every change: send the one you saw with a change.
    pub revision: i64,
    /// When it was made.
    pub created_at: DateTime<Utc>,
    /// When it last changed.
    pub updated_at: DateTime<Utc>,
}

/// One entry of a task's history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct HistoryEntry {
    /// The entry.
    pub id: HistoryId,
    /// When.
    pub at: DateTime<Utc>,
    /// Who: the member, Scout or the calendar.
    pub actor: Actor,
    /// The member Scout or the calendar acted for, or who acted.
    pub actor_member: Option<UserId>,
    /// What changed.
    pub what: Change,
    /// Ids, enums and dates of the change.
    pub detail: HistoryDetail,
}

/// The columns of a task row, in [`TaskRow`] order.
macro_rules! columns {
    () => {
        "id, title, verb, origin, due_on, due_at, plan, plan_on, later_until, state, closed_at, \
         assignee, created_by, handed_by, source_type, source_id, source_container, revision, \
         created_at, updated_at"
    };
}

/// One task by id.
pub const SELECT_ONE: &str = concat!("SELECT ", columns!(), " FROM tasks.tasks WHERE id = $1");

/// One task by id, locked for the change.
pub const SELECT_LOCKED: &str = concat!(
    "SELECT ",
    columns!(),
    " FROM tasks.tasks WHERE id = $1 FOR UPDATE"
);

/// The columns, for queries that filter their own way.
pub const SELECT_COLUMNS: &str = columns!();

/// A task row.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TaskRow {
    pub id: TaskId,
    pub title: String,
    pub verb: String,
    pub origin: String,
    pub due_on: Option<NaiveDate>,
    pub due_at: Option<DateTime<Utc>>,
    pub plan: String,
    pub plan_on: Option<NaiveDate>,
    pub later_until: Option<DateTime<Utc>>,
    pub state: String,
    pub closed_at: Option<DateTime<Utc>>,
    pub assignee: UserId,
    pub created_by: UserId,
    pub handed_by: Option<UserId>,
    pub source_type: Option<String>,
    pub source_id: Option<String>,
    pub source_container: Option<String>,
    pub revision: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TaskRow {
    /// The record it came from.
    pub fn source(&self) -> Option<RecordRef> {
        match (&self.source_type, &self.source_id) {
            (Some(record_type), Some(id)) => Some(RecordRef::new(record_type.clone(), id)),
            _ => None,
        }
    }

    /// Whether it is open.
    pub fn is_open(&self) -> bool {
        self.state == State::Open.as_str()
    }

    /// The view at `now`, with `today` (the caller's) deciding whether it is
    /// overdue.
    pub fn view(&self, now: DateTime<Utc>, today: NaiveDate) -> Result<TaskView> {
        let state: State = self.state.parse()?;
        let mut plan: PlanKind = self.plan.parse()?;
        let (mut plan_on, mut later_until) = (self.plan_on, self.later_until);
        // Later ends by itself: from then on it reads as no plan.
        if plan == PlanKind::Later && self.later_until.is_none_or(|until| until <= now) {
            plan = PlanKind::None;
            plan_on = None;
            later_until = None;
        }
        Ok(TaskView {
            id: self.id,
            title: self.title.clone(),
            verb: self.verb.parse()?,
            origin: self.origin.parse()?,
            due_on: self.due_on,
            due_at: self.due_at,
            overdue: state == State::Open && self.due_on.is_some_and(|due| due < today),
            plan,
            plan_on,
            later_until,
            state,
            closed_at: self.closed_at,
            assignee: self.assignee,
            created_by: self.created_by,
            handed_by: self.handed_by,
            source: self.source(),
            revision: self.revision,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}

/// The revision of a task the caller saw, for a change that needs nothing
/// else.
#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskRevision {
    /// The revision you saw.
    pub revision: i64,
}

/// A history row.
#[derive(sqlx::FromRow)]
pub struct HistoryRow {
    pub id: HistoryId,
    pub at: DateTime<Utc>,
    pub actor: String,
    pub actor_member: Option<UserId>,
    pub what: String,
    pub detail: sqlx::types::Json<HistoryDetail>,
}

impl HistoryRow {
    /// The entry.
    pub fn entry(self) -> Result<HistoryEntry> {
        Ok(HistoryEntry {
            id: self.id,
            at: self.at,
            actor: self.actor.parse()?,
            actor_member: self.actor_member,
            what: self.what.parse()?,
            detail: self.detail.0,
        })
    }
}

/// A title as it is kept: whitespace as plain spaces, no bidirectional,
/// zero-width or control characters, trimmed, 1 to 200 characters.
pub fn clean_title(raw: &str) -> Result<String> {
    let spaced: String = raw
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .collect();
    let cleaned = text::displayable(&spaced);
    let title = cleaned.trim();
    let length = title.chars().count();
    if length == 0 || length > 200 {
        return Err(Error::BadRequest(crate::MESSAGES.reason("invalid_title")));
    }
    Ok(title.to_owned())
}

/// A sender's address as it is kept for matching: trimmed, lower case, one
/// `@`, no spaces. A structural check of an address, nothing about content.
pub fn clean_sender(raw: &str) -> Result<String> {
    let address = raw.trim().to_lowercase();
    let well_formed = address.len() <= 320
        && address.matches('@').count() == 1
        && !address.starts_with('@')
        && !address.ends_with('@')
        && address
            .chars()
            .all(|c| !c.is_whitespace() && !c.is_control());
    if well_formed {
        Ok(address)
    } else {
        Err(Error::BadRequest(
            crate::MESSAGES.reason("invalid_source_sender"),
        ))
    }
}

/// The caller's own today, given their offset from UTC in minutes.
pub fn today(now: DateTime<Utc>, utc_offset_minutes: i32) -> NaiveDate {
    (now + chrono::TimeDelta::minutes(i64::from(utc_offset_minutes))).date_naive()
}

/// The widest offset from UTC a time zone has (UTC+14:00), in minutes.
pub const MAX_OFFSET_MINUTES: i32 = 14 * 60;

/// Who is acting: the member, or Scout when the assistant or an automation
/// runs the call for them.
pub fn actor_of(invoked_by: Option<campbooks_kernel::operations::Caller>) -> Actor {
    if invoked_by.is_some() {
        Actor::Scout
    } else {
        Actor::Member
    }
}

/// What a finished change needs besides the row.
pub struct Outcome {
    /// Who made it.
    pub actor: Actor,
    /// What it was.
    pub change: Change,
    /// What to keep in the history.
    pub detail: HistoryDetail,
    /// The assignee before a hand-off or a take-back.
    pub previous_assignee: Option<UserId>,
}

/// Finish a change to task `id` made in `tx`: append the history entry,
/// publish `tasks.task_changed` and return the task as it is now. The caller
/// has already written the row (bumping `revision`).
pub async fn finish(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    access: &Access,
    id: TaskId,
    outcome: &Outcome,
) -> Result<TaskView> {
    let row: TaskRow = sqlx::query_as(SELECT_ONE)
        .bind(id)
        .fetch_one(&mut **tx)
        .await?;
    sqlx::query(
        "INSERT INTO tasks.history (id, workspace_id, task_id, actor, actor_member, what, detail)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(HistoryId::new())
    .bind(access.workspace.as_uuid())
    .bind(id)
    .bind(outcome.actor.as_str())
    .bind(access.user.map(|user| *user.as_uuid()))
    .bind(outcome.change.as_str())
    .bind(sqlx::types::Json(&outcome.detail))
    .execute(&mut **tx)
    .await?;
    let event = TaskChanged {
        workspace: access.workspace.to_string(),
        task: id,
        revision: row.revision,
        change: outcome.change,
        state: row.state.parse()?,
        verb: row.verb.parse()?,
        origin: row.origin.parse()?,
        due_on: row.due_on,
        due_at: row.due_at,
        plan: row.plan.parse()?,
        plan_on: row.plan_on,
        later_until: row.later_until,
        assignee: row.assignee,
        handed_by: row.handed_by,
        previous_assignee: outcome.previous_assignee,
        source: row.source(),
    };
    state.events.publish(tx, &event).await?;
    let now = Utc::now();
    row.view(now, now.date_naive())
}
