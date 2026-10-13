//! The Postgres job queue (ADR 0012).
//!
//! Background work — event deliveries, provider sync, the AI pass,
//! maintenance — is a row in `kernel.jobs`, enqueued in the caller's
//! transaction and claimed by workers with `FOR UPDATE SKIP LOCKED`.
//!
//! - **Declare** a job as a serializable type implementing [`Job`], and
//!   register it in the module's [`Background`] (`.job::<J>()`), or schedule
//!   it (`.schedule::<J>(every)`).
//! - **Enqueue** it with [`enqueue`] inside a transaction, so it exists if and
//!   only if the caller's change commits. [`EnqueueOptions`] sets a delay,
//!   priority, a `unique_key` (at most one unfinished job per key) and an
//!   `ordering_key` (jobs sharing it run one at a time, in enqueue order).
//! - **Run**: a [`Worker`] claims jobs fairly across workspaces, runs each in
//!   a [`JobContext`] whose transaction carries the job's workspace
//!   (`campbooks.workspace_id`), retries [`JobError::Retry`] with
//!   exponential backoff and jitter, and marks [`JobError::Fatal`] or
//!   exhausted jobs failed (dead, kept for inspection).

mod context;
mod maintenance;
mod registry;
mod worker;

#[cfg(test)]
mod tests;

use std::{future::Future, time::Duration};

use chrono::{DateTime, Utc};
use serde::{Serialize, de::DeserializeOwned};
use sqlx::{PgConnection, Postgres, Transaction};

pub use context::JobContext;
pub use maintenance::PruneJobs;
pub(crate) use registry::is_dotted;
pub use registry::{Background, ConsumerInfo, Registry, RegistryError};
pub use worker::{Worker, WorkerStats};

crate::define_id!(
    /// A job in `kernel.jobs`.
    JobId
);

/// Attempts a job gets unless its type says otherwise.
pub const DEFAULT_MAX_ATTEMPTS: u32 = 10;

/// The `LISTEN/NOTIFY` channel enqueues signal on; the payload is the queue.
pub const NOTIFY_CHANNEL: &str = "kernel_jobs";

/// Longest `last_error` kept, in characters.
pub const MAX_ERROR_LEN: usize = 2_000;

/// A unit of background work.
///
/// The value is the job's payload, stored as JSON: keep it small and made of
/// ids, never secrets or message bodies. `NAME` is its stable wire name;
/// renaming it orphans enqueued jobs.
///
/// ```ignore
/// #[derive(Serialize, Deserialize)]
/// struct SyncMailbox { mailbox: MailboxId }
/// impl Job for SyncMailbox {
///     const NAME: &'static str = "mail.sync_mailbox";
///     const QUEUE: &'static str = "sync";
///     async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> { … }
/// }
/// ```
pub trait Job: Serialize + DeserializeOwned + Send + Sync + 'static {
    /// Stable wire name, `<module>.<job>`.
    const NAME: &'static str;
    /// The queue it runs on.
    const QUEUE: &'static str;
    /// Attempts before the job is marked failed (dead).
    const MAX_ATTEMPTS: u32 = DEFAULT_MAX_ATTEMPTS;

    /// Do the work.
    fn run(self, ctx: &mut JobContext) -> impl Future<Output = Result<(), JobError>> + Send;
}

/// Why a job did not succeed. The message is stored in `last_error`
/// (truncated) and logged: never put secrets or user content in it.
#[derive(Debug, thiserror::Error)]
pub enum JobError {
    /// Try again later: after `after` if given, else with exponential
    /// backoff. Fails for good once the attempts run out.
    #[error("{message}")]
    Retry {
        /// What went wrong.
        message: String,
        /// Earliest time to try again (e.g. a provider's `Retry-After`).
        after: Option<Duration>,
    },
    /// Retrying cannot help (bad payload, the record is gone): fail now.
    #[error("{0}")]
    Fatal(String),
}

impl JobError {
    /// [`JobError::Retry`] with backoff.
    pub fn retry(message: impl Into<String>) -> Self {
        Self::Retry {
            message: message.into(),
            after: None,
        }
    }

    /// [`JobError::Retry`] no sooner than `after`.
    pub fn retry_after(message: impl Into<String>, after: Duration) -> Self {
        Self::Retry {
            message: message.into(),
            after: Some(after),
        }
    }

    /// [`JobError::Fatal`].
    pub fn fatal(message: impl Into<String>) -> Self {
        Self::Fatal(message.into())
    }
}

/// Database errors are usually transient: retry.
impl From<sqlx::Error> for JobError {
    fn from(err: sqlx::Error) -> Self {
        Self::retry(format!("database: {err}"))
    }
}

/// Kernel errors: missing records and bad requests are permanent, the rest
/// is retried. Internal detail is not copied into the message.
impl From<crate::Error> for JobError {
    fn from(err: crate::Error) -> Self {
        match err {
            crate::Error::NotFound
            | crate::Error::BadRequest(_)
            | crate::Error::Forbidden
            | crate::Error::Denied(_) => Self::fatal(err.to_string()),
            other => Self::retry(other.to_string()),
        }
    }
}

/// How to enqueue a job. `Default` runs it as soon as possible, at priority
/// 0, with no unique or ordering key, and no workspace.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnqueueOptions {
    /// The workspace the job works for: it runs with that workspace set and
    /// counts towards its fair share.
    pub workspace: Option<String>,
    /// While a job of the same kind with this key is pending or running,
    /// enqueueing another is a no-op ([`Enqueued::Duplicate`]).
    pub unique_key: Option<String>,
    /// Jobs with the same key run strictly one at a time, in enqueue order.
    /// Enqueuers sharing a key are serialized until their transactions end.
    pub ordering_key: Option<String>,
    /// Higher runs first among the workspace's ready jobs.
    pub priority: i16,
    /// Not before this instant.
    pub run_at: Option<DateTime<Utc>>,
}

impl EnqueueOptions {
    /// Options for work belonging to `workspace`.
    pub fn for_workspace(workspace: impl Into<String>) -> Self {
        Self {
            workspace: Some(workspace.into()),
            ..Self::default()
        }
    }

    /// Set the unique key.
    #[must_use]
    pub fn unique(mut self, key: impl Into<String>) -> Self {
        self.unique_key = Some(key.into());
        self
    }

    /// Set the ordering key.
    #[must_use]
    pub fn ordered_by(mut self, key: impl Into<String>) -> Self {
        self.ordering_key = Some(key.into());
        self
    }

    /// Set the priority.
    #[must_use]
    pub fn priority(mut self, priority: i16) -> Self {
        self.priority = priority;
        self
    }

    /// Run no earlier than `at`.
    #[must_use]
    pub fn run_at(mut self, at: DateTime<Utc>) -> Self {
        self.run_at = Some(at);
        self
    }
}

/// What [`enqueue`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Enqueued {
    /// A new job.
    Created(JobId),
    /// An unfinished job with the same kind and unique key exists already.
    Duplicate,
}

/// Enqueue `job` inside `tx`: it exists for workers once `tx` commits, and
/// never if it rolls back.
pub async fn enqueue<J: Job>(
    tx: &mut Transaction<'_, Postgres>,
    job: &J,
    options: EnqueueOptions,
) -> Result<Enqueued, sqlx::Error> {
    let payload = serde_json::to_value(job).map_err(|e| sqlx::Error::Encode(e.into()))?;
    insert(
        tx,
        NewJob {
            kind: J::NAME,
            queue: J::QUEUE,
            max_attempts: J::MAX_ATTEMPTS,
            payload,
        },
        options,
    )
    .await
}

/// A job row about to be inserted.
struct NewJob {
    kind: &'static str,
    queue: &'static str,
    max_attempts: u32,
    payload: serde_json::Value,
}

async fn insert(
    tx: &mut Transaction<'_, Postgres>,
    job: NewJob,
    options: EnqueueOptions,
) -> Result<Enqueued, sqlx::Error> {
    if let Some(key) = &options.ordering_key {
        // Serialize enqueuers of one ordering key until they commit, so
        // enqueue order (`seq`) is also commit order for that key.
        sqlx::query_scalar!(
            "SELECT pg_advisory_xact_lock(hashtextextended('kernel.jobs.ordering_key:' || $1, 0))",
            key
        )
        .fetch_one(&mut **tx)
        .await?;
    }
    let id = sqlx::query_scalar!(
        r#"
        INSERT INTO kernel.jobs
            (queue, kind, payload, workspace_id, unique_key, ordering_key, priority,
             run_at, max_attempts)
        VALUES ($1, $2, $3, $4, $5, $6, $7, coalesce($8, now()), $9)
        ON CONFLICT (kind, unique_key)
            WHERE unique_key IS NOT NULL AND state IN ('pending', 'running')
            DO NOTHING
        RETURNING id AS "id: JobId"
        "#,
        job.queue,
        job.kind,
        job.payload,
        options.workspace,
        options.unique_key,
        options.ordering_key,
        options.priority,
        options.run_at,
        attempts_to_db(job.max_attempts),
    )
    .fetch_optional(&mut **tx)
    .await?;
    let Some(id) = id else {
        return Ok(Enqueued::Duplicate);
    };
    notify(tx, job.queue).await?;
    Ok(Enqueued::Created(id))
}

/// Wake idle workers once the surrounding transaction commits.
pub(crate) async fn notify(conn: &mut PgConnection, queue: &str) -> Result<(), sqlx::Error> {
    sqlx::query_scalar!("SELECT pg_notify($1, $2)", NOTIFY_CHANNEL, queue)
        .fetch_one(conn)
        .await?;
    Ok(())
}

/// `max_attempts` as stored (Postgres `integer`).
pub(crate) fn attempts_to_db(attempts: u32) -> i32 {
    i32::try_from(attempts).unwrap_or(i32::MAX)
}

/// `message` cut to [`MAX_ERROR_LEN`] characters.
pub(crate) fn truncate_error(message: &str) -> String {
    match message.char_indices().nth(MAX_ERROR_LEN) {
        Some((cut, _)) => format!("{}…", &message[..cut]),
        None => message.to_owned(),
    }
}
