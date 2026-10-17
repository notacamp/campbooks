//! What the engine and the runs read about a conversation: the facts of its
//! newest inbound message, found with a prefilter an index can serve.

use campbooks_ai::decide::{
    signals::{email::subject_template, facts::sender_checks_failed},
    taught::{Facts, normalise_list_id},
};
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
    /// One of its messages looks suspicious (not only the newest), or it
    /// has an open Review need.
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
            auth_failed: self.auth_failed(),
        }
    }

    /// Whether the receiving provider's authentication results say its
    /// sender failed its checks.
    pub fn auth_failed(&self) -> bool {
        sender_checks_failed(
            &self.sender_address,
            self.headers.authentication_results.as_deref(),
        )
    }
}

/// Columns of a prefilter: the narrowing conditions a query can use an
/// index for. The engine has the last word on every row.
#[derive(Debug, Default)]
struct Prefilter {
    sender: Option<String>,
    domain: Option<String>,
    /// `domain` escaped for `LIKE`.
    domain_like: Option<String>,
    list: Option<String>,
    template: Option<String>,
}

impl Prefilter {
    fn of(conditions: &[RuleCondition]) -> Self {
        let mut p = Self::default();
        for c in conditions {
            match c {
                RuleCondition::SenderAddress(v) => p.sender = Some(v.trim().to_lowercase()),
                RuleCondition::SenderDomain(v) => {
                    p.domain = Some(v.trim().to_lowercase());
                    p.domain_like = Some(like_escape(&v.trim().to_lowercase()));
                }
                // The engine compares the id inside the brackets; the stored
                // header holds it, whatever is around it.
                RuleCondition::ListId(v) => p.list = Some(like_escape(&normalise_list_id(v))),
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
        // `''` means "no template"; NULL means not worked out yet.
        let template = match r.subject_template {
            None => subject_template(&r.subject),
            Some(t) if t.is_empty() => None,
            Some(t) => Some(t),
        };
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
            subject_template: template,
            headers: serde_json::from_value(r.headers).unwrap_or_default(),
            suspicious: r.suspicious,
            sent_at: r.sent_at,
        })
    }
}

/// `value` escaped for a `LIKE` pattern with `ESCAPE '\'`.
fn like_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// One window of an inbox's conversations, in id order.
#[derive(Debug)]
pub struct Window {
    /// The conversations in it that pass the prefilter, with their facts.
    pub candidates: Vec<Candidate>,
    /// Where the next window starts; `None` at the end of the inbox.
    pub next: Option<ConversationId>,
    /// How many conversations it looked at.
    pub examined: usize,
}

/// The next `size` conversations of `inbox` after `after` (by id): those
/// whose newest inbound message passes `conditions`' prefilter, with their
/// facts. Looks at `size` conversations at most, however few match.
pub async fn scan(
    conn: &mut PgConnection,
    inbox: InboxId,
    conditions: &[RuleCondition],
    after: Option<ConversationId>,
    size: i64,
) -> Result<Window> {
    let ids = sqlx::query_scalar!(
        r#"SELECT id AS "id: ConversationId" FROM conversations.conversations
            WHERE inbox_id = $1 AND ($2::uuid IS NULL OR id > $2)
            ORDER BY id LIMIT $3"#,
        inbox as InboxId,
        after.map(|a| *a.as_uuid()),
        size,
    )
    .fetch_all(&mut *conn)
    .await?;
    let examined = ids.len();
    let next = ids
        .last()
        .copied()
        .filter(|_| i64::try_from(examined).unwrap_or(i64::MAX) >= size);
    if ids.is_empty() {
        return Ok(Window {
            candidates: Vec::new(),
            next,
            examined,
        });
    }
    let candidates = rows(conn, &ids, &Prefilter::of(conditions)).await?;
    Ok(Window {
        candidates,
        next,
        examined,
    })
}

async fn rows(
    conn: &mut PgConnection,
    ids: &[ConversationId],
    p: &Prefilter,
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
                         WHERE d.conversation_id = c.id AND d.question = 'suspicious'
                           AND d.value = 'true'::jsonb)
                OR EXISTS (SELECT 1 FROM conversations.needs n
                            WHERE n.conversation_id = c.id AND n.state = 'open'
                              AND n.reason = 'review')) AS "suspicious!",
               m.sent_at AS "sent_at!"
          FROM conversations.conversations c
          CROSS JOIN LATERAL (
              SELECT id, sender_address, subject, subject_template, headers, sent_at
                FROM conversations.messages
               WHERE conversation_id = c.id AND direction = 'inbound'
               ORDER BY sent_at DESC, id DESC LIMIT 1
          ) m
         WHERE c.id = ANY($1) AND NOT c.trashed
           AND ($2::text IS NULL OR m.sender_address = $2)
           AND ($3::text IS NULL OR split_part(m.sender_address, '@', 2) = $3
                OR split_part(m.sender_address, '@', 2) LIKE '%.' || $4 ESCAPE '\')
           AND ($5::text IS NULL OR lower(m.headers ->> 'list_id') LIKE '%' || $5 || '%' ESCAPE '\')
           AND ($6::text IS NULL OR m.subject_template = $6 OR m.subject_template IS NULL)
         ORDER BY c.id
        "#,
        ids as &[ConversationId],
        p.sender,
        p.domain,
        p.domain_like,
        p.list,
        p.template,
    )
    .fetch_all(conn)
    .await?;
    rows.into_iter().map(Candidate::try_from).collect()
}

/// Messages whose subject template one [`fill_templates`] call works out.
pub const TEMPLATE_PAGE: i64 = 1_000;

/// Work out the subject template of up to [`TEMPLATE_PAGE`] of `inbox`'s
/// inbound messages stored before templates existed (`NULL`; `''` means
/// "none"). Returns how many it did: fewer than a page means none is left.
/// Rules match such messages anyway (their template is worked out when they
/// are read); this only lets the template index serve them.
pub async fn fill_templates(conn: &mut PgConnection, inbox: InboxId) -> Result<usize> {
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
        return Ok(0);
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
    .execute(conn)
    .await?;
    Ok(rows.len())
}

/// Whether `inbox` still has inbound messages without a worked-out template.
pub async fn templates_missing(conn: &mut PgConnection, inbox: InboxId) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM conversations.messages
                           WHERE inbox_id = $1 AND direction = 'inbound'
                             AND subject_template IS NULL) AS "e!""#,
        inbox as InboxId,
    )
    .fetch_one(conn)
    .await?)
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
                         WHERE d.conversation_id = c.id AND d.question = 'suspicious'
                           AND d.value = 'true'::jsonb)
                OR EXISTS (SELECT 1 FROM conversations.needs n
                            WHERE n.conversation_id = c.id AND n.state = 'open'
                              AND n.reason = 'review')) AS "suspicious!",
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

/// Sent messages [`corresponds`] looks through, newest first.
pub const CORRESPONDENCE_WINDOW: i64 = 5_000;

/// Whether `inbox` wrote to `address` in its last [`CORRESPONDENCE_WINDOW`]
/// sent messages: they correspond, so mail from them is never hidden by a
/// suggested rule. Served by `messages_outbound_idx`.
pub async fn corresponds(conn: &mut PgConnection, inbox: InboxId, address: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS (
               SELECT 1 FROM (
                   SELECT recipients FROM conversations.messages
                    WHERE inbox_id = $1 AND direction = 'outbound'
                    ORDER BY sent_at DESC LIMIT $3
               ) sent
                WHERE position(('"' || $2 || '"') IN lower(sent.recipients::text)) > 0
           ) AS "e!""#,
        inbox as InboxId,
        address.to_lowercase(),
        CORRESPONDENCE_WINDOW,
    )
    .fetch_one(conn)
    .await?)
}
