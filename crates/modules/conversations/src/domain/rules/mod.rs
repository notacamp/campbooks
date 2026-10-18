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
//! rule acts only while its author still may change the inbox and is not
//! suspended (`member_suspensions`, campbooks-cloud#147); a member who
//! changed back what a rule did makes that conversation an **exception** the
//! rule leaves alone ([`exceptions`]). A rule acts only while its MAC binds
//! it to the member who taught it ([`binding`], campbooks-cloud#242).

mod apply;
mod arrival;
pub mod binding;
mod effects;
mod exceptions;
mod facts;
mod images;

use campbooks_ai::decide::{
    signals::email::{TEMPLATE_MAX_CHARS, subject_words},
    taught::{self, Surface, TaughtRule},
};
use campbooks_kernel::{Error, Result, binding::Binding, tenancy::UserId};
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
pub use images::{ImagesHeld, images};

use crate::model::{
    ConversationId, InboxId, InboxRole, LabelId, PausedReason, RuleId, RuleOrigin, RuleState,
    Stream, TeachingId,
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
    /// The receiving provider's authentication results say the sender's
    /// domain failed its checks (with a sender or domain: "mail that says it
    /// is from paypal.com but is not").
    SenderChecksFail,
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
    /// Put it in the trash.
    Trash,
    /// Report it as spam ("Never see emails like this again").
    Spam,
    /// Its bills are already paid: Campbooks never asks "Is it paid?" about
    /// it.
    MarkPaid,
    /// Show its remote images without asking ("Always show images from
    /// <sender>", campbooks-cloud#200): one sender, nothing else; only for
    /// mail whose sender passes authentication; tracking pixels stay
    /// removed. Read when a message is opened, so it changes no mail.
    ShowImages,
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
            RuleCondition::SenderChecksFail => Self::SenderChecksFail,
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
            RuleAction::Trash => Self::Trash,
            RuleAction::Spam => Self::Spam,
            RuleAction::MarkPaid => Self::MarkPaid,
            RuleAction::ShowImages => Self::ShowImages,
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
        RuleCondition::Label(_) | RuleCondition::Stream(_) | RuleCondition::SenderChecksFail => {
            false
        }
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

/// 400 `invalid_rule` unless `inbox` holds inbound mail from the sender a
/// rule that shows images names (its one address, or a domain with its
/// subdomains): a member trusts only a sender whose mail they can see in an
/// inbox they may teach in (campbooks-cloud#200). Any other rule passes.
pub async fn sender_seen(
    conn: &mut PgConnection,
    inbox: InboxId,
    conditions: &[RuleCondition],
    actions: &[RuleAction],
) -> Result<()> {
    if !actions.contains(&RuleAction::ShowImages) {
        return Ok(());
    }
    let invalid = || Error::BadRequest(crate::MESSAGES.reason("invalid_rule"));
    let (address, domain) = match conditions {
        [RuleCondition::SenderAddress(a)] => (a.trim().to_lowercase(), String::new()),
        [RuleCondition::SenderDomain(d)] => (String::new(), d.trim().to_lowercase()),
        _ => return Err(invalid()),
    };
    let seen: bool = sqlx::query_scalar(
        "SELECT EXISTS (
             SELECT 1 FROM conversations.messages
              WHERE inbox_id = $1 AND direction = 'inbound'
                AND (($2 <> '' AND sender_address = $2)
                  OR ($3 <> '' AND (split_part(sender_address, '@', 2) = $3
                       OR right(split_part(sender_address, '@', 2), length($3) + 1) = '.' || $3))))",
    )
    .bind(inbox)
    .bind(&address)
    .bind(&domain)
    .fetch_one(conn)
    .await?;
    if seen { Ok(()) } else { Err(invalid()) }
}

/// 409 `images_already_trusted` when a rule of `inbox` (not removed, other
/// than `this`) already shows the images of the sender a new image rule
/// names: the same address, or a domain that covers it (campbooks-cloud#235).
/// Any other rule passes.
pub async fn not_trusted_yet(
    conn: &mut PgConnection,
    inbox: InboxId,
    conditions: &[RuleCondition],
    actions: &[RuleAction],
    this: Option<RuleId>,
) -> Result<()> {
    if !actions.contains(&RuleAction::ShowImages) {
        return Ok(());
    }
    let existing: Vec<Stored> = of_inboxes(conn, &[inbox])
        .await?
        .into_iter()
        .filter(|r| Some(r.id) != this && r.actions.contains(&RuleAction::ShowImages))
        .collect();
    let covers = |rule: &Stored| match conditions {
        [RuleCondition::SenderAddress(address)] => rule.engine().matches(&taught::Facts {
            sender_address: address,
            ..taught::Facts::default()
        }),
        // Anyone at the domain: the engine decides whether an existing rule
        // already reaches them, the same way it would on a real message.
        [RuleCondition::SenderDomain(domain)] => rule.engine().matches(&taught::Facts {
            sender_address: &format!("anyone@{domain}"),
            ..taught::Facts::default()
        }),
        _ => false,
    };
    if existing.iter().any(covers) {
        return Err(Error::Conflict(
            crate::MESSAGES.reason("images_already_trusted"),
        ));
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
    /// Whether its author's account is suspended (it is paused until the
    /// suspension is lifted).
    pub author_suspended: bool,
    /// Whether its inbox is kept read-only (its connection was removed): no
    /// rule acts there any more.
    pub inbox_detached: bool,
    /// Its workspace.
    pub workspace: sqlx::types::Uuid,
    /// Its binding generation: one more at every signing ([`binding`]).
    pub generation: i64,
    /// Its MAC ([`binding`]); `None` until a member's request signs it.
    pub mac: Option<Vec<u8>>,
}

impl Stored {
    /// The state the rule is in: an active rule whose author can no longer
    /// change the inbox or is suspended, whose inbox is kept read-only, or
    /// that names a sender domain which is a public suffix, is paused.
    pub fn effective_state(&self) -> RuleState {
        if self.state == RuleState::Active
            && (!self.author_may_act
                || self.author_suspended
                || self.inbox_detached
                || self.domain_too_broad())
        {
            RuleState::Paused
        } else {
            self.state
        }
    }

    /// Why the rule is paused, when it is: the reason that would still hold
    /// once the most passing one clears (a lifted suspension resumes it on
    /// its own; a member's pause, or an author who left, needs a member).
    pub fn paused_reason(&self) -> Option<PausedReason> {
        if self.effective_state() != RuleState::Paused {
            return None;
        }
        Some(if self.inbox_detached {
            PausedReason::InboxReadOnly
        } else if self.domain_too_broad() {
            // Before the author's reasons: adopting it would not make it act.
            PausedReason::DomainTooBroad
        } else if !self.author_may_act {
            PausedReason::AuthorCannotWrite
        } else if self.state == RuleState::Paused {
            PausedReason::PausedByMember
        } else {
            PausedReason::AuthorUnavailable
        })
    }

    /// For a proposed rule, why it would be paused once a member says yes
    /// (confirming keeps its author, campbooks-cloud#197): an author away
    /// for now, one who can no longer change the inbox, an inbox kept
    /// read-only. `None` for any other rule, or when it would act.
    pub fn paused_once_kept(&self) -> Option<PausedReason> {
        if self.state != RuleState::Proposed {
            return None;
        }
        let kept = Self {
            state: RuleState::Active,
            ..self.clone()
        };
        kept.paused_reason()
    }

    /// Whether a sender-domain condition names a public suffix: a rule
    /// stored before the Public Suffix List was complete may. The engine
    /// never matches it; only removing the rule clears this.
    fn domain_too_broad(&self) -> bool {
        self.conditions
            .iter()
            .any(|c| matches!(c, RuleCondition::SenderDomain(d) if !taught::is_owned_domain(d)))
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
    author_suspended: bool,
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
            author_suspended: row.author_suspended,
            inbox_detached: row.inbox_detached,
            // Read next to the row (`binding::attach`).
            workspace: sqlx::types::Uuid::nil(),
            generation: 0,
            mac: None,
        })
    }
}

/// Lock rule `id`'s row (after its runs: `rule_runs::lock_running`) and
/// return its state as it is now, so a check on it holds until the
/// transaction ends.
pub async fn lock_state(conn: &mut PgConnection, id: RuleId) -> Result<Option<RuleState>> {
    sqlx::query_scalar!(
        "SELECT state FROM conversations.rules WHERE id = $1 FOR UPDATE",
        id as RuleId,
    )
    .fetch_optional(conn)
    .await?
    .map(|state| state.parse())
    .transpose()
}

/// Rule `id`, if it exists in the transaction's workspace (not removed).
pub async fn find(conn: &mut PgConnection, id: RuleId) -> Result<Option<Stored>> {
    Ok(find_any(conn, id)
        .await?
        .filter(|r| r.state != RuleState::Removed))
}

/// Rule `id`, removed or not; `None` when there is none, or its row does
/// not decode (a WARN with its id).
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
               EXISTS (
                   SELECT 1 FROM conversations.member_suspensions s
                    WHERE s.workspace_id = r.workspace_id AND s.user_id = r.taught_by
                      AND s.suspended) AS "author_suspended!",
               i.detached_at IS NOT NULL AS "inbox_detached!"
          FROM conversations.rules r
          JOIN conversations.inboxes i ON i.id = r.inbox_id
         WHERE r.id = $1
        "#,
        id as RuleId,
    )
    .fetch_optional(&mut *conn)
    .await?;
    let Some(row) = row else { return Ok(None) };
    // A row that does not even decode was not written by Campbooks: it is
    // treated as no rule (it could never act), never as an error a job
    // would retry or a request answer 500 with.
    let id = row.id;
    let Ok(mut rule) = Stored::try_from(row) else {
        tracing::warn!(rule = %id, "a taught rule does not decode; it is left out");
        return Ok(None);
    };
    binding::attach(conn, std::slice::from_mut(&mut rule)).await?;
    Ok(Some(rule))
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
               EXISTS (
                   SELECT 1 FROM conversations.member_suspensions s
                    WHERE s.workspace_id = r.workspace_id AND s.user_id = r.taught_by
                      AND s.suspended) AS "author_suspended!",
               i.detached_at IS NOT NULL AS "inbox_detached!"
          FROM conversations.rules r
          JOIN conversations.inboxes i ON i.id = r.inbox_id
         WHERE r.inbox_id = ANY($1) AND r.state <> 'removed'
         ORDER BY r.created_at DESC, r.id DESC
        "#,
        inboxes as &[InboxId],
    )
    .fetch_all(&mut *conn)
    .await?;
    // A row that does not even decode was not written by Campbooks: it is
    // left out (it could never act), so it fails neither a list nor the
    // mail arriving in its inbox.
    let mut rules: Vec<Stored> = rows
        .into_iter()
        .filter_map(|row| {
            let id = row.id;
            Stored::try_from(row)
                .inspect_err(|_| {
                    tracing::warn!(rule = %id, "a taught rule does not decode; it is left out");
                })
                .ok()
        })
        .collect();
    binding::attach(conn, &mut rules).await?;
    Ok(rules)
}

/// The rules acting in `inbox`, as the engine reads them: active, and bound
/// to the member who taught them (campbooks-cloud#242). An active rule
/// whose MAC does not verify (planted or changed with SQL, or kept from
/// before rules were bound and not signed again) is left out, with a WARN
/// carrying ids only: it changes nothing, and the other rules still act.
pub async fn acting(
    conn: &mut PgConnection,
    binding: &Binding,
    inbox: InboxId,
) -> Result<Vec<TaughtRule>> {
    Ok(of_inboxes(conn, &[inbox])
        .await?
        .iter()
        .filter(|r| r.effective_state() == RuleState::Active)
        .filter(|r| {
            let bound = r.bound(binding);
            if !bound {
                tracing::warn!(
                    workspace = %r.workspace,
                    rule = %r.id,
                    "a taught rule is not bound to the member who taught it; it does not act"
                );
            }
            bound
        })
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

/// Whether a rule's actions hide mail (or a question about it) from the
/// member: never for a correspondent's mail, and, except the trash and spam
/// ([`bins`]), never for suspicious mail.
pub fn hides(actions: &[RuleAction]) -> bool {
    actions.iter().any(|a| match a {
        RuleAction::Archive
        | RuleAction::MarkRead
        | RuleAction::Trash
        | RuleAction::Spam
        | RuleAction::MarkPaid => true,
        RuleAction::MoveToStream { stream } => *stream != Stream::Primary,
        RuleAction::NeedsReply { value } | RuleAction::AsksAction { value } => !value,
        RuleAction::Surface { value } => *value == SurfaceWhen::Never,
        RuleAction::AddLabel { .. } | RuleAction::ShowImages => false,
    })
}

/// Whether a rule only shows a sender's images: it changes no mail, so it
/// has nothing to apply to the mail already here.
pub fn only_shows_images(actions: &[RuleAction]) -> bool {
    !actions.is_empty() && actions.iter().all(|a| *a == RuleAction::ShowImages)
}

/// Whether a rule puts mail in the trash or spam: it acts on suspicious
/// mail too, since that mail is what it is about.
pub fn bins(actions: &[RuleAction]) -> bool {
    actions
        .iter()
        .any(|a| matches!(a, RuleAction::Trash | RuleAction::Spam))
}

#[cfg(test)]
mod stored_tests {
    use super::*;

    fn stored(domain: &str, state: RuleState) -> Stored {
        Stored {
            id: RuleId::new(),
            inbox: InboxId::new(),
            teaching: TeachingId::new(),
            conditions: vec![RuleCondition::SenderDomain(domain.into())],
            actions: vec![RuleAction::Archive],
            origin: RuleOrigin::Instruction,
            origin_text: None,
            state,
            taught_by: None,
            from_conversation: None,
            created_at: Utc::now(),
            confirmed_at: None,
            applied_count: 0,
            last_applied_at: None,
            author_may_act: true,
            author_suspended: false,
            inbox_detached: false,
            workspace: sqlx::types::Uuid::nil(),
            generation: 0,
            mac: None,
        }
    }

    #[test]
    fn a_stored_rule_naming_a_public_suffix_is_paused_for_good() {
        let rule = stored("github.io", RuleState::Active);
        assert_eq!(rule.effective_state(), RuleState::Paused);
        assert_eq!(rule.paused_reason(), Some(PausedReason::DomainTooBroad));
        // Even a member's pause does not hide it: resuming would not help.
        let paused = stored("github.io", RuleState::Paused);
        assert_eq!(paused.paused_reason(), Some(PausedReason::DomainTooBroad));
    }

    #[test]
    fn it_ranks_right_after_a_read_only_inbox() {
        let mut rule = stored("github.io", RuleState::Active);
        rule.author_may_act = false;
        rule.author_suspended = true;
        assert_eq!(rule.paused_reason(), Some(PausedReason::DomainTooBroad));
        rule.inbox_detached = true;
        assert_eq!(rule.paused_reason(), Some(PausedReason::InboxReadOnly));
    }

    #[test]
    fn a_proposal_says_why_it_would_stay_paused_once_kept() {
        let mut rule = stored("github.com", RuleState::Proposed);
        assert_eq!(rule.effective_state(), RuleState::Proposed);
        assert_eq!(rule.paused_reason(), None);
        assert_eq!(rule.paused_once_kept(), None);
        rule.author_suspended = true;
        assert_eq!(
            rule.paused_once_kept(),
            Some(PausedReason::AuthorUnavailable)
        );
        // Only for a proposal: an active rule says so with `paused_reason`.
        rule.state = RuleState::Active;
        assert_eq!(rule.paused_once_kept(), None);
        assert_eq!(rule.paused_reason(), Some(PausedReason::AuthorUnavailable));
    }

    #[test]
    fn an_owned_domain_rule_stays_active() {
        let rule = stored("github.com", RuleState::Active);
        assert_eq!(rule.effective_state(), RuleState::Active);
        assert_eq!(rule.paused_reason(), None);
    }
}
