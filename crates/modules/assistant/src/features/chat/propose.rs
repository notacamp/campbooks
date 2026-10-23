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
//! in their own change. The three-per-answer cap and the dedupe are shared
//! by every proposing tool.

use campbooks_ai::{ToolCall, ToolDefinition};
use campbooks_kernel::RecordRef;
use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde_json::{Map, Value, json};

use super::{
    member::{Member, Unread},
    types::{
        ChatProposal, ProposalAction, ProposalDetail, ProposalFacts, ProposalId, ProposalKind,
        ProposalMethod, ProposalRequest, ProposalState, ProposalTask,
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
/// The streams a conversation may be moved to.
const STREAMS: [&str; 4] = ["primary", "updates", "receipts", "newsletters"];

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
}

impl Proposing {
    /// Every proposing tool, in the order they are offered.
    pub const ALL: [Self; 4] = [Self::Change, Self::Rule, Self::Reply, Self::Task];

    /// The name the model calls it by.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Change => "propose_change",
            Self::Rule => "propose_rule",
            Self::Reply => "propose_reply",
            Self::Task => "propose_task_change",
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
pub fn definitions() -> Vec<ToolDefinition> {
    let actions: Vec<&str> = ProposalAction::ALL.iter().map(|a| a.name()).collect();
    let task_actions: Vec<&str> = ProposalAction::TASKS
        .iter()
        .map(|a| a.task_name())
        .collect();
    let conversation_id = json!({
        "type": "string",
        "description": "The conversation's id, as `about` or another tool gave it.",
        "minLength": 1,
        "maxLength": 64,
    });
    vec![
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
    ]
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
}

/// `raw`'s arguments when they fit `tool`'s schema: only its properties,
/// each of the right type and length, and for a change only the detail its
/// action takes.
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
                        detail: None,
                        request: None,
                        undo: None,
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
                        detail: None,
                        request: None,
                        undo: None,
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
                        detail: Some(built.detail),
                        request: Some(built.request),
                        undo: Some(built.undo),
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
            detail: Some(detail),
            request: Some(request),
            undo: Some(undo),
            sentence: None,
            intent: None,
        })
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
            // Only a task's edit uses it (the client allows it for that
            // entry alone).
            "PATCH" if operation == "tasks.update" => ProposalMethod::Patch,
            _ => return Err(Refusal::Unavailable),
        };
        let path = fill(&info.path, params).ok_or(Refusal::Unavailable)?;
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

/// A fact, cleaned and bounded.
fn fact(text: &str) -> String {
    cut_bytes(&clean(text), FACT_BYTES).to_owned()
}

/// Whether two proposals are the same card.
fn same_card(one: &ChatProposal, other: &ChatProposal) -> bool {
    one.kind == other.kind
        && one.action == other.action
        && one.record == other.record
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_has_one_row_and_a_strict_schema() {
        for action in ProposalAction::ALL.into_iter().chain(ProposalAction::TASKS) {
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
        assert_eq!(
            ROWS.len(),
            ProposalAction::ALL.len() + ProposalAction::TASKS.len()
        );
        for definition in definitions() {
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
