//! "Scout noticed" (campbooks-cloud#169, #356): mail an inbox gets a lot of,
//! and the smart rule Scout offers for it, one at a time at the bottom of
//! Home.
//!
//! - `conversations.notice_rules` (a job, one per inbox, at most every
//!   [`NoticeSettings::every_hours`]) counts each frequent sender's recent
//!   mail ([`analysis`]: how often and how regularly it comes, the stream the
//!   decide pass sorted it to, whether it asked anything or a payment is
//!   owed, and what the inbox did with it: opened, archived or deleted
//!   unread, replied), split into kinds where the sender's mail is mixed (by
//!   subject template, or by words the `complete` model proposed for its
//!   subjects, [`grouping`]), and keeps a suggestion per sender, kind of
//!   suggestion and pattern (`conversations.rule_notices`), refreshed on
//!   every pass and dropped once it no longer qualifies.
//! - `GET …/rule-notices` lists the suggestions of the inboxes the caller may
//!   teach in, without those the caller answered "Not now" (for a while) or
//!   "Don't suggest this" (for good, per sender, kind and pattern), and
//!   starts a pass for an inbox not looked at lately.
//! - `GET …/rule-notices/{id}` reads one of them, as the list shows it (Scout's
//!   chat about a suggestion reads it so, as the member), and starts no pass.
//! - `POST …/rule-notices/{id}/answer` records that answer, per member;
//!   `DELETE` takes it back (Undo).
//! - Saying yes goes through `conversations.create_rule` with the
//!   suggestion's `notice_id`, after the member confirmed the smart rule:
//!   the suggestion is then accepted, and Undo of the new rule offers it
//!   again.
//!
//! No body is read. Subjects are grouped by template equality, and only for
//! a mixed sender the model sees its subjects, nothing else, while the
//! `complete` role may receive mail here; the member's preference becomes a
//! deterministic rule, never prompt text.

mod analysis;
mod grouping;
#[cfg(test)]
mod tests;

use std::collections::HashMap;

use axum::{Extension, Json, extract::State, http::StatusCode};
use campbooks_ai::{
    Gateway, Role,
    decide::signals::email::{Envelope, Party, is_machine_sender, subject_template, subject_words},
    usage::Scope,
};
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
        rules::{self, RuleAction, RuleCondition, Stored},
        tracked,
    },
    features::{
        read_message::error_kind,
        rules::{PREVIEW_DEADLINE, compute_preview, may_teach, member_only},
    },
    ingest::Headers,
    model::{InboxId, NoticeAnswer, NoticeKind, RuleId, RuleNoticeId, Stream},
    scopes::{ConversationsRead, ConversationsWrite},
    tracking,
};

use analysis::{Mail, Pattern, Sender};
use grouping::ToGroup;

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
    gateway: Option<Gateway>,
}

impl NoticeRules {
    /// The pass, under `settings`; the model's grouping of mixed senders'
    /// subjects through `gateway`'s `complete` role, `None` (the role is not
    /// configured, or may not receive mail here) never asking it.
    pub const fn new(settings: NoticeSettings, gateway: Option<Gateway>) -> Self {
        Self { settings, gateway }
    }
}

impl JobHandler for NoticeRules {
    type Payload = NoticeRulesPayload;
    const NAME: &'static str = "conversations.notice_rules";
    const QUEUE: &'static str = "rules";

    async fn run(&self, ctx: &mut JobContext, payload: NoticeRulesPayload) -> Result<(), JobError> {
        // The model first, with no transaction held while it answers; what
        // it found is kept, then the pass reads it.
        if let Some(gateway) = &self.gateway {
            group(ctx, gateway, payload.inbox, &self.settings).await?;
        }
        let tx = ctx.tx().await?;
        let kept = scan(tx, payload.inbox, &self.settings).await?;
        tracing::info!(suggestions = kept, "rule notices refreshed");
        Ok(())
    }
}

/// Ask the model to group the subjects of the inbox's mixed senders whose
/// grouping is missing or stale, at most `model_senders_per_pass`, and keep
/// what it answers. Any failure of the model leaves things as they were:
/// the pass goes on with the templates alone.
async fn group(
    ctx: &JobContext,
    gateway: &Gateway,
    inbox: InboxId,
    settings: &NoticeSettings,
) -> Result<(), JobError> {
    if settings.model_senders_per_pass == 0 {
        return Ok(());
    }
    let Some(workspace) = ctx.workspace().and_then(|w| w.parse::<WorkspaceId>().ok()) else {
        return Ok(());
    };
    let state = ctx.state();
    let mut tx = state.db.begin_for(workspace).await?;
    let asks = to_group(&mut tx, inbox, settings).await?;
    tx.commit().await?;
    if asks.is_empty() {
        return Ok(());
    }
    let usage = Scope::workspace(state.db.clone(), workspace, grouping::FEATURE);
    let found = usage
        .run(async {
            let mut found = Vec::new();
            for ask in &asks {
                match gateway
                    .chat(Role::Complete, grouping::request_for(&ask.subjects))
                    .await
                {
                    // An answer that is not the JSON asked for is kept as
                    // "no kinds", so the sender is not asked about again soon.
                    Ok(response) => found.push((
                        ask.sender.clone(),
                        grouping::phrases(&response).unwrap_or_default(),
                    )),
                    Err(error) => {
                        tracing::info!(
                            feature = grouping::FEATURE,
                            error = error_kind(&error),
                            "the grouping call failed"
                        );
                        // Over the daily limit, or refused: the others would
                        // be too.
                        if matches!(
                            error,
                            campbooks_ai::Error::OverDailyCap { .. }
                                | campbooks_ai::Error::ExternalContentRefused { .. }
                                | campbooks_ai::Error::RoleNotConfigured(_)
                        ) {
                            break;
                        }
                    }
                }
            }
            found
        })
        .await;
    if found.is_empty() {
        return Ok(());
    }
    let mut tx = state.db.begin_for(workspace).await?;
    if let Some(at) = inbox_workspace(&mut tx, inbox).await? {
        grouping::keep(&mut tx, (at, inbox), &found).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// The workspace of `inbox`, unless it is kept read-only or gone.
async fn inbox_workspace(
    conn: &mut PgConnection,
    inbox: InboxId,
) -> Result<Option<sqlx::types::Uuid>> {
    Ok(sqlx::query_scalar!(
        r#"SELECT workspace_id FROM conversations.inboxes
            WHERE id = $1 AND detached_at IS NULL"#,
        inbox as InboxId,
    )
    .fetch_optional(&mut *conn)
    .await?)
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

/// A frequent sender's conversation in the window, as one query reads it.
#[derive(sqlx::FromRow)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "one row of independent yes/no facts, read straight from SQL"
)]
struct MailRow {
    sender_address: String,
    name: Option<String>,
    subject: String,
    subject_template: Option<String>,
    sent_at: DateTime<Utc>,
    opened: bool,
    archived_unread: bool,
    replied: bool,
    suspicious: bool,
    owed: bool,
    asked: bool,
    stream: String,
    headers: serde_json::Value,
}

/// The conversations of `inbox`'s senders over the window whose newest
/// inbound message is theirs: of the most frequent senders with at least
/// `min` (or of `only`), most frequent first, each sender's newest first.
const SENDER_MAIL: &str = "
    WITH latest AS (
        SELECT c.id, c.stream, c.archived_at IS NOT NULL AS archived, c.trashed,
               c.unread_count > 0 AS unread, m.sender_address, m.sender ->> 'name' AS name,
               m.subject, m.subject_template, m.sent_at, m.headers,
               EXISTS (SELECT 1 FROM conversations.messages o
                        WHERE o.conversation_id = c.id AND o.direction = 'outbound') AS replied,
               EXISTS (SELECT 1 FROM conversations.decisions d
                        WHERE d.conversation_id = c.id AND d.question = 'suspicious'
                          AND d.value = 'true'::jsonb) AS suspicious,
               EXISTS (SELECT 1 FROM conversations.decisions d
                        WHERE d.conversation_id = c.id AND d.question = 'money'
                          AND d.value = '\"owed\"'::jsonb) AS owed,
               EXISTS (SELECT 1 FROM conversations.needs n
                        WHERE n.conversation_id = c.id AND n.reason IN ('reply', 'do')
                          AND (n.state = 'open' OR n.verdict IN ('done', 'confirmed'))) AS asked
          FROM conversations.conversations c
          CROSS JOIN LATERAL (
              SELECT sender_address, sender, subject, subject_template, sent_at, headers
                FROM conversations.messages
               WHERE conversation_id = c.id AND direction = 'inbound'
               ORDER BY sent_at DESC, id DESC LIMIT 1
          ) m
         WHERE c.inbox_id = $1 AND c.last_message_at > now() - make_interval(days => $2)
           AND ($5::text IS NULL OR m.sender_address = $5)
    ),
    frequent AS (
        SELECT sender_address, count(*) AS n
          FROM latest
         GROUP BY sender_address
        HAVING count(*) >= $3
         ORDER BY count(*) DESC, sender_address
         LIMIT $4
    )
    SELECT l.sender_address, l.name, l.subject, l.subject_template, l.sent_at,
           NOT l.unread AS opened, l.unread AND (l.archived OR l.trashed) AS archived_unread,
           l.replied, l.suspicious, l.owed, l.asked, l.stream, l.headers
      FROM latest l
      JOIN frequent f ON f.sender_address = l.sender_address
     ORDER BY f.n DESC, l.sender_address, l.sent_at DESC";

/// A sender and its mail, with the name it gives.
struct Loaded {
    sender: Sender,
    name: Option<String>,
}

/// The frequent senders of `inbox` and their mail over the window (or only
/// `only`'s, however little), the most frequent first.
async fn senders(
    conn: &mut PgConnection,
    inbox: InboxId,
    settings: &NoticeSettings,
    only: Option<&str>,
) -> Result<Vec<Loaded>> {
    let window = i32::try_from(settings.window_days).unwrap_or(i32::MAX);
    let min = if only.is_some() {
        1
    } else {
        i64::from(settings.min_messages)
    };
    let rows: Vec<MailRow> = sqlx::query_as(SENDER_MAIL)
        .bind(inbox)
        .bind(window)
        .bind(min)
        .bind(SENDERS_PER_PASS)
        .bind(only)
        .fetch_all(&mut *conn)
        .await?;
    let mut out: Vec<Loaded> = Vec::new();
    for row in rows {
        // `''` means "no template"; NULL means not worked out yet.
        let template = match row.subject_template {
            None => subject_template(&row.subject),
            Some(t) if t.is_empty() => None,
            Some(t) => Some(t),
        };
        let mail = Mail {
            subject: row.subject,
            template,
            sent_at: row.sent_at,
            opened: row.opened,
            archived_unread: row.archived_unread,
            replied: row.replied,
            suspicious: row.suspicious,
            owed: row.owed,
            asked: row.asked,
            stream: row.stream.parse().unwrap_or(Stream::Primary),
        };
        if let Some(last) = out
            .last_mut()
            .filter(|l| l.sender.address == row.sender_address)
        {
            last.name = last.name.take().max(row.name);
            last.sender.mail.push(mail);
            continue;
        }
        // The newest conversation's headers say whether a machine sent it.
        let headers: Headers = serde_json::from_value(row.headers).unwrap_or_default();
        out.push(Loaded {
            sender: Sender {
                machine: is_machine_sender(&envelope(&row.sender_address, &headers)),
                correspondent: rules::corresponds(conn, inbox, &row.sender_address).await?,
                address: row.sender_address,
                mail: vec![mail],
            },
            name: row.name,
        });
    }
    Ok(out)
}

/// What rules of the inbox already say about `sender`'s mail: all of it, or
/// the patterns they name.
fn taught(rules: &[Stored], sender: &str) -> (bool, Vec<Pattern>) {
    let mut whole = false;
    let mut patterns = Vec::new();
    for rule in rules {
        let names = rule.conditions.iter().any(
            |c| matches!(c, RuleCondition::SenderAddress(a) if a.eq_ignore_ascii_case(sender)),
        );
        if !names {
            continue;
        }
        let subject = rule.conditions.iter().find_map(|c| match c {
            RuleCondition::SubjectTemplate(t) => Some(Pattern::Template(t.trim().to_lowercase())),
            RuleCondition::SubjectContains(w) => Some(Pattern::Phrase(subject_words(w).join(" "))),
            _ => None,
        });
        match subject {
            Some(pattern) => patterns.push(pattern),
            None => whole = true,
        }
    }
    (whole, patterns)
}

/// The senders of `inbox` the model should group now: mixed, with much of
/// their mail fitting no template, not wholly taught, and with no fresh
/// grouping; the most frequent first, at most `model_senders_per_pass`.
async fn to_group(
    conn: &mut PgConnection,
    inbox: InboxId,
    settings: &NoticeSettings,
) -> Result<Vec<ToGroup>> {
    if inbox_workspace(conn, inbox).await?.is_none() {
        return Ok(Vec::new());
    }
    let loaded = senders(conn, inbox, settings, None).await?;
    let rules = rules::of_inboxes(conn, &[inbox]).await?;
    let fresh: Vec<String> = grouping::cached(conn, inbox, settings)
        .await?
        .into_iter()
        .filter_map(|(sender, _, fresh)| fresh.then_some(sender))
        .collect();
    let max = usize::try_from(settings.model_senders_per_pass).unwrap_or(usize::MAX);
    let now = Utc::now();
    let mut out = Vec::new();
    for Loaded { sender, .. } in loaded {
        if out.len() >= max {
            break;
        }
        if fresh.contains(&sender.address) || taught(&rules, &sender.address).0 {
            continue;
        }
        if !analysis::read(&sender, &[], settings, now).to_group {
            continue;
        }
        let subjects =
            grouping::distinct_subjects(sender.mail.iter().map(|m| m.subject.as_str()), settings);
        out.push(ToGroup {
            sender: sender.address,
            subjects,
        });
    }
    Ok(out)
}

/// Look at `inbox`'s recent mail and keep what Scout noticed: a suggestion
/// per sender, kind and pattern, refreshed, and none left for one that no
/// longer qualifies. Returns how many it kept.
pub async fn scan(
    conn: &mut PgConnection,
    inbox: InboxId,
    settings: &NoticeSettings,
) -> Result<usize> {
    // An inbox kept read-only, or gone, gets nothing.
    let Some(workspace) = inbox_workspace(conn, inbox).await? else {
        return Ok(0);
    };
    let loaded = senders(conn, inbox, settings, None).await?;
    let rules = rules::of_inboxes(conn, &[inbox]).await?;
    let groupings: HashMap<String, Vec<String>> = grouping::cached(conn, inbox, settings)
        .await?
        .into_iter()
        .map(|(sender, phrases, _)| (sender, phrases))
        .collect();
    let max = usize::try_from(settings.max_per_inbox).unwrap_or(usize::MAX);
    let now = Utc::now();
    let mut kept = 0;
    'senders: for Loaded { sender, name } in loaded {
        let (whole, patterns) = taught(&rules, &sender.address);
        if whole {
            continue;
        }
        let phrases = groupings
            .get(&sender.address)
            .map_or(&[][..], Vec::as_slice);
        for noticed in analysis::read(&sender, phrases, settings, now).notices {
            if kept >= max {
                break 'senders;
            }
            if patterns.contains(&noticed.pattern) {
                continue;
            }
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
                &sender.address,
                name.as_deref(),
            )
            .await?;
            kept += 1;
        }
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

/// Whether `sender`'s mail in `inbox` is all of one kind for what the inbox
/// does with it, as the pass would read it now: teach once's default is
/// then the whole sender, else only mail like the one acted on. A sender
/// with little or no mail in the window is.
pub async fn uniform(
    conn: &mut PgConnection,
    inbox: InboxId,
    sender: &str,
    settings: &NoticeSettings,
) -> Result<bool> {
    let address = sender.trim().to_lowercase();
    let Some(Loaded { mut sender, .. }) = senders(conn, inbox, settings, Some(&address))
        .await?
        .into_iter()
        .next()
    else {
        return Ok(true);
    };
    // Teach once asks this only of a machine sender the inbox does not
    // write to.
    sender.machine = true;
    sender.correspondent = false;
    Ok(analysis::uniform(&sender, settings, Utc::now()))
}

/// The words the model proposed for `sender`'s mail in `inbox`, kept from a
/// pass; never a new call.
pub async fn phrases_of(
    conn: &mut PgConnection,
    inbox: InboxId,
    sender: &str,
) -> Result<Vec<String>> {
    Ok(grouping::of_sender(conn, inbox, &sender.trim().to_lowercase()).await?)
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
    sqlx::query(
        "INSERT INTO conversations.rule_notices
             (id, workspace_id, inbox_id, kind, sender_address, sender_name, conditions,
              actions, evidence, pattern, example_subject)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
         ON CONFLICT (inbox_id, kind, sender_address, pattern) DO UPDATE
            SET sender_name = EXCLUDED.sender_name, conditions = EXCLUDED.conditions,
                actions = EXCLUDED.actions, evidence = EXCLUDED.evidence,
                example_subject = EXCLUDED.example_subject, refreshed_at = now()
          WHERE conversations.rule_notices.state = 'open'",
    )
    .bind(RuleNoticeId::new())
    .bind(workspace)
    .bind(inbox)
    .bind(noticed.kind.as_str())
    .bind(sender)
    .bind(name.map(|n| n.chars().take(200).collect::<String>()))
    .bind(serde_json::json!(noticed.conditions))
    .bind(serde_json::json!(noticed.actions))
    .bind(serde_json::json!(noticed.evidence))
    .bind(noticed.pattern.key())
    .bind(noticed.example.as_deref())
    .execute(conn)
    .await?;
    Ok(())
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
    /// Why, in numbers: the counts of the part of the sender's mail it is
    /// about.
    pub evidence: NoticeEvidence,
    /// The part of the sender's mail it is about; none for all of it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<NoticePattern>,
    /// Other members use the inbox: the rule would act for them too.
    pub shared: bool,
}

/// The part of a sender's mail a suggestion is about (campbooks-cloud#356).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct NoticePattern {
    /// What picks it out.
    pub source: NoticePatternSource,
    /// One recent subject of that mail, as plain text.
    pub example: String,
    /// What narrows a search for the sender's mail to it, in the search
    /// grammar (`subject:"…"`); empty when no words of it can.
    pub search: String,
}

/// What picks out the part of a sender's mail a suggestion is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum NoticePatternSource {
    /// The subject's template (the rule's `subject_template`).
    Template,
    /// Words the model proposed for the sender's subjects and the pass
    /// checked against its mail (the rule's `subject_contains`).
    Phrase,
}

/// The search for mail whose subject has `template`: its longest run of
/// words between the parts that change (`#`, `*`), quoted.
fn template_search(template: &str) -> String {
    let best = template
        .split(['#', '*'])
        .map(|run| {
            run.replace('"', " ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|run| run.chars().any(char::is_alphanumeric))
        .fold(String::new(), |best, run| {
            let words = |s: &str| s.split(' ').count();
            if best.is_empty() || words(&run) > words(&best) {
                run
            } else {
                best
            }
        });
    if best.is_empty() {
        String::new()
    } else {
        format!("subject:\"{best}\"")
    }
}

/// `pattern` (as stored) and its example as the API shows them.
fn pattern_of(pattern: &str, example: Option<String>) -> Option<NoticePattern> {
    let (source, search) = match Pattern::of_key(pattern) {
        Pattern::Sender => return None,
        Pattern::Template(template) => (NoticePatternSource::Template, template_search(&template)),
        Pattern::Phrase(words) => (
            NoticePatternSource::Phrase,
            format!("subject:\"{}\"", words.replace('"', " ")),
        ),
    };
    Some(NoticePattern {
        source,
        example: example.unwrap_or_default(),
        search,
    })
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
    pattern: String,
    example_subject: Option<String>,
}

/// The open suggestions of the inboxes `$1` that member `$2` did not hide.
const LIST: &str = "
    SELECT n.id, n.inbox_id, n.kind, n.sender_address, n.sender_name, n.conditions, n.actions,
           n.evidence, n.pattern, n.example_subject
      FROM conversations.rule_notices n
     WHERE n.inbox_id = ANY($1) AND n.state = 'open'
       AND NOT EXISTS (
           SELECT 1 FROM conversations.rule_notice_answers a
            WHERE a.inbox_id = n.inbox_id AND a.user_id = $2 AND a.kind = n.kind
              AND a.sender_address = n.sender_address AND a.pattern = n.pattern
              AND (a.answer = 'never' OR a.until > now()))
     ORDER BY (n.evidence ->> 'received')::int DESC, n.id";

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
    let rows: Vec<NoticeRow> = sqlx::query_as(LIST)
        .bind(&teachable)
        .bind(access.user.map(|u| *u.as_uuid()))
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
        pattern: pattern_of(&row.pattern, row.example_subject),
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
    let Target { inbox, .. } = target(&mut tx, path.notice_id).await?;
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
                n.actions, n.evidence, n.pattern, n.example_subject \
           FROM conversations.rule_notices n \
          WHERE n.id = $1 AND n.state = 'open' \
            AND NOT EXISTS ( \
                SELECT 1 FROM conversations.rule_notice_answers a \
                 WHERE a.inbox_id = n.inbox_id AND a.user_id = $2 AND a.kind = n.kind \
                   AND a.sender_address = n.sender_address AND a.pattern = n.pattern \
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

/// What an answer to a suggestion is about.
#[derive(sqlx::FromRow)]
struct Target {
    inbox: InboxId,
    kind: String,
    sender: String,
    pattern: String,
}

/// A suggestion's inbox, kind, sender and pattern; 404 when it is gone or
/// in another workspace (row-level security).
async fn target(conn: &mut PgConnection, id: RuleNoticeId) -> Result<Target> {
    sqlx::query_as(
        "SELECT inbox_id AS inbox, kind, sender_address AS sender, pattern
           FROM conversations.rule_notices WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(conn)
    .await?
    .ok_or(Error::NotFound)
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
    let Target {
        inbox,
        kind,
        sender,
        pattern,
    } = target(&mut tx, path.notice_id).await?;
    // 404 unless the caller sees its inbox, 403 when they only read it; an
    // app gets 403 too: answers are a member's own.
    may_teach(&state, &mut tx, access, inbox).await?;
    member_only(&workspace.principal)?;
    let user = access.user.ok_or(Error::NotFound)?;
    let days = match body.answer {
        NoticeAnswer::NotNow => Some(i32::try_from(settings.not_now_days).unwrap_or(i32::MAX)),
        NoticeAnswer::Never => None,
    };
    sqlx::query(
        "INSERT INTO conversations.rule_notice_answers
             (workspace_id, inbox_id, user_id, kind, sender_address, pattern, answer, until)
         VALUES ($1, $2, $3, $4, $5, $6, $7, now() + make_interval(days => $8))
         ON CONFLICT (inbox_id, user_id, kind, sender_address, pattern) DO UPDATE
            SET answer = EXCLUDED.answer, until = EXCLUDED.until, answered_at = now()",
    )
    .bind(access.workspace.as_uuid())
    .bind(inbox)
    .bind(user.as_uuid())
    .bind(&kind)
    .bind(&sender)
    .bind(&pattern)
    .bind(body.answer.as_str())
    .bind(days)
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
    let Target {
        inbox,
        kind,
        sender,
        pattern,
    } = target(&mut tx, path.notice_id).await?;
    // 404 unless the caller sees its inbox, 403 when they only read it; an
    // app gets 403 too: answers are a member's own.
    may_teach(&state, &mut tx, access, inbox).await?;
    member_only(&workspace.principal)?;
    let user = access.user.ok_or(Error::NotFound)?;
    let cleared = sqlx::query(
        "DELETE FROM conversations.rule_notice_answers
          WHERE inbox_id = $1 AND user_id = $2 AND kind = $3 AND sender_address = $4
            AND pattern = $5",
    )
    .bind(inbox)
    .bind(user.as_uuid())
    .bind(&kind)
    .bind(&sender)
    .bind(&pattern)
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
    sqlx::query("DELETE FROM conversations.rule_notice_groupings WHERE inbox_id = $1")
        .bind(inbox)
        .execute(&mut *conn)
        .await?;
    Ok(())
}
