//! `GET /api/v1/w/{workspace_id}/conversations/{conversation_id}/history` —
//! what the records say happened to a conversation, and who did it
//! (campbooks-cloud#305): its messages, Scout's answers about them, the
//! corrections members made, what taught rules did to it, and its need.
//!
//! A read for people and for Scout's chat (the assistant may call it): the
//! same access as reading the conversation, and no message text. Scout's
//! answers are read as "Suggestions with their reasons" reads them: a model
//! answer counts only when given under a passed answers check.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, RecordRef, Result,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{UserId, Workspace, WorkspaceId},
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

use crate::{
    domain::{
        access::visible_inboxes,
        paths::ConversationPath,
        resources,
        rules::{RuleAction, RuleCondition},
        suggestions::{AnswerView, DecisionSource},
    },
    events::CONVERSATION_RECORD,
    model::{Address, ConversationId, MessageId, RuleId},
    scopes::ConversationsRead,
};

/// Most events answered: the newest, oldest first.
pub const MAX_EVENTS: usize = 200;
/// Newest messages read, with Scout's answers about them.
const MAX_MESSAGES: i64 = 100;
/// Newest corrections read.
const MAX_CORRECTIONS: i64 = 100;
/// Newest rule applications read.
const MAX_APPLICATIONS: i64 = 50;

/// Who did something.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum HistoryActor {
    /// The caller.
    You,
    /// Another member (or an app acting for nobody).
    Member,
    /// A taught rule.
    Rule,
    /// Campbooks on its own: Scout, or what the mail itself did (a reply,
    /// time passing).
    Scout,
}

/// A taught rule, as an event names it: its id, and its conditions and
/// actions, which clients read as its sentence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RuleRef {
    /// The rule.
    pub id: RuleId,
    /// When it applies.
    pub conditions: Vec<RuleCondition>,
    /// What it does.
    pub actions: Vec<RuleAction>,
}

/// One thing that happened, by kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HistoryEvent {
    /// A message arrived.
    Received {
        /// When it was sent.
        at: DateTime<Utc>,
        /// The message.
        message_id: MessageId,
        /// Who wrote it.
        from: Address,
    },
    /// The inbox sent a message.
    Sent {
        /// When it was sent.
        at: DateTime<Utc>,
        /// The message.
        message_id: MessageId,
        /// Who wrote it.
        from: Address,
    },
    /// One of Scout's answers about a message.
    ScoutRead {
        /// When it was given.
        at: DateTime<Utc>,
        /// The message it is about.
        message_id: MessageId,
        /// The question (`stream`, `needs_reply`, `money`, …).
        question: String,
        /// The answer.
        value: AnswerView,
        /// Who answered.
        source: DecisionSource,
        /// How sure, 0 to 100, when the source says.
        confidence: Option<u8>,
        /// Whether it changed anything; otherwise it was a suggestion.
        acted: bool,
    },
    /// A member changed an answer.
    Corrected {
        /// When.
        at: DateTime<Utc>,
        /// The question.
        question: String,
        /// The answer before.
        from: String,
        /// The answer after.
        to: String,
        /// `you` or `member`.
        by: HistoryActor,
    },
    /// A taught rule changed the conversation.
    RuleApplied {
        /// When.
        at: DateTime<Utc>,
        /// The rule.
        rule: RuleRef,
    },
    /// What a rule did was put back (the rule was undone).
    RuleReverted {
        /// When.
        at: DateTime<Utc>,
        /// The rule.
        rule: RuleRef,
    },
    /// A member changed back what a rule did: the rule leaves the
    /// conversation alone.
    RuleExcepted {
        /// When.
        at: DateTime<Utc>,
        /// The rule.
        rule: RuleRef,
        /// `you` or `member`.
        by: HistoryActor,
    },
    /// The conversation's need, raised by one of its messages.
    NeedOpened {
        /// When it was raised: Scout's answer raising it, else its last change.
        at: DateTime<Utc>,
        /// `reply`, `do`, `pay`, `review`, `worth_a_look`, `waiting` or
        /// `nudge`.
        reason: String,
        /// What raised it: `rules`, `model`, `taught` or `member`.
        basis: String,
        /// When it is due, when known.
        due_on: Option<NaiveDate>,
        /// When it lapses on its own, when it does.
        lapses_at: Option<DateTime<Utc>>,
        /// When it next changes with time alone, when it does.
        wakes_at: Option<DateTime<Utc>>,
        /// Whether it is still open.
        open: bool,
    },
    /// The need left the list.
    NeedCleared {
        /// When.
        at: DateTime<Utc>,
        /// Why: `replied`, `archived`, `trashed`, `done`, `not_needed`,
        /// `superseded`, `rule`, `opened`, `lapsed` or `heard_back`.
        because: String,
        /// Who.
        by: HistoryActor,
    },
    /// A member's verdict on the need.
    NeedVerdict {
        /// When.
        at: DateTime<Utc>,
        /// `done`, `not_needed`, `confirmed`, `already_paid` or
        /// `still_to_pay`.
        verdict: String,
        /// `you` or `member`.
        by: HistoryActor,
    },
}

impl HistoryEvent {
    fn at(&self) -> DateTime<Utc> {
        match self {
            Self::Received { at, .. }
            | Self::Sent { at, .. }
            | Self::ScoutRead { at, .. }
            | Self::Corrected { at, .. }
            | Self::RuleApplied { at, .. }
            | Self::RuleReverted { at, .. }
            | Self::RuleExcepted { at, .. }
            | Self::NeedOpened { at, .. }
            | Self::NeedCleared { at, .. }
            | Self::NeedVerdict { at, .. } => *at,
        }
    }
}

/// What happened to a conversation, oldest first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct ConversationHistory {
    /// The conversation.
    pub conversation_id: ConversationId,
    /// At most the 200 newest events, oldest first.
    pub events: Vec<HistoryEvent>,
    /// Older events were left out.
    pub truncated: bool,
}

/// The newest messages: id, direction, sender, sent.
const MESSAGES: &str = "SELECT id, direction, sender, sent_at FROM conversations.messages \
     WHERE conversation_id = $1 ORDER BY sent_at DESC, id DESC LIMIT $2";

/// Scout's answers about those messages: per message and question, the one
/// the client shows (taught before model before the rest, newest), and a
/// model's only when given under a passed answers check.
const DECISIONS: &str = "SELECT DISTINCT ON (message_id, question) \
            message_id, question, value, source, \
            round(confidence * 100)::smallint AS confidence, acted, created_at \
       FROM conversations.decisions \
      WHERE conversation_id = $1 AND message_id = ANY($2) \
        AND (source <> 'model' OR model_version LIKE 'checked:%') \
      ORDER BY message_id, question, (source = 'taught') DESC, (source = 'model') DESC, \
               created_at DESC";

/// The corrections still standing (not taken back), newest first.
const CORRECTIONS: &str = "SELECT question, from_value, to_value, corrected_by, created_at \
       FROM conversations.corrections \
      WHERE conversation_id = $1 AND withdrawn_at IS NULL \
      ORDER BY created_at DESC LIMIT $2";

/// What taught rules did to it, newest first, with the rule.
const APPLICATIONS: &str = "SELECT r.id, r.conditions, r.actions, a.applied_at, a.reverted_at, \
            a.excepted_at, a.excepted_by \
       FROM conversations.rule_applications a \
       JOIN conversations.rules r ON r.id = a.rule_id \
      WHERE a.conversation_id = $1 \
      ORDER BY a.applied_at DESC LIMIT $2";

/// The conversation's need; when it was raised (the answer that raised it,
/// else its last change), its clock, and when its verdict was given.
const NEED: &str = "SELECT n.reason, n.basis, n.due_on, n.lapses_at, n.state, n.cleared_because, \
            n.cleared_by, n.cleared_at, n.verdict, n.verdict_by, n.updated_at, \
            coalesce(d.created_at, n.updated_at) AS raised_at, k.wake_at, \
            v.created_at AS verdict_at \
       FROM conversations.needs n \
       LEFT JOIN conversations.decisions d ON d.id = n.decision_id \
       LEFT JOIN conversations.need_clocks k ON k.conversation_id = n.conversation_id \
       LEFT JOIN conversations.corrections v ON v.id = n.verdict_correction_id \
      WHERE n.conversation_id = $1";

#[derive(sqlx::FromRow)]
struct MessageRow {
    id: MessageId,
    direction: String,
    sender: Value,
    sent_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct DecisionRow {
    message_id: MessageId,
    question: String,
    value: Value,
    source: String,
    confidence: Option<i16>,
    acted: bool,
    created_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct CorrectionRow {
    question: String,
    from_value: String,
    to_value: String,
    corrected_by: Option<sqlx::types::Uuid>,
    created_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct ApplicationRow {
    id: RuleId,
    conditions: Value,
    actions: Value,
    applied_at: DateTime<Utc>,
    reverted_at: Option<DateTime<Utc>>,
    excepted_at: Option<DateTime<Utc>>,
    excepted_by: Option<sqlx::types::Uuid>,
}

#[derive(sqlx::FromRow)]
struct NeedRow {
    reason: String,
    basis: String,
    due_on: Option<NaiveDate>,
    lapses_at: Option<DateTime<Utc>>,
    state: String,
    cleared_because: Option<String>,
    cleared_by: Option<sqlx::types::Uuid>,
    cleared_at: Option<DateTime<Utc>>,
    verdict: Option<String>,
    verdict_by: Option<sqlx::types::Uuid>,
    updated_at: DateTime<Utc>,
    raised_at: Option<DateTime<Utc>>,
    wake_at: Option<DateTime<Utc>>,
    verdict_at: Option<DateTime<Utc>>,
}

/// What happened to a conversation, and who did it.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/conversations/{conversation_id}/history",
    operation_id = "conversations.get_history",
    tag = "conversations",
    security(("bearer" = ["conversations:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant()
        .records(["conversations.conversation", "conversations.message", "conversations.rule"])
        .access("conversations.conversation", campbooks_kernel::authz::Action::Read)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("conversation_id" = ConversationId, Path, description = "The conversation."),
    ),
    responses(
        (status = OK, description = "What happened to it, oldest first; no message text.",
            body = ConversationHistory),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such conversation, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsRead>,
    extract::Path(path): extract::Path<ConversationPath>,
) -> Result<Json<ConversationHistory>> {
    let id = path.conversation_id;
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    resources::authorize(
        &state,
        &mut tx,
        access,
        Action::Read,
        &RecordRef::new(CONVERSATION_RECORD, &id),
    )
    .await?;
    let inboxes = visible_inboxes(&mut tx, access).await?;
    let visible: Option<i32> = sqlx::query_scalar(
        "SELECT 1 FROM conversations.conversations WHERE id = $1 AND inbox_id = ANY($2)",
    )
    .bind(id)
    .bind(inboxes.as_slice())
    .fetch_optional(&mut *tx)
    .await?;
    if visible.is_none() {
        return Err(Error::NotFound);
    }
    let messages: Vec<MessageRow> = sqlx::query_as(MESSAGES)
        .bind(id)
        .bind(MAX_MESSAGES)
        .fetch_all(&mut *tx)
        .await?;
    let ids: Vec<MessageId> = messages.iter().map(|m| m.id).collect();
    let decisions: Vec<DecisionRow> = sqlx::query_as(DECISIONS)
        .bind(id)
        .bind(ids.as_slice())
        .fetch_all(&mut *tx)
        .await?;
    let corrections: Vec<CorrectionRow> = sqlx::query_as(CORRECTIONS)
        .bind(id)
        .bind(MAX_CORRECTIONS)
        .fetch_all(&mut *tx)
        .await?;
    let applications: Vec<ApplicationRow> = sqlx::query_as(APPLICATIONS)
        .bind(id)
        .bind(MAX_APPLICATIONS)
        .fetch_all(&mut *tx)
        .await?;
    let need: Option<NeedRow> = sqlx::query_as(NEED)
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
    tx.commit().await?;
    let mut events = Records {
        messages,
        decisions,
        corrections,
        applications,
        need,
    }
    .events(access.user)?;
    // Oldest first; at the same instant, the order they were read in (a
    // message before Scout's answers about it).
    events.sort_by_key(HistoryEvent::at);
    let truncated = events.len() > MAX_EVENTS;
    if truncated {
        events.drain(..events.len() - MAX_EVENTS);
    }
    Ok(Json(ConversationHistory {
        conversation_id: id,
        events,
        truncated,
    }))
}

/// The rows read about one conversation.
struct Records {
    messages: Vec<MessageRow>,
    decisions: Vec<DecisionRow>,
    corrections: Vec<CorrectionRow>,
    applications: Vec<ApplicationRow>,
    need: Option<NeedRow>,
}

impl Records {
    /// Their events, unordered, for the caller `you`.
    fn events(self, you: Option<UserId>) -> Result<Vec<HistoryEvent>> {
        let mut events = Vec::new();
        for message in self.messages {
            let from: Address = serde_json::from_value(message.sender)
                .map_err(|e| Error::internal(crate::domain::redact::json(&e)))?;
            events.push(if message.direction == "outbound" {
                HistoryEvent::Sent {
                    at: message.sent_at,
                    message_id: message.id,
                    from,
                }
            } else {
                HistoryEvent::Received {
                    at: message.sent_at,
                    message_id: message.id,
                    from,
                }
            });
        }
        for decision in self.decisions {
            let value = serde_json::from_value(decision.value)
                .map_err(|e| Error::internal(crate::domain::redact::json(&e)))?;
            events.push(HistoryEvent::ScoutRead {
                at: decision.created_at,
                message_id: decision.message_id,
                question: decision.question,
                value,
                source: DecisionSource::parse(&decision.source)?,
                confidence: decision.confidence.and_then(|c| u8::try_from(c).ok()),
                acted: decision.acted,
            });
        }
        for correction in self.corrections {
            events.push(HistoryEvent::Corrected {
                at: correction.created_at,
                question: correction.question,
                from: correction.from_value,
                to: correction.to_value,
                by: member(you, correction.corrected_by),
            });
        }
        for application in self.applications {
            let rule = RuleRef {
                id: application.id,
                conditions: serde_json::from_value(application.conditions).unwrap_or_default(),
                actions: serde_json::from_value(application.actions).unwrap_or_default(),
            };
            if let Some(at) = application.excepted_at {
                events.push(HistoryEvent::RuleExcepted {
                    at,
                    rule: rule.clone(),
                    by: member(you, application.excepted_by),
                });
            }
            if let Some(at) = application.reverted_at {
                events.push(HistoryEvent::RuleReverted {
                    at,
                    rule: rule.clone(),
                });
            }
            events.push(HistoryEvent::RuleApplied {
                at: application.applied_at,
                rule,
            });
        }
        if let Some(need) = self.need {
            need_events(need, you, &mut events);
        }
        Ok(events)
    }
}

/// The need's events: raised, then its verdict, then why it left the list.
fn need_events(need: NeedRow, you: Option<UserId>, events: &mut Vec<HistoryEvent>) {
    let open = need.state == "open";
    events.push(HistoryEvent::NeedOpened {
        at: need.raised_at.unwrap_or(need.updated_at),
        reason: need.reason,
        basis: need.basis,
        due_on: need.due_on,
        lapses_at: need.lapses_at,
        wakes_at: if open { need.wake_at } else { None },
        open,
    });
    if let Some(verdict) = need.verdict {
        events.push(HistoryEvent::NeedVerdict {
            at: need.verdict_at.unwrap_or(need.updated_at),
            verdict,
            by: member(you, need.verdict_by),
        });
    }
    if !open && let Some(because) = need.cleared_because {
        let by = match (need.cleared_by, because.as_str()) {
            (Some(_), _) => member(you, need.cleared_by),
            (None, "rule") => HistoryActor::Rule,
            (None, _) => HistoryActor::Scout,
        };
        events.push(HistoryEvent::NeedCleared {
            at: need.cleared_at.unwrap_or(need.updated_at),
            because,
            by,
        });
    }
}

/// `you` when `by` is the caller, else `member`.
fn member(you: Option<UserId>, by: Option<sqlx::types::Uuid>) -> HistoryActor {
    match (you, by) {
        (Some(you), Some(by)) if *you.as_uuid() == by => HistoryActor::You,
        _ => HistoryActor::Member,
    }
}

#[cfg(test)]
mod tests;
