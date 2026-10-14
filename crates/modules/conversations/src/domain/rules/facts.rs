//! What the engine and the runs read about a conversation: the facts of its
//! newest inbound message, found with a prefilter an index can serve.

use campbooks_ai::decide::{signals::email::subject_template, taught::Facts};
use campbooks_kernel::{Error, Result};
use chrono::{DateTime, Utc};
use sqlx::PgConnection;

use super::RuleCondition;
use crate::{
    ingest::Headers,
    model::{ConversationId, InboxId, MessageId, Stream},
};

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
            subject_template: r.subject_template.filter(|t| !t.is_empty()),
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
    scan_rows(conn, inbox, &Prefilter::of(conditions), after, limit).await
}

async fn scan_rows(
    conn: &mut PgConnection,
    inbox: InboxId,
    p: &Prefilter,
    after: Option<ConversationId>,
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
    )
    .fetch_all(conn)
    .await?;
    rows.into_iter().map(Candidate::try_from).collect()
}

/// Messages whose subject template is filled per call of [`fill_templates`].
const TEMPLATE_PAGE: i64 = 1_000;

/// Work out the subject template of `inbox`'s inbound messages stored
/// before templates existed (`NULL`; `''` means "none"), so template
/// conditions match them. Cheap once done: only `NULL` rows are read.
pub async fn fill_templates(conn: &mut PgConnection, inbox: InboxId) -> Result<()> {
    loop {
        let rows = sqlx::query!(
            r#"SELECT id AS "id: MessageId", subject FROM conversations.messages
                WHERE inbox_id = $1 AND direction = 'inbound' AND subject_template IS NULL
                LIMIT $2"#,
            inbox as InboxId,
            TEMPLATE_PAGE,
        )
        .fetch_all(&mut *conn)
        .await?;
        if rows.is_empty() {
            return Ok(());
        }
        let ids: Vec<MessageId> = rows.iter().map(|r| r.id).collect();
        let templates: Vec<String> = rows
            .iter()
            .map(|r| subject_template(&r.subject).unwrap_or_default())
            .collect();
        sqlx::query!(
            r#"UPDATE conversations.messages m SET subject_template = t.template
                 FROM unnest($1::uuid[], $2::text[]) AS t(id, template)
                WHERE m.id = t.id"#,
            &ids as &[MessageId],
            &templates,
        )
        .execute(&mut *conn)
        .await?;
        if i64::try_from(rows.len()).unwrap_or(0) < TEMPLATE_PAGE {
            return Ok(());
        }
    }
}

/// The facts of conversation `id` (its newest inbound message), if it has
/// one.
pub async fn candidate(conn: &mut PgConnection, id: ConversationId) -> Result<Option<Candidate>> {
    let row = sqlx::query_as!(
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
         WHERE c.id = $1
        "#,
        id as ConversationId,
    )
    .fetch_optional(conn)
    .await?;
    row.map(Candidate::try_from).transpose()
}

/// Whether `inbox` ever wrote to `address`: they correspond, so mail from
/// them is never hidden by a suggested rule.
pub async fn corresponds(conn: &mut PgConnection, inbox: InboxId, address: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS (
               SELECT 1 FROM conversations.messages
                WHERE inbox_id = $1 AND direction = 'outbound'
                  AND position(('"' || $2 || '"') IN lower(recipients::text)) > 0
           ) AS "e!""#,
        inbox as InboxId,
        address.to_lowercase(),
    )
    .fetch_one(conn)
    .await?)
}
