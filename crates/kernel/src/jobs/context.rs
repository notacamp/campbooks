//! What a running job or event handler can reach.

use sqlx::{PgPool, Postgres, Transaction};

use super::{EnqueueOptions, Enqueued, Job, JobError, JobId};
use crate::{
    AppState,
    events::{DomainEvent, EventId},
};

/// The Postgres setting a job's transaction carries its workspace in (ADR
/// 0011): `current_setting('campbooks.workspace_id', true)`.
pub const WORKSPACE_SETTING: &str = "campbooks.workspace_id";

/// A claimed job, as the worker hands it over.
#[derive(Debug, Clone)]
pub(crate) struct JobInfo {
    pub id: JobId,
    pub kind: String,
    pub queue: String,
    pub attempt: u32,
    pub max_attempts: u32,
    pub workspace: Option<String>,
    pub event_id: Option<EventId>,
}

/// The context a [`Job`] or [`EventHandler`](crate::events::EventHandler)
/// runs in.
///
/// [`tx`](Self::tx) opens the job's transaction on first use, with the job's
/// workspace set as `campbooks.workspace_id`. It commits together with the
/// job being marked succeeded, so a handler's database writes and the job's
/// completion are atomic: if the handler fails, or its worker lost the job's
/// lease in the meantime, they roll back. Long calls to other systems belong
/// before the first `tx()` (or in a job of their own), not inside it.
pub struct JobContext {
    state: AppState,
    info: JobInfo,
    tx: Option<Transaction<'static, Postgres>>,
}

impl std::fmt::Debug for JobContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobContext")
            .field("job", &self.info)
            .field("in_transaction", &self.tx.is_some())
            .finish_non_exhaustive()
    }
}

impl JobContext {
    pub(crate) fn new(state: AppState, info: JobInfo) -> Self {
        Self {
            state,
            info,
            tx: None,
        }
    }

    /// The job's id.
    pub fn id(&self) -> JobId {
        self.info.id
    }

    /// The job's kind (`Job::NAME` or `EventHandler::NAME`).
    pub fn kind(&self) -> &str {
        &self.info.kind
    }

    /// This attempt's number, from 1.
    pub fn attempt(&self) -> u32 {
        self.info.attempt
    }

    /// Whether this is the last attempt before the job is marked failed.
    pub fn is_last_attempt(&self) -> bool {
        self.info.attempt >= self.info.max_attempts
    }

    /// The workspace the job works for.
    pub fn workspace(&self) -> Option<&str> {
        self.info.workspace.as_deref()
    }

    /// Application state: configuration and the event bus. Prefer
    /// [`tx`](Self::tx) over `state().db` for writes.
    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// The connection pool, for work outside the job's transaction (it does
    /// not carry the workspace setting).
    pub fn pool(&self) -> &PgPool {
        &self.state.db
    }

    /// The job's transaction, opened on first use with the workspace set.
    pub async fn tx(&mut self) -> Result<&mut Transaction<'static, Postgres>, JobError> {
        if let Some(tx) = self.tx.take() {
            return Ok(self.tx.insert(tx));
        }
        let mut tx = self.state.db.begin().await?;
        if let Some(workspace) = &self.info.workspace {
            sqlx::query_scalar!(
                "SELECT set_config($1, $2, true)",
                WORKSPACE_SETTING,
                workspace
            )
            .fetch_one(&mut *tx)
            .await?;
        }
        Ok(self.tx.insert(tx))
    }

    /// Publish an event in the job's transaction.
    pub async fn publish<E: DomainEvent>(&mut self, event: &E) -> Result<EventId, JobError> {
        let events = self.state.events.clone();
        let tx = self.tx().await?;
        Ok(events.publish(tx, event).await?)
    }

    /// Enqueue a job in the job's transaction. Unless `options` says
    /// otherwise, it belongs to this job's workspace.
    pub async fn enqueue<J: Job>(
        &mut self,
        job: &J,
        mut options: EnqueueOptions,
    ) -> Result<Enqueued, JobError> {
        if options.workspace.is_none() {
            options.workspace.clone_from(&self.info.workspace);
        }
        let tx = self.tx().await?;
        Ok(super::enqueue(tx, job, options).await?)
    }

    /// Hand the open transaction (if any) to the worker for completion.
    pub(crate) fn take_tx(&mut self) -> Option<Transaction<'static, Postgres>> {
        self.tx.take()
    }

    pub(crate) fn info(&self) -> &JobInfo {
        &self.info
    }
}
