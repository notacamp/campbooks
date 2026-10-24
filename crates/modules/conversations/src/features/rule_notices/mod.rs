//! "Scout noticed" (campbooks-cloud#169): mail an inbox gets a lot of, and
//! the smart rule Scout offers for it, one at a time at the bottom of Home.
//!
//! - `conversations.notice_rules` (a job, one per inbox, at most every
//!   [`NoticeSettings::every_hours`]) counts each sender's recent mail
//!   ([`analysis`]: how often and how regularly it comes, the stream the
//!   decide pass sorted it to, whether it asked anything or a payment is
//!   owed, and what the inbox did with it: opened, archived or deleted
//!   unread, replied), and keeps a suggestion per sender and kind
//!   (`conversations.rule_notices`), refreshed on every pass and dropped
//!   once the sender no longer qualifies.
//! - `GET …/rule-notices` lists the suggestions of the inboxes the caller may
//!   teach in, without those the caller answered "Not now" (for a while) or
//!   "Don't suggest this" (for good, per sender and kind), and starts a pass
//!   for an inbox not looked at lately.
//! - `GET …/rule-notices/{id}` reads one of them, as the list shows it (Scout's
//!   chat about a suggestion reads it so, as the member), and starts no pass.
//! - `POST …/rule-notices/{id}/answer` records that answer, per member;
//!   `DELETE` takes it back (Undo).
//! - Saying yes goes through `conversations.create_rule` with the
//!   suggestion's `notice_id`, after the member confirmed the smart rule:
//!   the suggestion is then accepted, and Undo of the new rule offers it
//!   again.
//!
//! No model is asked and no email text is read; the member's preference
//! becomes a deterministic rule, never prompt text.

mod analysis;
#[cfg(test)]
mod tests;

use axum::{Extension, Json, extract::State, http::StatusCode};
use campbooks_ai::decide::signals::email::{Envelope, Party, is_machine_sender};
use campbooks_kernel::{
    AppState, Error, JobContext, JobError, JobHandler, Operation, Problem, Result, Risk,
    db::BeginFor as _,
    extract,
    jobs::{EnqueueOptions, Enqueued},
    tenancy::{Workspace, WorkspaceId},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, Postgres, Transaction};
use utoipa::ToSchema;

pub use analysis::NoticeEvidence;

use crate::{
    config::NoticeSettings,
    domain::{
        access::visible_inboxes,
        rules::{self, RuleAction, RuleCondition},
        tracked,
    },
    features::rules::{PREVIEW_DEADLINE, compute_preview, may_teach, member_only},
    ingest::Headers,
    model::{InboxId, NoticeAnswer, NoticeKind, RuleId, RuleNoticeId, Stream},
    scopes::{ConversationsRead, ConversationsWrite},
    tracking,
};

use analysis::SenderStats;

/// Senders looked at per pass, the most frequent first.
const SENDERS_PER_PASS: i64 = 40;

// --- The pass ---------------------------------------------------------------

/// One inbox to look at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoticeRulesPayload {
    /// The inbox.
    pub inbox: InboxId,
}

/// Looks at an inbox's mail and keeps what Scout noticed.
pub struct NoticeRules {
    settings: NoticeSettings,
}

impl NoticeRules {
    /// The pass, under `settings`.
    pub const fn new(settings: NoticeSettings) -> Self {
        Self { settings }
    }
}

impl JobHandler for NoticeRules {
    type Payload = NoticeRulesPayload;
    const NAME: &'static str = "conversations.notice_rules";
    const QUEUE: &'static str = "rules";

    async fn run(&self, ctx: &mut JobContext, payload: NoticeRulesPayload) -> Result<(), JobError> {
        let tx = ctx.tx().await?;
        let kept = scan(tx, payload.inbox, &self.settings).await?;
        tracing::info!(suggestions = kept, "rule notices refreshed");
        Ok(())
    }
}

/// Enqueue a pass over `inbox` in `workspace`, unless one is pending.
async fn enqueue(
    tx: &mut Transaction<'_, Postgres>,
    workspace: &str,
    inbox: InboxId,
) -> Result<Enqueued, sqlx::Error> {
    campbooks_kernel::jobs::enqueue_to::<NoticeRules>(
        tx,
        &NoticeRulesPayload { inbox },
        EnqueueOptions::for_workspace(workspace.to_owned())
            .unique(format!("conversations.notice_rules:{inbox}"))
            .priority(-10),
    )
    .await
}

/// A sender's mail in the window, as one query counts it.
#[allow(
    clippy::struct_excessive_bools,
    reason = "one row of independent yes/no facts, read straight from SQL"
)]
struct StatsRow {
    sender_address: String,
    sender_name: Option<String>,
    received: i64,
    opened: i64,
    archived_unread: i64,
    last_week: i64,
    days: i64,
    replied: bool,
    suspicious: bool,
    owed: bool,
    asked: bool,
    stream: String,
    headers: serde_json::Value,
    dates: Vec<DateTime<Utc>>,
}

/// Look at `inbox`'s recent mail and keep what Scout noticed: a suggestion
/// per sender and kind, refreshed, and none left for a sender that no
/// longer qualifies. Returns how many it kept.
pub async fn scan(
    conn: &mut PgConnection,
    inbox: InboxId,
    settings: &NoticeSettings,
) -> Result<usize> {
    // An inbox kept read-only, or gone, gets nothing.
    let Some(workspace) = sqlx::query_scalar!(
        r#"SELECT workspace_id FROM conversations.inboxes
            WHERE id = $1 AND detached_at IS NULL"#,
        inbox as InboxId,
    )
    .fetch_optional(&mut *conn)
    .await?
    else {
        return Ok(0);
    };
    let rows = senders(conn, inbox, settings).await?;
    // Senders a rule of the inbox already speaks for.
    let taught: Vec<String> = rules::of_inboxes(conn, &[inbox])
        .await?
        .iter()
        .flat_map(|rule| rule.conditions.iter())
        .filter_map(|c| match c {
            RuleCondition::SenderAddress(address) => Some(address.clone()),
            _ => None,
        })
        .collect();
    let max = usize::try_from(settings.max_per_inbox).unwrap_or(usize::MAX);
    let mut kept = 0;
    for row in rows {
        if kept >= max {
            break;
        }
        if taught.contains(&row.sender_address) {
            continue;
        }
        let headers: Headers = serde_json::from_value(row.headers).unwrap_or_default();
        let stats = SenderStats {
            received: count(row.received),
            opened: count(row.opened),
            archived_unread: count(row.archived_unread),
            last_week: count(row.last_week),
            days: count(row.days),
            replied: row.replied,
            suspicious: row.suspicious,
            owed: row.owed,
            asked: row.asked,
            stream: row.stream.parse().unwrap_or(Stream::Primary),
            machine: is_machine_sender(&envelope(&row.sender_address, &headers)),
            correspondent: rules::corresponds(conn, inbox, &row.sender_address).await?,
            dates: row.dates,
            sender: row.sender_address,
        };
        let Some(noticed) = analysis::notice(&stats, settings) else {
            continue;
        };
        // Only what a rule would still change is worth a word.
        let deadline = std::time::Instant::now() + PREVIEW_DEADLINE;
        let preview = compute_preview(
            conn,
            inbox,
            (&noticed.conditions, &noticed.actions),
            deadline,
        )
        .await?;
        if preview.would_change == 0 {
            continue;
        }
        store(
            conn,
            (workspace, inbox),
            &noticed,
            &stats.sender,
            row.sender_name.as_deref(),
        )
        .await?;
        kept += 1;
    }
    // What no longer holds goes; accepted ones stay (they name a rule).
    sqlx::query!(
        r#"DELETE FROM conversations.rule_notices
            WHERE inbox_id = $1 AND state = 'open' AND refreshed_at < now()"#,
        inbox as InboxId,
    )
    .execute(&mut *conn)
    .await?;
    record_scan(conn, (workspace, inbox), settings).await?;
    Ok(kept)
}

/// How soon an inbox with no mail yet is looked at again: soon after its
/// first sync lands, never on every Home load.
const RETRY_EMPTY_MINUTES: i32 = 10;

/// Record that `inbox` was looked at, so the next pass waits
/// `every_hours`; or, with no mail in the window yet (its first sync still
/// to come, as on a new member's first Home), only [`RETRY_EMPTY_MINUTES`].
async fn record_scan(
    conn: &mut PgConnection,
    (workspace, inbox): (sqlx::types::Uuid, InboxId),
    settings: &NoticeSettings,
) -> Result<()> {
    let window = i32::try_from(settings.window_days).unwrap_or(i32::MAX);
    let has_mail = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM conversations.messages
                           WHERE inbox_id = $1 AND direction = 'inbound'
                             AND NOT in_spam AND NOT in_trash
                             AND sent_at > now() - make_interval(days => $2)) AS "e!""#,
        inbox as InboxId,
        window,
    )
    .fetch_one(&mut *conn)
    .await?;
    let every = i32::try_from(settings.every_hours).unwrap_or(i32::MAX);
    sqlx::query!(
        r#"INSERT INTO conversations.rule_notice_scans (inbox_id, workspace_id, scanned_at)
           VALUES ($1, $2, CASE WHEN $3 THEN now()
                                ELSE now() - make_interval(hours => $4)
                                     + make_interval(mins => $5) END)
           ON CONFLICT (inbox_id) DO UPDATE SET scanned_at = EXCLUDED.scanned_at"#,
        inbox as InboxId,
        workspace,
        has_mail,
        every,
        RETRY_EMPTY_MINUTES,
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Each sender's mail in `inbox` over the window, the most frequent first.
async fn senders(
    conn: &mut PgConnection,
    inbox: InboxId,
    settings: &NoticeSettings,
) -> Result<Vec<StatsRow>> {
    let window = i32::try_from(settings.window_days).unwrap_or(i32::MAX);
    Ok(sqlx::query_as!(
        StatsRow,
        r#"
        WITH latest AS (
            SELECT c.id, c.stream, c.archived_at IS NOT NULL AS archived, c.trashed,
                   c.unread_count > 0 AS unread, m.sender_address, m.sender ->> 'name' AS name,
                   m.sent_at, m.headers,
                   EXISTS (SELECT 1 FROM conversations.messages o
                            WHERE o.conversation_id = c.id AND o.direction = 'outbound') AS replied,
                   EXISTS (SELECT 1 FROM conversations.decisions d
                            WHERE d.conversation_id = c.id AND d.question = 'suspicious'
                              AND d.value = 'true'::jsonb) AS suspicious,
                   EXISTS (SELECT 1 FROM conversations.decisions d
                            WHERE d.conversation_id = c.id AND d.question = 'money'
                              AND d.value = '"owed"'::jsonb) AS owed,
                   EXISTS (SELECT 1 FROM conversations.needs n
                            WHERE n.conversation_id = c.id AND n.reason IN ('reply', 'do')
                              AND (n.state = 'open' OR n.verdict IN ('done', 'confirmed'))) AS asked
              FROM conversations.conversations c
              CROSS JOIN LATERAL (
                  SELECT sender_address, sender, sent_at, headers
                    FROM conversations.messages
                   WHERE conversation_id = c.id AND direction = 'inbound'
                   ORDER BY sent_at DESC, id DESC LIMIT 1
              ) m
             WHERE c.inbox_id = $1 AND c.last_message_at > now() - make_interval(days => $2)
        )
        SELECT sender_address AS "sender_address!",
               max(name) AS sender_name,
               count(*) AS "received!",
               count(*) FILTER (WHERE NOT unread) AS "opened!",
               count(*) FILTER (WHERE unread AND (archived OR trashed)) AS "archived_unread!",
               count(*) FILTER (WHERE sent_at > now() - interval '7 days') AS "last_week!",
               count(DISTINCT date_trunc('day', sent_at)) AS "days!",
               bool_or(replied) AS "replied!",
               bool_or(suspicious) AS "suspicious!",
               bool_or(owed) AS "owed!",
               bool_or(asked) AS "asked!",
               mode() WITHIN GROUP (ORDER BY stream) AS "stream!",
               (array_agg(headers ORDER BY sent_at DESC))[1] AS "headers!",
               array_agg(sent_at ORDER BY sent_at) AS "dates!"
          FROM latest
         GROUP BY sender_address
        HAVING count(*) >= $3
         ORDER BY count(*) DESC, sender_address
         LIMIT $4
        "#,
        inbox as InboxId,
        window,
        i64::from(settings.min_messages),
        SENDERS_PER_PASS,
    )
    .fetch_all(conn)
    .await?)
}

/// Keep `noticed` for `sender` in `inbox`: new, or refreshed while still
/// open (an accepted one stays as it is).
async fn store(
    conn: &mut PgConnection,
    at: (sqlx::types::Uuid, InboxId),
    noticed: &analysis::Noticed,
    sender: &str,
    name: Option<&str>,
) -> Result<()> {
    let (workspace, inbox) = at;
    sqlx::query!(
        r#"
        INSERT INTO conversations.rule_notices
            (id, workspace_id, inbox_id, kind, sender_address, sender_name, conditions,
             actions, evidence)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        ON CONFLICT (inbox_id, kind, sender_address) DO UPDATE
           SET sender_name = EXCLUDED.sender_name, conditions = EXCLUDED.conditions,
               actions = EXCLUDED.actions, evidence = EXCLUDED.evidence,
               refreshed_at = now()
         WHERE conversations.rule_notices.state = 'open'
        "#,
        RuleNoticeId::new() as RuleNoticeId,
        workspace,
        inbox as InboxId,
        noticed.kind.as_str(),
        sender,
        name.map(|n| n.chars().take(200).collect::<String>()),
        serde_json::json!(noticed.conditions),
        serde_json::json!(noticed.actions),
        serde_json::json!(noticed.evidence),
    )
    .execute(conn)
    .await?;
    Ok(())
}

fn count(n: i64) -> u32 {
    u32::try_from(n).unwrap_or_default()
}

/// The envelope the machine-sender facts read: the address and the bulk
/// headers, nothing of the text.
fn envelope(sender: &str, headers: &Headers) -> Envelope {
    let mut map = std::collections::BTreeMap::new();
    for (name, value) in [
        ("List-Id", &headers.list_id),
        ("List-Unsubscribe", &headers.list_unsubscribe),
        ("Precedence", &headers.precedence),
        ("Auto-Submitted", &headers.auto_submitted),
    ] {
        if let Some(value) = value {
            map.insert(name.to_owned(), value.clone());
        }
    }
    Envelope {
        from: Party {
            name: String::new(),
            address: sender.to_owned(),
        },
        headers: map,
        ..Envelope::default()
    }
}

// --- Listing ----------------------------------------------------------------

/// A smart rule Scout offers for mail an inbox gets a lot of.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RuleNotice {
    /// The suggestion.
    pub id: RuleNoticeId,
    /// Its inbox.
    pub inbox_id: InboxId,
    /// What Scout noticed.
    pub kind: NoticeKind,
    /// The sender's address.
    pub sender: String,
    /// The name the sender gives, as plain text.
    pub sender_name: Option<String>,
    /// The rule's conditions.
    pub conditions: Vec<RuleCondition>,
    /// Its actions.
    pub actions: Vec<RuleAction>,
    /// Why, in numbers.
    pub evidence: NoticeEvidence,
    /// Other members use the inbox: the rule would act for them too.
    pub shared: bool,
}

/// What Scout noticed, the most frequent sender first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RuleNoticeList {
    /// The suggestions.
    pub notices: Vec<RuleNotice>,
}

#[derive(sqlx::FromRow)]
struct NoticeRow {
    id: RuleNoticeId,
    inbox_id: InboxId,
    kind: String,
    sender_address: String,
    sender_name: Option<String>,
    conditions: serde_json::Value,
    actions: serde_json::Value,
    evidence: serde_json::Value,
}

/// List what Scout noticed in the inboxes you may teach in: smart rules it
/// offers for mail you get a lot of.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/rule-notices",
    operation_id = "conversations.list_rule_notices",
    tag = "conversations",
    security(("bearer" = ["conversations:read"])),
    extensions(("x-campbooks" = json!(Operation::query().records(["conversations.rule"]).access("conversations.inbox", campbooks_kernel::authz::Action::Write)))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    responses(
        (status = OK, description = "The suggestions.", body = RuleNoticeList),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or an app asked.", body = Problem,
            content_type = "application/problem+json"),
    )
)]
pub async fn list(
    State(state): State<AppState>,
    Extension(settings): Extension<NoticeSettings>,
    workspace: Workspace<ConversationsRead>,
) -> Result<Json<RuleNoticeList>> {
    let access = &workspace.access;
    // Suggestions are about a person's mail: an app acting for nobody is
    // refused (403).
    if access.user.is_none() {
        return Err(Error::Denied(crate::MESSAGES.reason("rule_needs_member")));
    }
    let mut tx = state.db.begin_for(access.workspace).await?;
    let mut teachable = Vec::new();
    for inbox in visible_inboxes(&mut tx, access).await? {
        if may_teach(&state, &mut tx, access, inbox).await.is_ok() {
            teachable.push(inbox);
        }
    }
    if teachable.is_empty() {
        tx.commit().await?;
        return Ok(Json(RuleNoticeList {
            notices: Vec::new(),
        }));
    }
    // Inboxes not looked at lately get a pass; what it finds shows next time.
    let every = i32::try_from(settings.every_hours).unwrap_or(i32::MAX);
    let to_scan = sqlx::query_scalar!(
        r#"SELECT i.id AS "id!: InboxId"
             FROM unnest($1::uuid[]) AS i(id)
             LEFT JOIN conversations.rule_notice_scans s ON s.inbox_id = i.id
            WHERE s.scanned_at IS NULL OR s.scanned_at < now() - make_interval(hours => $2)"#,
        &teachable as &[InboxId],
        every,
    )
    .fetch_all(&mut *tx)
    .await?;
    let ws = access.workspace.to_string();
    for inbox in to_scan {
        enqueue(&mut tx, &ws, inbox).await?;
    }
    let rows = sqlx::query_as!(
        NoticeRow,
        r#"
        SELECT n.id AS "id: RuleNoticeId", n.inbox_id AS "inbox_id: InboxId", n.kind,
               n.sender_address, n.sender_name, n.conditions, n.actions, n.evidence
          FROM conversations.rule_notices n
         WHERE n.inbox_id = ANY($1) AND n.state = 'open'
           AND NOT EXISTS (
               SELECT 1 FROM conversations.rule_notice_answers a
                WHERE a.inbox_id = n.inbox_id AND a.user_id = $2 AND a.kind = n.kind
                  AND a.sender_address = n.sender_address
                  AND (a.answer = 'never' OR a.until > now()))
         ORDER BY (n.evidence ->> 'received')::int DESC, n.id
        "#,
        &teachable as &[InboxId],
        access.user.map(|u| *u.as_uuid()),
    )
    .fetch_all(&mut *tx)
    .await?;
    let shared = rules::shared_of(&mut tx, &teachable).await?;
    tx.commit().await?;
    let notices = rows
        .into_iter()
        .map(|row| {
            let shared = shared.contains(&row.inbox_id);
            notice_of(row, shared)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Json(RuleNoticeList { notices }))
}

/// A suggestion as the lists show it.
fn notice_of(row: NoticeRow, shared: bool) -> Result<RuleNotice> {
    let json = |e: serde_json::Error| Error::internal(crate::domain::redact::json(&e));
    Ok(RuleNotice {
        id: row.id,
        inbox_id: row.inbox_id,
        kind: row.kind.parse()?,
        sender: row.sender_address,
        sender_name: row.sender_name,
        conditions: serde_json::from_value(row.conditions).map_err(json)?,
        actions: serde_json::from_value(row.actions).map_err(json)?,
        evidence: serde_json::from_value(row.evidence).map_err(json)?,
        shared,
    })
}

/// Read one of Scout's suggestions, as `conversations.list_rule_notices`
/// lists it: an open one, in an inbox you may teach in, that you did not
/// answer "Not now" (for a while) or "Don't suggest this". It starts no
/// pass over the inbox.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/rule-notices/{notice_id}",
    operation_id = "conversations.get_rule_notice",
    tag = "conversations",
    security(("bearer" = ["conversations:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant().records(["conversations.rule"]).access("conversations.inbox", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("notice_id" = RuleNoticeId, Path, description = "The suggestion."),
    ),
    responses(
        (status = OK, description = "The suggestion.", body = RuleNotice),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or an app asked.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such suggestion, it is no longer offered, \
            you answered it, or you cannot teach in its inbox.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn get(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsRead>,
    extract::Path(path): extract::Path<NoticePath>,
) -> Result<Json<RuleNotice>> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let (inbox, _, _) = target(&mut tx, path.notice_id).await?;
    // As the list: only an inbox the caller may teach in. One they only read
    // answers as one they cannot see (404), since the list never shows them
    // its suggestions; an app gets 403: suggestions are a member's own.
    may_teach(&state, &mut tx, access, inbox)
        .await
        .map_err(|error| match error {
            Error::Denied(_) => Error::NotFound,
            other => other,
        })?;
    member_only(&workspace.principal)?;
    let user = access.user.ok_or(Error::NotFound)?;
    let row: NoticeRow = sqlx::query_as(
        "SELECT n.id, n.inbox_id, n.kind, n.sender_address, n.sender_name, n.conditions, \
                n.actions, n.evidence \
           FROM conversations.rule_notices n \
          WHERE n.id = $1 AND n.state = 'open' \
            AND NOT EXISTS ( \
                SELECT 1 FROM conversations.rule_notice_answers a \
                 WHERE a.inbox_id = n.inbox_id AND a.user_id = $2 AND a.kind = n.kind \
                   AND a.sender_address = n.sender_address \
                   AND (a.answer = 'never' OR a.until > now()))",
    )
    .bind(path.notice_id)
    .bind(user.as_uuid())
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(Error::NotFound)?;
    let shared = rules::shared_of(&mut tx, &[inbox]).await?.contains(&inbox);
    tx.commit().await?;
    Ok(Json(notice_of(row, shared)?))
}

// --- Answers ----------------------------------------------------------------

/// `…/rule-notices/{notice_id}…`
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct NoticePath {
    /// The suggestion.
    pub notice_id: RuleNoticeId,
}

/// "Not now" or "Don't suggest this".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AnswerRuleNotice {
    /// The answer.
    pub answer: NoticeAnswer,
}

/// A suggestion's inbox, kind and sender; 404 when it is gone or in another
/// workspace (row-level security).
async fn target(conn: &mut PgConnection, id: RuleNoticeId) -> Result<(InboxId, String, String)> {
    let row = sqlx::query!(
        r#"SELECT inbox_id AS "inbox: InboxId", kind, sender_address
             FROM conversations.rule_notices WHERE id = $1"#,
        id as RuleNoticeId,
    )
    .fetch_optional(conn)
    .await?
    .ok_or(Error::NotFound)?;
    Ok((row.inbox, row.kind, row.sender_address))
}

/// Answer a suggestion: "Not now" hides it for a while, "Don't suggest
/// this" for good (for that sender and kind). For you alone.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/rule-notices/{notice_id}/answer",
    operation_id = "conversations.answer_rule_notice",
    tag = "conversations",
    security(("bearer" = ["conversations:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("conversations.clear_rule_notice_answer")
        .records(["conversations.rule"]).access("conversations.inbox", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("notice_id" = RuleNoticeId, Path, description = "The suggestion."),
    ),
    request_body = AnswerRuleNotice,
    responses(
        (status = NO_CONTENT, description = "Answered."),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, you may only read this inbox, or an app asked.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such suggestion, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn answer(
    State(state): State<AppState>,
    Extension(settings): Extension<NoticeSettings>,
    workspace: Workspace<ConversationsWrite>,
    extract::Path(path): extract::Path<NoticePath>,
    extract::Json(body): extract::Json<AnswerRuleNotice>,
) -> Result<StatusCode> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let (inbox, kind, sender) = target(&mut tx, path.notice_id).await?;
    // 404 unless the caller sees its inbox, 403 when they only read it; an
    // app gets 403 too: answers are a member's own.
    may_teach(&state, &mut tx, access, inbox).await?;
    member_only(&workspace.principal)?;
    let user = access.user.ok_or(Error::NotFound)?;
    let days = match body.answer {
        NoticeAnswer::NotNow => Some(i32::try_from(settings.not_now_days).unwrap_or(i32::MAX)),
        NoticeAnswer::Never => None,
    };
    sqlx::query!(
        r#"INSERT INTO conversations.rule_notice_answers
               (workspace_id, inbox_id, user_id, kind, sender_address, answer, until)
           VALUES ($1, $2, $3, $4, $5, $6, now() + make_interval(days => $7))
           ON CONFLICT (inbox_id, user_id, kind, sender_address) DO UPDATE
              SET answer = EXCLUDED.answer, until = EXCLUDED.until, answered_at = now()"#,
        access.workspace.as_uuid(),
        inbox as InboxId,
        user.as_uuid(),
        kind,
        sender,
        body.answer.as_str(),
        days,
    )
    .execute(&mut *tx)
    .await?;
    state
        .tracker
        .track(
            &mut tx,
            &tracking::RuleNoticeAnswered {
                workspace: access.workspace,
                notice_id: *path.notice_id.as_uuid(),
                inbox_id: *inbox.as_uuid(),
                kind: tracked::notice_kind(kind.parse()?),
                answer: tracked::notice_answer(body.answer),
            },
        )
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Take back your answer to a suggestion (Undo): it is offered again.
#[utoipa::path(
    delete,
    path = "/api/v1/w/{workspace_id}/rule-notices/{notice_id}/answer",
    operation_id = "conversations.clear_rule_notice_answer",
    tag = "conversations",
    security(("bearer" = ["conversations:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .records(["conversations.rule"]).access("conversations.inbox", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("notice_id" = RuleNoticeId, Path, description = "The suggestion."),
    ),
    responses(
        (status = NO_CONTENT, description = "Offered again."),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, you may only read this inbox, or an app asked.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such suggestion, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn clear_answer(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsWrite>,
    extract::Path(path): extract::Path<NoticePath>,
) -> Result<StatusCode> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let (inbox, kind, sender) = target(&mut tx, path.notice_id).await?;
    // 404 unless the caller sees its inbox, 403 when they only read it; an
    // app gets 403 too: answers are a member's own.
    may_teach(&state, &mut tx, access, inbox).await?;
    member_only(&workspace.principal)?;
    let user = access.user.ok_or(Error::NotFound)?;
    let cleared = sqlx::query!(
        r#"DELETE FROM conversations.rule_notice_answers
            WHERE inbox_id = $1 AND user_id = $2 AND kind = $3 AND sender_address = $4"#,
        inbox as InboxId,
        user.as_uuid(),
        kind,
        sender,
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if cleared > 0 {
        state
            .tracker
            .track(
                &mut tx,
                &tracking::RuleNoticeAnswerCleared {
                    workspace: access.workspace,
                    notice_id: *path.notice_id.as_uuid(),
                    inbox_id: *inbox.as_uuid(),
                    kind: tracked::notice_kind(kind.parse()?),
                },
            )
            .await?;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

// --- Saying yes -------------------------------------------------------------

/// 400 `invalid_rule` unless `notice` is an open suggestion of `inbox`.
pub async fn check_open(
    conn: &mut PgConnection,
    notice: RuleNoticeId,
    inbox: InboxId,
) -> Result<()> {
    let open = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM conversations.rule_notices
                           WHERE id = $1 AND inbox_id = $2 AND state = 'open') AS "open!""#,
        notice as RuleNoticeId,
        inbox as InboxId,
    )
    .fetch_one(conn)
    .await?;
    if open {
        Ok(())
    } else {
        Err(Error::BadRequest(crate::MESSAGES.reason("invalid_rule")))
    }
}

/// The member said yes to `notice`: `rule` is its smart rule. Only an open
/// one: of two saves at once, the second waits for the first and is refused
/// (`invalid_rule`), so a suggestion becomes one rule.
pub async fn accept(conn: &mut PgConnection, notice: RuleNoticeId, rule: RuleId) -> Result<()> {
    let accepted = sqlx::query!(
        r#"UPDATE conversations.rule_notices
              SET state = 'accepted', rule_id = $2, accepted_at = now()
            WHERE id = $1 AND state = 'open'"#,
        notice as RuleNoticeId,
        rule as RuleId,
    )
    .execute(conn)
    .await?
    .rows_affected();
    if accepted == 1 {
        Ok(())
    } else {
        Err(Error::BadRequest(crate::MESSAGES.reason("invalid_rule")))
    }
}

/// `rule` was removed within `minutes` of being saved from a suggestion
/// (Undo): the suggestion is offered again. It keeps naming the rule, so
/// bringing the rule back ([`reaccept`]) takes the suggestion with it.
pub async fn reopen(conn: &mut PgConnection, rule: RuleId, minutes: i32) -> Result<()> {
    sqlx::query!(
        r#"UPDATE conversations.rule_notices
              SET state = 'open', accepted_at = NULL, refreshed_at = now()
            WHERE rule_id = $1 AND accepted_at > now() - make_interval(mins => $2)"#,
        rule as RuleId,
        minutes,
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// `rule` was brought back (Undo of its removal): the suggestion it came
/// from, offered again meanwhile, is accepted again, so Scout does not offer
/// it twice and saving it cannot make a second rule.
pub async fn reaccept(conn: &mut PgConnection, rule: RuleId) -> Result<()> {
    sqlx::query!(
        r#"UPDATE conversations.rule_notices SET state = 'accepted', accepted_at = now()
            WHERE rule_id = $1 AND state = 'open'"#,
        rule as RuleId,
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Forget everything noticed in `inbox` (its connection was removed and its
/// mail kept read-only).
pub async fn forget_inbox(conn: &mut PgConnection, inbox: InboxId) -> Result<()> {
    sqlx::query!(
        "DELETE FROM conversations.rule_notices WHERE inbox_id = $1",
        inbox as InboxId
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!(
        "DELETE FROM conversations.rule_notice_answers WHERE inbox_id = $1",
        inbox as InboxId
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}
