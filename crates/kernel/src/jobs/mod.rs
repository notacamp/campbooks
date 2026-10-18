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
//!   priority, a `unique_key` (at most one unfinished job per kind and key
//!   in a workspace) and an `ordering_key` (a workspace's jobs sharing it
//!   run one at a time, in enqueue order). Both keys are per workspace, with
//!   instance-wide work as one more: a workspace cannot hold or delay
//!   another's key (campbooks-cloud#209).
//! - **Run**: a [`Worker`] claims jobs fairly across workspaces, runs each in
//!   a [`JobContext`] whose transaction carries the job's workspace
//!   (`campbooks.workspace_id`), retries [`JobError::Retry`] with
//!   exponential backoff and jitter, and marks [`JobError::Fatal`] or
//!   exhausted jobs failed (dead, kept for inspection).

pub mod admin;
mod context;
mod maintenance;
mod registry;
mod rekey;
mod worker;

#[cfg(test)]
mod tests;

use std::{future::Future, time::Duration};

use chrono::{DateTime, Utc};
use serde::{Serialize, de::DeserializeOwned};
use sqlx::{PgConnection, Postgres, Transaction};

pub use context::JobContext;
pub use maintenance::{PruneEvents, PruneJobs, purge_workspace};
pub(crate) use registry::is_dotted;
pub use registry::{Background, ConsumerInfo, InstanceRows, Registry, RegistryError};
pub use rekey::{DispatchRekeys, REKEY_EVERY};
pub use worker::{Worker, WorkerStats};

crate::define_id!(
    /// A job in `kernel.jobs`.
    JobId
);

/// Attempts a job gets unless its type says otherwise.
pub const DEFAULT_MAX_ATTEMPTS: u32 = 10;

/// The `LISTEN/NOTIFY` channel enqueues signal on; the payload is the queue.
pub const NOTIFY_CHANNEL: &str = "kernel_jobs";

/// Longest `last_error` kept, in characters, after
/// [`redact::job_error`](crate::redact::job_error) kept its first line.
pub const MAX_ERROR_LEN: usize = 500;

/// A kind of background work an operator must hear about when it dies: a
/// dead job or delivery of it is counted under `code` in Admin → Jobs (with
/// every other kind watched under the same code), and its death is logged
/// at WARN as `died`, a stable line an alert can match. Declared with the
/// work (`WATCH` on [`Job`], [`JobHandler`], [`EventHandler`] and
/// [`EventConsumer`]); kinds sharing a code must share the line.
///
/// [`EventHandler`]: crate::EventHandler
/// [`EventConsumer`]: crate::EventConsumer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Watch {
    /// What the watched kinds are for, lower-case snake case
    /// (`suspension_fanout`); the admin client words it.
    pub code: &'static str,
    /// The WARN line logged when one dies (`suspension fan-out delivery
    /// died`): no ids or content, so alerts and logs can quote it.
    pub died: &'static str,
}

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
    /// Whether an operator must hear when one dies ([`Watch`]).
    const WATCH: Option<Watch> = None;
    /// Whether the job is the instance's only, never a workspace's
    /// (maintenance across workspaces, a scheduler's dispatcher): the
    /// worker refuses, without running it, a job of this kind that carries
    /// a workspace (campbooks-cloud#211). A scheduled job
    /// ([`Background::schedule`]) is instance-only whatever this says.
    const INSTANCE_ONLY: bool = false;

    /// Do the work.
    fn run(self, ctx: &mut JobContext) -> impl Future<Output = Result<(), JobError>> + Send;
}

/// A job kind run by a value that owns its dependencies: the module builds
/// it from what it was composed with (an AI gateway, settings) and registers
/// it with [`Background::handler`]. Enqueue its payload with
/// [`enqueue_to::<H>`](enqueue_to).
///
/// Use [`Job`] when the job needs nothing but its payload and the
/// [`JobContext`]. The payload rules are the same: ids, never secrets or
/// message bodies.
pub trait JobHandler: Send + Sync + 'static {
    /// The job's payload, stored as JSON.
    type Payload: Serialize + DeserializeOwned + Send + Sync + 'static;
    /// Stable wire name, `<module>.<job>`.
    const NAME: &'static str;
    /// The queue it runs on.
    const QUEUE: &'static str;
    /// Attempts before the job is marked failed (dead).
    const MAX_ATTEMPTS: u32 = DEFAULT_MAX_ATTEMPTS;
    /// Whether an operator must hear when one dies ([`Watch`]).
    const WATCH: Option<Watch> = None;
    /// Whether the kind is the instance's only, never a workspace's
    /// ([`Job::INSTANCE_ONLY`]).
    const INSTANCE_ONLY: bool = false;

    /// Do the work.
    fn run(
        &self,
        ctx: &mut JobContext,
        payload: Self::Payload,
    ) -> impl Future<Output = Result<(), JobError>> + Send;
}

/// Why a job did not succeed. The message is stored in `last_error` and
/// logged, after [`redact::job_error`](crate::redact::job_error) kept its
/// first line, hid anything quoted, addresses, links and tokens, and
/// [`MAX_ERROR_LEN`] bounded it. That is a backstop: never put secrets or
/// user content in it; say what failed, with ids and status codes.
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

/// Database errors are usually transient: retry. The message names the
/// error's kind (SQLSTATE, constraint, table), never the values it quotes.
impl From<sqlx::Error> for JobError {
    fn from(err: sqlx::Error) -> Self {
        Self::retry(crate::redact::sqlx(&err))
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
    /// While a job of the same kind with this key is pending or running in
    /// the same workspace (or, for instance-wide work, in none), enqueueing
    /// another is a no-op ([`Enqueued::Duplicate`]). Another workspace's
    /// job with the same key is another job (campbooks-cloud#209).
    pub unique_key: Option<String>,
    /// Jobs of one workspace (or instance-wide ones) with the same key run
    /// strictly one at a time, in enqueue order; another workspace's jobs
    /// with the same key do not wait for them. Enqueuers sharing a key in a
    /// workspace are serialized until their transactions end.
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
    /// An unfinished job with the same kind and unique key exists already
    /// in the same workspace (or, instance-wide, in none).
    Duplicate,
}

/// Enqueue `job` inside `tx`: it exists for workers once `tx` commits, and
/// never if it rolls back. The job's span will link to the current span.
///
/// # Errors
/// [`sqlx::Error::InvalidArgument`] when `options` names a workspace that
/// `tx` does not work in (another one, or none and no kernel scope).
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

/// Enqueue a job of `H`'s kind inside `tx`, like [`enqueue`].
pub async fn enqueue_to<H: JobHandler>(
    tx: &mut Transaction<'_, Postgres>,
    payload: &H::Payload,
    options: EnqueueOptions,
) -> Result<Enqueued, sqlx::Error> {
    let payload = serde_json::to_value(payload).map_err(|e| sqlx::Error::Encode(e.into()))?;
    insert(
        tx,
        NewJob {
            kind: H::NAME,
            queue: H::QUEUE,
            max_attempts: H::MAX_ATTEMPTS,
            payload,
        },
        options,
    )
    .await
}

/// From a transaction that works in no workspace (a scheduler's dispatcher
/// in the maintenance scope, campbooks-cloud#196): enqueue `job` in
/// `workspace`, like [`enqueue`] with `options.workspace` set to it. The
/// transaction works in `workspace` for this enqueue only: the kernel sets
/// it, enqueues, and clears it again, so the next statement of the
/// transaction (another workspace's enqueue, a read of the dispatcher's
/// index) does not run in it (campbooks-cloud#207).
///
/// # Errors
/// [`sqlx::Error::InvalidArgument`] when `tx` already works in a workspace
/// (use [`enqueue`] there); the enqueue's own errors. On an error the
/// workspace is cleared too, unless the database aborted the transaction.
pub async fn enqueue_in<J: Job>(
    tx: &mut Transaction<'_, Postgres>,
    workspace: &str,
    job: &J,
    options: EnqueueOptions,
) -> Result<Enqueued, sqlx::Error> {
    let payload = serde_json::to_value(job).map_err(|e| sqlx::Error::Encode(e.into()))?;
    insert_in(
        tx,
        workspace,
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

/// Enqueue a job of `H`'s kind in `workspace` from a transaction that works
/// in none, like [`enqueue_in`].
///
/// # Errors
/// As [`enqueue_in`].
pub async fn enqueue_to_in<H: JobHandler>(
    tx: &mut Transaction<'_, Postgres>,
    workspace: &str,
    payload: &H::Payload,
    options: EnqueueOptions,
) -> Result<Enqueued, sqlx::Error> {
    let payload = serde_json::to_value(payload).map_err(|e| sqlx::Error::Encode(e.into()))?;
    insert_in(
        tx,
        workspace,
        NewJob {
            kind: H::NAME,
            queue: H::QUEUE,
            max_attempts: H::MAX_ATTEMPTS,
            payload,
        },
        options,
    )
    .await
}

/// Set `workspace`, insert `job` there, and clear the workspace again.
async fn insert_in(
    tx: &mut Transaction<'_, Postgres>,
    workspace: &str,
    job: NewJob,
    mut options: EnqueueOptions,
) -> Result<Enqueued, sqlx::Error> {
    if crate::db::scope(tx).await?.workspace.is_some() {
        return Err(sqlx::Error::InvalidArgument(format!(
            "job `{}`: `enqueue_in` is for a transaction that works in no workspace; this one \
             works in one already (use `enqueue`)",
            job.kind
        )));
    }
    options.workspace = Some(workspace.to_owned());
    crate::db::set_workspace(tx, workspace).await?;
    let enqueued = insert(tx, job, options).await;
    let cleared = crate::db::clear_workspace(tx).await;
    let enqueued = enqueued?;
    cleared?;
    Ok(enqueued)
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
    // A workspace's job is enqueued where that workspace's rows are in
    // reach: refused in a transaction that set no workspace (row-level
    // security would hide the queue from it). An instance-wide job is
    // enqueued from a transaction that set nothing (the kernel lifts it for
    // this one insert) or from the kernel scope, never from a workspace's
    // (campbooks-cloud#207: row-level security refuses it there).
    let what = format!("job `{}`", job.kind);
    let lifted = match &options.workspace {
        Some(workspace) => {
            crate::db::require_scope(tx, workspace, &what).await?;
            false
        }
        None => crate::db::lift_for_instance_rows(tx, &what, false).await?,
    };
    let enqueued = insert_row(tx, job, options).await?;
    crate::db::lower(tx, lifted).await?;
    Ok(enqueued)
}

async fn insert_row(
    tx: &mut Transaction<'_, Postgres>,
    job: NewJob,
    options: EnqueueOptions,
) -> Result<Enqueued, sqlx::Error> {
    if let Some(key) = &options.ordering_key {
        // Serialize enqueuers of one ordering key in one workspace (or of
        // the instance-wide one) until they commit, so enqueue order (`seq`)
        // is also commit order for that key there. Another workspace's
        // enqueuers of the same key take another lock (campbooks-cloud#209).
        sqlx::query_scalar!(
            "SELECT pg_advisory_xact_lock(hashtextextended(
                 'kernel.jobs.ordering_key:' || jsonb_build_array($1::text, $2::text)::text, 0))",
            options.workspace,
            key
        )
        .fetch_one(&mut **tx)
        .await?;
    }
    let id = sqlx::query_scalar!(
        r#"
        INSERT INTO kernel.jobs
            (queue, kind, payload, workspace_id, unique_key, ordering_key, priority,
             run_at, max_attempts, trace_context)
        VALUES ($1, $2, $3, $4, $5, $6, $7, coalesce($8, now()), $9, $10)
        ON CONFLICT (kind, unique_key, workspace_id)
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
        crate::telemetry::current_traceparent(),
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
