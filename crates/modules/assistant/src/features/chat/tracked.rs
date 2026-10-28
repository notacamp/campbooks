//! The snapshots the chats' tracked events carry (ADR 0020), built from the
//! module's own rows read in the transaction of the change, and the plan's
//! enums for their values.
//!
//! A chat is content: ids, record references, enums, counts and times only.
//! Never a question, an answer, a chat's first question, or anything the
//! model wrote (a rule's sentence, a reply's intent, a label, an event's
//! title, a teammate's name): a proposal is its kind, its action and the
//! record it is about.

use crate::tracking;

use super::{
    store::ChatRow,
    types::{
        ChatFinish, ChatId, ChatMessageView, ChatProposal, ChatTool, ProposalAction, ProposalKind,
        ProposalState,
    },
};

/// The plan's snapshot of chat `row`.
pub fn chat(row: &ChatRow) -> tracking::ChatSnapshot {
    tracking::ChatSnapshot {
        id: *row.id.as_uuid(),
        about: row.about(),
        questions: i64::from(row.questions),
        created_at: row.created_at,
        last_message_at: row.last_message_at,
    }
}

/// The plan's snapshot of Scout's stored answer `answer` in `chat`, with the
/// kind of failure the answer loop reported, when it failed.
pub fn answer(
    chat: ChatId,
    answer: &ChatMessageView,
    failure: Option<&str>,
) -> tracking::AnswerSnapshot {
    tracking::AnswerSnapshot {
        id: *answer.id.as_uuid(),
        chat_id: *chat.as_uuid(),
        finish: match answer.finish.unwrap_or(ChatFinish::Failed) {
            ChatFinish::Done => tracking::AnswerFinish::Done,
            ChatFinish::CutShort => tracking::AnswerFinish::CutShort,
            ChatFinish::Failed => tracking::AnswerFinish::Failed,
        },
        tools: answer.steps.iter().map(|step| tool(step.tool)).collect(),
        proposals: i64::try_from(answer.proposals.len()).unwrap_or(i64::MAX),
        failure: failure.and_then(failure_kind),
    }
}

/// The plan's kind of a failed answer, from the answer loop's word for it
/// (`Failure::kind`, `answer::error_kind`). A word the plan does not know is
/// left out rather than guessed.
fn failure_kind(kind: &str) -> Option<tracking::AnswerFailure> {
    use tracking::AnswerFailure as F;
    Some(match kind {
        "over_cap" | "over_daily_cap" => F::OverCap,
        "rounds" => F::Rounds,
        "timed_out" => F::TimedOut,
        "provider" => F::Provider,
        "timeout" => F::Timeout,
        "transport" => F::Transport,
        "status" => F::Status,
        "invalid_response" => F::InvalidResponse,
        "config" => F::Config,
        "external_content_refused" => F::ExternalContentRefused,
        "role_not_configured" => F::RoleNotConfigured,
        "answers_not_trusted" => F::AnswersNotTrusted,
        "unattributed" => F::Unattributed,
        "metering" => F::Metering,
        _ => return None,
    })
}

/// The plan's snapshot of proposal `proposal`, made in `chat`.
pub fn proposal(chat: ChatId, proposal: &ChatProposal) -> tracking::ProposalSnapshot {
    tracking::ProposalSnapshot {
        id: *proposal.id.as_uuid(),
        chat_id: *chat.as_uuid(),
        kind: match proposal.kind {
            ProposalKind::Change => tracking::ProposalKind::Change,
            ProposalKind::Rule => tracking::ProposalKind::Rule,
            ProposalKind::Reply => tracking::ProposalKind::Reply,
        },
        state: match proposal.state {
            ProposalState::Pending => tracking::ProposalState::Pending,
            ProposalState::Done => tracking::ProposalState::Done,
            ProposalState::Declined => tracking::ProposalState::Declined,
            ProposalState::Failed => tracking::ProposalState::Failed,
            ProposalState::Undone => tracking::ProposalState::Undone,
        },
        action: proposal.action.map(action),
        record: proposal.record.clone(),
    }
}

/// The plan's tool.
fn tool(tool: ChatTool) -> tracking::ChatTool {
    use tracking::ChatTool as T;
    match tool {
        ChatTool::FindConversations => T::FindConversations,
        ChatTool::ReadConversation => T::ReadConversation,
        ChatTool::ConversationHistory => T::ConversationHistory,
        ChatTool::FindPeople => T::FindPeople,
        ChatTool::WhatNeedsMe => T::WhatNeedsMe,
        ChatTool::MyInboxes => T::MyInboxes,
        ChatTool::MyTasks => T::MyTasks,
        ChatTool::ReadTask => T::ReadTask,
        ChatTool::MyCalendar => T::MyCalendar,
        ChatTool::FreeTime => T::FreeTime,
        ChatTool::ReadEvent => T::ReadEvent,
        ChatTool::FindPages => T::FindPages,
        ChatTool::ReadPage => T::ReadPage,
    }
}

/// The plan's proposal action.
fn action(action: ProposalAction) -> tracking::ProposalAction {
    use tracking::ProposalAction as A;
    match action {
        ProposalAction::Archive => A::Archive,
        ProposalAction::Unarchive => A::Unarchive,
        ProposalAction::MarkRead => A::MarkRead,
        ProposalAction::MarkUnread => A::MarkUnread,
        ProposalAction::Star => A::Star,
        ProposalAction::Unstar => A::Unstar,
        ProposalAction::AddLabel => A::AddLabel,
        ProposalAction::RemoveLabel => A::RemoveLabel,
        ProposalAction::MoveToStream => A::MoveToStream,
        ProposalAction::Trash => A::Trash,
        ProposalAction::Untrash => A::Untrash,
        ProposalAction::ReportSpam => A::ReportSpam,
        ProposalAction::NotSpam => A::NotSpam,
        ProposalAction::Done => A::Done,
        ProposalAction::NotNeeded => A::NotNeeded,
        ProposalAction::AlreadyPaid => A::AlreadyPaid,
        ProposalAction::StillToPay => A::StillToPay,
        ProposalAction::YesToQuestion => A::YesToQuestion,
        ProposalAction::Later => A::Later,
        ProposalAction::TaskDone => A::TaskDone,
        ProposalAction::TaskNotNeeded => A::TaskNotNeeded,
        ProposalAction::PlanDay => A::PlanDay,
        ProposalAction::TaskLater => A::TaskLater,
        ProposalAction::Due => A::Due,
        ProposalAction::HandOff => A::HandOff,
        ProposalAction::CreateEvent => A::CreateEvent,
        ProposalAction::UpdateEvent => A::UpdateEvent,
        ProposalAction::DeleteEvent => A::DeleteEvent,
        ProposalAction::RespondEvent => A::RespondEvent,
        ProposalAction::PushEvent => A::PushEvent,
        ProposalAction::UnpushEvent => A::UnpushEvent,
        ProposalAction::AppendSection => A::AppendSection,
        ProposalAction::NewPage => A::NewPage,
    }
}
