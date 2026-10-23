//! The open-tasks port: what a member already has to do about a
//! conversation or its sender, for the single email read to recognise a task
//! that exists instead of creating a second one (campbooks-cloud#43).
//!
//! The `tasks` module owns the answer; until the server registers it there
//! are none. Titles are for the read's prompt only: they never go into an
//! event.

use std::{future::Future, pin::Pin, sync::Arc};

use chrono::NaiveDate;
use sqlx::PgConnection;

use crate::{Error, records::RecordRef, tenancy::WorkspaceId};

/// The most tasks the port returns.
pub const MOST_OPEN_TASKS: usize = 10;

/// An open task, as the email read sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenTask {
    /// The task (`tasks.task`).
    pub record: RecordRef,
    /// Its title.
    pub title: String,
    /// When it is due.
    pub due_on: Option<NaiveDate>,
}

/// A future returned by an [`OpenTasks`] port.
pub type OpenTasksFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Vec<OpenTask>, Error>> + Send + 'a>>;

/// The port that answers which tasks are open about a message.
pub trait OpenTasks: std::fmt::Debug + Send + Sync + 'static {
    /// At most [`MOST_OPEN_TASKS`] open tasks, newest first, whose source is
    /// `conversation` or whose source sender is `sender_address` within
    /// `inbox`. Read in `conn`, a transaction working in `workspace`.
    fn open_tasks<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        workspace: WorkspaceId,
        inbox: &'a str,
        conversation: &'a str,
        sender_address: &'a str,
    ) -> OpenTasksFuture<'a>;
}

/// The port before a module provides one: no task is open.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoOpenTasks;

impl OpenTasks for NoOpenTasks {
    fn open_tasks<'a>(
        &'a self,
        _conn: &'a mut PgConnection,
        _workspace: WorkspaceId,
        _inbox: &'a str,
        _conversation: &'a str,
        _sender_address: &'a str,
    ) -> OpenTasksFuture<'a> {
        Box::pin(async { Ok(Vec::new()) })
    }
}

/// Shared handle to the registered [`OpenTasks`] port.
pub type OpenTasksPort = Arc<dyn OpenTasks>;
