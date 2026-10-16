//! Taught rules (campbooks-cloud#125): deterministic condition → action
//! pairs a member taught Campbooks for one inbox.
//!
//! The engine (`campbooks_ai::decide::taught`) decides; this module stores
//! rules ([`Stored`]), reads the facts the engine needs from its own tables
//! ([`facts`]), and carries out an outcome on a conversation with the same
//! domain functions the member's operations use ([`apply`], [`arrival`]),
//! recording each change with what it was before so it can be put back.
//!
//! A conversation matches a rule when its newest inbound message does. A
//! rule acts only while its author still may change the inbox; a member who
//! changed back what a rule did makes that conversation an **exception** the
//! rule leaves alone ([`exceptions`]).

mod apply;
mod arrival;
mod effects;
mod exceptions;
mod facts;

use campbooks_ai::decide::{
    signals::email::{TEMPLATE_MAX_CHARS, subject_words},
    taught::{self, Surface, TaughtRule},
};
use campbooks_kernel::{Error, Result, tenancy::UserId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgConnection;
use utoipa::ToSchema;

pub use apply::{Applied, Before, Context, acting_on, apply, revert};
pub use arrival::{Arrival, on_arrival};
pub use exceptions::{RuleException, RuleMark, Undone, attach, except, recent_exceptions_of};
pub use facts::{
    Candidate, TEMPLATE_PAGE, candidate, corresponds, fill_templates, scan, templates_missing,
};

use crate::model::{
    ConversationId, InboxId, InboxRole, LabelId, RuleId, RuleOrigin, RuleState, Stream, TeachingId,
};

/// How far back exceptions count when a rule is questioned.
pub const EXCEPTION_DAYS: i64 = 30;

// --- The wire shape ----------------------------------------------------------

/// A stored header a condition may test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum HeaderField {
    /// `List-Id`.
    ListId,
    /// `List-Unsubscribe`.
    ListUnsubscribe,
    /// `Precedence`.
    Precedence,
    /// `Auto-Submitted`.
    AutoSubmitted,
}

/// A header test: present, or equal to a value.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub struct HeaderCondition {
    /// Which header.
    pub name: HeaderField,
    /// The value it must have (case-insensitive); absent means "present".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

/// One test on a message; all of a rule's conditions must hold.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(tag = "field", content = "value", rename_all = "snake_case")]
pub enum RuleCondition {
    /// The sender's address is this one.
    SenderAddress(String),
    /// The sender's domain is this one, or a subdomain of it (never a free
    /// mail domain on its own).
    SenderDomain(String),
    /// The `List-Id` is this one.
    ListId(String),
    /// The subject's template is this one ("your order # has shipped").
    SubjectTemplate(String),
    /// The subject holds these words, in this order (case and accents ignored).
    SubjectContains(String),
    /// A stored header is present, or has a value.
    Header(HeaderCondition),
    /// The conversation has this label.
    Label(LabelId),
    /// The conversation is in this stream.
    Stream(Stream),
}

/// Always or never show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceWhen {
    /// Never hide it.
    Always,
    /// It never needs anyone.
    Never,
}

/// What a rule does; at most one of each kind, labels excepted.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum RuleAction {
    /// Put the conversation in this stream.
    MoveToStream {
        /// The stream.
        stream: Stream,
    },
    /// Take it out of the inbox.
    Archive,
    /// Put this label on it.
    AddLabel {
        /// The label.
        label: LabelId,
    },
    /// Mark it read.
    MarkRead,
    /// Whether it needs a reply.
    NeedsReply {
        /// Yes or no.
        value: bool,
    },
    /// Whether it asks for an action.
    AsksAction {
        /// Yes or no.
        value: bool,
    },
    /// Always or never show it.
    Surface {
        /// Which.
        value: SurfaceWhen,
    },
}

impl From<&RuleCondition> for taught::Condition {
    fn from(c: &RuleCondition) -> Self {
        match c {
            RuleCondition::SenderAddress(v) => Self::SenderAddress(v.trim().to_lowercase()),
            RuleCondition::SenderDomain(v) => Self::SenderDomain(v.trim().to_lowercase()),
            RuleCondition::ListId(v) => Self::ListId(v.trim().to_owned()),
            RuleCondition::SubjectTemplate(v) => Self::SubjectTemplate(v.trim().to_owned()),
            RuleCondition::SubjectContains(v) => Self::SubjectContains(v.trim().to_owned()),
            RuleCondition::Header(h) => Self::Header(taught::HeaderTest {
                name: match h.name {
                    HeaderField::ListId => taught::HeaderName::ListId,
                    HeaderField::ListUnsubscribe => taught::HeaderName::ListUnsubscribe,
                    HeaderField::Precedence => taught::HeaderName::Precedence,
                    HeaderField::AutoSubmitted => taught::HeaderName::AutoSubmitted,
                },
                value: h.value.clone(),
            }),
            RuleCondition::Label(l) => Self::Label(l.to_string()),
            RuleCondition::Stream(s) => Self::Stream(s.as_str().to_owned()),
        }
    }
}

impl From<&RuleAction> for taught::Action {
    fn from(a: &RuleAction) -> Self {
        match a {
            RuleAction::MoveToStream { stream } => Self::MoveToStream {
                stream: stream.as_str().to_owned(),
            },
            RuleAction::Archive => Self::Archive,
            RuleAction::AddLabel { label } => Self::AddLabel {
                label: label.to_string(),
            },
            RuleAction::MarkRead => Self::MarkRead,
            RuleAction::NeedsReply { value } => Self::NeedsReply { value: *value },
            RuleAction::AsksAction { value } => Self::AsksAction { value: *value },
            RuleAction::Surface { value } => Self::Surface {
                value: match value {
                    SurfaceWhen::Always => Surface::Always,
                    SurfaceWhen::Never => Surface::Never,
                },
            },
        }
    }
}

/// The engine's view of `conditions` and `actions`.
pub fn engine_parts(
    conditions: &[RuleCondition],
    actions: &[RuleAction],
) -> (Vec<taught::Condition>, Vec<taught::Action>) {
    (
        conditions.iter().map(Into::into).collect(),
        actions.iter().map(Into::into).collect(),
    )
}

/// Longest value a condition may hold, in characters (an address is at
/// most 320).
pub const MAX_VALUE_CHARS: usize = 320;

/// Check a rule's shape: 400 `invalid_rule` when it is not one.
pub fn validate(conditions: &[RuleCondition], actions: &[RuleAction]) -> Result<()> {
    let invalid = || Error::BadRequest(crate::MESSAGES.reason("invalid_rule"));
    let too_long = conditions.iter().any(|c| match c {
        RuleCondition::SenderAddress(v)
        | RuleCondition::SenderDomain(v)
        | RuleCondition::ListId(v)
        | RuleCondition::SubjectContains(v) => v.chars().count() > MAX_VALUE_CHARS,
        // No stored template is longer: a longer one could never match.
        RuleCondition::SubjectTemplate(v) => v.chars().count() > TEMPLATE_MAX_CHARS,
        RuleCondition::Header(h) => h
            .value
            .as_ref()
            .is_some_and(|v| v.chars().count() > MAX_VALUE_CHARS),
        RuleCondition::Label(_) | RuleCondition::Stream(_) => false,
    });
    if too_long {
        return Err(invalid());
    }
    let (c, a) = engine_parts(conditions, actions);
    TaughtRule::validate(&c, &a).map_err(|_| invalid())
}

/// `conditions` as stored and compared, so the same rule is always written
/// the same way: trimmed; addresses, domains and templates lower-cased; list
/// ids as the engine compares them (no angle brackets); subject words as the
/// engine reads them (lower-cased, accents dropped, one space apart); and
/// sorted, without repeats.
pub fn normalise(conditions: &[RuleCondition]) -> Vec<RuleCondition> {
    let lower = |v: &str| v.trim().to_lowercase();
    let mut normal: Vec<RuleCondition> = conditions
        .iter()
        .map(|c| match c {
            RuleCondition::SenderAddress(v) => RuleCondition::SenderAddress(lower(v)),
            RuleCondition::SenderDomain(v) => RuleCondition::SenderDomain(lower(v)),
            RuleCondition::ListId(v) => RuleCondition::ListId(taught::normalise_list_id(v)),
            RuleCondition::SubjectTemplate(v) => RuleCondition::SubjectTemplate(lower(v)),
            RuleCondition::SubjectContains(v) => {
                RuleCondition::SubjectContains(subject_words(v).join(" "))
            }
            RuleCondition::Header(h) => RuleCondition::Header(HeaderCondition {
                name: h.name,
                value: h.value.as_ref().map(|v| v.trim().to_owned()),
            }),
            other => other.clone(),
        })
        .collect();
    // Compared as JSON: one order, whatever order they came in.
    normal.sort_by_cached_key(|c| serde_json::to_string(c).unwrap_or_default());
    normal.dedup();
    normal
}

/// 400 `invalid_rule` unless every label the rule names (in a condition or
/// an `add_label`) is one of `inbox`'s.
pub async fn labels_of_inbox(
    conn: &mut PgConnection,
    inbox: InboxId,
    conditions: &[RuleCondition],
    actions: &[RuleAction],
) -> Result<()> {
    let mut named: Vec<LabelId> = conditions
        .iter()
        .filter_map(|c| match c {
            RuleCondition::Label(label) => Some(*label),
            _ => None,
        })
        .chain(actions.iter().filter_map(|a| match a {
            RuleAction::AddLabel { label } => Some(*label),
            _ => None,
        }))
        .collect();
    named.sort_unstable();
    named.dedup();
    if named.is_empty() {
        return Ok(());
    }
    let found = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM conversations.labels
            WHERE inbox_id = $1 AND id = ANY($2)"#,
        inbox as InboxId,
        &named as &[LabelId],
    )
    .fetch_one(conn)
    .await?;
    if usize::try_from(found).unwrap_or_default() != named.len() {
        return Err(Error::BadRequest(crate::MESSAGES.reason("invalid_rule")));
    }
    Ok(())
}

/// The sum of the conditions' weights.
pub fn specificity(conditions: &[RuleCondition]) -> i32 {
    let sum: u32 = conditions
        .iter()
        .map(|c| taught::Condition::from(c).weight())
        .sum();
    i32::try_from(sum).unwrap_or(i32::MAX)
}

// --- Stored rules ------------------------------------------------------------

/// A stored rule.
#[derive(Debug, Clone)]
pub struct Stored {
    /// The rule.
    pub id: RuleId,
    /// Its inbox.
    pub inbox: InboxId,
    /// The teaching it came with.
    pub teaching: TeachingId,
    /// All must hold.
    pub conditions: Vec<RuleCondition>,
    /// What it does.
    pub actions: Vec<RuleAction>,
    /// Where it came from.
    pub origin: RuleOrigin,
    /// The member's own words, for an instruction.
    pub origin_text: Option<String>,
    /// As stored.
    pub state: RuleState,
    /// Who taught it.
    pub taught_by: Option<UserId>,
    /// The conversation it was taught from.
    pub from_conversation: Option<ConversationId>,
    /// When.
    pub created_at: DateTime<Utc>,
    /// When a member confirmed it.
    pub confirmed_at: Option<DateTime<Utc>>,
    /// Conversations it changed.
    pub applied_count: i32,
    /// When it last changed one.
    pub last_applied_at: Option<DateTime<Utc>>,
    /// Whether its author still may change its inbox (else it is paused).
    pub author_may_act: bool,
    /// Whether its inbox is kept read-only (its connection was removed): no
    /// rule acts there any more.
    pub inbox_detached: bool,
}

impl Stored {
    /// The state the rule is in: an active rule whose author can no longer
    /// change the inbox, or whose inbox is kept read-only, is paused.
    pub fn effective_state(&self) -> RuleState {
        if self.state == RuleState::Active && (!self.author_may_act || self.inbox_detached) {
            RuleState::Paused
        } else {
            self.state
        }
    }

    /// The rule as the engine reads it.
    pub fn engine(&self) -> TaughtRule {
        let (conditions, actions) = engine_parts(&self.conditions, &self.actions);
        TaughtRule {
            id: self.id.to_string(),
            conditions,
            actions,
            confirmed_at: self
                .confirmed_at
                .unwrap_or(self.created_at)
                .timestamp_millis(),
        }
    }
}

struct RuleRow {
    id: RuleId,
    inbox_id: InboxId,
    teaching_id: TeachingId,
    conditions: serde_json::Value,
    actions: serde_json::Value,
    origin: String,
    origin_text: Option<String>,
    state: String,
    taught_by: Option<sqlx::types::Uuid>,
    from_conversation_id: Option<ConversationId>,
    created_at: DateTime<Utc>,
    confirmed_at: Option<DateTime<Utc>>,
    applied_count: i32,
    last_applied_at: Option<DateTime<Utc>>,
    author_may_act: bool,
    inbox_detached: bool,
}

impl TryFrom<RuleRow> for Stored {
    type Error = Error;

    fn try_from(row: RuleRow) -> Result<Self> {
        let json = |e: serde_json::Error| Error::internal(super::redact::json(&e));
        Ok(Self {
            id: row.id,
            inbox: row.inbox_id,
            teaching: row.teaching_id,
            conditions: serde_json::from_value(row.conditions).map_err(json)?,
            actions: serde_json::from_value(row.actions).map_err(json)?,
            origin: row.origin.parse()?,
            origin_text: row.origin_text,
            state: row.state.parse()?,
            taught_by: row.taught_by.map(UserId::from_uuid),
            from_conversation: row.from_conversation_id,
            created_at: row.created_at,
            confirmed_at: row.confirmed_at,
            applied_count: row.applied_count,
            last_applied_at: row.last_applied_at,
            author_may_act: row.author_may_act,
            inbox_detached: row.inbox_detached,
        })
    }
}

/// Rule `id`, if it exists in the transaction's workspace (not removed).
pub async fn find(conn: &mut PgConnection, id: RuleId) -> Result<Option<Stored>> {
    Ok(find_any(conn, id)
        .await?
        .filter(|r| r.state != RuleState::Removed))
}

/// Rule `id`, removed or not.
pub async fn find_any(conn: &mut PgConnection, id: RuleId) -> Result<Option<Stored>> {
    let row = sqlx::query_as!(
        RuleRow,
        r#"
        SELECT r.id AS "id: RuleId", r.inbox_id AS "inbox_id: InboxId",
               r.teaching_id AS "teaching_id: TeachingId", r.conditions, r.actions, r.origin,
               r.origin_text, r.state, r.taught_by,
               r.from_conversation_id AS "from_conversation_id: ConversationId",
               r.created_at, r.confirmed_at,
               r.applied_count - (
                   SELECT count(*)::int FROM conversations.rule_applications x
                    WHERE x.rule_id = r.id AND x.excepted_at IS NOT NULL
                      AND x.reverted_at IS NULL) AS "applied_count!",
               r.last_applied_at,
               (r.taught_by IS NULL OR EXISTS (
                   SELECT 1 FROM conversations.inbox_members im
                    WHERE im.inbox_id = r.inbox_id AND im.user_id = r.taught_by
                      AND im.role IN ('sender', 'manager'))) AS "author_may_act!",
               i.detached_at IS NOT NULL AS "inbox_detached!"
          FROM conversations.rules r
          JOIN conversations.inboxes i ON i.id = r.inbox_id
         WHERE r.id = $1
        "#,
        id as RuleId,
    )
    .fetch_optional(conn)
    .await?;
    row.map(Stored::try_from).transpose()
}

/// The rules of `inboxes` (not removed), newest first.
pub async fn of_inboxes(conn: &mut PgConnection, inboxes: &[InboxId]) -> Result<Vec<Stored>> {
    let rows = sqlx::query_as!(
        RuleRow,
        r#"
        SELECT r.id AS "id: RuleId", r.inbox_id AS "inbox_id: InboxId",
               r.teaching_id AS "teaching_id: TeachingId", r.conditions, r.actions, r.origin,
               r.origin_text, r.state, r.taught_by,
               r.from_conversation_id AS "from_conversation_id: ConversationId",
               r.created_at, r.confirmed_at,
               r.applied_count - (
                   SELECT count(*)::int FROM conversations.rule_applications x
                    WHERE x.rule_id = r.id AND x.excepted_at IS NOT NULL
                      AND x.reverted_at IS NULL) AS "applied_count!",
               r.last_applied_at,
               (r.taught_by IS NULL OR EXISTS (
                   SELECT 1 FROM conversations.inbox_members im
                    WHERE im.inbox_id = r.inbox_id AND im.user_id = r.taught_by
                      AND im.role IN ('sender', 'manager'))) AS "author_may_act!",
               i.detached_at IS NOT NULL AS "inbox_detached!"
          FROM conversations.rules r
          JOIN conversations.inboxes i ON i.id = r.inbox_id
         WHERE r.inbox_id = ANY($1) AND r.state <> 'removed'
         ORDER BY r.created_at DESC, r.id DESC
        "#,
        inboxes as &[InboxId],
    )
    .fetch_all(conn)
    .await?;
    rows.into_iter().map(Stored::try_from).collect()
}

/// The rules acting in `inbox`, as the engine reads them.
pub async fn acting(conn: &mut PgConnection, inbox: InboxId) -> Result<Vec<TaughtRule>> {
    Ok(of_inboxes(conn, &[inbox])
        .await?
        .iter()
        .filter(|r| r.effective_state() == RuleState::Active)
        .map(Stored::engine)
        .collect())
}

/// Whether more than one member uses `inbox`: its rules act for others too.
pub async fn is_shared(conn: &mut PgConnection, inbox: InboxId) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT count(*) > 1 AS "shared!" FROM conversations.inbox_members WHERE inbox_id = $1"#,
        inbox as InboxId,
    )
    .fetch_one(conn)
    .await?)
}

/// The inboxes among `inboxes` used by more than one member.
pub async fn shared_of(
    conn: &mut PgConnection,
    inboxes: &[InboxId],
) -> Result<std::collections::HashSet<InboxId>> {
    Ok(sqlx::query_scalar!(
        r#"SELECT inbox_id AS "inbox: InboxId" FROM conversations.inbox_members
            WHERE inbox_id = ANY($1) GROUP BY inbox_id HAVING count(*) > 1"#,
        inboxes as &[InboxId],
    )
    .fetch_all(conn)
    .await?
    .into_iter()
    .collect())
}

/// `user`'s role in each of `inboxes` they belong to.
pub async fn roles_of(
    conn: &mut PgConnection,
    inboxes: &[InboxId],
    user: Option<UserId>,
) -> Result<std::collections::HashMap<InboxId, InboxRole>> {
    let Some(user) = user else {
        return Ok(std::collections::HashMap::new());
    };
    // Membership alone, as the catalogue's `conversations.rule` reads it.
    let rows = sqlx::query!(
        r#"SELECT inbox_id AS "inbox!: InboxId", role AS "role!"
             FROM conversations.inbox_members
            WHERE inbox_id = ANY($1) AND user_id = $2"#,
        inboxes as &[InboxId],
        user.as_uuid(),
    )
    .fetch_all(conn)
    .await?;
    rows.into_iter()
        .map(|r| Ok((r.inbox, r.role.parse::<InboxRole>()?)))
        .collect()
}

/// The caller's role in `inbox`; `None` for a non-member (an owner without a
/// membership row too).
pub async fn role_in(
    conn: &mut PgConnection,
    inbox: InboxId,
    user: Option<UserId>,
) -> Result<Option<InboxRole>> {
    let Some(user) = user else { return Ok(None) };
    Ok(roles_of(conn, &[inbox], Some(user)).await?.remove(&inbox))
}

/// Whether `role` may change a rule taught by `author`: its author while a
/// sender, or a manager of its inbox.
pub fn may_change(role: Option<InboxRole>, user: Option<UserId>, author: Option<UserId>) -> bool {
    match role {
        Some(InboxRole::Manager) => true,
        Some(InboxRole::Sender) => user.is_some() && user == author,
        _ => false,
    }
}

/// A new rule's state and teaching.
pub struct NewRule<'a> {
    /// Its inbox.
    pub inbox: InboxId,
    /// Its teaching (shared across inboxes).
    pub teaching: TeachingId,
    /// All must hold.
    pub conditions: &'a [RuleCondition],
    /// What it does.
    pub actions: &'a [RuleAction],
    /// Where it came from.
    pub origin: RuleOrigin,
    /// The member's words.
    pub origin_text: Option<&'a str>,
    /// Proposed or active.
    pub state: RuleState,
    /// Who taught it.
    pub taught_by: Option<UserId>,
    /// The conversation it was taught from.
    pub from_conversation: Option<ConversationId>,
}

/// The active rules of `inbox`, other than `this`, that a rule with
/// `conditions` (normalised) and `actions` would replace: the same conditions
/// and an action of the same kind (labels add up, so they replace nothing).
/// With their authors.
pub async fn replaced(
    conn: &mut PgConnection,
    inbox: InboxId,
    conditions: &[RuleCondition],
    actions: &[RuleAction],
    this: Option<RuleId>,
) -> Result<Vec<(RuleId, Option<UserId>)>> {
    let kinds: Vec<String> = actions
        .iter()
        .filter(|a| !matches!(a, RuleAction::AddLabel { .. }))
        .filter_map(|a| serde_json::to_value(a).ok())
        .filter_map(|v| {
            v.get("action")
                .and_then(|k| k.as_str())
                .map(ToOwned::to_owned)
        })
        .collect();
    // Their running runs first, then the rules: the lock order every path
    // keeps (`rule_runs::lock_running`).
    sqlx::query!(
        r#"SELECT u.id FROM conversations.rule_runs u
            WHERE u.state = 'running' AND u.rule_id IN (
                  SELECT id FROM conversations.rules
                   WHERE inbox_id = $1 AND state = 'active' AND conditions = $2
                     AND id IS DISTINCT FROM $3)
            ORDER BY u.id FOR UPDATE OF u"#,
        inbox as InboxId,
        json!(conditions),
        this as Option<RuleId>,
    )
    .fetch_all(&mut *conn)
    .await?;
    let rows = sqlx::query!(
        r#"SELECT id AS "id: RuleId", taught_by FROM conversations.rules
            WHERE inbox_id = $1 AND state = 'active' AND conditions = $2
              AND id IS DISTINCT FROM $4
              AND EXISTS (SELECT 1 FROM jsonb_array_elements(actions) a
                           WHERE a ->> 'action' = ANY($3))
            ORDER BY id FOR UPDATE"#,
        inbox as InboxId,
        json!(conditions),
        &kinds,
        this as Option<RuleId>,
    )
    .fetch_all(conn)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| (r.id, r.taught_by.map(UserId::from_uuid)))
        .collect())
}

/// Store a rule (its conditions normalised by the caller). Returns its id.
pub async fn insert(
    conn: &mut PgConnection,
    workspace: sqlx::types::Uuid,
    new: &NewRule<'_>,
) -> Result<RuleId> {
    let id = RuleId::new();
    sqlx::query!(
        r#"
        INSERT INTO conversations.rules
            (id, workspace_id, inbox_id, teaching_id, conditions, actions, specificity, origin,
             origin_text, state, taught_by, from_conversation_id, confirmed_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
                CASE WHEN $10 = 'active' THEN now() END)
        "#,
        id as RuleId,
        workspace,
        new.inbox as InboxId,
        new.teaching as TeachingId,
        json!(new.conditions),
        json!(new.actions),
        specificity(new.conditions),
        new.origin.as_str(),
        new.origin_text,
        new.state.as_str(),
        new.taught_by.map(|u| *u.as_uuid()),
        new.from_conversation.map(|c| *c.as_uuid()),
    )
    .execute(conn)
    .await?;
    Ok(id)
}

/// Whether a rule's stream move is blocked by suspicious mail.
pub fn hides(actions: &[RuleAction]) -> bool {
    actions.iter().any(|a| match a {
        RuleAction::Archive | RuleAction::MarkRead => true,
        RuleAction::MoveToStream { stream } => *stream != Stream::Primary,
        RuleAction::NeedsReply { value } | RuleAction::AsksAction { value } => !value,
        RuleAction::Surface { value } => *value == SurfaceWhen::Never,
        RuleAction::AddLabel { .. } => false,
    })
}
