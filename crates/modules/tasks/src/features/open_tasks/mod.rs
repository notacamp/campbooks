//! The kernel's `OpenTasks` port: the open tasks about a conversation and
//! its sender, for the single email read (campbooks-cloud#43).

use std::sync::Arc;

use campbooks_kernel::{
    RecordRef,
    tasks::{MOST_OPEN_TASKS, OpenTask, OpenTasks, OpenTasksFuture, OpenTasksPort},
    tenancy::WorkspaceId,
};
use chrono::NaiveDate;
use sqlx::PgConnection;

use crate::domain::{TaskId, resources::TASK_RECORD};

/// The port, for the server to register.
pub fn open_tasks() -> OpenTasksPort {
    Arc::new(OpenTaskReader)
}

/// Reads open tasks from this module's tables.
#[derive(Debug, Clone, Copy, Default)]
pub struct OpenTaskReader;

impl OpenTasks for OpenTaskReader {
    fn open_tasks<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        workspace: WorkspaceId,
        inbox: &'a str,
        conversation: &'a str,
        sender_address: &'a str,
    ) -> OpenTasksFuture<'a> {
        Box::pin(async move {
            let sender = sender_address.trim().to_lowercase();
            let rows: Vec<(TaskId, String, Option<NaiveDate>)> = sqlx::query_as(
                "SELECT id, title, due_on FROM tasks.tasks
                  WHERE workspace_id = $1 AND state = 'open'
                    AND ((source_type = 'conversations.conversation' AND source_id = $3)
                         OR ($4 <> '' AND source_container = $2 AND source_sender = $4))
                  ORDER BY created_at DESC, id DESC
                  LIMIT $5",
            )
            .bind(workspace.as_uuid())
            .bind(inbox)
            .bind(conversation)
            .bind(&sender)
            .bind(i64::try_from(MOST_OPEN_TASKS).unwrap_or(10))
            .fetch_all(conn)
            .await?;
            Ok(rows
                .into_iter()
                .map(|(id, title, due_on)| OpenTask {
                    record: RecordRef::new(TASK_RECORD, &id),
                    title,
                    due_on,
                })
                .collect())
        })
    }
}

#[cfg(test)]
mod tests;
