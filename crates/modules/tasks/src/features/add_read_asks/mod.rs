//! `tasks.add_read_asks` ← `conversations.message_read`: the obligations
//! Scout read in an email (`do` and `pay` asks) become open tasks of the
//! inbox's personal owner.
//!
//! - Keyed by the ask's key: a repeated delivery changes nothing, and a task
//!   whose ask is known is never reopened or changed by a read.
//! - Nothing is made for a shared inbox (a task needs one member).
//! - An ask that is an open task of the owner (`same_as`, or a `pay` ask of
//!   the model read whose message the structured read already made a `pay`
//!   task for) makes nothing; it only gives that task a due date it did not
//!   have.
//! - No model is called: the title and amount come from the `read-texts`
//!   blob the event points at, so the event keeps no text.

use campbooks_kernel::{
    AppState, Event, EventHandler, JobContext, JobError,
    tenancy::{Access, UserId, WorkspaceId},
    text,
};
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{Postgres, Transaction};

use crate::{
    domain::{Outcome, TaskActor, TaskChange, TaskHistoryDetail, TaskId, TaskVerb, finish},
    inbound::{AskAmount, AskText, MessageRead, ReadAsk, ReadTexts},
};

/// Largest `read-texts` document read.
const MAX_TEXTS_BYTES: usize = 1024 * 1024;

/// Longest title a task has.
const MAX_TITLE: usize = 200;

/// The title of a task whose ask came without words.
const fn fallback_title(verb: TaskVerb) -> &'static str {
    match verb {
        TaskVerb::Do => "Do what this email asks",
        TaskVerb::Pay => "Pay what this email asks",
    }
}

/// Makes tasks of what Scout read.
pub struct AddReadAsks;

impl EventHandler for AddReadAsks {
    type Event = MessageRead;
    const NAME: &'static str = "tasks.add_read_asks";
    const QUEUE: &'static str = "default";

    async fn handle(ctx: &mut JobContext, event: Event<MessageRead>) -> Result<(), JobError> {
        let read = event.payload;
        // A shared inbox makes none; neither does a read with no ask.
        let Some(owner) = read.owner else {
            return Ok(());
        };
        if read.asks.is_empty() {
            return Ok(());
        }
        let workspace: WorkspaceId = read
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("message_read names an invalid workspace"))?;
        let state = ctx.state().clone();
        let texts: ReadTexts = match &read.texts {
            Some(blob) => read_json(&state, &read.workspace, blob).await?,
            None => ReadTexts::default(),
        };
        let tx = ctx.tx().await?;
        let access = Access {
            workspace,
            user: Some(owner),
            role: None,
        };
        for ask in &read.asks {
            let words = texts.asks.iter().find(|text| text.key == ask.key);
            add_ask(&state, tx, &access, owner, &read, ask, words).await?;
        }
        Ok(())
    }
}

/// The JSON document `blob` holds in `workspace`, within the limit.
async fn read_json<T: serde::de::DeserializeOwned>(
    state: &AppState,
    workspace: &str,
    blob: &campbooks_kernel::blobs::BlobRef,
) -> Result<T, JobError> {
    use futures_util::StreamExt as _;
    let mut stream = state.blobs.get(workspace, blob).await?;
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if bytes.len() + chunk.len() > MAX_TEXTS_BYTES {
            return Err(JobError::fatal("the document is larger than the limit"));
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| JobError::fatal("the document does not parse"))
}

/// A task that an ask may repeat.
#[derive(sqlx::FromRow)]
struct Known {
    id: TaskId,
    state: String,
    due_on: Option<NaiveDate>,
}

/// A due day, and a time when the message gave one.
fn parse_due(raw: Option<&str>) -> Option<(NaiveDate, Option<DateTime<Utc>>)> {
    let raw = raw?.trim();
    if let Ok(day) = NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
        return Some((day, None));
    }
    // The day is the one the sender wrote, in their offset.
    let at = DateTime::parse_from_rfc3339(raw).ok()?;
    Some((at.date_naive(), Some(at.with_timezone(&Utc))))
}

/// A title as the task keeps it: one line, displayable, at most 200
/// characters; the verb's own words when the ask has none.
fn title_of(words: Option<&AskText>, verb: TaskVerb) -> String {
    let raw = words.map_or("", |words| words.title.as_str());
    let one_line = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let shown = text::displayable(&one_line);
    let title: String = shown.trim().chars().take(MAX_TITLE).collect();
    if title.is_empty() {
        fallback_title(verb).to_owned()
    } else {
        title
    }
}

/// The amount, when it is a sum of money.
fn amount_of(words: Option<&AskText>) -> Option<&AskAmount> {
    words
        .and_then(|words| words.amount.as_ref())
        .filter(|amount| {
            amount.minor > 0
                && amount.currency.len() == 3
                && amount.currency.chars().all(|c| c.is_ascii_uppercase())
        })
}

/// The task of the owner that `ask` repeats: the open one `same_as` names
/// (from the same conversation or inbox), or, for a `pay` ask, the `pay` task the
/// other layer's read of the same message made.
async fn repeated(
    tx: &mut Transaction<'static, Postgres>,
    workspace: WorkspaceId,
    owner: UserId,
    read: &MessageRead,
    ask: &ReadAsk,
    verb: TaskVerb,
) -> Result<Option<Known>, JobError> {
    if let Some(reference) = ask
        .same_as
        .as_deref()
        .and_then(|r| r.parse::<TaskId>().ok())
    {
        let same: Option<Known> = sqlx::query_as(
            "SELECT id, state, due_on FROM tasks.tasks
              WHERE workspace_id = $1 AND id = $2 AND assignee = $3 AND state = 'open'
                AND ((source_type = 'conversations.conversation' AND source_id = $4)
                     OR source_container = $5)",
        )
        .bind(workspace.as_uuid())
        .bind(reference)
        .bind(owner.as_uuid())
        .bind(&read.conversation)
        .bind(&read.inbox)
        .fetch_optional(&mut **tx)
        .await?;
        if same.is_some() {
            return Ok(same);
        }
    }
    // The other layer's read of this message made the `pay` task already.
    if verb == TaskVerb::Pay {
        let other = if read.layer == "structured" {
            "model"
        } else {
            "structured"
        };
        let made: Option<Known> = sqlx::query_as(
            "SELECT id, state, due_on FROM tasks.tasks
              WHERE workspace_id = $1 AND assignee = $2 AND verb = 'pay' AND origin = 'ask'
                AND starts_with(ask_key, $3)
              ORDER BY created_at LIMIT 1",
        )
        .bind(workspace.as_uuid())
        .bind(owner.as_uuid())
        .bind(format!("{}:{other}:ask:", read.message))
        .fetch_optional(&mut **tx)
        .await?;
        return Ok(made);
    }
    Ok(None)
}

async fn add_ask(
    state: &AppState,
    tx: &mut Transaction<'static, Postgres>,
    access: &Access,
    owner: UserId,
    read: &MessageRead,
    ask: &ReadAsk,
    words: Option<&AskText>,
) -> Result<(), JobError> {
    let verb = match ask.verb.as_str() {
        "do" => TaskVerb::Do,
        "pay" => TaskVerb::Pay,
        _ => return Ok(()),
    };
    let workspace = access.workspace.as_uuid();
    // A delivery repeated, or an ask already applied.
    let known: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM tasks.tasks WHERE workspace_id = $1 AND ask_key = $2)",
    )
    .bind(workspace)
    .bind(&ask.key)
    .fetch_one(&mut **tx)
    .await?;
    if known {
        return Ok(());
    }
    let due = parse_due(ask.due.as_deref());

    if let Some(task) = repeated(tx, access.workspace, owner, read, ask, verb).await? {
        return fill_due(state, tx, access, &task, due).await;
    }

    let amount = amount_of(words);
    let id = TaskId::new();
    let inserted = sqlx::query(
        "INSERT INTO tasks.tasks
             (id, workspace_id, title, verb, origin, due_on, due_at, assignee, created_by,
              source_type, source_id, source_container, amount_minor, currency, ask_key)
         VALUES ($1, $2, $3, $4, 'ask', $5, $6, $7, $7, 'conversations.conversation', $8, $9,
                 $10, $11, $12)
         ON CONFLICT (workspace_id, ask_key) WHERE ask_key IS NOT NULL DO NOTHING",
    )
    .bind(id)
    .bind(workspace)
    .bind(title_of(words, verb))
    .bind(verb.as_str())
    .bind(due.map(|(day, _)| day))
    .bind(due.and_then(|(_, at)| at))
    .bind(owner.as_uuid())
    .bind(&read.conversation)
    .bind(&read.inbox)
    .bind(amount.map(|amount| amount.minor))
    .bind(amount.map(|amount| amount.currency.clone()))
    .bind(&ask.key)
    .execute(&mut **tx)
    .await?
    .rows_affected();
    if inserted == 0 {
        return Ok(());
    }
    finish(
        state,
        tx,
        access,
        id,
        &Outcome {
            actor: TaskActor::Scout,
            change: TaskChange::Created,
            detail: TaskHistoryDetail {
                due_on: due.map(|(day, _)| day),
                ..TaskHistoryDetail::default()
            },
            previous_assignee: None,
            previous_handed_by: None,
        },
        Utc::now().date_naive(),
    )
    .await?;
    Ok(())
}

/// Give `task` the due date the ask names, when it is open and has none:
/// Scout never overrides the member, nor reopens a closed task.
async fn fill_due(
    state: &AppState,
    tx: &mut Transaction<'static, Postgres>,
    access: &Access,
    task: &Known,
    due: Option<(NaiveDate, Option<DateTime<Utc>>)>,
) -> Result<(), JobError> {
    let Some((day, at)) = due else {
        return Ok(());
    };
    if task.state != "open" || task.due_on.is_some() {
        return Ok(());
    }
    let changed = sqlx::query(
        "UPDATE tasks.tasks SET due_on = $2, due_at = $3, revision = revision + 1,
                updated_at = now()
          WHERE id = $1 AND state = 'open' AND due_on IS NULL",
    )
    .bind(task.id)
    .bind(day)
    .bind(at)
    .execute(&mut **tx)
    .await?
    .rows_affected();
    if changed == 1 {
        finish(
            state,
            tx,
            access,
            task.id,
            &Outcome {
                actor: TaskActor::Scout,
                change: TaskChange::DueChanged,
                detail: TaskHistoryDetail {
                    due_on: Some(day),
                    ..TaskHistoryDetail::default()
                },
                previous_assignee: None,
                previous_handed_by: None,
            },
            Utc::now().date_naive(),
        )
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
