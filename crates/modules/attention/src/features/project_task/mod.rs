//! `attention.project_task` — keep one item per task from
//! `tasks.task_changed` (campbooks-cloud#43).
//!
//! Like the conversation projection it is idempotent and order-proof: an
//! event applies only when its revision is newer than the item's, and a clear
//! that arrives before its open leaves a tombstone holding its revision. The
//! item belongs to the assignee's container (`tasks.assignee:<member>`, seen
//! by that member only), so a hand-off moves it to the new assignee. A task
//! planned Later keeps its item hidden until `later_until`; the list leaves
//! it out until then and shows it again by itself. Its plan and planned day
//! are kept too: Home shows a task only when it is for today, or when it
//! asks "You replied. Is this done?" (`asks_done`, campbooks-cloud#396).

use campbooks_kernel::{
    Event, EventHandler, JobContext, JobError, RecordRef, tenancy::WorkspaceId,
};
use sqlx::{Postgres, Transaction};

use crate::{
    domain::ItemId,
    events::{ItemChanged, TaskChanged, TaskState},
};

/// Record type of a task.
const TASK: &str = "tasks.task";

/// Container type of a member's tasks.
const ASSIGNEE: &str = "tasks.assignee";

/// Projects tasks into items.
pub struct ProjectTask;

impl EventHandler for ProjectTask {
    type Event = TaskChanged;
    const NAME: &'static str = "attention.project_task";
    const QUEUE: &'static str = "default";

    async fn handle(ctx: &mut JobContext, event: Event<TaskChanged>) -> Result<(), JobError> {
        let since = event.occurred_at;
        let task = event.payload;
        let workspace: WorkspaceId = task
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("task_changed names an invalid workspace"))?;
        let events = ctx.state().events.clone();
        let tx = ctx.tx().await?;
        let current: Option<(ItemId, i64, String)> = sqlx::query_as(
            "SELECT id, producer_revision, container_id FROM attention.items
              WHERE workspace_id = $1 AND record_type = $2 AND record_id = $3 FOR UPDATE",
        )
        .bind(workspace.as_uuid())
        .bind(TASK)
        .bind(&task.task)
        .fetch_optional(&mut **tx)
        .await?;
        if current
            .as_ref()
            .is_some_and(|(_, revision, _)| *revision >= task.revision)
        {
            // Stale or replayed.
            return Ok(());
        }
        let id = current.as_ref().map_or_else(ItemId::new, |(id, _, _)| *id);
        let written = if task.state == TaskState::Open {
            open(tx, workspace, id, &task, since).await?
        } else {
            clear(tx, workspace, id, &task, since).await?
        };
        // Nobody saw a tombstone: announce a clear only over an item that was
        // there.
        let Some(item) = written.filter(|_| task.state == TaskState::Open || current.is_some())
        else {
            return Ok(());
        };
        // The assignee's Home refreshes; so does the one it moved from.
        let mut containers = vec![task.assignee.to_string()];
        if let Some((_, _, before)) = &current
            && *before != containers[0]
        {
            containers.push(before.clone());
        }
        for container in containers {
            events
                .publish(
                    tx,
                    &ItemChanged {
                        workspace: task.workspace.clone(),
                        item,
                        container: RecordRef::new(ASSIGNEE, &container),
                    },
                )
                .await?;
        }
        Ok(())
    }
}

/// The record that raised the item: the task's source, else the task.
fn cause(task: &TaskChanged) -> (String, String) {
    task.source.as_ref().map_or_else(
        || (TASK.to_owned(), task.task.clone()),
        |source| (source.record_type.clone(), source.id.clone()),
    )
}

/// Open (or update) item `id` for an open task, keeping its plan and planned
/// day (Home shows a task only on its day, campbooks-cloud#358): the item
/// written, if any (an item a newer revision reached first, in a race, is
/// left as it is).
async fn open(
    tx: &mut Transaction<'static, Postgres>,
    workspace: WorkspaceId,
    id: ItemId,
    task: &TaskChanged,
    since: chrono::DateTime<chrono::Utc>,
) -> Result<Option<ItemId>, JobError> {
    let (cause_type, cause_id) = cause(task);
    let hidden_until = if task.plan == "later" {
        task.later_until
    } else {
        None
    };
    let reason = match task.verb {
        crate::events::TaskVerb::Do => "do",
        crate::events::TaskVerb::Pay => "pay",
    };
    let written: Option<(ItemId,)> = sqlx::query_as(
        r"
        INSERT INTO attention.items
            (id, workspace_id, record_type, record_id, container_type, container_id,
             cause_type, cause_id, reason, trust, due_on, urgent, since, state,
             producer_revision, hidden_until, plan, plan_on, asks_done)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'sure', $10, false, $11, 'open', $12, $13,
                $14, $15, $16)
        ON CONFLICT (workspace_id, record_type, record_id) DO UPDATE
           SET container_type = EXCLUDED.container_type,
               container_id = EXCLUDED.container_id,
               cause_type = EXCLUDED.cause_type, cause_id = EXCLUDED.cause_id,
               reason = EXCLUDED.reason, trust = EXCLUDED.trust, due_on = EXCLUDED.due_on,
               hidden_until = EXCLUDED.hidden_until, plan = EXCLUDED.plan,
               plan_on = EXCLUDED.plan_on, asks_done = EXCLUDED.asks_done, state = 'open',
               cleared_because = NULL, cleared_by = NULL, cleared_at = NULL,
               producer_revision = EXCLUDED.producer_revision, updated_at = now()
         WHERE attention.items.producer_revision < EXCLUDED.producer_revision
        RETURNING id
        ",
    )
    .bind(id)
    .bind(workspace.as_uuid())
    .bind(TASK)
    .bind(&task.task)
    .bind(ASSIGNEE)
    .bind(task.assignee.to_string())
    .bind(cause_type)
    .bind(cause_id)
    .bind(reason)
    .bind(task.due_on)
    .bind(since)
    .bind(task.revision)
    .bind(hidden_until)
    .bind(&task.plan)
    .bind(task.plan_on)
    .bind(task.asks_done)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(written.map(|(id,)| id))
}

/// Clear item `id`; without one yet, leave a tombstone at this revision.
async fn clear(
    tx: &mut Transaction<'static, Postgres>,
    workspace: WorkspaceId,
    id: ItemId,
    task: &TaskChanged,
    since: chrono::DateTime<chrono::Utc>,
) -> Result<Option<ItemId>, JobError> {
    let (cause_type, cause_id) = cause(task);
    let because = if task.state == TaskState::Done {
        "done"
    } else {
        "not_needed"
    };
    let written: Option<(ItemId,)> = sqlx::query_as(
        r"
        INSERT INTO attention.items
            (id, workspace_id, record_type, record_id, container_type, container_id,
             cause_type, cause_id, since, state, cleared_because, cleared_at, producer_revision)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'cleared', $10, now(), $11)
        ON CONFLICT (workspace_id, record_type, record_id) DO UPDATE
           SET container_type = EXCLUDED.container_type,
               container_id = EXCLUDED.container_id,
               state = 'cleared', cleared_because = EXCLUDED.cleared_because,
               cleared_at = now(), hidden_until = NULL, asks_done = false,
               producer_revision = EXCLUDED.producer_revision, updated_at = now()
         WHERE attention.items.producer_revision < EXCLUDED.producer_revision
        RETURNING id
        ",
    )
    .bind(id)
    .bind(workspace.as_uuid())
    .bind(TASK)
    .bind(&task.task)
    .bind(ASSIGNEE)
    .bind(task.assignee.to_string())
    .bind(cause_type)
    .bind(cause_id)
    .bind(since)
    .bind(because)
    .bind(task.revision)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(written.map(|(id,)| id))
}

#[cfg(test)]
mod tests;
