//! The open tasks of a conversation and its sender, for another module to
//! read as context.
//!
//! The one `complete` read of an email (the `conversations` module) shows
//! the model the tasks already open for that mail, so an ask it finds can say
//! it is the same obligation as one of them instead of being created twice.
//! The module that owns tasks implements [`OpenTasks`] and the server
//! registers it with [`AppState::with_open_tasks`](crate::AppState::with_open_tasks).
//! Until then the list is empty.
//!
//! Titles reach the model in the caller's process only: they are never put in
//! an event, a job or a log.

use std::{future::Future, pin::Pin, sync::Arc};

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::{Error, tenancy::WorkspaceId};

/// The most tasks [`OpenTasks::open_tasks`] returns.
pub const MAX_OPEN_TASKS: usize = 10;

/// An open task, as much of it as a reader of the email needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenTask {
    /// The task's reference: an opaque string the owner of tasks understands,
    /// given back when an ask says it repeats this task.
    pub reference: String,
    /// What the task says to do.
    pub title: String,
    /// The day it is due, when it has one.
    pub due_on: Option<NaiveDate>,
    /// The time it is due, when it has one.
    pub due_at: Option<DateTime<Utc>>,
}

/// A future returned by [`OpenTasks::open_tasks`].
pub type OpenTasksFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Vec<OpenTask>, Error>> + Send + 'a>>;

/// Answers which tasks are open for an email.
pub trait OpenTasks: std::fmt::Debug + Send + Sync + 'static {
    /// The open tasks whose source is `conversation` or mail from
    /// `sender_address` within `inbox`, whoever holds them, most recent
    /// first, at most [`MAX_OPEN_TASKS`]. Read in `conn`, a transaction
    /// working in `workspace`.
    fn open_tasks<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        workspace: WorkspaceId,
        inbox: Uuid,
        conversation: Uuid,
        sender_address: &'a str,
    ) -> OpenTasksFuture<'a>;
}

/// The port before a module provides one: no task is ever open.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoOpenTasks;

impl OpenTasks for NoOpenTasks {
    fn open_tasks<'a>(
        &'a self,
        _conn: &'a mut PgConnection,
        _workspace: WorkspaceId,
        _inbox: Uuid,
        _conversation: Uuid,
        _sender_address: &'a str,
    ) -> OpenTasksFuture<'a> {
        Box::pin(async { Ok(Vec::new()) })
    }
}

/// Shared handle to the registered [`OpenTasks`] port.
pub type OpenTasksPort = Arc<dyn OpenTasks>;

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use super::*;

    #[sqlx::test(migrations = "../../migrations")]
    async fn without_a_module_nothing_is_open(db: PgPool) {
        let port: OpenTasksPort = Arc::new(NoOpenTasks);
        let mut conn = db.acquire().await.unwrap();
        let open = port
            .open_tasks(
                &mut conn,
                WorkspaceId::new(),
                Uuid::nil(),
                Uuid::nil(),
                "ana@example.com",
            )
            .await
            .unwrap();
        assert!(open.is_empty());
    }
}
