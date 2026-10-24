//! The snapshots of the module's tracked events (ADR 0020,
//! campbooks-cloud#325): read from the module's own rows in the transaction
//! of the change, after it, and the mapping of the module's vocabulary onto
//! the plan's enums. Ids, enums, counts and times only: never a subject,
//! an address, a name, a body, a label's name or a rule's conditions.
//!
//! The mappings match the module's own enums exhaustively, so a value added
//! to the module without the plan fails to build rather than tracking
//! something the plan does not know.

use campbooks_kernel::{
    Result,
    tracking::{TrackedEvent, Tracker},
};
use chrono::{DateTime, Utc};
use sqlx::{PgConnection, Postgres, Transaction, types::Uuid};

use crate::{
    domain::{
        conversation::ConversationSummary,
        rules::{self, RuleAction, Stored},
    },
    model::{
        ClearCause, ConversationId, DraftId, DraftKind, DraftState, NeedBasis, NeedQuestion,
        NeedReason, NeedTrust, NeedVerdict, NoticeAnswer, NoticeKind, PausedReason, RuleId,
        RuleOrigin, RuleRunId, RuleState, RunKind, RunState, Stream, StreamSource,
    },
    tracking,
};

/// The tracked snapshot of a conversation, from its summary: ids, its
/// stream, where it is, counts and its last activity; never its subject or
/// who wrote it.
pub fn conversation(summary: &ConversationSummary) -> tracking::ConversationSnapshot {
    tracking::ConversationSnapshot {
        id: *summary.id.as_uuid(),
        inbox_id: *summary.inbox_id.as_uuid(),
        last_message_at: summary.last_message_at,
        message_count: i64::from(summary.message_count),
        starred: summary.starred,
        status: status(summary.spam, summary.trashed, summary.archived),
        stream: stream(summary.stream),
        stream_source: stream_source(summary.stream_source),
        unread_count: i64::from(summary.unread_count),
    }
}

/// A row of [`CONVERSATION`].
type ConversationRow = (
    Uuid,
    Uuid,
    String,
    String,
    bool,
    bool,
    bool,
    i32,
    i32,
    bool,
    DateTime<Utc>,
);

const CONVERSATION: &str = "SELECT id, inbox_id, stream, stream_source, \
            archived_at IS NOT NULL, trashed, spam, message_count, unread_count, starred, \
            last_message_at \
       FROM conversations.conversations WHERE id = $1";

/// The tracked snapshot of conversation `id` as it is in `conn` now, read
/// from its row alone (no summary); `None` once it is gone.
pub async fn conversation_now(
    conn: &mut PgConnection,
    id: ConversationId,
) -> Result<Option<tracking::ConversationSnapshot>> {
    let row: Option<ConversationRow> = sqlx::query_as(CONVERSATION)
        .bind(*id.as_uuid())
        .fetch_optional(&mut *conn)
        .await?;
    let Some((
        id,
        inbox_id,
        stream_name,
        source,
        archived,
        trashed,
        spam,
        messages,
        unread,
        starred,
        last,
    )) = row
    else {
        return Ok(None);
    };
    Ok(Some(tracking::ConversationSnapshot {
        id,
        inbox_id,
        last_message_at: last,
        message_count: i64::from(messages),
        starred,
        status: status(spam, trashed, archived),
        stream: stream(stream_name.parse()?),
        stream_source: stream_source(source.parse()?),
        unread_count: i64::from(unread),
    }))
}

/// Where a conversation is, from its flags (spam only ever with trashed).
const fn status(spam: bool, trashed: bool, archived: bool) -> tracking::ConversationStatus {
    if spam {
        tracking::ConversationStatus::Spam
    } else if trashed {
        tracking::ConversationStatus::Trash
    } else if archived {
        tracking::ConversationStatus::Archived
    } else {
        tracking::ConversationStatus::Inbox
    }
}

pub const fn stream(stream: Stream) -> tracking::Stream {
    match stream {
        Stream::Primary => tracking::Stream::Primary,
        Stream::Updates => tracking::Stream::Updates,
        Stream::Receipts => tracking::Stream::Receipts,
        Stream::Newsletters => tracking::Stream::Newsletters,
    }
}

pub const fn stream_source(source: StreamSource) -> tracking::StreamSource {
    match source {
        StreamSource::Default => tracking::StreamSource::Default,
        StreamSource::Rules => tracking::StreamSource::Rules,
        StreamSource::Model => tracking::StreamSource::Model,
        StreamSource::Taught => tracking::StreamSource::Taught,
        StreamSource::Member => tracking::StreamSource::Member,
    }
}

// --- Drafts ------------------------------------------------------------------

/// A row of [`DRAFT`].
type DraftRow = (
    Uuid,
    Uuid,
    Option<Uuid>,
    Option<Uuid>,
    String,
    String,
    i32,
    i32,
    i32,
    i32,
    i32,
    Option<DateTime<Utc>>,
    Option<String>,
);

const DRAFT: &str = "SELECT id, inbox_id, conversation_id, source_message_id, kind, state, \
            version, \
            CASE WHEN jsonb_typeof(to_recipients) = 'array' \
                 THEN jsonb_array_length(to_recipients) ELSE 0 END, \
            CASE WHEN jsonb_typeof(cc_recipients) = 'array' \
                 THEN jsonb_array_length(cc_recipients) ELSE 0 END, \
            CASE WHEN jsonb_typeof(bcc_recipients) = 'array' \
                 THEN jsonb_array_length(bcc_recipients) ELSE 0 END, \
            CASE WHEN jsonb_typeof(attachments) = 'array' \
                 THEN jsonb_array_length(attachments) ELSE 0 END, \
            send_at, failure_reason \
       FROM conversations.drafts WHERE id = $1";

/// The tracked snapshot of draft `id` as it is in `conn` now: ids, kind,
/// state and counts, never who it goes to or what it says; `None` once it
/// is gone.
pub async fn draft(
    conn: &mut PgConnection,
    id: DraftId,
) -> Result<Option<tracking::DraftSnapshot>> {
    let row: Option<DraftRow> = sqlx::query_as(DRAFT)
        .bind(*id.as_uuid())
        .fetch_optional(&mut *conn)
        .await?;
    let Some((
        id,
        inbox_id,
        conversation_id,
        message_id,
        kind,
        state,
        version,
        to,
        cc,
        bcc,
        attachments,
        send_at,
        failure,
    )) = row
    else {
        return Ok(None);
    };
    let state: DraftState = state.parse()?;
    Ok(Some(tracking::DraftSnapshot {
        attachment_count: i64::from(attachments),
        bcc_count: i64::from(bcc),
        cc_count: i64::from(cc),
        conversation_id,
        failure: failure
            .as_deref()
            .filter(|_| state == DraftState::Failed)
            .map(send_error),
        id,
        inbox_id,
        kind: draft_kind(kind.parse()?),
        message_id,
        send_at: send_at.filter(|_| state == DraftState::Scheduled),
        state: draft_state(state),
        to_count: i64::from(to),
        version: i64::from(version),
    }))
}

/// Track the event `make` builds from draft `id` as it is in `tx` now;
/// nothing when it is gone.
pub async fn track_draft<E: TrackedEvent>(
    tracker: &Tracker,
    tx: &mut Transaction<'_, Postgres>,
    id: DraftId,
    make: impl FnOnce(tracking::DraftSnapshot) -> E,
) -> Result<()> {
    if let Some(snapshot) = draft(tx, id).await? {
        tracker.track(tx, &make(snapshot)).await?;
    }
    Ok(())
}

pub const fn draft_kind(kind: DraftKind) -> tracking::DraftKind {
    match kind {
        DraftKind::New => tracking::DraftKind::New,
        DraftKind::Reply => tracking::DraftKind::Reply,
        DraftKind::ReplyAll => tracking::DraftKind::ReplyAll,
        DraftKind::Forward => tracking::DraftKind::Forward,
        DraftKind::Nudge => tracking::DraftKind::Nudge,
    }
}

pub const fn draft_state(state: DraftState) -> tracking::DraftState {
    match state {
        DraftState::Draft => tracking::DraftState::Draft,
        DraftState::Scheduled => tracking::DraftState::Scheduled,
        DraftState::Sending => tracking::DraftState::Sending,
        DraftState::Failed => tracking::DraftState::Failed,
    }
}

/// The kind of a send's failure, from the connector's reason
/// (`contracts/connectors.send_completed.v1.schema.json`): a reason the plan
/// does not know yet is `other`, never its text.
pub fn send_error(reason: &str) -> tracking::SendError {
    match reason {
        "not_connected" => tracking::SendError::NotConnected,
        "needs_reauth" => tracking::SendError::NeedsReauth,
        "rejected" => tracking::SendError::Rejected,
        "too_large" => tracking::SendError::TooLarge,
        "unavailable" => tracking::SendError::Unavailable,
        "unknown_outcome" => tracking::SendError::UnknownOutcome,
        _ => tracking::SendError::Other,
    }
}

// --- Needs -------------------------------------------------------------------

/// A row of [`NEED`].
type NeedRow = (
    Uuid,
    Uuid,
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    bool,
    bool,
);

const NEED: &str = "SELECT conversation_id, message_id, reason, question, trust, basis, state, \
            cleared_because, verdict, urgent, due_on IS NOT NULL \
       FROM conversations.needs WHERE conversation_id = $1";

/// The tracked snapshot of conversation `id`'s need as it is in `conn` now,
/// `None` when it has none: its reason, question, state and verdict, never
/// what it asks or by when.
pub async fn need(
    conn: &mut PgConnection,
    id: ConversationId,
) -> Result<Option<tracking::NeedSnapshot>> {
    let row: Option<NeedRow> = sqlx::query_as(NEED)
        .bind(*id.as_uuid())
        .fetch_optional(&mut *conn)
        .await?;
    let Some((
        conversation_id,
        message_id,
        reason,
        question,
        trust,
        basis,
        state,
        cleared_because,
        verdict,
        urgent,
        has_due_date,
    )) = row
    else {
        return Ok(None);
    };
    Ok(Some(tracking::NeedSnapshot {
        basis: need_basis(basis.parse()?),
        cleared_because: cleared_because
            .map(|c| c.parse().map(clear_cause))
            .transpose()?,
        conversation_id,
        has_due_date,
        message_id,
        question: need_question(question.parse()?),
        reason: need_reason(reason.parse()?),
        state: if state == "open" {
            tracking::NeedState::Open
        } else {
            tracking::NeedState::Cleared
        },
        trust: need_trust(trust.parse()?),
        urgent,
        verdict: verdict.map(|v| v.parse().map(need_verdict)).transpose()?,
    }))
}

const fn need_reason(reason: NeedReason) -> tracking::NeedReason {
    match reason {
        NeedReason::Reply => tracking::NeedReason::Reply,
        NeedReason::Do => tracking::NeedReason::Do,
        NeedReason::Pay => tracking::NeedReason::Pay,
        NeedReason::Review => tracking::NeedReason::Review,
        NeedReason::WorthALook => tracking::NeedReason::WorthALook,
        NeedReason::Waiting => tracking::NeedReason::Waiting,
        NeedReason::Nudge => tracking::NeedReason::Nudge,
    }
}

const fn need_question(question: NeedQuestion) -> tracking::NeedQuestion {
    match question {
        NeedQuestion::NeedsReply => tracking::NeedQuestion::NeedsReply,
        NeedQuestion::AsksAction => tracking::NeedQuestion::AsksAction,
        NeedQuestion::Suspicious => tracking::NeedQuestion::Suspicious,
        NeedQuestion::Money => tracking::NeedQuestion::Money,
        NeedQuestion::WorthALook => tracking::NeedQuestion::WorthALook,
        NeedQuestion::StillToDo => tracking::NeedQuestion::StillToDo,
        NeedQuestion::StillNeedsReply => tracking::NeedQuestion::StillNeedsReply,
        NeedQuestion::NextMove => tracking::NeedQuestion::NextMove,
    }
}

const fn need_trust(trust: NeedTrust) -> tracking::NeedTrust {
    match trust {
        NeedTrust::Sure => tracking::NeedTrust::Sure,
        NeedTrust::Check => tracking::NeedTrust::Check,
    }
}

const fn need_basis(basis: NeedBasis) -> tracking::NeedBasis {
    match basis {
        NeedBasis::Rules => tracking::NeedBasis::Rules,
        NeedBasis::Model => tracking::NeedBasis::Model,
        NeedBasis::Taught => tracking::NeedBasis::Taught,
        NeedBasis::Member => tracking::NeedBasis::Member,
    }
}

const fn clear_cause(cause: ClearCause) -> tracking::ClearCause {
    match cause {
        ClearCause::Replied => tracking::ClearCause::Replied,
        ClearCause::Archived => tracking::ClearCause::Archived,
        ClearCause::Trashed => tracking::ClearCause::Trashed,
        ClearCause::Done => tracking::ClearCause::Done,
        ClearCause::NotNeeded => tracking::ClearCause::NotNeeded,
        ClearCause::Superseded => tracking::ClearCause::Superseded,
        ClearCause::Rule => tracking::ClearCause::Rule,
        ClearCause::Opened => tracking::ClearCause::Opened,
        ClearCause::Lapsed => tracking::ClearCause::Lapsed,
        ClearCause::HeardBack => tracking::ClearCause::HeardBack,
    }
}

const fn need_verdict(verdict: NeedVerdict) -> tracking::NeedVerdict {
    match verdict {
        NeedVerdict::Done => tracking::NeedVerdict::Done,
        NeedVerdict::NotNeeded => tracking::NeedVerdict::NotNeeded,
        NeedVerdict::Confirmed => tracking::NeedVerdict::Confirmed,
        NeedVerdict::AlreadyPaid => tracking::NeedVerdict::AlreadyPaid,
        NeedVerdict::StillToPay => tracking::NeedVerdict::StillToPay,
    }
}

// --- Rules -------------------------------------------------------------------

/// The tracked snapshot of a taught rule, from its row: its state as
/// readers see it, how it was taught and the kinds of what it does; never
/// whom or what it matches, nor the label it puts on.
pub fn rule(rule: &Stored) -> tracking::RuleSnapshot {
    tracking::RuleSnapshot {
        actions: rule.actions.iter().map(rule_action).collect(),
        applied_count: i64::from(rule.applied_count),
        condition_count: i64::try_from(rule.conditions.len()).unwrap_or(i64::MAX),
        id: *rule.id.as_uuid(),
        inbox_id: *rule.inbox.as_uuid(),
        origin: rule_origin(rule.origin),
        paused_reason: rule.paused_reason().map(paused_reason),
        state: rule_state(rule.effective_state()),
    }
}

/// Track the event `make` builds from rule `id` as it is in `tx` now
/// (removed or not); nothing when it is gone.
pub async fn track_rule<E: TrackedEvent>(
    tracker: &Tracker,
    tx: &mut Transaction<'_, Postgres>,
    id: RuleId,
    make: impl FnOnce(tracking::RuleSnapshot) -> E,
) -> Result<()> {
    if let Some(stored) = rules::find_any(tx, id).await? {
        tracker.track(tx, &make(rule(&stored))).await?;
    }
    Ok(())
}

const fn rule_action(action: &RuleAction) -> tracking::RuleAction {
    match action {
        RuleAction::MoveToStream { .. } => tracking::RuleAction::MoveToStream,
        RuleAction::Archive => tracking::RuleAction::Archive,
        RuleAction::AddLabel { .. } => tracking::RuleAction::AddLabel,
        RuleAction::MarkRead => tracking::RuleAction::MarkRead,
        RuleAction::NeedsReply { .. } => tracking::RuleAction::NeedsReply,
        RuleAction::AsksAction { .. } => tracking::RuleAction::AsksAction,
        RuleAction::Surface { .. } => tracking::RuleAction::Surface,
        RuleAction::Trash => tracking::RuleAction::Trash,
        RuleAction::Spam => tracking::RuleAction::Spam,
        RuleAction::MarkPaid => tracking::RuleAction::MarkPaid,
        RuleAction::ShowImages => tracking::RuleAction::ShowImages,
        RuleAction::WorthALook => tracking::RuleAction::WorthALook,
    }
}

pub const fn rule_state(state: RuleState) -> tracking::RuleState {
    match state {
        RuleState::Proposed => tracking::RuleState::Proposed,
        RuleState::Active => tracking::RuleState::Active,
        RuleState::Paused => tracking::RuleState::Paused,
        RuleState::Removed => tracking::RuleState::Removed,
    }
}

const fn paused_reason(reason: PausedReason) -> tracking::PausedReason {
    match reason {
        PausedReason::InboxReadOnly => tracking::PausedReason::InboxReadOnly,
        PausedReason::DomainTooBroad => tracking::PausedReason::DomainTooBroad,
        PausedReason::AuthorCannotWrite => tracking::PausedReason::AuthorCannotWrite,
        PausedReason::PausedByMember => tracking::PausedReason::PausedByMember,
        PausedReason::AuthorUnavailable => tracking::PausedReason::AuthorUnavailable,
    }
}

const fn rule_origin(origin: RuleOrigin) -> tracking::RuleOrigin {
    match origin {
        RuleOrigin::Action => tracking::RuleOrigin::Action,
        RuleOrigin::Onboarding => tracking::RuleOrigin::Onboarding,
        RuleOrigin::Instruction => tracking::RuleOrigin::Instruction,
        RuleOrigin::Correction => tracking::RuleOrigin::Correction,
        RuleOrigin::Suggestion => tracking::RuleOrigin::Suggestion,
    }
}

/// A row of [`RULE_RUN`].
type RuleRunRow = (Uuid, Uuid, String, String, i32, i32, i32, i32, i32);

const RULE_RUN: &str = "SELECT id, rule_id, kind, state, total, done, changed, \
            skipped_suspicious, skipped_member \
       FROM conversations.rule_runs WHERE id = $1";

/// The tracked snapshot of rule run `id` as it is in `conn` now: its kind,
/// state and counts.
pub async fn rule_run(
    conn: &mut PgConnection,
    id: RuleRunId,
) -> Result<Option<tracking::RuleRunSnapshot>> {
    let row: Option<RuleRunRow> = sqlx::query_as(RULE_RUN)
        .bind(*id.as_uuid())
        .fetch_optional(&mut *conn)
        .await?;
    let Some((id, rule_id, kind, state, total, done, changed, suspicious, member)) = row else {
        return Ok(None);
    };
    Ok(Some(tracking::RuleRunSnapshot {
        changed: i64::from(changed),
        done: i64::from(done),
        id,
        kind: match kind.parse()? {
            RunKind::Apply => tracking::RunKind::Apply,
            RunKind::Revert => tracking::RunKind::Revert,
        },
        rule_id,
        skipped_member: i64::from(member),
        skipped_suspicious: i64::from(suspicious),
        state: match state.parse()? {
            RunState::Running => tracking::RunState::Running,
            RunState::Done => tracking::RunState::Done,
            RunState::Cancelled => tracking::RunState::Cancelled,
        },
        total: i64::from(total),
    }))
}

// --- Suggestions and reads ---------------------------------------------------

pub const fn notice_kind(kind: NoticeKind) -> tracking::NoticeKind {
    match kind {
        NoticeKind::KeepOffHome => tracking::NoticeKind::KeepOffHome,
        NoticeKind::ArchiveOnArrival => tracking::NoticeKind::ArchiveOnArrival,
    }
}

pub const fn notice_answer(answer: NoticeAnswer) -> tracking::NoticeAnswer {
    match answer {
        NoticeAnswer::NotNow => tracking::NoticeAnswer::NotNow,
        NoticeAnswer::Never => tracking::NoticeAnswer::Never,
    }
}

/// A count as the plan carries it.
pub fn count(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}
