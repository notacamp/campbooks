//! Taught rules (campbooks-cloud#125): deterministic condition → action
//! pairs a member taught Campbooks for one inbox.
//!
//! The engine (`campbooks_ai::decide::taught`) decides; this module stores
//! rules, reads the facts the engine needs from its own tables, and carries
//! out an outcome on a conversation with the same domain functions the
//! member's operations use, recording each change with what it was before so
//! it can be put back.
//!
//! A conversation matches a rule when its newest inbound message does. A
//! rule acts only while its author still may change the inbox; a member who
//! changed back what a rule did makes that conversation an **exception** the
//! rule leaves alone.

use std::collections::BTreeSet;

use campbooks_ai::decide::{
    signals::email::triage,
    taught::{self, Facts, Outcome, Surface, TaughtRule},
};
use campbooks_kernel::{Error, EventBus, Result, tenancy::UserId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgConnection, Postgres, Transaction};
use utoipa::ToSchema;

use super::{
    conversation::{Changed, ConversationSummary, request},
    decide::STATE_VERSION,
    flags::{self, Flag},
    needs::{self, NeedSettings},
};
use crate::{
    events::{Change, Origin, RequestedChange},
    ingest::Headers,
    model::{
        ConversationId, DecisionId, InboxId, InboxRole, LabelId, MessageId, RuleId, RuleOrigin,
        RuleRunId, RuleState, Stream, TeachingId,
    },
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

/// Check a rule's shape: 400 `invalid_rule` with what is wrong.
pub fn validate(conditions: &[RuleCondition], actions: &[RuleAction]) -> Result<()> {
    let (c, a) = engine_parts(conditions, actions);
    TaughtRule::validate(&c, &a)
        .map_err(|_| Error::BadRequest(crate::MESSAGES.reason("invalid_rule")))
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
}

impl Stored {
    /// The state the rule is in: an active rule whose author can no longer
    /// change the inbox is paused.
    pub fn effective_state(&self) -> RuleState {
        if self.state == RuleState::Active && !self.author_may_act {
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
               r.created_at, r.confirmed_at, r.applied_count, r.last_applied_at,
               (r.taught_by IS NULL OR EXISTS (
                   SELECT 1 FROM conversations.inbox_members im
                    WHERE im.inbox_id = r.inbox_id AND im.user_id = r.taught_by
                      AND im.role IN ('sender', 'manager'))) AS "author_may_act!"
          FROM conversations.rules r
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
               r.created_at, r.confirmed_at, r.applied_count, r.last_applied_at,
               (r.taught_by IS NULL OR EXISTS (
                   SELECT 1 FROM conversations.inbox_members im
                    WHERE im.inbox_id = r.inbox_id AND im.user_id = r.taught_by
                      AND im.role IN ('sender', 'manager'))) AS "author_may_act!"
          FROM conversations.rules r
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

/// The caller's role in `inbox`; `None` for a client token or a non-member.
pub async fn role_in(
    conn: &mut PgConnection,
    inbox: InboxId,
    user: Option<UserId>,
) -> Result<Option<InboxRole>> {
    let Some(user) = user else { return Ok(None) };
    sqlx::query_scalar!(
        "SELECT role FROM conversations.inbox_members WHERE inbox_id = $1 AND user_id = $2",
        inbox as InboxId,
        user.as_uuid(),
    )
    .fetch_optional(conn)
    .await?
    .map(|r| r.parse::<InboxRole>())
    .transpose()
}

// --- Facts -------------------------------------------------------------------

/// What the engine and the bulk runs read about one conversation.
#[derive(Debug, Clone)]
pub struct Candidate {
    /// The conversation.
    pub id: ConversationId,
    /// Its newest inbound message.
    pub message: MessageId,
    /// Its stream.
    pub stream: Stream,
    /// Who put it there.
    pub stream_source: String,
    /// Whether it is archived.
    pub archived: bool,
    /// Whether it has unread inbound messages.
    pub unread: bool,
    /// Its label ids.
    pub labels: Vec<String>,
    /// The newest inbound message's sender.
    pub sender_address: String,
    /// Its subject.
    pub subject: String,
    /// Its subject template.
    pub subject_template: Option<String>,
    /// Its headers subset.
    pub headers: Headers,
    /// It looks suspicious, or has an open Review need.
    pub suspicious: bool,
    /// When the newest inbound message was sent.
    pub sent_at: DateTime<Utc>,
}

impl Candidate {
    /// The engine's facts.
    pub fn facts(&self) -> Facts<'_> {
        Facts {
            sender_address: &self.sender_address,
            subject: &self.subject,
            subject_template: self.subject_template.as_deref(),
            list_id: self.headers.list_id.as_deref(),
            list_unsubscribe: self.headers.list_unsubscribe.as_deref(),
            precedence: self.headers.precedence.as_deref(),
            auto_submitted: self.headers.auto_submitted.as_deref(),
            labels: &self.labels,
            stream: self.stream.as_str(),
            suspicious: self.suspicious,
        }
    }
}

/// Columns of a prefilter: the narrowing conditions a query can use an
/// index for. The engine has the last word on every row.
#[derive(Debug, Default)]
struct Prefilter {
    sender: Option<String>,
    domain: Option<String>,
    list: Option<String>,
    template: Option<String>,
}

impl Prefilter {
    fn of(conditions: &[RuleCondition]) -> Self {
        let mut p = Self::default();
        for c in conditions {
            match c {
                RuleCondition::SenderAddress(v) => p.sender = Some(v.trim().to_lowercase()),
                RuleCondition::SenderDomain(v) => p.domain = Some(v.trim().to_lowercase()),
                RuleCondition::ListId(v) => {
                    let v = v.trim().to_lowercase();
                    let v = v.trim_start_matches('<').trim_end_matches('>').to_owned();
                    p.list = Some(v);
                }
                RuleCondition::SubjectTemplate(v) => p.template = Some(v.trim().to_owned()),
                _ => {}
            }
        }
        p
    }
}

struct CandidateRow {
    id: ConversationId,
    message: MessageId,
    stream: String,
    stream_source: String,
    archived: bool,
    unread: bool,
    labels: Vec<String>,
    sender_address: String,
    subject: String,
    subject_template: Option<String>,
    headers: serde_json::Value,
    suspicious: bool,
    sent_at: DateTime<Utc>,
}

impl TryFrom<CandidateRow> for Candidate {
    type Error = Error;

    fn try_from(r: CandidateRow) -> Result<Self> {
        Ok(Self {
            id: r.id,
            message: r.message,
            stream: r.stream.parse()?,
            stream_source: r.stream_source,
            archived: r.archived,
            unread: r.unread,
            labels: r.labels,
            sender_address: r.sender_address,
            subject: r.subject,
            subject_template: r.subject_template,
            headers: serde_json::from_value(r.headers).unwrap_or_default(),
            suspicious: r.suspicious,
            sent_at: r.sent_at,
        })
    }
}

/// Up to `limit` conversations of `inbox` after `after` (by id) whose newest
/// inbound message passes `conditions`' prefilter, with their facts, and the
/// last conversation id looked at (the next cursor).
pub async fn scan(
    conn: &mut PgConnection,
    inbox: InboxId,
    conditions: &[RuleCondition],
    after: Option<ConversationId>,
    limit: i64,
) -> Result<Vec<Candidate>> {
    scan_rows(conn, inbox, &Prefilter::of(conditions), after, None, limit).await
}

async fn scan_rows(
    conn: &mut PgConnection,
    inbox: InboxId,
    p: &Prefilter,
    after: Option<ConversationId>,
    only: Option<ConversationId>,
    limit: i64,
) -> Result<Vec<Candidate>> {
    let rows = sqlx::query_as!(
        CandidateRow,
        r#"
        SELECT c.id AS "id!: ConversationId", m.id AS "message!: MessageId", c.stream AS "stream!",
               c.stream_source AS "stream_source!", c.archived_at IS NOT NULL AS "archived!",
               c.unread_count > 0 AS "unread!",
               coalesce((SELECT array_agg(cl.label_id::text ORDER BY cl.label_id)
                           FROM conversations.conversation_labels cl
                          WHERE cl.conversation_id = c.id), '{}') AS "labels!",
               m.sender_address AS "sender_address!", m.subject AS "subject!",
               m.subject_template, m.headers AS "headers!",
               (EXISTS (SELECT 1 FROM conversations.decisions d
                         WHERE d.message_id = m.id AND d.question = 'suspicious'
                           AND d.value = 'true'::jsonb)
                OR EXISTS (SELECT 1 FROM conversations.needs n
                            WHERE n.conversation_id = c.id AND n.state = 'open'
                              AND n.question = 'suspicious')) AS "suspicious!",
               m.sent_at AS "sent_at!"
          FROM conversations.conversations c
          CROSS JOIN LATERAL (
              SELECT id, sender_address, subject, subject_template, headers, sent_at
                FROM conversations.messages
               WHERE conversation_id = c.id AND direction = 'inbound'
               ORDER BY sent_at DESC, id DESC LIMIT 1
          ) m
         WHERE c.inbox_id = $1 AND NOT c.trashed
           AND ($2::uuid IS NULL OR c.id > $2)
           AND ($3::text IS NULL OR m.sender_address = $3)
           AND ($4::text IS NULL OR split_part(m.sender_address, '@', 2) = $4
                OR split_part(m.sender_address, '@', 2) LIKE '%.' || $4)
           AND ($5::text IS NULL OR lower(m.headers ->> 'list_id') LIKE '%' || $5 || '%')
           AND ($6::text IS NULL OR m.subject_template = $6)
           AND ($8::uuid IS NULL OR c.id = $8)
         ORDER BY c.id
         LIMIT $7
        "#,
        inbox as InboxId,
        after.map(|a| *a.as_uuid()),
        p.sender,
        p.domain,
        p.list,
        p.template,
        limit,
        only.map(|o| *o.as_uuid()),
    )
    .fetch_all(conn)
    .await?;
    rows.into_iter().map(Candidate::try_from).collect()
}

/// The facts of conversation `id` (its newest inbound message), if it has one.
pub async fn candidate(conn: &mut PgConnection, id: ConversationId) -> Result<Option<Candidate>> {
    let inbox = sqlx::query_scalar!(
        r#"SELECT inbox_id AS "inbox: InboxId" FROM conversations.conversations WHERE id = $1"#,
        id as ConversationId,
    )
    .fetch_optional(&mut *conn)
    .await?;
    let Some(inbox) = inbox else { return Ok(None) };
    Ok(
        scan_rows(conn, inbox, &Prefilter::default(), None, Some(id), 1)
            .await?
            .into_iter()
            .next(),
    )
}

// --- Carrying out an outcome -------------------------------------------------

/// What a rule did to one conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applied {
    /// It changed something.
    Changed,
    /// Nothing to do: already so, or another rule wins every slot.
    Unchanged,
    /// Left alone: it looks suspicious.
    Suspicious,
    /// Left alone: a member placed it or changed back what the rule did.
    Member,
}

/// What a conversation was before a rule changed it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Before {
    /// Its stream and who put it there, when the rule moved it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<(Stream, String)>,
    /// Whether the rule archived it.
    #[serde(default)]
    pub archived: bool,
    /// Whether the rule marked it read.
    #[serde(default)]
    pub read: bool,
    /// Labels the rule put on it.
    #[serde(default)]
    pub labels: Vec<LabelId>,
    /// Questions the rule answered on its newest inbound message.
    #[serde(default)]
    pub answered: Vec<String>,
}

/// Where a rule acts: the inbox, the workspace's event key and the events.
pub struct Context<'a> {
    /// The event bus.
    pub events: &'a EventBus,
    /// The workspace, as events key it.
    pub workspace: &'a str,
    /// The workspace id.
    pub workspace_id: sqlx::types::Uuid,
    /// The inbox.
    pub inbox: InboxId,
    /// How needs are raised.
    pub needs: &'a NeedSettings,
}

/// Carry out what `rule` wins in `rules`' outcome on `candidate`, as part of
/// `run` (`None`: as mail arrived). Records the application; a
/// conversation the rule already changed, or that a member made an exception
/// of, is left alone.
pub async fn apply(
    tx: &mut Transaction<'_, Postgres>,
    ctx: &Context<'_>,
    rule: RuleId,
    rules: &[TaughtRule],
    candidate: &Candidate,
    run: Option<RuleRunId>,
) -> Result<Applied> {
    let existing = sqlx::query!(
        r#"SELECT excepted_at IS NOT NULL AS "excepted!", reverted_at IS NOT NULL AS "reverted!"
             FROM conversations.rule_applications WHERE rule_id = $1 AND conversation_id = $2"#,
        rule as RuleId,
        candidate.id as ConversationId,
    )
    .fetch_optional(&mut **tx)
    .await?;
    if let Some(existing) = &existing {
        if existing.excepted {
            return Ok(Applied::Member);
        }
        if run.is_some() && !existing.reverted {
            return Ok(Applied::Unchanged);
        }
    }
    let excepted = excepted_rules(tx, candidate.id).await?;
    let rules: Vec<TaughtRule> = rules
        .iter()
        .filter(|r| !excepted.contains(&r.id))
        .cloned()
        .collect();
    let outcome = taught::evaluate(&rules, &candidate.facts());
    let id = rule.to_string();
    let mine = Mine::of(&outcome, &id);
    if mine.is_empty() {
        let wanted = rules
            .iter()
            .find(|r| r.id == id)
            .is_some_and(|r| r.matches(&candidate.facts()));
        return Ok(if wanted && candidate.suspicious {
            Applied::Suspicious
        } else {
            Applied::Unchanged
        });
    }

    let mut before = Before::default();
    let mut changes = Vec::new();
    let mut requested = Vec::new();
    if let Some(stream) = mine.stream
        && stream != candidate.stream
    {
        if candidate.stream_source == "member" {
            return Ok(Applied::Member);
        }
        sqlx::query!(
            r#"UPDATE conversations.conversations
                  SET stream = $2, stream_source = 'taught', updated_at = now()
                WHERE id = $1 AND stream_source <> 'member'"#,
            candidate.id as ConversationId,
            stream.as_str(),
        )
        .execute(&mut **tx)
        .await?;
        before.stream = Some((candidate.stream, candidate.stream_source.clone()));
        changes.push(Change::Stream);
        requested.push((
            super::conversation::provider_message_ids(tx, candidate.id).await?,
            RequestedChange::MoveToStream { stream },
        ));
    }
    for (wanted, flag) in [(mine.archive, Flag::Archive), (mine.read, Flag::Read)] {
        if !wanted {
            continue;
        }
        let ids = flags::change(tx, candidate.id, flag).await?;
        if ids.is_empty() {
            continue;
        }
        match flag {
            Flag::Archive => before.archived = true,
            _ => before.read = true,
        }
        changes.push(flag.change());
        requested.push((ids, flag.requested()));
    }
    for label in &mine.labels {
        let provider_key = sqlx::query_scalar!(
            r#"
            WITH linked AS (
                INSERT INTO conversations.conversation_labels (workspace_id, conversation_id, label_id)
                SELECT $1, $2, l.id FROM conversations.labels l WHERE l.id = $3 AND l.inbox_id = $4
                ON CONFLICT DO NOTHING
                RETURNING label_id
            )
            SELECT l.provider_key FROM linked JOIN conversations.labels l ON l.id = linked.label_id
            "#,
            ctx.workspace_id,
            candidate.id as ConversationId,
            *label as LabelId,
            ctx.inbox as InboxId,
        )
        .fetch_optional(&mut **tx)
        .await?;
        let Some(provider_key) = provider_key else {
            continue;
        };
        before.labels.push(*label);
        changes.push(Change::Labels);
        requested.push((
            super::conversation::provider_message_ids(tx, candidate.id).await?,
            RequestedChange::AddLabel {
                label: *label,
                provider_key,
            },
        ));
    }
    before.answered = answer(tx, ctx, candidate, &outcome, &id).await?;

    if before == Before::default() {
        return Ok(Applied::Unchanged);
    }
    record(tx, ctx, rule, candidate.id, run, &before).await?;
    if !changes.is_empty() {
        let mut changed = Changed {
            workspace: ctx.workspace.to_owned(),
            inbox: ctx.inbox,
            conversation: candidate.id,
            origin: Origin::Member,
            changes,
            provider_message_ids: Vec::new(),
        };
        for (ids, change) in requested {
            changed.provider_message_ids = ids;
            request(ctx.events, tx, &changed, &change).await?;
        }
        changed.provider_message_ids =
            super::conversation::provider_message_ids(tx, candidate.id).await?;
        changed.publish(ctx.events, tx).await?;
    }
    needs::refresh(tx, ctx.events, ctx.workspace, candidate.id, ctx.needs).await?;
    Ok(Applied::Changed)
}

/// The slots one rule wins in an outcome.
#[derive(Debug, Default)]
struct Mine {
    stream: Option<Stream>,
    archive: bool,
    read: bool,
    labels: Vec<LabelId>,
    answers: bool,
}

impl Mine {
    fn of(outcome: &Outcome, id: &str) -> Self {
        Self {
            stream: outcome
                .stream
                .as_ref()
                .filter(|a| a.rule == id)
                .and_then(|a| a.value.parse().ok()),
            archive: outcome.archive.as_deref() == Some(id),
            read: outcome.read.as_deref() == Some(id),
            labels: outcome
                .labels
                .iter()
                .filter(|a| a.rule == id)
                .filter_map(|a| a.value.parse().ok())
                .collect(),
            answers: outcome.needs_reply.as_ref().is_some_and(|a| a.rule == id)
                || outcome.asks_action.as_ref().is_some_and(|a| a.rule == id)
                || outcome.surface.as_ref().is_some_and(|a| a.rule == id),
        }
    }

    fn is_empty(&self) -> bool {
        self.stream.is_none()
            && !self.archive
            && !self.read
            && self.labels.is_empty()
            && !self.answers
    }
}

/// Store the answers `rule` wins on `candidate`'s newest inbound message as
/// `taught` decisions (replacing older taught ones). Returns the questions.
async fn answer(
    tx: &mut Transaction<'_, Postgres>,
    ctx: &Context<'_>,
    candidate: &Candidate,
    outcome: &Outcome,
    rule: &str,
) -> Result<Vec<String>> {
    let mut answered = Vec::new();
    let flag = |a: &Option<taught::Applied<bool>>| {
        a.as_ref()
            .filter(|a| a.rule == rule)
            .map(|a| (json!(a.value), json!(["false", "true"])))
    };
    let surface = outcome
        .surface
        .as_ref()
        .filter(|a| a.rule == rule)
        .map(|a| {
            let value = match a.value {
                Surface::Always => "always",
                Surface::Never => "never",
            };
            (json!(value), json!(["always", "never"]))
        });
    for (question, answer) in [
        (triage::NEEDS_REPLY, flag(&outcome.needs_reply)),
        (triage::ASKS_ACTION, flag(&outcome.asks_action)),
        (needs::SURFACE, surface),
    ] {
        let Some((value, options)) = answer else {
            continue;
        };
        store_decision(
            tx,
            ctx,
            candidate.message,
            candidate.id,
            question,
            value,
            options,
            rule,
        )
        .await?;
        answered.push(question.to_owned());
    }
    Ok(answered)
}

/// Store a taught decision about `message` (replacing an older taught one).
#[allow(clippy::too_many_arguments)]
pub async fn store_decision(
    conn: &mut PgConnection,
    ctx: &Context<'_>,
    message: MessageId,
    conversation: ConversationId,
    question: &str,
    value: serde_json::Value,
    options: serde_json::Value,
    rule: &str,
) -> Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO conversations.decisions
            (id, workspace_id, message_id, conversation_id, question, source, strategy, value,
             options, probabilities, confidence, model, latency_ms, reasons, acted, state_version)
        VALUES ($1, $2, $3, $4, $5, 'taught', 'taught', $6, $7, NULL, 1.0, 'taught', 0,
                jsonb_build_array(jsonb_build_object('rule', 'taught.rule', 'evidence', $8::text)),
                true, $9)
        ON CONFLICT (message_id, question, source) DO UPDATE
           SET value = EXCLUDED.value, options = EXCLUDED.options, reasons = EXCLUDED.reasons
        "#,
        DecisionId::new() as DecisionId,
        ctx.workspace_id,
        message as MessageId,
        conversation as ConversationId,
        question,
        value,
        options,
        rule,
        STATE_VERSION,
    )
    .execute(conn)
    .await?;
    Ok(())
}

async fn record(
    tx: &mut Transaction<'_, Postgres>,
    ctx: &Context<'_>,
    rule: RuleId,
    conversation: ConversationId,
    run: Option<RuleRunId>,
    before: &Before,
) -> Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO conversations.rule_applications
            (rule_id, conversation_id, workspace_id, run_id, before)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (rule_id, conversation_id) DO UPDATE
           SET run_id = EXCLUDED.run_id, before = EXCLUDED.before, applied_at = now(),
               reverted_at = NULL
         WHERE conversations.rule_applications.reverted_at IS NOT NULL
        "#,
        rule as RuleId,
        conversation as ConversationId,
        ctx.workspace_id,
        run as Option<RuleRunId>,
        json!(before),
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query!(
        r#"UPDATE conversations.rules
              SET applied_count = applied_count + 1, last_applied_at = now(), updated_at = now()
            WHERE id = $1"#,
        rule as RuleId,
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// The rules (ids as the engine names them) a member made an exception of
/// on conversation `id`.
async fn excepted_rules(conn: &mut PgConnection, id: ConversationId) -> Result<BTreeSet<String>> {
    Ok(sqlx::query_scalar!(
        r#"SELECT rule_id AS "rule: RuleId" FROM conversations.rule_applications
            WHERE conversation_id = $1 AND excepted_at IS NOT NULL"#,
        id as ConversationId,
    )
    .fetch_all(conn)
    .await?
    .into_iter()
    .map(|r| r.to_string())
    .collect())
}

/// Put back what `rule` changed on conversation `id`, where the
/// conversation still holds what the rule set. Returns whether anything
/// changed.
pub async fn revert(
    tx: &mut Transaction<'_, Postgres>,
    ctx: &Context<'_>,
    rule: &Stored,
    id: ConversationId,
) -> Result<bool> {
    let row = sqlx::query!(
        r#"SELECT before FROM conversations.rule_applications
            WHERE rule_id = $1 AND conversation_id = $2
              AND reverted_at IS NULL AND excepted_at IS NULL
            FOR UPDATE"#,
        rule.id as RuleId,
        id as ConversationId,
    )
    .fetch_optional(&mut **tx)
    .await?;
    let Some(row) = row else { return Ok(false) };
    let before: Before = serde_json::from_value(row.before).unwrap_or_default();
    let mut changes = Vec::new();
    let mut requested = Vec::new();
    if let Some((stream, source)) = &before.stream {
        let moved = sqlx::query!(
            r#"UPDATE conversations.conversations
                  SET stream = $2, stream_source = $3, updated_at = now()
                WHERE id = $1 AND stream_source = 'taught'"#,
            id as ConversationId,
            stream.as_str(),
            source,
        )
        .execute(&mut **tx)
        .await?
        .rows_affected();
        if moved > 0 {
            changes.push(Change::Stream);
            requested.push((
                super::conversation::provider_message_ids(tx, id).await?,
                RequestedChange::MoveToStream { stream: *stream },
            ));
        }
    }
    for (did, flag) in [
        (before.archived, Flag::Unarchive),
        (before.read, Flag::Unread),
    ] {
        if !did {
            continue;
        }
        let ids = flags::change(tx, id, flag).await?;
        if !ids.is_empty() {
            changes.push(flag.change());
            requested.push((ids, flag.requested()));
        }
    }
    for label in &before.labels {
        let removed = sqlx::query_scalar!(
            r#"DELETE FROM conversations.conversation_labels cl
                USING conversations.labels l
                WHERE cl.conversation_id = $1 AND cl.label_id = $2 AND l.id = cl.label_id
               RETURNING l.provider_key"#,
            id as ConversationId,
            *label as LabelId,
        )
        .fetch_optional(&mut **tx)
        .await?;
        if let Some(provider_key) = removed {
            changes.push(Change::Labels);
            requested.push((
                super::conversation::provider_message_ids(tx, id).await?,
                RequestedChange::RemoveLabel {
                    label: *label,
                    provider_key,
                },
            ));
        }
    }
    if !before.answered.is_empty() {
        sqlx::query!(
            r#"DELETE FROM conversations.decisions
                WHERE conversation_id = $1 AND source = 'taught' AND question = ANY($2)
                  AND reasons @> jsonb_build_array(jsonb_build_object('evidence', $3::text))"#,
            id as ConversationId,
            &before.answered,
            rule.id.to_string(),
        )
        .execute(&mut **tx)
        .await?;
    }
    sqlx::query!(
        r#"UPDATE conversations.rule_applications SET reverted_at = now()
            WHERE rule_id = $1 AND conversation_id = $2"#,
        rule.id as RuleId,
        id as ConversationId,
    )
    .execute(&mut **tx)
    .await?;
    if !changes.is_empty() {
        let mut changed = Changed {
            workspace: ctx.workspace.to_owned(),
            inbox: ctx.inbox,
            conversation: id,
            origin: Origin::Member,
            changes,
            provider_message_ids: Vec::new(),
        };
        for (ids, change) in requested {
            changed.provider_message_ids = ids;
            request(ctx.events, tx, &changed, &change).await?;
        }
        changed.provider_message_ids = super::conversation::provider_message_ids(tx, id).await?;
        changed.publish(ctx.events, tx).await?;
    }
    needs::refresh(tx, ctx.events, ctx.workspace, id, ctx.needs).await?;
    Ok(true)
}

// --- Exceptions --------------------------------------------------------------

/// What a member changed back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Undone {
    /// Moved it to another stream.
    Stream,
    /// Put it back in the inbox.
    Archive,
    /// Marked it unread.
    Read,
    /// Took a label off.
    Label(LabelId),
    /// Reopened a need a rule cleared.
    Need,
}

/// A rule a member just changed back on one conversation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RuleException {
    /// The rule.
    pub rule_id: RuleId,
    /// Its exceptions in the last 30 days, this one included.
    pub recent_exceptions: u32,
}

/// Make conversation `id` an exception of the rule whose change `undone`
/// takes back, if a rule made it. Returns that rule.
pub async fn except(
    conn: &mut PgConnection,
    id: ConversationId,
    undone: Undone,
    by: Option<UserId>,
) -> Result<Option<RuleException>> {
    let rows = sqlx::query!(
        r#"SELECT rule_id AS "rule: RuleId", before FROM conversations.rule_applications
            WHERE conversation_id = $1 AND reverted_at IS NULL AND excepted_at IS NULL
            ORDER BY applied_at DESC"#,
        id as ConversationId,
    )
    .fetch_all(&mut *conn)
    .await?;
    let found = rows.into_iter().find(|row| {
        let before: Before = serde_json::from_value(row.before.clone()).unwrap_or_default();
        match undone {
            Undone::Stream => before.stream.is_some(),
            Undone::Archive => before.archived,
            Undone::Read => before.read,
            Undone::Label(label) => before.labels.contains(&label),
            Undone::Need => !before.answered.is_empty(),
        }
    });
    let Some(found) = found else { return Ok(None) };
    sqlx::query!(
        r#"UPDATE conversations.rule_applications SET excepted_at = now(), excepted_by = $3
            WHERE rule_id = $1 AND conversation_id = $2"#,
        found.rule as RuleId,
        id as ConversationId,
        by.map(|u| *u.as_uuid()),
    )
    .execute(&mut *conn)
    .await?;
    let recent = recent_exceptions(conn, found.rule).await?;
    Ok(Some(RuleException {
        rule_id: found.rule,
        recent_exceptions: recent,
    }))
}

/// How many exceptions `rule` had in the last [`EXCEPTION_DAYS`].
pub async fn recent_exceptions(conn: &mut PgConnection, rule: RuleId) -> Result<u32> {
    let n = sqlx::query_scalar!(
        r#"SELECT count(*)::int AS "n!" FROM conversations.rule_applications
            WHERE rule_id = $1 AND excepted_at > now() - make_interval(days => $2)"#,
        rule as RuleId,
        i32::try_from(EXCEPTION_DAYS).unwrap_or(30),
    )
    .fetch_one(conn)
    .await?;
    Ok(u32::try_from(n).unwrap_or_default())
}

// --- On summaries ------------------------------------------------------------

/// The rule that placed or changed a conversation, as rows show it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RuleMark {
    /// The rule.
    pub id: RuleId,
    /// What it does.
    pub actions: Vec<RuleAction>,
    /// Its conditions, for "like … others from …".
    pub conditions: Vec<RuleCondition>,
    /// How many conversations it has changed.
    pub applied_count: u32,
}

/// Fill in each summary's [`RuleMark`]: the newest rule that changed it and
/// that no member changed back.
pub async fn attach(conn: &mut PgConnection, summaries: &mut [ConversationSummary]) -> Result<()> {
    let ids: Vec<ConversationId> = summaries.iter().map(|s| s.id).collect();
    if ids.is_empty() {
        return Ok(());
    }
    let rows = sqlx::query!(
        r#"
        SELECT DISTINCT ON (a.conversation_id)
               a.conversation_id AS "conversation: ConversationId", r.id AS "rule: RuleId",
               r.actions, r.conditions, r.applied_count
          FROM conversations.rule_applications a
          JOIN conversations.rules r ON r.id = a.rule_id
         WHERE a.conversation_id = ANY($1) AND a.reverted_at IS NULL AND a.excepted_at IS NULL
           AND r.state <> 'removed'
         ORDER BY a.conversation_id, a.applied_at DESC
        "#,
        &ids as &[ConversationId],
    )
    .fetch_all(conn)
    .await?;
    for row in rows {
        let Some(summary) = summaries.iter_mut().find(|s| s.id == row.conversation) else {
            continue;
        };
        summary.rule = Some(RuleMark {
            id: row.rule,
            actions: serde_json::from_value(row.actions).unwrap_or_default(),
            conditions: serde_json::from_value(row.conditions).unwrap_or_default(),
            applied_count: u32::try_from(row.applied_count).unwrap_or_default(),
        });
    }
    Ok(())
}

/// Whether `role` may change a rule taught by `author` (its author while a
/// sender, or a manager of its inbox).
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

/// Store a rule, replacing an active one with the same conditions and an
/// action of the same kind (without putting its mail back). Returns its id.
pub async fn insert(
    conn: &mut PgConnection,
    workspace: sqlx::types::Uuid,
    new: &NewRule<'_>,
) -> Result<RuleId> {
    let id = RuleId::new();
    let conditions = json!(new.conditions);
    if new.state == RuleState::Active {
        let kinds: Vec<String> = new
            .actions
            .iter()
            .filter(|a| !matches!(a, RuleAction::AddLabel { .. }))
            .filter_map(|a| serde_json::to_value(a).ok())
            .filter_map(|v| {
                v.get("action")
                    .and_then(|k| k.as_str())
                    .map(ToOwned::to_owned)
            })
            .collect();
        sqlx::query!(
            r#"UPDATE conversations.rules SET state = 'removed', removed_at = now(), updated_at = now()
                WHERE inbox_id = $1 AND state = 'active' AND conditions = $2
                  AND EXISTS (SELECT 1 FROM jsonb_array_elements(actions) a
                               WHERE a ->> 'action' = ANY($3))"#,
            new.inbox as InboxId,
            conditions,
            &kinds,
        )
        .execute(&mut *conn)
        .await?;
    }
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
        conditions,
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
