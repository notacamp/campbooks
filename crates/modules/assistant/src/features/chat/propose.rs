//! Scout's proposing tools (campbooks-cloud#305, phase 2): `propose_change`,
//! `propose_rule` and `propose_reply`. A proposal is checked and stored with
//! Scout's answer, and nothing else: the server never invokes a change for a
//! chat. The member confirms a card, and their own client sends the request
//! the server built here from the catalogue, with their own token.
//!
//! Every check reads as the member, through [`Member::read`], which carries
//! no idempotency key: a mutating operation could not run through it even
//! by mistake (`invoke` refuses it), and the tests prove none is asked.
//!
//! The action table ([`ROWS`]) maps each action to one catalogue operation,
//! its input and its undo. A row's undo is the one its operation names in
//! `x-campbooks.undo`, or, for a *restoring* row, the same operation with
//! the record's values read when the proposal is checked (the catalogue
//! forbids an operation naming itself as its undo). Tasks add their rows
//! (campbooks-cloud#43, `propose_task_change`); calendar events add theirs
//! (campbooks-cloud#42, `propose_event_change`); pages add theirs
//! (campbooks-cloud#364, `propose_page_change`, only while mail and Scout work
//! with pages). The three-per-answer cap and the dedupe are shared by every
//! proposing tool.

use campbooks_ai::{ToolCall, ToolDefinition};
use campbooks_kernel::RecordRef;
use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde_json::{Map, Value, json};

use super::{
    member::{Member, Unread},
    types::{
        ChatProposal, ProposalAction, ProposalAudience, ProposalDetail, ProposalEvent,
        ProposalFacts, ProposalId, ProposalKind, ProposalMethod, ProposalPage, ProposalRequest,
        ProposalState, ProposalTask,
    },
    words::{clean, cut_bytes},
};

/// Most proposals in one answer.
pub const MAX_PROPOSALS: usize = 3;
/// Shortest and longest rule sentence, in characters.
const SENTENCE_CHARS: (usize, usize) = (3, 300);
/// Shortest and longest reply intent, in characters.
const INTENT_CHARS: (usize, usize) = (3, 200);
/// Longest label name taken, in characters.
const LABEL_CHARS: usize = 64;
/// Longest teammate id or name taken, in characters.
const MEMBER_CHARS: usize = 120;
/// Most bytes of a fact (a sender, a subject).
const FACT_BYTES: usize = 300;
/// Record type of a conversation (the conversations module's).
const CONVERSATION: &str = "conversations.conversation";
/// Record type of a task (the tasks module's).
const TASK: &str = "tasks.task";
/// The furthest a day or a Later goes, as `tasks.plan` allows.
const YEAR: i64 = 365;
/// The workspace's API root, as the catalogue's paths start.
const WORKSPACE_ROOT: &str = "/api/v1/w/{workspace_id}/";
/// Record type of a calendar event (the schedule module's).
const EVENT: &str = "schedule.event";
/// Longest event title Scout may propose, in characters.
const EVENT_TITLE_CHARS: usize = 200;
/// Longest place Scout may propose, in characters.
const EVENT_PLACE_CHARS: usize = 300;
/// The streams a conversation may be moved to.
const STREAMS: [&str; 4] = ["primary", "updates", "receipts", "newsletters"];

/// The answers to an invitation a member may give.
const RESPONSES: [&str; 3] = ["accepted", "tentative", "declined"];
/// Record type of a page (the pages module's).
const PAGE: &str = "pages.page";
/// Most bytes of the Markdown of a section: the pages module's own cap
/// (`pages.append`, `pages.create`), so a proposal that could only fail on
/// confirm is refused now.
const PAGE_MARKDOWN_BYTES: usize = 16 * 1024;
/// Most characters of the Markdown the schema takes (a character may be
/// several bytes).
const PAGE_MARKDOWN_CHARS: usize = 16_000;
/// Longest page title Scout may propose, in characters.
const PAGE_TITLE_CHARS: usize = 200;
/// Most sources a section names.
const PAGE_SOURCES: usize = 10;
/// The kinds of record a section may cite, with the read that proves the
/// member can read one and the name its id goes by.
const SOURCE_READS: [(&str, &str, &str); 4] = [
    (CONVERSATION, "conversations.get", "conversation_id"),
    (TASK, "tasks.get", "task_id"),
    (EVENT, "schedule.get_event", "event_id"),
    (PAGE, "pages.read_text", "page_id"),
];

/// A proposing tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Proposing {
    /// One change to one conversation.
    Change,
    /// A sentence for Teach Scout.
    Rule,
    /// An intent for a reply draft.
    Reply,
    /// One change to one task.
    Task,
    /// One change to one calendar event.
    Event,
    /// A section for a page, or a new page (campbooks-cloud#364).
    Page,
}

impl Proposing {
    /// Every proposing tool, in the order they are offered.
    pub const ALL: [Self; 6] = [
        Self::Change,
        Self::Rule,
        Self::Reply,
        Self::Task,
        Self::Event,
        Self::Page,
    ];

    /// Whether it is the tool for pages, offered only while mail and Scout
    /// work with pages.
    pub const fn is_page(self) -> bool {
        matches!(self, Self::Page)
    }

    /// The name the model calls it by.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Change => "propose_change",
            Self::Rule => "propose_rule",
            Self::Reply => "propose_reply",
            Self::Task => "propose_task_change",
            Self::Event => "propose_event_change",
            Self::Page => "propose_page_change",
        }
    }

    /// The proposing tool named `name`, if there is one.
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tool| tool.name() == name)
    }
}

impl ProposalAction {
    /// Every action, in the order the model is offered them.
    pub const ALL: [Self; 19] = [
        Self::Archive,
        Self::Unarchive,
        Self::MarkRead,
        Self::MarkUnread,
        Self::Star,
        Self::Unstar,
        Self::AddLabel,
        Self::RemoveLabel,
        Self::MoveToStream,
        Self::Trash,
        Self::Untrash,
        Self::ReportSpam,
        Self::NotSpam,
        Self::Done,
        Self::NotNeeded,
        Self::AlreadyPaid,
        Self::StillToPay,
        Self::YesToQuestion,
        Self::Later,
    ];

    /// The task actions, in the order the model is offered them.
    pub const TASKS: [Self; 6] = [
        Self::TaskDone,
        Self::TaskNotNeeded,
        Self::PlanDay,
        Self::TaskLater,
        Self::Due,
        Self::HandOff,
    ];

    /// The event actions, in the order the model is offered them.
    pub const EVENTS: [Self; 6] = [
        Self::CreateEvent,
        Self::UpdateEvent,
        Self::DeleteEvent,
        Self::RespondEvent,
        Self::PushEvent,
        Self::UnpushEvent,
    ];

    /// The page actions, in the order the model is offered them.
    pub const PAGES: [Self; 2] = [Self::AppendSection, Self::NewPage];

    /// The page action the model names `name` (`append_section`,
    /// `new_page`), if there is one.
    pub fn page_named(name: &str) -> Option<Self> {
        Self::PAGES.into_iter().find(|a| a.name() == name)
    }

    /// The event action the model names `name` (`create`, `update`,
    /// `delete`, `respond`, `push`, `unpush`), if there is one.
    pub fn event_named(name: &str) -> Option<Self> {
        Self::EVENTS.into_iter().find(|a| a.event_name() == name)
    }

    /// An event action's name for the model.
    pub const fn event_name(self) -> &'static str {
        match self {
            Self::CreateEvent => "create",
            Self::UpdateEvent => "update",
            Self::DeleteEvent => "delete",
            Self::RespondEvent => "respond",
            Self::PushEvent => "push",
            Self::UnpushEvent => "unpush",
            _ => self.name(),
        }
    }

    /// The task action the model names `name` (`done`, `not_needed`,
    /// `plan_day`, `later`, `due`, `hand_off`), if there is one.
    pub fn task_named(name: &str) -> Option<Self> {
        Self::TASKS.into_iter().find(|a| a.task_name() == name)
    }

    /// A task action's name for the model.
    pub const fn task_name(self) -> &'static str {
        match self {
            Self::TaskDone => "done",
            Self::TaskNotNeeded => "not_needed",
            Self::PlanDay => "plan_day",
            Self::TaskLater => "later",
            Self::Due => "due",
            Self::HandOff => "hand_off",
            _ => self.name(),
        }
    }

    /// Its name, as the client says it.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Archive => "archive",
            Self::Unarchive => "unarchive",
            Self::MarkRead => "mark_read",
            Self::MarkUnread => "mark_unread",
            Self::Star => "star",
            Self::Unstar => "unstar",
            Self::AddLabel => "add_label",
            Self::RemoveLabel => "remove_label",
            Self::MoveToStream => "move_to_stream",
            Self::Trash => "trash",
            Self::Untrash => "untrash",
            Self::ReportSpam => "report_spam",
            Self::NotSpam => "not_spam",
            Self::Done => "done",
            Self::NotNeeded => "not_needed",
            Self::AlreadyPaid => "already_paid",
            Self::StillToPay => "still_to_pay",
            Self::YesToQuestion => "yes_to_question",
            Self::Later => "later",
            Self::TaskDone => "task_done",
            Self::TaskNotNeeded => "task_not_needed",
            Self::PlanDay => "plan_day",
            Self::TaskLater => "task_later",
            Self::Due => "due",
            Self::HandOff => "hand_off",
            Self::CreateEvent => "create_event",
            Self::UpdateEvent => "update_event",
            Self::DeleteEvent => "delete_event",
            Self::RespondEvent => "respond_event",
            Self::PushEvent => "push_event",
            Self::UnpushEvent => "unpush_event",
            Self::AppendSection => "append_section",
            Self::NewPage => "new_page",
        }
    }

    /// The action named `name`, if there is one.
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|action| action.name() == name)
    }

    /// Its row of the action table.
    pub fn row(self) -> &'static Row {
        // Every action has a row (tested).
        ROWS.iter()
            .find(|row| row.action == self)
            .unwrap_or(&ROWS[0])
    }
}

/// What an action's undo is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Undo {
    /// The operation its operation names in `x-campbooks.undo`, taking from
    /// the change's answer the top-level fields named (none for any
    /// conversations action).
    Catalogue {
        /// The undo's `operationId`.
        operation: &'static str,
        /// Fields of the change's answer the undo's body takes.
        from_response: &'static [&'static str],
    },
    /// The same operation with the record's values before the change, read
    /// when the proposal is checked, taking from the change's answer the
    /// top-level fields named (a task's `revision`, so a stale undo is a
    /// 409 and never overwrites a later change).
    Restoring {
        /// Fields of the change's answer the undo's body takes.
        from_response: &'static [&'static str],
    },
}

/// One row of the action table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Row {
    /// The action.
    pub action: ProposalAction,
    /// The catalogue operation that makes the change.
    pub operation: &'static str,
    /// Its undo.
    pub undo: Undo,
}

const fn row(action: ProposalAction, operation: &'static str, undo: &'static str) -> Row {
    Row {
        action,
        operation,
        undo: Undo::Catalogue {
            operation: undo,
            from_response: &[],
        },
    }
}

/// A task row whose undo is its catalogue undo, copying the `revision`.
const fn task_row(action: ProposalAction, operation: &'static str, undo: &'static str) -> Row {
    Row {
        action,
        operation,
        undo: Undo::Catalogue {
            operation: undo,
            from_response: REVISION,
        },
    }
}

/// A task row whose undo is itself with the values before, copying the
/// `revision`.
const fn restoring(action: ProposalAction, operation: &'static str) -> Row {
    Row {
        action,
        operation,
        undo: Undo::Restoring {
            from_response: REVISION,
        },
    }
}

/// The one field of a task's answer an undo takes.
const REVISION: &[&str] = &["revision"];
/// The event a change made is the one its undo deletes or takes off: its
/// `id`, in the path (`@<path parameter>:<field>`).
const NEW_EVENT: &[&str] = &["@event_id:id"];
/// The token a delete gives, which brings the event back.
const UNDO_TOKEN: &[&str] = &["undo_token"];
/// The page a new-page change made is the one its undo trashes: its `id`, in
/// the path.
const NEW_PAGE: &[&str] = &["@page_id:id"];
/// The append a section made is the one its undo takes back.
const APPEND_ID: &[&str] = &["append_id"];
/// An unpush gives the Campbooks event (`id`, the path) and the calendar it
/// was taken off (`calendar_id`, the body), which push it back.
const PUT_BACK: &[&str] = &["@event_id:id", "calendar_id"];

/// An event row whose undo is its catalogue undo, taking `from_response`
/// from the change's answer.
const fn event_row(
    action: ProposalAction,
    operation: &'static str,
    undo: &'static str,
    from_response: &'static [&'static str],
) -> Row {
    Row {
        action,
        operation,
        undo: Undo::Catalogue {
            operation: undo,
            from_response,
        },
    }
}

/// The action table: each action's operation and undo.
pub const ROWS: &[Row] = &[
    row(
        ProposalAction::Archive,
        "conversations.archive",
        "conversations.unarchive",
    ),
    row(
        ProposalAction::Unarchive,
        "conversations.unarchive",
        "conversations.archive",
    ),
    row(
        ProposalAction::MarkRead,
        "conversations.mark_read",
        "conversations.mark_unread",
    ),
    row(
        ProposalAction::MarkUnread,
        "conversations.mark_unread",
        "conversations.mark_read",
    ),
    row(
        ProposalAction::Star,
        "conversations.star",
        "conversations.unstar",
    ),
    row(
        ProposalAction::Unstar,
        "conversations.unstar",
        "conversations.star",
    ),
    row(
        ProposalAction::AddLabel,
        "conversations.add_label",
        "conversations.remove_label",
    ),
    row(
        ProposalAction::RemoveLabel,
        "conversations.remove_label",
        "conversations.add_label",
    ),
    Row {
        action: ProposalAction::MoveToStream,
        operation: "conversations.move_to_stream",
        undo: Undo::Restoring { from_response: &[] },
    },
    row(
        ProposalAction::Trash,
        "conversations.trash",
        "conversations.untrash",
    ),
    row(
        ProposalAction::Untrash,
        "conversations.untrash",
        "conversations.trash",
    ),
    row(
        ProposalAction::ReportSpam,
        "conversations.report_spam",
        "conversations.not_spam",
    ),
    row(
        ProposalAction::NotSpam,
        "conversations.not_spam",
        "conversations.report_spam",
    ),
    row(
        ProposalAction::Done,
        "conversations.resolve_need",
        "conversations.reopen_need",
    ),
    row(
        ProposalAction::NotNeeded,
        "conversations.resolve_need",
        "conversations.reopen_need",
    ),
    row(
        ProposalAction::AlreadyPaid,
        "conversations.resolve_need",
        "conversations.reopen_need",
    ),
    row(
        ProposalAction::StillToPay,
        "conversations.resolve_need",
        "conversations.reopen_need",
    ),
    row(
        ProposalAction::YesToQuestion,
        "conversations.confirm_need",
        "conversations.reopen_need",
    ),
    row(
        ProposalAction::Later,
        "attention.snooze",
        "attention.unsnooze",
    ),
    task_row(ProposalAction::TaskDone, "tasks.done", "tasks.reopen"),
    task_row(
        ProposalAction::TaskNotNeeded,
        "tasks.not_needed",
        "tasks.reopen",
    ),
    restoring(ProposalAction::PlanDay, "tasks.plan"),
    restoring(ProposalAction::TaskLater, "tasks.plan"),
    restoring(ProposalAction::Due, "tasks.update"),
    task_row(ProposalAction::HandOff, "tasks.hand_off", "tasks.take_back"),
    event_row(
        ProposalAction::CreateEvent,
        "schedule.create_event",
        "schedule.delete_event",
        NEW_EVENT,
    ),
    // The event's own values before, read when the proposal is checked.
    Row {
        action: ProposalAction::UpdateEvent,
        operation: "schedule.update_event",
        undo: Undo::Restoring { from_response: &[] },
    },
    event_row(
        ProposalAction::DeleteEvent,
        "schedule.delete_event",
        "schedule.restore_event",
        UNDO_TOKEN,
    ),
    // The answer it had, read when the proposal is checked.
    Row {
        action: ProposalAction::RespondEvent,
        operation: "schedule.respond_event",
        undo: Undo::Restoring { from_response: &[] },
    },
    event_row(
        ProposalAction::PushEvent,
        "schedule.push_event",
        "schedule.unpush_event",
        NEW_EVENT,
    ),
    event_row(
        ProposalAction::UnpushEvent,
        "schedule.unpush_event",
        "schedule.push_event",
        PUT_BACK,
    ),
    // A section at the end of a page: taken back by its append id.
    Row {
        action: ProposalAction::AppendSection,
        operation: "pages.append",
        undo: Undo::Catalogue {
            operation: "pages.unappend",
            from_response: APPEND_ID,
        },
    },
    // A new private page: trashed by the id it was given.
    Row {
        action: ProposalAction::NewPage,
        operation: "pages.create",
        undo: Undo::Catalogue {
            operation: "pages.trash",
            from_response: NEW_PAGE,
        },
    },
];

/// Why a proposal was not stored: a fixed code the model reads, and the log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// Not a proposing tool's arguments.
    InvalidArguments,
    /// The member cannot see the conversation.
    NotVisible,
    /// Their role in its inbox does not allow it.
    ReadOnly,
    /// It would change nothing.
    AlreadySo,
    /// It needs an open need of a kind the conversation does not have.
    NoSuchNeed,
    /// The label is not the inbox's, or not on the conversation.
    NoSuchLabel,
    /// `until` not later than now, or a year or more away.
    UntilOutOfRange,
    /// A task that is closed cannot be planned, edited or handed off.
    TaskClosed,
    /// `day` before the member's today, or a year or more away.
    DayOutOfRange,
    /// The teammate is the member, or not a member of the workspace.
    NotATeammate,
    /// The event waits for its provider, is a series, or sits on a calendar
    /// that cannot be written.
    EventNotChangeable,
    /// The event is not an invitation the member answers.
    NotAnInvitation,
    /// A new event's start is before now, or a year or more away.
    StartOutOfRange,
    /// The times cannot be written: a time on an all-day event, a day on a
    /// timed one, or an end before the start.
    InvalidWhen,
    /// No calendar of theirs by that id or name that an event can be added
    /// to.
    NoSuchCalendar,
    /// The member cannot see the page (or it is in the trash, or a table).
    PageNotVisible,
    /// The member may only read the page.
    PageReadOnly,
    /// A new page goes under a private page only.
    ParentNotPrivate,
    /// A source is not a record the member can read.
    SourceNotVisible,
    /// The Markdown is over the pages module's cap.
    MarkdownTooLong,
    /// The answer holds [`MAX_PROPOSALS`] already.
    TooMany,
    /// A read failed, or the operation is not here.
    Unavailable,
}

impl Refusal {
    /// The code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidArguments => "invalid_arguments",
            Self::NotVisible => "not_visible",
            Self::ReadOnly => "read_only",
            Self::AlreadySo => "already_so",
            Self::NoSuchNeed => "no_such_need",
            Self::NoSuchLabel => "no_such_label",
            Self::UntilOutOfRange => "until_out_of_range",
            Self::TaskClosed => "task_closed",
            Self::DayOutOfRange => "day_out_of_range",
            Self::NotATeammate => "not_a_teammate",
            Self::EventNotChangeable => "event_not_changeable",
            Self::NotAnInvitation => "not_an_invitation",
            Self::StartOutOfRange => "start_out_of_range",
            Self::InvalidWhen => "invalid_when",
            Self::NoSuchCalendar => "no_such_calendar",
            Self::PageNotVisible => "page_not_visible",
            Self::PageReadOnly => "page_read_only",
            Self::ParentNotPrivate => "parent_not_private",
            Self::SourceNotVisible => "source_not_visible",
            Self::MarkdownTooLong => "markdown_too_long",
            Self::TooMany => "too_many",
            Self::Unavailable => "unavailable",
        }
    }

    /// What the model is told.
    const fn message(self) -> &'static str {
        match self {
            Self::InvalidArguments => {
                "The arguments do not fit this tool's schema. Nothing was proposed."
            }
            Self::NotVisible => {
                "The member cannot see this conversation, or it does not exist. Nothing was \
                 proposed."
            }
            Self::ReadOnly => {
                "The member may only read this inbox: they cannot make this change. Nothing was \
                 proposed."
            }
            Self::AlreadySo => "It is already so: this would change nothing. Nothing was proposed.",
            Self::NoSuchNeed => {
                "The conversation has no open need this answers (Already paid and Still to pay \
                 are for a Pay need only; Yes is for a question only). Nothing was proposed."
            }
            Self::NoSuchLabel => {
                "No label by that name: add_label takes one of the inbox's own labels, \
                 remove_label one the conversation carries. Nothing was proposed."
            }
            Self::UntilOutOfRange => {
                "`until` must be later than now and less than a year away, RFC 3339 with an \
                 offset. Nothing was proposed."
            }
            Self::TaskClosed => {
                "The task is closed: it cannot be planned, edited, handed off or closed again. \
                 Nothing was proposed."
            }
            Self::DayOutOfRange => {
                "`day` must be today or later in the member's days, and less than a year \
                 away, YYYY-MM-DD. Nothing was proposed."
            }
            Self::NotATeammate => {
                "No teammate by that id or name, or it is the member themself: hand_off takes \
                 another member of the workspace. Nothing was proposed."
            }
            Self::EventNotChangeable => {
                "The event is a series, is still waiting to reach its calendar, or is on a \
                 calendar that cannot be written: it cannot be changed from here. Nothing was \
                 proposed."
            }
            Self::NotAnInvitation => {
                "The event is not an invitation the member can answer. Nothing was proposed."
            }
            Self::StartOutOfRange => {
                "`starts` must be later than now and less than a year away (RFC 3339 with an \
                 offset, or YYYY-MM-DD for a day in the member's days). Nothing was proposed."
            }
            Self::InvalidWhen => {
                "The times do not fit the event: an all-day event takes days (YYYY-MM-DD), a \
                 timed one RFC 3339 times with an offset, and it cannot end before it starts. \
                 Nothing was proposed."
            }
            Self::NoSuchCalendar => {
                "No calendar of the member's by that id or name that an event can be added to \
                 (one that is not their Campbooks calendar, can be written and is shown). \
                 Nothing was proposed."
            }
            Self::PageNotVisible => {
                "The member cannot see this page, or it does not exist, or it is in the trash, \
                 or it is a table. Nothing was proposed."
            }
            Self::PageReadOnly => {
                "The member may only read this page: they cannot add to it. Nothing was \
                 proposed."
            }
            Self::ParentNotPrivate => {
                "A new page is private, so its parent must be a private page the member can \
                 write. Nothing was proposed."
            }
            Self::SourceNotVisible => {
                "A source is not a thread, task, event or page the member can read, or it does \
                 not exist. Cite only what the tools showed you. Nothing was proposed."
            }
            Self::MarkdownTooLong => {
                "The Markdown is too long: at most 16 KiB. Shorten it. Nothing was proposed."
            }
            Self::TooMany => {
                "This answer holds three proposals already: propose no more in it. Nothing was \
                 proposed."
            }
            Self::Unavailable => "This could not be checked just now. Nothing was proposed.",
        }
    }

    /// The result the model reads.
    pub fn result(self) -> String {
        json!({ "error": self.code(), "message": self.message() }).to_string()
    }
}

/// A proposing tool's argument schema.
fn object(properties: &Value, required: &[&str]) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": properties,
        "required": required,
    })
}

/// Every proposing tool's definition, for the model.
#[allow(
    clippy::too_many_lines,
    reason = "one schema per proposing tool, read top to bottom"
)]
pub fn definitions(pages: bool) -> Vec<ToolDefinition> {
    let actions: Vec<&str> = ProposalAction::ALL.iter().map(|a| a.name()).collect();
    let task_actions: Vec<&str> = ProposalAction::TASKS
        .iter()
        .map(|a| a.task_name())
        .collect();
    let event_actions: Vec<&str> = ProposalAction::EVENTS
        .iter()
        .map(|a| a.event_name())
        .collect();
    let conversation_id = json!({
        "type": "string",
        "description": "The conversation's id, as `about` or another tool gave it.",
        "minLength": 1,
        "maxLength": 64,
    });
    let mut all = vec![
        ToolDefinition::new(
            Proposing::Change.name(),
            "Propose one change to one conversation, for the member to confirm on a card. \
             Nothing changes unless they confirm. `stream` for move_to_stream, `label` (one of \
             the inbox's labels) for add_label and remove_label, `until` (RFC 3339 with an \
             offset, in the member's time zone) for later; nothing else. done, not_needed, \
             already_paid, still_to_pay, yes_to_question and later answer the conversation's \
             open need.",
            object(
                &json!({
                    "action": {"type": "string", "enum": actions},
                    "conversation_id": conversation_id,
                    "stream": {"type": "string", "enum": STREAMS},
                    "label": {"type": "string", "minLength": 1, "maxLength": LABEL_CHARS},
                    "until": {"type": "string", "format": "date-time"},
                }),
                &["action", "conversation_id"],
            ),
        ),
        ToolDefinition::new(
            Proposing::Rule.name(),
            "Propose a rule as one plain sentence (\"Keep newsletters from The Weekly off \
             Home\"): the member reviews it in Teach Scout, which saves it only on their own \
             yes.",
            object(
                &json!({
                    "sentence": {"type": "string", "minLength": SENTENCE_CHARS.0,
                                 "maxLength": SENTENCE_CHARS.1},
                }),
                &["sentence"],
            ),
        ),
        ToolDefinition::new(
            Proposing::Reply.name(),
            "Propose a reply to a conversation, as what it should say (\"Confirm Thursday at \
             10 and ask who else is coming\"): the member opens the draft, edits it and sends \
             it, or not. Never write the reply itself here.",
            object(
                &json!({
                    "conversation_id": conversation_id,
                    "intent": {"type": "string", "minLength": INTENT_CHARS.0,
                               "maxLength": INTENT_CHARS.1},
                }),
                &["conversation_id", "intent"],
            ),
        ),
        ToolDefinition::new(
            Proposing::Task.name(),
            "Propose one change to one of the member's tasks, for the member to confirm on a \
             card. Nothing changes unless they confirm. done (paid, for a bill) and not_needed \
             close it; plan_day plans it for `day` (YYYY-MM-DD, the member's days); later puts \
             it off until `until` (RFC 3339 with an offset, in the member's time zone); due \
             sets its due day to `day`; hand_off gives it to the teammate `member_id` (the \
             member's user id, or their full name as the workspace lists it). Nothing else is \
             sent with an action. This cannot create or rename a task.",
            object(
                &json!({
                    "action": {"type": "string", "enum": task_actions},
                    "task_id": {"type": "string", "minLength": 1, "maxLength": 64,
                                "description": "The task's id, as `about` or another tool gave it."},
                    "day": {"type": "string", "minLength": 10, "maxLength": 10,
                            "description": "YYYY-MM-DD, for plan_day and due."},
                    "until": {"type": "string", "format": "date-time"},
                    "member_id": {"type": "string", "minLength": 1, "maxLength": MEMBER_CHARS},
                }),
                &["action", "task_id"],
            ),
        ),
        ToolDefinition::new(
            Proposing::Event.name(),
            "Propose one change to the member's calendar, for the member to confirm on a card. \
             Nothing changes unless they confirm. create makes an event (`title` and `starts`, \
             optionally `ends` and `place`) on their Campbooks calendar; update changes the \
             event `event_id` (any of `title`, `starts`, `ends`, `place`; never a series); delete \
             removes it; respond answers an invitation with `response`; push adds an event of \
             their Campbooks calendar to the provider calendar `calendar` (its id or name); \
             unpush takes such a copy off the provider calendar again. `starts` and `ends` are \
             RFC 3339 with an offset, in the member's time zone, or YYYY-MM-DD for an all-day \
             event (the end is then the day after the last). Nothing else is sent with an \
             action.",
            object(
                &json!({
                    "action": {"type": "string", "enum": event_actions},
                    "event_id": {"type": "string", "minLength": 1, "maxLength": 64,
                                 "description": "The event's id, as `my_calendar` gave it."},
                    "title": {"type": "string", "minLength": 1,
                              "maxLength": EVENT_TITLE_CHARS},
                    "starts": {"type": "string", "minLength": 10, "maxLength": 40},
                    "ends": {"type": "string", "minLength": 10, "maxLength": 40},
                    "place": {"type": "string", "minLength": 1,
                              "maxLength": EVENT_PLACE_CHARS},
                    "response": {"type": "string", "enum": RESPONSES},
                    "calendar": {"type": "string", "minLength": 1, "maxLength": MEMBER_CHARS},
                }),
                &["action"],
            ),
        ),
    ];
    if pages {
        all.push(page_definition());
    }
    all
}

/// `propose_page_change`'s definition.
fn page_definition() -> ToolDefinition {
    let page_actions: Vec<&str> = ProposalAction::PAGES.iter().map(|a| a.name()).collect();
    let sources: Vec<&str> = SOURCE_READS.iter().map(|(kind, ..)| *kind).collect();
    ToolDefinition::new(
        Proposing::Page.name(),
        "Propose adding to the member's pages, for the member to confirm on a card. Nothing \
         changes unless they confirm, and what you propose is only added: you cannot change or \
         remove what is on a page. append_section adds a section at the end of the page \
         `page_id` (one the member can write); new_page makes a new private page with `title` \
         (and, if the member asked, under the private page `parent_id`). Both take `markdown`, \
         the section in plain Markdown (headings, short paragraphs, bullet and numbered lists, \
         to-dos, quotes, code, links; no tables and no HTML; at most 16 KiB), and `sources`: \
         the threads, tasks, events or pages you drew on, each with its id as the tools gave \
         it. Nothing else is sent with an action.",
        object(
            &json!({
                "action": {"type": "string", "enum": page_actions},
                "page_id": {"type": "string", "minLength": 1, "maxLength": 64,
                            "description": "The page's id, as `find_pages` or `about` gave it."},
                "title": {"type": "string", "minLength": 1, "maxLength": PAGE_TITLE_CHARS},
                "parent_id": {"type": "string", "minLength": 1, "maxLength": 64,
                              "description": "A private page the new page goes under."},
                "markdown": {"type": "string", "minLength": 1, "maxLength": PAGE_MARKDOWN_CHARS},
                "sources": {
                    "type": "array",
                    "maxItems": PAGE_SOURCES,
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "type": {"type": "string", "enum": sources},
                            "id": {"type": "string", "minLength": 1, "maxLength": 64},
                        },
                        "required": ["type", "id"],
                    },
                },
            }),
            &["action", "markdown"],
        ),
    )
}

/// A proposing call's arguments, checked against its schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asked {
    /// `propose_change`.
    Change {
        action: ProposalAction,
        conversation: String,
        stream: Option<String>,
        label: Option<String>,
        until: Option<String>,
    },
    /// `propose_rule`.
    Rule { sentence: String },
    /// `propose_reply`.
    Reply {
        conversation: String,
        intent: String,
    },
    /// `propose_task_change`.
    Task {
        action: ProposalAction,
        task: String,
        day: Option<NaiveDate>,
        until: Option<String>,
        member: Option<String>,
    },
    /// `propose_event_change`.
    Event {
        action: ProposalAction,
        event: Option<String>,
        title: Option<String>,
        starts: Option<String>,
        ends: Option<String>,
        place: Option<String>,
        response: Option<String>,
        calendar: Option<String>,
    },
    /// `propose_page_change`.
    Page {
        action: ProposalAction,
        page: Option<String>,
        title: Option<String>,
        parent: Option<String>,
        markdown: String,
        sources: Vec<RecordRef>,
    },
}

/// A start or an end: a day for an all-day event, else a moment.
enum When {
    Day(NaiveDate),
    At(DateTime<Utc>),
}

impl When {
    /// `text` as an RFC 3339 time with an offset, or as `YYYY-MM-DD`.
    fn parse(text: &str) -> Option<Self> {
        match DateTime::parse_from_rfc3339(text) {
            Ok(at) => Some(Self::At(at.with_timezone(&Utc))),
            Err(_) => NaiveDate::parse_from_str(text, "%Y-%m-%d")
                .ok()
                .map(Self::Day),
        }
    }

    /// Whether `self` is before `other`, both of the same form.
    fn before(&self, other: &Self) -> Option<bool> {
        match (self, other) {
            (Self::Day(a), Self::Day(b)) => Some(a < b),
            (Self::At(a), Self::At(b)) => Some(a < b),
            _ => None,
        }
    }
}

/// Whether the event arguments are the ones `action` takes, and no others.
fn event_shape(action: ProposalAction, given: [bool; 8]) -> bool {
    // event, title, starts, ends, place, response, calendar, (unused)
    let [event, title, starts, ends, place, response, calendar, _] = given;
    match action {
        ProposalAction::CreateEvent => !event && title && starts && !response && !calendar,
        ProposalAction::UpdateEvent => {
            event && (title || starts || ends || place) && !response && !calendar
        }
        ProposalAction::DeleteEvent | ProposalAction::UnpushEvent => {
            event && !(title || starts || ends || place || response || calendar)
        }
        ProposalAction::RespondEvent => {
            event && response && !(title || starts || ends || place || calendar)
        }
        ProposalAction::PushEvent => {
            event && calendar && !(title || starts || ends || place || response)
        }
        _ => false,
    }
}

/// A section's sources as the model gave them: at most ten, each a record of
/// a kind a section may cite with a plain id, each once; none when absent.
fn page_sources(given: Option<&Value>) -> Option<Vec<RecordRef>> {
    let Some(given) = given else {
        return Some(Vec::new());
    };
    let items = given
        .as_array()
        .filter(|items| items.len() <= PAGE_SOURCES)?;
    let mut sources: Vec<RecordRef> = Vec::new();
    for item in items {
        let fields = item.as_object()?;
        if !fields.keys().all(|key| key == "type" || key == "id") {
            return None;
        }
        let kind = fields.get("type")?.as_str()?;
        let id = fields.get("id")?.as_str()?;
        if !SOURCE_READS.iter().any(|(allowed, ..)| *allowed == kind) || !safe_segment(id) {
            return None;
        }
        let source = RecordRef::new(kind, id);
        if !sources.contains(&source) {
            sources.push(source);
        }
    }
    Some(sources)
}

/// `raw`'s arguments when they fit `tool`'s schema: only its properties,
/// each of the right type and length, and for a change only the detail its
/// action takes.
#[allow(
    clippy::too_many_lines,
    reason = "one arm per proposing tool, read top to bottom"
)]
pub fn arguments(tool: Proposing, raw: &str) -> Option<Asked> {
    let raw = raw.trim();
    let object: Map<String, Value> = if raw.is_empty() {
        Map::new()
    } else {
        match serde_json::from_str::<Value>(raw).ok()? {
            Value::Object(object) => object,
            _ => return None,
        }
    };
    // A null optional argument is as if it were absent.
    let object: Map<String, Value> = object.into_iter().filter(|(_, v)| !v.is_null()).collect();
    let text = |key: &str, (min, max): (usize, usize)| -> Option<Option<String>> {
        match object.get(key) {
            None => Some(None),
            Some(value) => {
                let cleaned = clean(value.as_str()?);
                let chars = cleaned.chars().count();
                (min..=max).contains(&chars).then_some(Some(cleaned))
            }
        }
    };
    let allowed: &[&str] = match tool {
        Proposing::Change => &["action", "conversation_id", "stream", "label", "until"],
        Proposing::Rule => &["sentence"],
        Proposing::Reply => &["conversation_id", "intent"],
        Proposing::Task => &["action", "task_id", "day", "until", "member_id"],
        Proposing::Event => &[
            "action", "event_id", "title", "starts", "ends", "place", "response", "calendar",
        ],
        Proposing::Page => &[
            "action",
            "page_id",
            "title",
            "parent_id",
            "markdown",
            "sources",
        ],
    };
    if !object.keys().all(|key| allowed.contains(&key.as_str())) {
        return None;
    }
    match tool {
        Proposing::Change => {
            let action = ProposalAction::named(object.get("action")?.as_str()?)?;
            let conversation = text("conversation_id", (1, 64))??;
            let stream = text("stream", (1, 32))?;
            if stream.as_deref().is_some_and(|s| !STREAMS.contains(&s)) {
                return None;
            }
            let label = text("label", (1, LABEL_CHARS))?;
            let until = text("until", (1, 64))?;
            let takes = (
                action == ProposalAction::MoveToStream,
                matches!(
                    action,
                    ProposalAction::AddLabel | ProposalAction::RemoveLabel
                ),
                action == ProposalAction::Later,
            );
            if takes != (stream.is_some(), label.is_some(), until.is_some()) {
                return None;
            }
            Some(Asked::Change {
                action,
                conversation,
                stream,
                label,
                until,
            })
        }
        Proposing::Task => {
            let action = ProposalAction::task_named(object.get("action")?.as_str()?)?;
            let task = text("task_id", (1, 64))??;
            let day = match text("day", (10, 10))? {
                None => None,
                Some(day) => Some(NaiveDate::parse_from_str(&day, "%Y-%m-%d").ok()?),
            };
            let until = text("until", (1, 64))?;
            let member = text("member_id", (1, MEMBER_CHARS))?;
            let takes = (
                matches!(action, ProposalAction::PlanDay | ProposalAction::Due),
                action == ProposalAction::TaskLater,
                action == ProposalAction::HandOff,
            );
            if takes != (day.is_some(), until.is_some(), member.is_some()) {
                return None;
            }
            Some(Asked::Task {
                action,
                task,
                day,
                until,
                member,
            })
        }
        Proposing::Event => {
            let action = ProposalAction::event_named(object.get("action")?.as_str()?)?;
            let event = text("event_id", (1, 64))?;
            let title = text("title", (1, EVENT_TITLE_CHARS))?;
            let starts = text("starts", (10, 40))?;
            let ends = text("ends", (10, 40))?;
            let place = text("place", (1, EVENT_PLACE_CHARS))?;
            let response = text("response", (1, 16))?;
            let calendar = text("calendar", (1, MEMBER_CHARS))?;
            if response.as_deref().is_some_and(|r| !RESPONSES.contains(&r)) {
                return None;
            }
            let first = starts
                .as_deref()
                .map(When::parse)
                .map(|w| w.ok_or(()))
                .transpose()
                .ok()?;
            let last = ends
                .as_deref()
                .map(When::parse)
                .map(|w| w.ok_or(()))
                .transpose()
                .ok()?;
            if let (Some(first), Some(last)) = (&first, &last)
                && !first.before(last)?
            {
                return None;
            }
            let given = [
                event.is_some(),
                title.is_some(),
                starts.is_some(),
                ends.is_some(),
                place.is_some(),
                response.is_some(),
                calendar.is_some(),
                false,
            ];
            if !event_shape(action, given) {
                return None;
            }
            Some(Asked::Event {
                action,
                event,
                title,
                starts,
                ends,
                place,
                response,
                calendar,
            })
        }
        Proposing::Page => {
            let action = ProposalAction::page_named(object.get("action")?.as_str()?)?;
            let page = text("page_id", (1, 64))?;
            let title = text("title", (1, PAGE_TITLE_CHARS))?;
            let parent = text("parent_id", (1, 64))?;
            let markdown = text("markdown", (1, PAGE_MARKDOWN_CHARS))??;
            let sources = page_sources(object.get("sources"))?;
            let shape = match action {
                ProposalAction::AppendSection => {
                    page.is_some() && title.is_none() && parent.is_none()
                }
                ProposalAction::NewPage => page.is_none() && title.is_some(),
                _ => false,
            };
            shape.then_some(Asked::Page {
                action,
                page,
                title,
                parent,
                markdown,
                sources,
            })
        }
        Proposing::Rule => Some(Asked::Rule {
            sentence: text("sentence", SENTENCE_CHARS)??,
        }),
        Proposing::Reply => Some(Asked::Reply {
            conversation: text("conversation_id", (1, 64))??,
            intent: text("intent", INTENT_CHARS)??,
        }),
    }
}

/// One answer's proposals: the member they are checked as, and those kept.
#[derive(Debug)]
pub struct Proposer {
    member: Member,
    /// The member's offset from UTC in minutes: where their day starts.
    utc_offset: i32,
    /// The proposals kept, in order.
    pub proposals: Vec<ChatProposal>,
    /// For the log: each refusal's code, in order.
    pub refusals: Vec<&'static str>,
}

/// What a proposing call answers the model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposed {
    /// The result, as the model reads it.
    pub result: String,
    /// For the log: a fixed word.
    pub kind: &'static str,
}

impl Proposer {
    /// Proposals checked as `member`, whose day starts `utc_offset` minutes
    /// from UTC's.
    pub fn new(member: Member, utc_offset: i32) -> Self {
        Self {
            member,
            utc_offset,
            proposals: Vec::new(),
            refusals: Vec::new(),
        }
    }

    /// Check `call` and keep its proposal, or say why not. Never runs a
    /// change.
    pub async fn propose(&mut self, tool: Proposing, call: &ToolCall) -> Proposed {
        match self.checked(tool, call).await {
            Ok((proposal, fresh)) => {
                let result = json!({
                    "proposed": proposal.id,
                    "message": if fresh {
                        "A card under your answer asks the member to confirm this. Nothing has \
                         changed: say plainly what you proposed, and never that it is done."
                    } else {
                        "You proposed exactly this already in this answer: it is one card."
                    },
                })
                .to_string();
                if fresh {
                    self.proposals.push(proposal);
                }
                Proposed {
                    result,
                    kind: if fresh { "proposed" } else { "duplicate" },
                }
            }
            Err(refusal) => {
                self.refusals.push(refusal.code());
                Proposed {
                    result: refusal.result(),
                    kind: refusal.code(),
                }
            }
        }
    }

    /// The proposal `call` makes, and whether it is a new one (not one of
    /// this answer's already).
    #[allow(clippy::too_many_lines)]
    async fn checked(
        &self,
        tool: Proposing,
        call: &ToolCall,
    ) -> Result<(ChatProposal, bool), Refusal> {
        let asked = arguments(tool, &call.arguments).ok_or(Refusal::InvalidArguments)?;
        let full = self.proposals.len() >= MAX_PROPOSALS;
        let proposal =
            match &asked {
                Asked::Rule { sentence } => {
                    if let Some(same) = self.proposals.iter().find(|p| {
                        p.kind == ProposalKind::Rule && p.sentence.as_ref() == Some(sentence)
                    }) {
                        return Ok((same.clone(), false));
                    }
                    if full {
                        return Err(Refusal::TooMany);
                    }
                    ChatProposal {
                        id: ProposalId::new(),
                        kind: ProposalKind::Rule,
                        state: ProposalState::Pending,
                        action: None,
                        record: None,
                        facts: None,
                        task: None,
                        event: None,
                        detail: None,
                        request: None,
                        undo: None,
                        page: None,
                        sentence: Some(sentence.clone()),
                        intent: None,
                    }
                }
                Asked::Reply {
                    conversation,
                    intent,
                } => {
                    if full && !self.holds(&asked) {
                        return Err(Refusal::TooMany);
                    }
                    let seen = self.read(conversation).await?;
                    if seen.role.writes() == Some(false) {
                        return Err(Refusal::ReadOnly);
                    }
                    ChatProposal {
                        id: ProposalId::new(),
                        kind: ProposalKind::Reply,
                        state: ProposalState::Pending,
                        action: None,
                        record: Some(RecordRef::new(CONVERSATION, &seen.id)),
                        facts: Some(seen.facts()),
                        task: None,
                        event: None,
                        detail: None,
                        request: None,
                        undo: None,
                        page: None,
                        sentence: None,
                        intent: Some(intent.clone()),
                    }
                }
                Asked::Change {
                    action,
                    conversation,
                    stream,
                    label,
                    until,
                } => {
                    if full && !self.holds(&asked) {
                        return Err(Refusal::TooMany);
                    }
                    let until = until.as_deref().map(until_within).transpose()?;
                    let seen = self.read(conversation).await?;
                    if *action != ProposalAction::Later && seen.role.writes() == Some(false) {
                        return Err(Refusal::ReadOnly);
                    }
                    let built = self
                        .build(*action, &seen, stream.as_deref(), label.as_deref(), until)
                        .await?;
                    ChatProposal {
                        id: ProposalId::new(),
                        kind: ProposalKind::Change,
                        state: ProposalState::Pending,
                        action: Some(*action),
                        record: Some(RecordRef::new(CONVERSATION, &seen.id)),
                        facts: Some(seen.facts()),
                        task: None,
                        event: None,
                        detail: Some(built.detail),
                        request: Some(built.request),
                        undo: Some(built.undo),
                        page: None,
                        sentence: None,
                        intent: None,
                    }
                }
                Asked::Task {
                    action,
                    task,
                    day,
                    until,
                    member,
                } => {
                    if full && !self.holds(&asked) {
                        return Err(Refusal::TooMany);
                    }
                    self.task_proposal(*action, task, *day, until.as_deref(), member.as_deref())
                        .await?
                }
                Asked::Event { .. } => {
                    if full && !self.holds(&asked) {
                        return Err(Refusal::TooMany);
                    }
                    self.event_proposal(&asked).await?
                }
                Asked::Page { .. } => {
                    if full && !self.holds(&asked) {
                        return Err(Refusal::TooMany);
                    }
                    self.page_proposal(&asked).await?
                }
            };
        if let Some(same) = self.proposals.iter().find(|p| same_card(p, &proposal)) {
            return Ok((same.clone(), false));
        }
        if full {
            return Err(Refusal::TooMany);
        }
        Ok((proposal, true))
    }

    /// Whether this answer holds a proposal the model's own arguments name
    /// again (before anything is read).
    fn holds(&self, asked: &Asked) -> bool {
        self.proposals.iter().any(|p| match asked {
            Asked::Change {
                action,
                conversation,
                stream,
                label,
                until: _,
            } => {
                p.action == Some(*action)
                    && p.record.as_ref().is_some_and(|r| r.id == *conversation)
                    && p.detail.as_ref().is_some_and(|d| {
                        d.stream == *stream
                            && d.label.as_deref().map(str::to_lowercase)
                                == label.as_deref().map(str::to_lowercase)
                    })
            }
            Asked::Reply {
                conversation,
                intent,
            } => {
                p.kind == ProposalKind::Reply
                    && p.record.as_ref().is_some_and(|r| r.id == *conversation)
                    && p.intent.as_ref() == Some(intent)
            }
            Asked::Rule { sentence } => p.sentence.as_ref() == Some(sentence),
            Asked::Task {
                action,
                task,
                day,
                until,
                member: _,
            } => {
                // Only a rough match, to spare a full answer a refusal it
                // does not deserve: the exact check is `same_card`, in
                // `checked`, after the proposal is built.
                let until = until.as_deref().and_then(|u| until_within(u).ok());
                p.action == Some(*action)
                    && p.record.as_ref().is_some_and(|r| r.id == *task)
                    && p.detail
                        .as_ref()
                        .is_some_and(|d| d.day == *day && d.until == until)
            }
            Asked::Page {
                action,
                page,
                title,
                markdown,
                ..
            } => {
                // Rough, like a task's: the exact check is `same_card`.
                p.action == Some(*action)
                    && p.record.as_ref().map(|r| r.id.as_str()) == page.as_deref()
                    && p.detail
                        .as_ref()
                        .is_some_and(|d| d.title == *title && d.markdown.as_ref() == Some(markdown))
            }
            Asked::Event {
                action,
                event,
                title,
                starts,
                response,
                ..
            } => {
                // Rough, like a task's: the exact check is `same_card`.
                p.action == Some(*action)
                    && p.record.as_ref().map(|r| r.id.as_str()) == event.as_deref()
                    && p.detail.as_ref().is_some_and(|d| {
                        d.title == *title && d.starts == *starts && d.response == *response
                    })
            }
        })
    }

    /// The conversation as the member sees it, and their role in its inbox.
    async fn read(&self, conversation: &str) -> Result<Seen, Refusal> {
        let detail = self
            .member
            .read(
                "conversations.get",
                json!({ "conversation_id": conversation }),
            )
            .await
            .map_err(|unread| match unread {
                Unread::NotVisible | Unread::Refused => Refusal::NotVisible,
                Unread::Broken(_) => Refusal::Unavailable,
            })?;
        let head = &detail["conversation"];
        let id = head["id"]
            .as_str()
            .filter(|id| safe_segment(id))
            .ok_or(Refusal::Unavailable)?
            .to_owned();
        let inboxes = self
            .member
            .read("conversations.list_inboxes", Value::Null)
            .await
            .map_err(|_| Refusal::Unavailable)?;
        let inbox = inboxes["inboxes"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|inbox| inbox["id"] == head["inbox_id"]);
        let role = match inbox {
            None => InboxAccess::Unknown,
            Some(inbox) if inbox["read_only"] == true || inbox["role"] == "reader" => {
                InboxAccess::ReadsOnly
            }
            Some(_) => InboxAccess::Writes,
        };
        // Who wrote its newest message in, as the reader names them.
        let inbound = detail["messages"]
            .as_array()
            .into_iter()
            .flatten()
            .rev()
            .find(|message| message["direction"] == "inbound")
            .map(|message| &message["from"]);
        let from = inbound.unwrap_or(&head["latest_from"]);
        let sender = from["name"]
            .as_str()
            .map(clean)
            .filter(|name| !name.is_empty())
            .or_else(|| from["address"].as_str().map(clean))
            .unwrap_or_default();
        Ok(Seen {
            id,
            sender: fact(&sender),
            subject: fact(&head["subject"].as_str().map(clean).unwrap_or_default()),
            head: head.clone(),
            role,
        })
    }

    /// A change's detail, request and undo, after checking it changes
    /// something the conversation has.
    #[allow(clippy::too_many_lines)]
    async fn build(
        &self,
        action: ProposalAction,
        seen: &Seen,
        stream: Option<&str>,
        label: Option<&str>,
        until: Option<DateTime<Utc>>,
    ) -> Result<Built, Refusal> {
        use ProposalAction as A;
        let head = &seen.head;
        let flag = |key: &str| head[key] == true;
        let unread = head["unread_count"].as_u64().unwrap_or(0) > 0;
        let need = &head["need"];
        let mut params = vec![("conversation_id", seen.id.clone())];
        let mut detail = ProposalDetail::default();
        let mut body: Option<Value> = None;
        let mut undo_body: Option<Value> = None;
        let mut undo_params: Option<Vec<(&'static str, String)>> = None;
        let already = match action {
            A::Archive => flag("archived"),
            A::Unarchive => !flag("archived"),
            A::MarkRead => !unread,
            A::MarkUnread => unread,
            A::Star => flag("starred"),
            A::Unstar => !flag("starred"),
            A::Trash => flag("trashed") && !flag("spam"),
            A::Untrash => !flag("trashed") || flag("spam"),
            A::ReportSpam => flag("spam"),
            A::NotSpam => !flag("spam"),
            A::MoveToStream => {
                let to = stream.ok_or(Refusal::InvalidArguments)?;
                let now = head["stream"].as_str().ok_or(Refusal::Unavailable)?;
                detail.stream = Some(to.to_owned());
                body = Some(json!({ "stream": to }));
                // The restoring row: back to the stream it is in now.
                undo_body = Some(json!({ "stream": now }));
                now == to
            }
            A::AddLabel | A::RemoveLabel => {
                let asked = label.ok_or(Refusal::InvalidArguments)?.to_lowercase();
                let named = |l: &&Value| {
                    l["name"]
                        .as_str()
                        .is_some_and(|name| name.trim().to_lowercase() == asked)
                };
                let carried = head["labels"].as_array().into_iter().flatten().find(named);
                let (id, name) = if action == A::RemoveLabel {
                    let carried = carried.ok_or(Refusal::NoSuchLabel)?;
                    (carried["id"].clone(), carried["name"].clone())
                } else {
                    // One of the inbox's own labels: its id is the undo's.
                    let labels = self
                        .member
                        .read("conversations.list_labels", Value::Null)
                        .await
                        .map_err(|_| Refusal::Unavailable)?;
                    let known = labels["labels"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter(|l| l["inbox"] == head["inbox_id"])
                        .find(named)
                        .ok_or(Refusal::NoSuchLabel)?;
                    (known["id"].clone(), known["name"].clone())
                };
                let id = id
                    .as_str()
                    .filter(|id| safe_segment(id))
                    .ok_or(Refusal::Unavailable)?
                    .to_owned();
                let name = name.as_str().map(fact).ok_or(Refusal::Unavailable)?;
                detail.label = Some(name.clone());
                if action == A::RemoveLabel {
                    params.push(("label_id", id));
                    undo_body = Some(json!({ "name": name }));
                } else {
                    body = Some(json!({ "name": name }));
                    let mut back = params.clone();
                    back.push(("label_id", id));
                    undo_params = Some(back);
                }
                action == A::AddLabel && carried.is_some()
            }
            A::Done | A::NotNeeded | A::AlreadyPaid | A::StillToPay | A::YesToQuestion => {
                if need.is_null() {
                    return Err(Refusal::NoSuchNeed);
                }
                let pay = need["reason"] == "pay";
                let question = need["trust"] == "check";
                match action {
                    A::AlreadyPaid | A::StillToPay if !pay => return Err(Refusal::NoSuchNeed),
                    A::YesToQuestion if !question => return Err(Refusal::NoSuchNeed),
                    _ => {}
                }
                let message = need["message_id"]
                    .as_str()
                    .ok_or(Refusal::NoSuchNeed)?
                    .to_owned();
                body = Some(match action {
                    A::YesToQuestion => json!({ "message_id": message }),
                    _ => json!({ "outcome": action.name(), "message_id": message }),
                });
                // Still to pay on a bill already confirmed changes nothing.
                action == A::StillToPay && !question
            }
            A::Later => {
                let until = until.ok_or(Refusal::InvalidArguments)?;
                if need.is_null() {
                    return Err(Refusal::NoSuchNeed);
                }
                let item = self.open_item(&seen.id).await?;
                detail.until = Some(until);
                params = vec![("item_id", item)];
                body = Some(json!({ "until": until.to_rfc3339() }));
                false
            }
            // Task actions are built by `task_proposal`, never here.
            A::TaskDone | A::TaskNotNeeded | A::PlanDay | A::TaskLater | A::Due | A::HandOff => {
                return Err(Refusal::InvalidArguments);
            }
            // Event actions are built by `event_proposal`, and page actions by
            // `page_proposal`, never here.
            A::CreateEvent
            | A::UpdateEvent
            | A::DeleteEvent
            | A::RespondEvent
            | A::PushEvent
            | A::UnpushEvent
            | A::AppendSection
            | A::NewPage => return Err(Refusal::InvalidArguments),
        };
        if already {
            return Err(Refusal::AlreadySo);
        }
        let row = action.row();
        let request = self.request(row.operation, &params, body, &[])?;
        let undo = match row.undo {
            Undo::Restoring { from_response } => {
                self.request(row.operation, &params, undo_body, from_response)?
            }
            Undo::Catalogue {
                operation,
                from_response,
            } => self.request(
                operation,
                undo_params.as_deref().unwrap_or(params.as_slice()),
                undo_body,
                from_response,
            )?,
        };
        Ok(Built {
            detail,
            request,
            undo,
        })
    }

    /// A task change's proposal, after reading the task as the member and
    /// checking it changes something, for someone who may change it.
    #[allow(clippy::too_many_lines)]
    async fn task_proposal(
        &self,
        action: ProposalAction,
        task: &str,
        day: Option<NaiveDate>,
        until: Option<&str>,
        member: Option<&str>,
    ) -> Result<ChatProposal, Refusal> {
        use ProposalAction as A;
        let now = Utc::now();
        // The bounds first: they need no read.
        let day = day
            .map(|day| day_within(day, now, self.utc_offset))
            .transpose()?;
        let until = until.map(until_within).transpose()?;
        let seen = self.read_task(task).await?;
        let (me, people) = self.members().await?;
        // The giver of a task reads it and takes it back: no more.
        if seen.assignee != me {
            return Err(Refusal::ReadOnly);
        }
        let open = seen.state == "open";
        let mut detail = ProposalDetail::default();
        let mut body = json!({ "revision": seen.revision });
        let mut undo_body: Option<Value> = None;
        match action {
            A::TaskDone | A::TaskNotNeeded => {
                let closes = if action == A::TaskDone {
                    "done"
                } else {
                    "not_needed"
                };
                if seen.state == closes {
                    return Err(Refusal::AlreadySo);
                }
                if !open {
                    return Err(Refusal::TaskClosed);
                }
            }
            A::PlanDay => {
                let day = day.ok_or(Refusal::InvalidArguments)?;
                if !open {
                    return Err(Refusal::TaskClosed);
                }
                if seen.plan == "date" && seen.plan_on == Some(day) {
                    return Err(Refusal::AlreadySo);
                }
                detail.day = Some(day);
                body["plan"] = "date".into();
                body["plan_on"] = day.to_string().into();
                undo_body = Some(seen.plan_before(now));
            }
            A::TaskLater => {
                let until = until.ok_or(Refusal::InvalidArguments)?;
                if !open {
                    return Err(Refusal::TaskClosed);
                }
                if seen.plan == "later" && seen.later_until == Some(until) {
                    return Err(Refusal::AlreadySo);
                }
                detail.until = Some(until);
                body["plan"] = "later".into();
                body["later_until"] = until.to_rfc3339().into();
                undo_body = Some(seen.plan_before(now));
            }
            A::Due => {
                let day = day.ok_or(Refusal::InvalidArguments)?;
                if !open {
                    return Err(Refusal::TaskClosed);
                }
                if seen.due_on == Some(day) && seen.due_at.is_none() {
                    return Err(Refusal::AlreadySo);
                }
                detail.day = Some(day);
                // A full replacement: the record's own title and verb, the
                // new day, no time of day.
                body["title"] = seen.title.clone().into();
                body["verb"] = seen.verb.clone().into();
                body["due_on"] = day.to_string().into();
                undo_body = Some(json!({
                    "title": seen.title,
                    "verb": seen.verb,
                    "due_on": seen.due_on.map(|d| d.to_string()),
                    "due_at": seen.due_at.map(|at| at.to_rfc3339()),
                }));
            }
            A::HandOff => {
                let asked = member.ok_or(Refusal::InvalidArguments)?;
                if !open {
                    return Err(Refusal::TaskClosed);
                }
                // A teammate by id, else by the full name the list gives;
                // the card's name is the list's, never the model's.
                let lowered = asked.trim().to_lowercase();
                let by_id = people.iter().find(|p| p.id == asked.trim());
                let named: Vec<&Teammate> = people
                    .iter()
                    .filter(|p| {
                        p.name.to_lowercase() == lowered
                            || p.email.as_deref().map(str::to_lowercase) == Some(lowered.clone())
                    })
                    .collect();
                let teammate = by_id
                    .or_else(|| (named.len() == 1).then(|| named[0]))
                    .ok_or(Refusal::NotATeammate)?;
                if teammate.id == me || !safe_segment(&teammate.id) {
                    return Err(Refusal::NotATeammate);
                }
                detail.member_id = Some(teammate.id.clone());
                detail.member_name = Some(teammate.name.clone());
                body["to"] = teammate.id.clone().into();
            }
            _ => return Err(Refusal::InvalidArguments),
        }
        let row = action.row();
        let params = [("task_id", seen.id.clone())];
        let request = self.request(row.operation, &params, Some(body), &[])?;
        let undo = match row.undo {
            Undo::Restoring { from_response } => {
                self.request(row.operation, &params, undo_body, from_response)?
            }
            Undo::Catalogue {
                operation,
                from_response,
            } => self.request(operation, &params, None, from_response)?,
        };
        Ok(ChatProposal {
            id: ProposalId::new(),
            kind: ProposalKind::Change,
            state: ProposalState::Pending,
            action: Some(action),
            record: Some(RecordRef::new(TASK, &seen.id)),
            facts: None,
            task: Some(ProposalTask {
                title: fact(&seen.title),
                verb: seen.verb.clone(),
                due_on: seen.due_on,
            }),
            event: None,
            detail: Some(detail),
            request: Some(request),
            undo: Some(undo),
            page: None,
            sentence: None,
            intent: None,
        })
    }

    /// An event change's proposal, after reading the event and the member's
    /// calendars as the member and checking it changes something, for
    /// someone who may change it. The card's facts are the record's (and,
    /// for a push, the calendar list's), never the model's words.
    #[allow(clippy::too_many_lines)]
    async fn event_proposal(&self, asked: &Asked) -> Result<ChatProposal, Refusal> {
        use ProposalAction as A;
        let Asked::Event {
            action,
            event,
            title,
            starts,
            ends,
            place,
            response,
            calendar,
        } = asked
        else {
            return Err(Refusal::InvalidArguments);
        };
        let action = *action;
        let row = action.row();
        let mut detail = ProposalDetail {
            title: title.clone(),
            starts: starts.clone(),
            ends: ends.clone(),
            place: place.clone(),
            response: response.clone(),
            ..ProposalDetail::default()
        };
        if action == A::CreateEvent {
            event_start_within(
                starts.as_deref().ok_or(Refusal::InvalidArguments)?,
                Utc::now(),
                self.utc_offset,
            )?;
            let mut body = json!({
                "title": title.as_deref().ok_or(Refusal::InvalidArguments)?,
                "starts": starts.as_deref().ok_or(Refusal::InvalidArguments)?,
            });
            if let Some(ends) = ends {
                body["ends"] = ends.clone().into();
            }
            if let Some(place) = place {
                body["place"] = place.clone().into();
            }
            let request = self.request(row.operation, &[], Some(body), &[])?;
            let undo = self.row_undo(row, &[], None)?;
            return Ok(ChatProposal {
                id: ProposalId::new(),
                kind: ProposalKind::Change,
                state: ProposalState::Pending,
                action: Some(action),
                record: None,
                facts: None,
                task: None,
                event: None,
                detail: Some(detail),
                request: Some(request),
                undo: Some(undo),
                page: None,
                sentence: None,
                intent: None,
            });
        }
        let seen = self
            .read_event(event.as_deref().ok_or(Refusal::InvalidArguments)?)
            .await?;
        let calendars = self.calendars().await?;
        let here = calendars.iter().find(|c| c.id == seen.calendar_id);
        // An answer that has not reached the provider yet is never stacked on.
        if seen.pending {
            return Err(Refusal::EventNotChangeable);
        }
        let params = [("event_id", seen.id.clone())];
        let mut body: Option<Value> = None;
        let mut undo_body: Option<Value> = None;
        match action {
            A::UpdateEvent => {
                let here = here.ok_or(Refusal::Unavailable)?;
                if seen.recurring || !here.writes() {
                    return Err(Refusal::EventNotChangeable);
                }
                let (now, before) = event_changes(
                    &seen,
                    title.as_deref(),
                    starts.as_deref(),
                    ends.as_deref(),
                    place.as_deref(),
                )?;
                // What the card says is what is sent, not what was asked.
                detail.title = now.get("title").and_then(Value::as_str).map(fact);
                detail.starts = now.get("starts").and_then(Value::as_str).map(str::to_owned);
                detail.ends = now.get("ends").and_then(Value::as_str).map(str::to_owned);
                detail.place = now.get("place").and_then(Value::as_str).map(fact);
                body = Some(Value::Object(now));
                undo_body = Some(Value::Object(before));
            }
            A::DeleteEvent => {
                let here = here.ok_or(Refusal::Unavailable)?;
                // A series is deleted whole: not from a card.
                if seen.recurring || !here.writes() {
                    return Err(Refusal::EventNotChangeable);
                }
            }
            A::RespondEvent => {
                // Only an invitation: the event carries the member's answer.
                let current = seen.response.clone().ok_or(Refusal::NotAnInvitation)?;
                let wanted = response.as_deref().ok_or(Refusal::InvalidArguments)?;
                if current == wanted {
                    return Err(Refusal::AlreadySo);
                }
                body = Some(json!({ "response": wanted }));
                undo_body = Some(json!({ "response": current }));
            }
            A::PushEvent => {
                let here = here.ok_or(Refusal::Unavailable)?;
                if !here.campbooks || seen.recurring {
                    return Err(Refusal::EventNotChangeable);
                }
                let asked = calendar.as_deref().ok_or(Refusal::InvalidArguments)?.trim();
                let lowered = asked.to_lowercase();
                let targets: Vec<&Cal> = calendars
                    .iter()
                    .filter(|c| !c.campbooks && c.writes())
                    .collect();
                let by_id = targets.iter().find(|c| c.id == asked);
                let named: Vec<&&Cal> = targets
                    .iter()
                    .filter(|c| c.name.to_lowercase() == lowered)
                    .collect();
                let target: &Cal = by_id
                    .copied()
                    .or_else(|| (named.len() == 1).then(|| *named[0]))
                    .ok_or(Refusal::NoSuchCalendar)?;
                if !safe_segment(&target.id) {
                    return Err(Refusal::Unavailable);
                }
                detail.calendar_name = Some(target.name.clone());
                body = Some(json!({ "calendar_id": target.id }));
            }
            A::UnpushEvent => {
                let here = here.ok_or(Refusal::Unavailable)?;
                if here.campbooks || !seen.on_provider {
                    return Err(Refusal::EventNotChangeable);
                }
            }
            _ => return Err(Refusal::InvalidArguments),
        }
        let request = self.request(row.operation, &params, body, &[])?;
        let undo = self.row_undo(row, &params, undo_body)?;
        Ok(ChatProposal {
            id: ProposalId::new(),
            kind: ProposalKind::Change,
            state: ProposalState::Pending,
            action: Some(action),
            record: Some(RecordRef::new(EVENT, &seen.id)),
            facts: None,
            task: None,
            event: Some(ProposalEvent {
                title: fact(&seen.title),
                starts: seen.starts.clone(),
                all_day: seen.all_day,
                calendar: here.map(|c| c.name.clone()),
            }),
            detail: Some(detail),
            request: Some(request),
            undo: Some(undo),
            page: None,
            sentence: None,
            intent: None,
        })
    }

    /// A section for a page, or a new page, checked as the member: the page
    /// is theirs to write (or the parent a private page of theirs), the
    /// Markdown is within the pages module's cap, and every source is a
    /// record they can read. The card shows what the member's own records
    /// say of the page, never the model's words about it.
    #[allow(clippy::too_many_lines)]
    async fn page_proposal(&self, asked: &Asked) -> Result<ChatProposal, Refusal> {
        let Asked::Page {
            action,
            page,
            title,
            parent,
            markdown,
            sources,
        } = asked
        else {
            return Err(Refusal::InvalidArguments);
        };
        let action = *action;
        let row = action.row();
        if markdown.len() > PAGE_MARKDOWN_BYTES {
            return Err(Refusal::MarkdownTooLong);
        }
        let list = self
            .member
            .read("pages.list", json!({}))
            .await
            .map_err(|_| Refusal::Unavailable)?;
        let pages: Vec<&Value> = list["pages"].as_array().into_iter().flatten().collect();
        // The ordinary page named `id`, and what the member may do with it.
        let find = |id: &str| -> Option<(&Value, bool)> {
            pages
                .iter()
                .copied()
                .find(|page| page["id"] == id && page["kind"] == "page")
                .map(|page| (page, page["role"] != "reader"))
        };
        self.sources_readable(sources, &pages).await?;
        let asked_sources: Vec<Value> = sources
            .iter()
            .map(|source| json!({ "type": source.record_type, "id": source.id }))
            .collect();
        let mut detail = ProposalDetail {
            markdown: Some(markdown.clone()),
            sources: sources.clone(),
            ..ProposalDetail::default()
        };
        match action {
            ProposalAction::AppendSection => {
                let id = page.as_deref().ok_or(Refusal::InvalidArguments)?;
                let (found, writes) = find(id).ok_or(Refusal::PageNotVisible)?;
                if !writes {
                    return Err(Refusal::PageReadOnly);
                }
                let id = found["id"]
                    .as_str()
                    .filter(|id| safe_segment(id))
                    .ok_or(Refusal::Unavailable)?;
                let params = [("page_id", id.to_owned())];
                let body = json!({ "markdown": markdown, "sources": asked_sources });
                let request = self.request(row.operation, &params, Some(body), &[])?;
                let undo = self.row_undo(row, &params, None)?;
                Ok(ChatProposal {
                    id: ProposalId::new(),
                    kind: ProposalKind::Change,
                    state: ProposalState::Pending,
                    action: Some(action),
                    record: Some(RecordRef::new(PAGE, id)),
                    facts: None,
                    task: None,
                    event: None,
                    page: Some(
                        self.page_facts(
                            Some(id),
                            found["title"].as_str().map(fact).unwrap_or_default(),
                            None,
                            true,
                        )
                        .await?,
                    ),
                    detail: Some(detail),
                    request: Some(request),
                    undo: Some(undo),
                    sentence: None,
                    intent: None,
                })
            }
            ProposalAction::NewPage => {
                let title = title.as_deref().ok_or(Refusal::InvalidArguments)?;
                // A new page is always private; under a parent only if that
                // is a private page the member can write.
                let mut body = json!({
                    "title": title,
                    "place": "private",
                    "markdown": markdown,
                    "sources": asked_sources,
                });
                let mut record = None;
                let mut parent_facts = None;
                if let Some(parent) = parent {
                    let (found, writes) = find(parent).ok_or(Refusal::PageNotVisible)?;
                    if found["place"] != "private" {
                        return Err(Refusal::ParentNotPrivate);
                    }
                    if !writes {
                        return Err(Refusal::PageReadOnly);
                    }
                    let id = found["id"]
                        .as_str()
                        .filter(|id| safe_segment(id))
                        .ok_or(Refusal::Unavailable)?;
                    body["parent"] = id.into();
                    record = Some(RecordRef::new(PAGE, id));
                    parent_facts =
                        Some((id, found["title"].as_str().map(fact).unwrap_or_default()));
                }
                // The new page takes its readers from the page it goes under
                // (grants reach sub-pages); with none, it is the member's alone.
                let facts = match parent_facts {
                    Some((id, parent_title)) => {
                        self.page_facts(Some(id), fact(title), Some(parent_title), false)
                            .await?
                    }
                    None => self.page_facts(None, fact(title), None, false).await?,
                };
                detail.title = Some(fact(title));
                let request = self.request(row.operation, &[], Some(body), &[])?;
                let undo = self.row_undo(row, &[], None)?;
                Ok(ChatProposal {
                    id: ProposalId::new(),
                    kind: ProposalKind::Change,
                    state: ProposalState::Pending,
                    action: Some(action),
                    record,
                    facts: None,
                    task: None,
                    event: None,
                    page: Some(facts),
                    detail: Some(detail),
                    request: Some(request),
                    undo: Some(undo),
                    sentence: None,
                    intent: None,
                })
            }
            _ => Err(Refusal::InvalidArguments),
        }
    }

    /// Who will read a page's content, as the pages module says it: the
    /// readers a page really has, grants of the pages above it included, and
    /// whether Notion keeps a copy. `of` is the page whose readers they are
    /// (none: a new top-level page, the member's alone); `notion` whether to
    /// report that page's Notion link (a new page has none).
    async fn page_facts(
        &self,
        of: Option<&str>,
        title: String,
        parent: Option<String>,
        notion: bool,
    ) -> Result<ProposalPage, Refusal> {
        let Some(id) = of else {
            return Ok(ProposalPage {
                title,
                audience: ProposalAudience::OnlyYou,
                readers: Vec::new(),
                others: 0,
                notion: false,
                parent,
            });
        };
        let sharing = self
            .member
            .read("pages.get_sharing", json!({ "page_id": id }))
            .await
            .map_err(|_| Refusal::Unavailable)?;
        let (me, people) = self.members().await?;
        let others_in_workspace =
            u32::try_from(people.iter().filter(|p| p.id != me).count()).unwrap_or(u32::MAX);
        let (audience, readers, others) = if sharing["everyone"] == true {
            (ProposalAudience::Everyone, Vec::new(), others_in_workspace)
        } else {
            let readers: Vec<&str> = sharing["readers"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .filter(|reader| *reader != me)
                .collect();
            if readers.is_empty() {
                (ProposalAudience::OnlyYou, Vec::new(), 0)
            } else {
                let named: Vec<String> = readers
                    .iter()
                    .filter_map(|reader| people.iter().find(|p| p.id == *reader))
                    .map(|p| p.name.clone())
                    .take(5)
                    .collect();
                let others = u32::try_from(readers.len() - named.len()).unwrap_or(u32::MAX);
                (ProposalAudience::Named, named, others)
            }
        };
        Ok(ProposalPage {
            title,
            audience,
            readers,
            others,
            notion: notion && sharing["notion"] == true,
            parent,
        })
    }

    /// Every source is a record the member can read now: a thread, a task or
    /// an event by the read that shows it, a page by the list.
    async fn sources_readable(
        &self,
        sources: &[RecordRef],
        pages: &[&Value],
    ) -> Result<(), Refusal> {
        for source in sources {
            if source.record_type == PAGE {
                if !pages.iter().any(|page| page["id"] == source.id.as_str()) {
                    return Err(Refusal::SourceNotVisible);
                }
                continue;
            }
            let (_, read, param) = SOURCE_READS
                .iter()
                .find(|(kind, ..)| *kind == source.record_type)
                .ok_or(Refusal::SourceNotVisible)?;
            let mut input = Map::new();
            input.insert((*param).to_owned(), Value::String(source.id.clone()));
            self.member
                .read(read, Value::Object(input))
                .await
                .map_err(|unread| match unread {
                    Unread::NotVisible | Unread::Refused => Refusal::SourceNotVisible,
                    Unread::Broken(_) => Refusal::Unavailable,
                })?;
        }
        Ok(())
    }

    /// The undo of `row`'s change: its catalogue undo, or the same operation
    /// with `undo_body`, the record's values before.
    fn row_undo(
        &self,
        row: &Row,
        params: &[(&str, String)],
        undo_body: Option<Value>,
    ) -> Result<ProposalRequest, Refusal> {
        match row.undo {
            Undo::Restoring { from_response } => {
                self.request(row.operation, params, undo_body, from_response)
            }
            Undo::Catalogue {
                operation,
                from_response,
            } => self.request(operation, params, None, from_response),
        }
    }

    /// The event as the member sees it (`schedule.get_event` refuses what
    /// they cannot see).
    async fn read_event(&self, event: &str) -> Result<SeenEvent, Refusal> {
        let view = self
            .member
            .read("schedule.get_event", json!({ "event_id": event }))
            .await
            .map_err(|unread| match unread {
                Unread::NotVisible | Unread::Refused => Refusal::NotVisible,
                Unread::Broken(_) => Refusal::Unavailable,
            })?;
        let text = |key: &str| view[key].as_str().map(ToOwned::to_owned);
        Ok(SeenEvent {
            id: text("id")
                .filter(|id| safe_segment(id))
                .ok_or(Refusal::Unavailable)?,
            title: text("title").ok_or(Refusal::Unavailable)?,
            starts: text("starts").ok_or(Refusal::Unavailable)?,
            ends: text("ends").ok_or(Refusal::Unavailable)?,
            all_day: view["all_day"] == true,
            calendar_id: text("calendar_id").ok_or(Refusal::Unavailable)?,
            place: text("place"),
            recurring: view["recurring"] == true,
            response: text("response"),
            pending: view["pending"] == true,
            on_provider: view["on_provider"] == true,
        })
    }

    /// The member's calendars, named as the list names them.
    async fn calendars(&self) -> Result<Vec<Cal>, Refusal> {
        let list = self
            .member
            .read("schedule.list_calendars", Value::Null)
            .await
            .map_err(|_| Refusal::Unavailable)?;
        Ok(list["calendars"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| {
                Some(Cal {
                    id: c["id"].as_str()?.to_owned(),
                    name: c["name"].as_str().map(fact)?,
                    campbooks: c["source"] == "campbooks",
                    writable: c["writable"] == true,
                    syncing: c["syncing"] == true,
                })
            })
            .collect())
    }

    /// The task as the member sees it (`tasks.get` refuses what they
    /// cannot see).
    async fn read_task(&self, task: &str) -> Result<SeenTask, Refusal> {
        let detail = self
            .member
            .read(
                "tasks.get",
                json!({ "task_id": task, "utc_offset": self.utc_offset }),
            )
            .await
            .map_err(|unread| match unread {
                Unread::NotVisible | Unread::Refused => Refusal::NotVisible,
                Unread::Broken(_) => Refusal::Unavailable,
            })?;
        let head = &detail["task"];
        let text = |key: &str| head[key].as_str().map(ToOwned::to_owned);
        let day = |key: &str| {
            head[key]
                .as_str()
                .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        };
        let at = |key: &str| {
            head[key]
                .as_str()
                .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
                .map(|t| t.with_timezone(&Utc))
        };
        Ok(SeenTask {
            id: text("id")
                .filter(|id| safe_segment(id))
                .ok_or(Refusal::Unavailable)?,
            title: text("title").ok_or(Refusal::Unavailable)?,
            verb: text("verb").ok_or(Refusal::Unavailable)?,
            state: text("state").ok_or(Refusal::Unavailable)?,
            plan: text("plan").ok_or(Refusal::Unavailable)?,
            assignee: text("assignee").ok_or(Refusal::Unavailable)?,
            revision: head["revision"].as_i64().ok_or(Refusal::Unavailable)?,
            due_on: day("due_on"),
            due_at: at("due_at"),
            plan_on: day("plan_on"),
            later_until: at("later_until"),
        })
    }

    /// The workspace's members: who the member is (their user id), and
    /// everyone, named as the list names them.
    async fn members(&self) -> Result<(String, Vec<Teammate>), Refusal> {
        let list = self
            .member
            .read("workspaces.list_members", Value::Null)
            .await
            .map_err(|_| Refusal::Unavailable)?;
        let mut me = None;
        let mut people = Vec::new();
        for member in list["members"].as_array().into_iter().flatten() {
            let Some(id) = member["user_id"].as_str() else {
                continue;
            };
            if member["you"] == true {
                me = Some(id.to_owned());
            }
            let email = member["email"].as_str().map(clean);
            let name = member["display_name"]
                .as_str()
                .map(clean)
                .filter(|n| !n.is_empty())
                .or_else(|| email.clone())
                .map(|n| fact(&n))
                .unwrap_or_default();
            people.push(Teammate {
                id: id.to_owned(),
                name,
                email,
            });
        }
        // Without who the member is, nothing about a hand-off or a role is
        // known: no proposal.
        Ok((me.ok_or(Refusal::Unavailable)?, people))
    }

    /// The id of the conversation's open item on the member's list.
    async fn open_item(&self, conversation: &str) -> Result<String, Refusal> {
        let list = self
            .member
            .read("attention.list", json!({ "state": "open" }))
            .await
            .map_err(|_| Refusal::Unavailable)?;
        list["items"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|item| {
                item["record"]["type"] == CONVERSATION && item["record"]["id"] == conversation
            })
            .and_then(|item| item["id"].as_str())
            .filter(|id| safe_segment(id))
            .map(ToOwned::to_owned)
            .ok_or(Refusal::NoSuchNeed)
    }

    /// `operation`'s request, from its catalogue entry: its method, and its
    /// path template under the workspace's root filled with `params`.
    fn request(
        &self,
        operation: &str,
        params: &[(&str, String)],
        body: Option<Value>,
        from_response: &[&str],
    ) -> Result<ProposalRequest, Refusal> {
        let catalogue = self
            .member
            .state
            .operations
            .catalogue()
            .ok_or(Refusal::Unavailable)?;
        let info = catalogue.get(operation).ok_or(Refusal::Unavailable)?;
        let method = match info.method.as_str() {
            "POST" => ProposalMethod::Post,
            "DELETE" => ProposalMethod::Delete,
            // Only a task's edit and an event's use it (the client allows it
            // for those entries alone).
            "PATCH" if operation == "tasks.update" || operation == "schedule.update_event" => {
                ProposalMethod::Patch
            }
            _ => return Err(Refusal::Unavailable),
        };
        // A path id the change's answer will give (`@event_id:id`): a
        // placeholder segment the client fills in from that answer.
        let late: Vec<&str> = from_response
            .iter()
            .filter_map(|entry| entry.strip_prefix('@')?.split_once(':'))
            .map(|(name, _)| name)
            .collect();
        let mut filled: Vec<(&str, String)> = params.to_vec();
        for name in &late {
            filled.retain(|(key, _)| key != name);
            filled.push((*name, format!("late-{name}")));
        }
        let mut path = fill(&info.path, &filled).ok_or(Refusal::Unavailable)?;
        for name in &late {
            path = path
                .split('/')
                .map(|segment| {
                    if segment == format!("late-{name}") {
                        format!("{{{name}}}")
                    } else {
                        segment.to_owned()
                    }
                })
                .collect::<Vec<_>>()
                .join("/");
        }
        Ok(ProposalRequest {
            operation: operation.to_owned(),
            method,
            path,
            body: if info.has_body { body } else { None },
            from_response: from_response.iter().map(ToString::to_string).collect(),
        })
    }
}

/// The path of `template` relative to the workspace's API root, its
/// parameters filled from `params`; `None` when it is not under the root or
/// a parameter is missing.
pub fn fill(template: &str, params: &[(&str, String)]) -> Option<String> {
    let relative = template.strip_prefix(WORKSPACE_ROOT)?;
    let mut out: Vec<String> = Vec::new();
    for segment in relative.split('/') {
        match segment.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
            Some(name) => {
                let (_, value) = params.iter().find(|(key, _)| *key == name)?;
                if !safe_segment(value) {
                    return None;
                }
                out.push(value.clone());
            }
            None => out.push(segment.to_owned()),
        }
    }
    Some(out.join("/"))
}

/// An id that is one plain path segment.
fn safe_segment(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// `until`, parsed, when it is later than now and less than a year away.
fn until_within(until: &str) -> Result<DateTime<Utc>, Refusal> {
    let at = DateTime::parse_from_rfc3339(until)
        .map_err(|_| Refusal::UntilOutOfRange)?
        .with_timezone(&Utc);
    let now = Utc::now();
    if at <= now || at >= now + Duration::days(365) {
        return Err(Refusal::UntilOutOfRange);
    }
    Ok(at)
}

/// `day`, when it is today or later in the member's days (`utc_offset`
/// minutes from UTC) and less than a year away.
fn day_within(day: NaiveDate, now: DateTime<Utc>, utc_offset: i32) -> Result<NaiveDate, Refusal> {
    let today = (now + Duration::minutes(i64::from(utc_offset))).date_naive();
    if day < today || day >= today + Duration::days(YEAR) {
        return Err(Refusal::DayOutOfRange);
    }
    Ok(day)
}

/// A time of an event: a day (all-day) or a moment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd)]
enum EventTime {
    Day(NaiveDate),
    At(DateTime<Utc>),
}

impl EventTime {
    fn parse(text: &str) -> Option<Self> {
        NaiveDate::parse_from_str(text, "%Y-%m-%d")
            .map(Self::Day)
            .ok()
            .or_else(|| {
                DateTime::parse_from_rfc3339(text)
                    .ok()
                    .map(|at| Self::At(at.with_timezone(&Utc)))
            })
    }

    const fn is_day(self) -> bool {
        matches!(self, Self::Day(_))
    }
}

/// A new event's `starts`: a day from today (in the member's days) to a year
/// away, or a moment later than now and a year or more away.
fn event_start_within(starts: &str, now: DateTime<Utc>, utc_offset: i32) -> Result<(), Refusal> {
    match EventTime::parse(starts).ok_or(Refusal::InvalidArguments)? {
        EventTime::Day(day) => day_within(day, now, utc_offset)
            .map(|_| ())
            .map_err(|_| Refusal::StartOutOfRange),
        EventTime::At(at) => {
            if at <= now || at >= now + Duration::days(YEAR) {
                Err(Refusal::StartOutOfRange)
            } else {
                Ok(())
            }
        }
    }
}

/// The fields of an event's edit that would really change it (`now`), and
/// their values before (`before`, which puts them back), judged against the
/// event as read. Refuses an edit that changes nothing, or that cannot be
/// written: a day on a timed event, a time on an all-day one, an end before
/// the start.
fn event_changes(
    seen: &SeenEvent,
    title: Option<&str>,
    starts: Option<&str>,
    ends: Option<&str>,
    place: Option<&str>,
) -> Result<EventChanges, Refusal> {
    let mut now = Map::new();
    let mut before = Map::new();
    if let Some(title) = title.filter(|t| *t != seen.title) {
        now.insert("title".into(), title.into());
        before.insert("title".into(), seen.title.clone().into());
    }
    let moved_start = starts.filter(|s| *s != seen.starts);
    let moved_end = ends.filter(|e| *e != seen.ends);
    // Every time given must be of the event's form.
    for given in [moved_start, moved_end].into_iter().flatten() {
        let when = EventTime::parse(given).ok_or(Refusal::InvalidWhen)?;
        if when.is_day() != seen.all_day {
            return Err(Refusal::InvalidWhen);
        }
    }
    // The end, against the start the event will have.
    if let Some(end) = moved_end {
        let start = moved_start.unwrap_or(&seen.starts);
        if let (Some(start), Some(end)) = (EventTime::parse(start), EventTime::parse(end))
            && end < start
        {
            return Err(Refusal::InvalidWhen);
        }
    }
    if let Some(starts) = moved_start {
        now.insert("starts".into(), starts.into());
    }
    if let Some(ends) = moved_end {
        now.insert("ends".into(), ends.into());
    }
    if moved_start.is_some() || moved_end.is_some() {
        // The start moves the end with it unless it is given: put both back.
        before.insert("starts".into(), seen.starts.clone().into());
        before.insert("ends".into(), seen.ends.clone().into());
    }
    if let Some(place) = place.filter(|p| Some(*p) != seen.place.as_deref()) {
        now.insert("place".into(), place.into());
        before.insert(
            "place".into(),
            seen.place.clone().unwrap_or_default().into(),
        );
    }
    if now.is_empty() {
        return Err(Refusal::AlreadySo);
    }
    Ok((now, before))
}

/// A fact, cleaned and bounded.
fn fact(text: &str) -> String {
    cut_bytes(&clean(text), FACT_BYTES).to_owned()
}

/// Whether two proposals are the same card.
fn same_card(one: &ChatProposal, other: &ChatProposal) -> bool {
    one.kind == other.kind
        && one.action == other.action
        && one.record == other.record
        && one.page == other.page
        && one.detail == other.detail
        && one.sentence == other.sentence
        && one.intent == other.intent
}

/// What the member may do in the conversation's inbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InboxAccess {
    /// Change and send.
    Writes,
    /// Read only (a reader, or an inbox kept read-only).
    ReadsOnly,
    /// Not listed (the real request still checks).
    Unknown,
}

impl InboxAccess {
    /// Whether the member may change it, when known.
    const fn writes(self) -> Option<bool> {
        match self {
            Self::Writes => Some(true),
            Self::ReadsOnly => Some(false),
            Self::Unknown => None,
        }
    }
}

/// A conversation, as read for a proposal.
struct Seen {
    /// Its id, as its record has it.
    id: String,
    sender: String,
    subject: String,
    /// `conversations.get`'s `conversation`.
    head: Value,
    role: InboxAccess,
}

impl Seen {
    fn facts(&self) -> ProposalFacts {
        ProposalFacts {
            sender: self.sender.clone(),
            subject: self.subject.clone(),
            stream: self.head["stream"].as_str().unwrap_or_default().to_owned(),
        }
    }
}

/// A member of the workspace, as the members list has them.
struct Teammate {
    id: String,
    /// Their name, else their address (bounded).
    name: String,
    email: Option<String>,
}

/// The fields an edit sets, and their values before.
type EventChanges = (Map<String, Value>, Map<String, Value>);

/// An event, as read for a proposal.
#[allow(
    clippy::struct_excessive_bools,
    reason = "the flags are the event as the provider reports it"
)]
struct SeenEvent {
    id: String,
    title: String,
    starts: String,
    ends: String,
    all_day: bool,
    calendar_id: String,
    place: Option<String>,
    recurring: bool,
    /// The member's answer, when the event is an invitation.
    response: Option<String>,
    pending: bool,
    on_provider: bool,
}

/// A calendar of the member's, as the list has it.
struct Cal {
    id: String,
    /// Its name (bounded).
    name: String,
    /// Whether it is their Campbooks calendar.
    campbooks: bool,
    writable: bool,
    syncing: bool,
}

impl Cal {
    /// Whether events may be made and changed on it.
    const fn writes(&self) -> bool {
        self.writable && (self.campbooks || self.syncing)
    }
}

/// A task, as read for a proposal.
struct SeenTask {
    id: String,
    title: String,
    verb: String,
    state: String,
    plan: String,
    assignee: String,
    revision: i64,
    due_on: Option<NaiveDate>,
    due_at: Option<DateTime<Utc>>,
    plan_on: Option<NaiveDate>,
    later_until: Option<DateTime<Utc>>,
}

impl SeenTask {
    /// The `tasks.plan` body that puts the plan back as it is now (a Later
    /// already over is no plan).
    fn plan_before(&self, now: DateTime<Utc>) -> Value {
        match (self.plan.as_str(), self.plan_on, self.later_until) {
            ("date", Some(day), _) => json!({ "plan": "date", "plan_on": day.to_string() }),
            ("later", _, Some(until)) if until > now => {
                json!({ "plan": "later", "later_until": until.to_rfc3339() })
            }
            _ => json!({ "plan": "none" }),
        }
    }
}

/// A change's detail, request and undo.
struct Built {
    detail: ProposalDetail,
    request: ProposalRequest,
    undo: ProposalRequest,
}

/// What the model reads after Scout's earlier answer, for one of its
/// proposals: a line the server writes, never the member's words.
pub fn line(proposal: &ChatProposal) -> String {
    if let (Some(task), Some(action)) = (&proposal.task, proposal.action) {
        return task_line(proposal, action, task);
    }
    if let Some(action) = proposal
        .action
        .filter(|action| ProposalAction::EVENTS.contains(action))
    {
        return event_line(proposal, action);
    }
    if let Some(action) = proposal
        .action
        .filter(|action| ProposalAction::PAGES.contains(action))
    {
        return page_line(proposal, action);
    }
    let about = proposal.facts.as_ref().map_or_else(String::new, |facts| {
        let subject = if facts.subject.is_empty() {
            "(no subject)"
        } else {
            facts.subject.as_str()
        };
        format!(" \"{} · {subject}\"", facts.sender)
    });
    let what = match proposal.kind {
        ProposalKind::Change => {
            let action = proposal.action.map_or("a change", ProposalAction::name);
            let detail = proposal.detail.as_ref().map_or_else(String::new, |d| {
                if let Some(stream) = &d.stream {
                    format!(" to {stream}")
                } else if let Some(label) = &d.label {
                    format!(" label \"{label}\"")
                } else if let Some(until) = d.until {
                    format!(" until {}", until.to_rfc3339())
                } else {
                    String::new()
                }
            });
            format!("{action}{detail} for{about}")
        }
        ProposalKind::Rule => format!(
            "the rule \"{}\"",
            proposal.sentence.as_deref().unwrap_or_default()
        ),
        ProposalKind::Reply => format!(
            "a reply to{about} saying \"{}\"",
            proposal.intent.as_deref().unwrap_or_default()
        ),
    };
    let done = match proposal.kind {
        ProposalKind::Change => "The member confirmed it.",
        ProposalKind::Rule => "The member saved the rule.",
        ProposalKind::Reply => "The member opened the draft; only they send it.",
    };
    let state = match proposal.state {
        ProposalState::Pending => "The member has not answered it.",
        ProposalState::Done => done,
        ProposalState::Declined => "The member said not now.",
        ProposalState::Failed => "The member confirmed it, and it could not be done.",
        ProposalState::Undone => "The member confirmed it, then undid it.",
    };
    format!("Proposed: {what}. {state}")
}

/// The line for one of a task proposal.
fn task_line(proposal: &ChatProposal, action: ProposalAction, task: &ProposalTask) -> String {
    let detail = proposal.detail.as_ref().map_or_else(String::new, |d| {
        if let Some(name) = &d.member_name {
            format!(" to {name}")
        } else if let Some(day) = d.day {
            format!(" for {day}")
        } else if let Some(until) = d.until {
            format!(" until {}", until.to_rfc3339())
        } else {
            String::new()
        }
    });
    let state = match proposal.state {
        ProposalState::Pending => "The member has not answered it.",
        ProposalState::Done => "The member confirmed it.",
        ProposalState::Declined => "The member said not now.",
        ProposalState::Failed => "The member confirmed it, and it could not be done.",
        ProposalState::Undone => "The member confirmed it, then undid it.",
    };
    format!(
        "Proposed: {}{detail} for the task \"{}\". {state}",
        action.task_name(),
        task.title
    )
}

/// The line for one of a page proposal: what it does and to which page, never
/// the Markdown.
fn page_line(proposal: &ChatProposal, action: ProposalAction) -> String {
    let title = proposal
        .page
        .as_ref()
        .map(|page| page.title.clone())
        .or_else(|| proposal.detail.as_ref().and_then(|d| d.title.clone()))
        .unwrap_or_default();
    // Who reads it, in the words the card uses: never "private" when anyone
    // else can read the page.
    let readers = proposal.page.as_ref().map_or_else(String::new, |page| {
        let notion = if page.notion {
            ", and it is kept in step with Notion"
        } else {
            ""
        };
        match page.audience {
            ProposalAudience::OnlyYou => format!(" (only the member reads it{notion})"),
            ProposalAudience::Everyone => {
                format!(" (every member of the workspace reads it{notion})")
            }
            ProposalAudience::Named => format!(
                " (readable by {} and {} more{notion})",
                page.readers.join(", "),
                page.others
            ),
        }
    });
    let what = match action {
        ProposalAction::NewPage => format!("a new page \"{title}\"{readers}"),
        _ => format!("a section at the end of the page \"{title}\"{readers}"),
    };
    let state = match proposal.state {
        ProposalState::Pending => "The member has not answered it.",
        ProposalState::Done => "The member confirmed it.",
        ProposalState::Declined => "The member said not now.",
        ProposalState::Failed => "The member confirmed it, and it could not be done.",
        ProposalState::Undone => "The member confirmed it, then undid it.",
    };
    format!("Proposed: {what}. {state}")
}

/// The line for one of an event proposal.
fn event_line(proposal: &ChatProposal, action: ProposalAction) -> String {
    let detail = proposal.detail.as_ref();
    let title = proposal
        .event
        .as_ref()
        .map(|e| e.title.clone())
        .or_else(|| detail.and_then(|d| d.title.clone()))
        .unwrap_or_default();
    let extra = detail.map_or_else(String::new, |d| {
        if let Some(name) = &d.calendar_name {
            format!(" to the calendar {name}")
        } else if let Some(response) = &d.response {
            format!(" ({response})")
        } else if let Some(starts) = &d.starts {
            format!(" starting {starts}")
        } else {
            String::new()
        }
    });
    let state = match proposal.state {
        ProposalState::Pending => "The member has not answered it.",
        ProposalState::Done => "The member confirmed it.",
        ProposalState::Declined => "The member said not now.",
        ProposalState::Failed => "The member confirmed it, and it could not be done.",
        ProposalState::Undone => "The member confirmed it, then undid it.",
    };
    format!(
        "Proposed: {}{extra} for the event \"{title}\". {state}",
        action.event_name()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(all_day: bool) -> SeenEvent {
        SeenEvent {
            id: "e1".into(),
            title: "Dentist".into(),
            starts: if all_day {
                "2026-11-05"
            } else {
                "2026-11-05T15:00:00+00:00"
            }
            .into(),
            ends: if all_day {
                "2026-11-06"
            } else {
                "2026-11-05T16:00:00+00:00"
            }
            .into(),
            all_day,
            calendar_id: "c1".into(),
            place: None,
            recurring: false,
            response: None,
            pending: false,
            on_provider: true,
        }
    }

    #[test]
    fn a_new_events_start_is_bounded_like_a_tasks_day() {
        let now = DateTime::parse_from_rfc3339("2026-10-23T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert!(event_start_within("2026-10-23T13:00:00+00:00", now, 0).is_ok());
        assert!(event_start_within("2026-10-24", now, 0).is_ok());
        for out in [
            "2026-10-23T11:00:00+00:00",
            "2027-10-23T13:00:00+00:00",
            "2026-10-22",
            "2027-10-23",
        ] {
            assert_eq!(
                event_start_within(out, now, 0),
                Err(Refusal::StartOutOfRange),
                "{out}"
            );
        }
        assert_eq!(
            event_start_within("next Thursday", now, 0),
            Err(Refusal::InvalidArguments)
        );
    }

    #[test]
    fn an_edit_is_what_it_really_changes() {
        let seen = event(false);
        // The same start with a new title renames; it does not move.
        let (now, before) = event_changes(
            &seen,
            Some("Dentist, Dr Lopes"),
            Some("2026-11-05T15:00:00+00:00"),
            None,
            None,
        )
        .unwrap();
        assert_eq!(now.keys().collect::<Vec<_>>(), ["title"]);
        assert_eq!(before["title"], "Dentist");
        // An end alone changes the end, and puts both times back.
        let (now, before) =
            event_changes(&seen, None, None, Some("2026-11-05T17:00:00+00:00"), None).unwrap();
        assert_eq!(now.keys().collect::<Vec<_>>(), ["ends"]);
        assert_eq!(before["starts"], "2026-11-05T15:00:00+00:00");
        assert_eq!(
            event_changes(&seen, Some("Dentist"), None, None, None),
            Err(Refusal::AlreadySo)
        );
    }

    #[test]
    fn an_edit_that_can_only_fail_is_refused_when_it_is_proposed() {
        let timed = event(false);
        let day = event(true);
        // A day on a timed event, a moment on an all-day one.
        assert_eq!(
            event_changes(&timed, None, Some("2026-11-07"), None, None),
            Err(Refusal::InvalidWhen)
        );
        assert_eq!(
            event_changes(&day, None, Some("2026-11-07T10:00:00+00:00"), None, None),
            Err(Refusal::InvalidWhen)
        );
        // An end alone before the current start, or before the new start.
        assert_eq!(
            event_changes(&timed, None, None, Some("2026-11-05T14:00:00+00:00"), None),
            Err(Refusal::InvalidWhen)
        );
        assert_eq!(
            event_changes(
                &timed,
                None,
                Some("2026-11-06T10:00:00+00:00"),
                Some("2026-11-05T17:00:00+00:00"),
                None
            ),
            Err(Refusal::InvalidWhen)
        );
        assert!(event_changes(&day, None, Some("2026-11-07"), Some("2026-11-08"), None).is_ok());
    }

    #[test]
    fn every_action_has_one_row_and_a_strict_schema() {
        for action in ProposalAction::ALL
            .into_iter()
            .chain(ProposalAction::TASKS)
            .chain(ProposalAction::EVENTS)
            .chain(ProposalAction::PAGES)
        {
            assert_eq!(
                ROWS.iter().filter(|row| row.action == action).count(),
                1,
                "{}",
                action.name()
            );
        }
        for action in ProposalAction::ALL {
            assert_eq!(ProposalAction::named(action.name()), Some(action));
        }
        for action in ProposalAction::TASKS {
            assert_eq!(ProposalAction::task_named(action.task_name()), Some(action));
            // A task action is never one `propose_change` takes.
            assert_eq!(ProposalAction::named(action.name()), None);
            // Every task row's undo copies the revision it will need.
            assert!(matches!(
                action.row().undo,
                Undo::Catalogue {
                    from_response: &["revision"],
                    ..
                } | Undo::Restoring {
                    from_response: &["revision"]
                }
            ));
        }
        for action in ProposalAction::EVENTS {
            assert_eq!(
                ProposalAction::event_named(action.event_name()),
                Some(action)
            );
            // An event action is never one `propose_change` takes.
            assert_eq!(ProposalAction::named(action.name()), None);
            assert!(action.row().operation.starts_with("schedule."));
        }
        for action in ProposalAction::PAGES {
            assert_eq!(ProposalAction::page_named(action.name()), Some(action));
            // A page action is never one `propose_change` takes.
            assert_eq!(ProposalAction::named(action.name()), None);
            assert!(action.row().operation.starts_with("pages."));
        }
        assert_eq!(
            ROWS.len(),
            ProposalAction::ALL.len()
                + ProposalAction::TASKS.len()
                + ProposalAction::EVENTS.len()
                + ProposalAction::PAGES.len()
        );
        for definition in definitions(true) {
            assert_eq!(definition.parameters["additionalProperties"], false);
            assert!(Proposing::named(&definition.name).is_some());
        }
    }

    #[test]
    fn arguments_outside_the_schema_propose_nothing() {
        let id = "\"conversation_id\": \"c-1\"";
        let fits = |raw: &str| arguments(Proposing::Change, raw).is_some();
        assert!(fits(&format!("{{\"action\": \"archive\", {id}}}")));
        assert!(fits(&format!(
            "{{\"action\": \"move_to_stream\", {id}, \"stream\": \"updates\"}}"
        )));
        assert!(fits(&format!(
            "{{\"action\": \"later\", {id}, \"until\": \"2026-10-26T09:00:00+00:00\"}}"
        )));
        for bad in [
            format!("{{\"action\": \"send\", {id}}}"),
            format!("{{\"action\": \"delete_forever\", {id}}}"),
            format!("{{\"action\": \"archive\", {id}, \"stream\": \"updates\"}}"),
            format!("{{\"action\": \"move_to_stream\", {id}}}"),
            format!("{{\"action\": \"move_to_stream\", {id}, \"stream\": \"spam\"}}"),
            format!("{{\"action\": \"add_label\", {id}}}"),
            format!("{{\"action\": \"later\", {id}}}"),
            format!("{{\"action\": \"archive\", {id}, \"to\": \"x@example.com\"}}"),
            "{\"action\": \"archive\"}".to_owned(),
            "[1]".to_owned(),
        ] {
            assert!(!fits(&bad), "{bad}");
        }
        assert!(arguments(Proposing::Rule, "{\"sentence\": \"Hi\"}").is_none());
        assert!(
            arguments(
                Proposing::Rule,
                &format!("{{\"sentence\": \"{}\"}}", "a".repeat(301))
            )
            .is_none()
        );
        assert_eq!(
            arguments(
                Proposing::Rule,
                "{\"sentence\": \"  Keep The Weekly off Home \"}"
            ),
            Some(Asked::Rule {
                sentence: "Keep The Weekly off Home".into()
            })
        );
        assert!(
            arguments(
                Proposing::Reply,
                "{\"conversation_id\": \"c-1\", \"intent\": \"Say yes\"}"
            )
            .is_some()
        );
        assert!(arguments(Proposing::Reply, "{\"intent\": \"Say yes\"}").is_none());
    }

    #[test]
    fn task_arguments_take_only_what_their_action_takes() {
        let id = "\"task_id\": \"t-1\"";
        let fits = |raw: &str| arguments(Proposing::Task, raw).is_some();
        assert!(fits(&format!("{{\"action\": \"done\", {id}}}")));
        assert!(fits(&format!(
            "{{\"action\": \"plan_day\", {id}, \"day\": \"2026-10-26\"}}"
        )));
        assert!(fits(&format!(
            "{{\"action\": \"due\", {id}, \"day\": \"2026-10-26\"}}"
        )));
        assert!(fits(&format!(
            "{{\"action\": \"later\", {id}, \"until\": \"2026-10-26T09:00:00+00:00\"}}"
        )));
        assert!(fits(&format!(
            "{{\"action\": \"hand_off\", {id}, \"member_id\": \"u-1\"}}"
        )));
        for bad in [
            format!("{{\"action\": \"create\", {id}}}"),
            format!("{{\"action\": \"rename\", {id}, \"title\": \"x\"}}"),
            format!("{{\"action\": \"archive\", {id}}}"),
            format!("{{\"action\": \"done\", {id}, \"day\": \"2026-10-26\"}}"),
            format!("{{\"action\": \"plan_day\", {id}}}"),
            format!("{{\"action\": \"plan_day\", {id}, \"day\": \"Monday\"}}"),
            format!("{{\"action\": \"plan_day\", {id}, \"day\": \"2026-13-40\"}}"),
            format!("{{\"action\": \"later\", {id}}}"),
            format!("{{\"action\": \"hand_off\", {id}}}"),
            format!("{{\"action\": \"done\", {id}, \"title\": \"Pay Rui\"}}"),
            "{\"action\": \"done\"}".to_owned(),
        ] {
            assert!(!fits(&bad), "{bad}");
        }
    }

    #[test]
    fn event_arguments_take_only_what_their_action_takes() {
        let id = "\"event_id\": \"e-1\"";
        let fits = |raw: &str| arguments(Proposing::Event, raw).is_some();
        assert!(fits(
            "{\"action\": \"create\", \"title\": \"Dentist\", \"starts\": \"2026-11-12T15:00:00+00:00\"}"
        ));
        assert!(fits(
            "{\"action\": \"create\", \"title\": \"Trip\", \"starts\": \"2026-11-12\", \"ends\": \"2026-11-14\"}"
        ));
        assert!(fits(&format!(
            "{{\"action\": \"update\", {id}, \"place\": \"Clinic\"}}"
        )));
        assert!(fits(&format!("{{\"action\": \"delete\", {id}}}")));
        assert!(fits(&format!("{{\"action\": \"unpush\", {id}}}")));
        assert!(fits(&format!(
            "{{\"action\": \"respond\", {id}, \"response\": \"declined\"}}"
        )));
        assert!(fits(&format!(
            "{{\"action\": \"push\", {id}, \"calendar\": \"Work\"}}"
        )));
        for bad in [
            // Not an action here.
            format!("{{\"action\": \"restore\", {id}}}"),
            format!("{{\"action\": \"archive\", {id}}}"),
            // A new event needs a title and a start, and has no id yet.
            "{\"action\": \"create\", \"title\": \"Dentist\"}".to_owned(),
            format!(
                "{{\"action\": \"create\", {id}, \"title\": \"Dentist\", \"starts\": \"2026-11-12\"}}"
            ),
            // A start that is not a moment or a day, an end before the start,
            // and a day with a time.
            "{\"action\": \"create\", \"title\": \"Dentist\", \"starts\": \"next Thursday\"}"
                .to_owned(),
            "{\"action\": \"create\", \"title\": \"Dentist\", \"starts\": \"2026-11-12T15:00:00+00:00\", \"ends\": \"2026-11-12T14:00:00+00:00\"}"
                .to_owned(),
            "{\"action\": \"create\", \"title\": \"Dentist\", \"starts\": \"2026-11-12\", \"ends\": \"2026-11-12T14:00:00+00:00\"}"
                .to_owned(),
            // Nothing to change; a change the action does not take.
            format!("{{\"action\": \"update\", {id}}}"),
            format!("{{\"action\": \"delete\", {id}, \"title\": \"x\"}}"),
            format!("{{\"action\": \"respond\", {id}}}"),
            format!("{{\"action\": \"respond\", {id}, \"response\": \"needs_action\"}}"),
            format!("{{\"action\": \"push\", {id}}}"),
            format!("{{\"action\": \"unpush\", {id}, \"calendar\": \"Work\"}}"),
            "{\"action\": \"delete\"}".to_owned(),
        ] {
            assert!(!fits(&bad), "{bad}");
        }
    }

    #[test]
    fn an_undo_path_may_wait_for_the_id_the_change_gives() {
        assert_eq!(
            fill(
                "/api/v1/w/{workspace_id}/schedule/events/{event_id}/push",
                &[("event_id", "late-event_id".to_owned())]
            )
            .as_deref(),
            Some("schedule/events/late-event_id/push")
        );
    }

    #[test]
    fn a_day_is_today_or_later_in_the_members_days_and_within_a_year() {
        // 23:30 UTC on the 23rd is already the 24th at +120 minutes.
        let now = DateTime::parse_from_rfc3339("2026-10-23T23:30:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let day = |d: &str| NaiveDate::parse_from_str(d, "%Y-%m-%d").unwrap();
        assert!(day_within(day("2026-10-24"), now, 120).is_ok());
        assert_eq!(
            day_within(day("2026-10-23"), now, 120),
            Err(Refusal::DayOutOfRange)
        );
        assert!(day_within(day("2026-10-23"), now, 0).is_ok());
        assert!(day_within(day("2027-10-22"), now, 0).is_ok());
        assert_eq!(
            day_within(day("2027-10-23"), now, 0),
            Err(Refusal::DayOutOfRange)
        );
        // West of UTC the day starts later: still the 23rd at -60 minutes.
        assert!(day_within(day("2026-10-23"), now, -60).is_ok());
        assert!(day_within(day("2026-10-22"), now, -60).is_err());
    }

    #[test]
    fn paths_stay_under_the_workspace_and_take_only_plain_ids() {
        let template = "/api/v1/w/{workspace_id}/conversations/{conversation_id}/labels/{label_id}";
        let params = [
            ("conversation_id", "c-1".to_owned()),
            ("label_id", "l-2".to_owned()),
        ];
        assert_eq!(
            fill(template, &params).as_deref(),
            Some("conversations/c-1/labels/l-2")
        );
        assert_eq!(fill("/api/v1/workspaces", &params), None);
        assert_eq!(
            fill(template, &[("conversation_id", "../../x".to_owned())]),
            None
        );
        assert_eq!(fill(template, &params[..1]), None);
    }

    #[test]
    fn until_is_later_than_now_and_within_a_year() {
        let at = |days: i64| (Utc::now() + Duration::days(days)).to_rfc3339();
        assert!(until_within(&at(3)).is_ok());
        assert_eq!(until_within(&at(-1)), Err(Refusal::UntilOutOfRange));
        assert_eq!(until_within(&at(366)), Err(Refusal::UntilOutOfRange));
        assert_eq!(until_within("Monday"), Err(Refusal::UntilOutOfRange));
    }
}
