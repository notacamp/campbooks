//! Claims and runs jobs.

use std::{
    collections::HashMap,
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use serde_json::Value;
use sqlx::{PgExecutor, PgPool, postgres::PgListener};
use tokio::{
    sync::Notify,
    task::{self, JoinError, JoinSet},
    time::Instant,
};
use tracing::Instrument as _;

use super::{
    EnqueueOptions, JobError, JobId, NOTIFY_CHANNEL, NewJob,
    context::{JobContext, JobInfo},
    registry::Registry,
    truncate_error,
};
use crate::{
    AppState,
    config::JobsConfig,
    events::EventId,
    telemetry::{self, JobOutcome},
};

/// How often a running worker logs its counters.
const STATS_EVERY: Duration = Duration::from_mins(5);
/// Longest pause between reclaiming expired leases and checking schedules.
const MAX_UPKEEP_EVERY: Duration = Duration::from_secs(5);
/// Shortest pause between two upkeep passes.
const MIN_UPKEEP_EVERY: Duration = Duration::from_secs(1);
/// Expired leases reclaimed per upkeep pass.
const REAP_BATCH: i64 = 1_000;
/// Pause before listening again after the notification connection failed.
const LISTEN_RETRY: Duration = Duration::from_secs(5);
/// How often a worker samples queue depth while metrics are exported.
const SAMPLE_QUEUES_EVERY: Duration = Duration::from_secs(15);

/// What a worker did, for logs and tests.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WorkerStats {
    /// Jobs claimed.
    pub claimed: u64,
    /// Jobs that succeeded.
    pub succeeded: u64,
    /// Failed attempts that will be retried.
    pub retried: u64,
    /// Jobs marked failed for good (dead).
    pub failed: u64,
    /// Jobs handed back to the queue unfinished at shutdown.
    pub released: u64,
    /// Jobs whose lease this worker lost before finishing (their effects
    /// were rolled back; another worker runs them).
    pub lost: u64,
}

#[derive(Default)]
struct Counters {
    claimed: AtomicU64,
    succeeded: AtomicU64,
    retried: AtomicU64,
    failed: AtomicU64,
    released: AtomicU64,
    lost: AtomicU64,
}

impl Counters {
    fn snapshot(&self) -> WorkerStats {
        WorkerStats {
            claimed: self.claimed.load(Ordering::Relaxed),
            succeeded: self.succeeded.load(Ordering::Relaxed),
            retried: self.retried.load(Ordering::Relaxed),
            failed: self.failed.load(Ordering::Relaxed),
            released: self.released.load(Ordering::Relaxed),
            lost: self.lost.load(Ordering::Relaxed),
        }
    }
}

fn bump(counter: &AtomicU64, by: u64) {
    counter.fetch_add(by, Ordering::Relaxed);
}

/// A claimed job and its payload.
struct Claimed {
    info: JobInfo,
    payload: Value,
    /// The `traceparent` of whoever enqueued it.
    trace_context: Option<String>,
}

/// Claims jobs from every queue in the registry and runs them.
///
/// Any number of workers, in any number of processes, can share one
/// database: claims use `FOR UPDATE SKIP LOCKED`, each claimed job is leased
/// to its worker (renewed while it runs), and an expired lease hands the job
/// to another worker.
#[derive(Clone)]
pub struct Worker {
    shared: Arc<Shared>,
}

impl std::fmt::Debug for Worker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Worker")
            .field("id", &self.shared.id)
            .field("queues", &self.shared.queues)
            .finish_non_exhaustive()
    }
}

struct Shared {
    state: AppState,
    registry: Arc<Registry>,
    config: JobsConfig,
    id: String,
    queues: Vec<&'static str>,
    counters: Counters,
}

impl Worker {
    /// A worker for `state`'s registry (`state.events.registry()`) and
    /// `state.config.jobs`.
    pub fn new(state: AppState) -> Self {
        let registry = Arc::clone(state.events.registry());
        let config = state.config.jobs.clone();
        let host = std::env::var("HOSTNAME").unwrap_or_else(|_| "worker".to_owned());
        Self {
            shared: Arc::new(Shared {
                queues: registry.queues(),
                registry,
                config,
                id: format!("{host}:{}", uuid::Uuid::now_v7()),
                counters: Counters::default(),
                state,
            }),
        }
    }

    /// This worker's id, as stored in `kernel.jobs.locked_by`.
    pub fn id(&self) -> &str {
        &self.shared.id
    }

    /// What this worker has done so far.
    pub fn stats(&self) -> WorkerStats {
        self.shared.counters.snapshot()
    }

    /// Run until `shutdown` resolves: wake on enqueue notifications (or every
    /// poll interval), reclaim expired leases, enqueue due schedules, and
    /// keep up to `concurrency` jobs running. On shutdown, stop claiming,
    /// give running jobs the grace period to finish, then hand the rest back
    /// to the queue. Database outages are logged and retried, never fatal.
    pub async fn run(self, shutdown: impl Future<Output = ()> + Send) -> WorkerStats {
        let shared = self.shared;
        let wake = Arc::new(Notify::new());
        let listener = tokio::spawn(listen(shared.state.db.clone(), Arc::clone(&wake)));
        tracing::info!(
            worker = %shared.id,
            queues = ?shared.queues,
            concurrency = shared.config.concurrency,
            per_workspace = shared.config.per_workspace,
            "job worker started"
        );

        let mut tasks = Tasks::default();
        let poll = Duration::from_millis(shared.config.poll_interval_ms);
        let upkeep_every = poll.clamp(MIN_UPKEEP_EVERY, MAX_UPKEEP_EVERY);
        let mut next_upkeep = Instant::now();
        let mut next_stats = Instant::now() + STATS_EVERY;
        let mut next_sample = Instant::now();
        let mut rotation = 0_usize;
        tokio::pin!(shutdown);

        loop {
            let now = Instant::now();
            if now >= next_upkeep {
                shared.upkeep().await;
                next_upkeep = now + upkeep_every;
            }
            if now >= next_stats {
                log_stats(&shared, "job worker counters");
                next_stats = now + STATS_EVERY;
            }
            if now >= next_sample && shared.state.metrics.exported() {
                shared.sample_queues().await;
                next_sample = now + SAMPLE_QUEUES_EVERY;
            }
            shared.fill(&mut tasks, rotation).await;
            rotation = rotation.wrapping_add(1);

            let wait_until = next_upkeep.min(now + poll);
            tokio::select! {
                biased;
                () = &mut shutdown => break,
                Some(done) = tasks.set.join_next_with_id(), if !tasks.set.is_empty() => {
                    shared.finished(&mut tasks, done).await;
                }
                () = wake.notified() => {}
                () = tokio::time::sleep_until(wait_until) => {}
            }
        }

        listener.abort();
        shared.drain(tasks).await;
        log_stats(&shared, "job worker stopped");
        shared.counters.snapshot()
    }

    /// Claim and run ready jobs until none is left, then return. Ignores
    /// jobs scheduled for later. For tests and one-shot tooling; a job that
    /// keeps enqueueing ready work keeps it going.
    pub async fn run_until_idle(&self) -> WorkerStats {
        let shared = &self.shared;
        let mut tasks = Tasks::default();
        let mut rotation = 0_usize;
        loop {
            shared.fill(&mut tasks, rotation).await;
            rotation = rotation.wrapping_add(1);
            if tasks.set.is_empty() {
                break;
            }
            while let Some(done) = tasks.set.join_next_with_id().await {
                shared.finished(&mut tasks, done).await;
            }
        }
        shared.counters.snapshot()
    }

    /// Reclaim expired leases and enqueue due schedules once.
    pub async fn upkeep(&self) {
        self.shared.upkeep().await;
    }

    /// Claim up to `limit` ready jobs of `queue` without running them.
    #[cfg(test)]
    pub(crate) async fn claim_for_test(
        &self,
        queue: &str,
        limit: usize,
    ) -> Result<Vec<(JobId, Option<String>, u32)>, sqlx::Error> {
        let claimed = self.shared.claim(queue, limit).await?;
        Ok(claimed
            .into_iter()
            .map(|c| (c.info.id, c.info.workspace, c.info.attempt))
            .collect())
    }

    /// Mark a job claimed with [`claim_for_test`](Self::claim_for_test)
    /// succeeded, as its worker would.
    #[cfg(test)]
    pub(crate) async fn complete_for_test(
        &self,
        id: JobId,
        attempt: u32,
    ) -> Result<bool, sqlx::Error> {
        mark_succeeded(&self.shared.state.db, &self.shared.id, id, attempt).await
    }
}

/// The jobs a worker is running, by task, with when they were claimed.
#[derive(Default)]
struct Tasks {
    set: JoinSet<()>,
    jobs: HashMap<task::Id, (JobInfo, Instant)>,
}

impl Shared {
    fn lease(&self) -> Duration {
        Duration::from_secs(self.config.lease_seconds)
    }

    /// Claim jobs for every free slot, rotating which queue goes first.
    async fn fill(self: &Arc<Self>, tasks: &mut Tasks, rotation: usize) {
        let mut free = self.config.concurrency.saturating_sub(tasks.set.len());
        let count = self.queues.len();
        for i in 0..count {
            if free == 0 {
                break;
            }
            let queue = self.queues[(rotation + i) % count];
            match self.claim(queue, free).await {
                Ok(claimed) => {
                    free = free.saturating_sub(claimed.len());
                    for job in claimed {
                        let info = job.info.clone();
                        let handle = tasks.set.spawn(execute(Arc::clone(self), job));
                        tasks.jobs.insert(handle.id(), (info, Instant::now()));
                    }
                }
                Err(e) => {
                    tracing::warn!(queue, error = %e, "could not claim jobs; will retry");
                    break;
                }
            }
        }
    }

    /// Claim up to `limit` ready jobs of `queue`, fairly across workspaces:
    /// each workspace with ready work and fewer than `per_workspace` running
    /// jobs offers its next jobs (by priority, then age), and the claim takes
    /// them in turns — every workspace's first job before any second one —
    /// starting with the workspace served least recently. A job waits while
    /// an earlier job with its ordering key is unfinished.
    async fn claim(&self, queue: &str, limit: usize) -> Result<Vec<Claimed>, sqlx::Error> {
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let rows = sqlx::query!(
            r#"
            WITH RECURSIVE ready(key) AS (
                (SELECT j.workspace_key FROM kernel.jobs j
                  WHERE j.state = 'pending' AND j.queue = $1 AND j.run_at <= now()
                  ORDER BY j.workspace_key LIMIT 1)
                UNION ALL
                SELECT (SELECT j.workspace_key FROM kernel.jobs j
                         WHERE j.state = 'pending' AND j.queue = $1 AND j.run_at <= now()
                           AND j.workspace_key > ready.key
                         ORDER BY j.workspace_key LIMIT 1)
                  FROM ready
                 WHERE ready.key IS NOT NULL
            ),
            slots AS (
                SELECT s.key, s.free, s.last_started
                  FROM (
                      SELECT ready.key,
                             $3::bigint - (SELECT count(*) FROM kernel.jobs r
                                            WHERE r.state = 'running' AND r.queue = $1
                                              AND r.workspace_key = ready.key) AS free,
                             (SELECT l.started_at FROM kernel.jobs l
                               WHERE l.queue = $1 AND l.workspace_key = ready.key
                                 AND l.started_at IS NOT NULL
                               ORDER BY l.started_at DESC LIMIT 1) AS last_started
                        FROM ready
                       WHERE ready.key IS NOT NULL
                  ) s
                 WHERE s.free > 0
            ),
            candidates AS (
                SELECT c.id, c.priority, c.run_at, c.seq, slots.last_started,
                       row_number() OVER (PARTITION BY slots.key
                                          ORDER BY c.priority DESC, c.run_at, c.seq) AS turn
                  FROM slots
                  CROSS JOIN LATERAL (
                      SELECT j.id, j.priority, j.run_at, j.seq
                        FROM kernel.jobs j
                       WHERE j.state = 'pending' AND j.queue = $1
                         AND j.workspace_key = slots.key AND j.run_at <= now()
                         AND (j.ordering_key IS NULL OR NOT EXISTS (
                              SELECT 1 FROM kernel.jobs o
                               WHERE o.ordering_key = j.ordering_key
                                 AND o.state IN ('pending', 'running')
                                 AND o.seq < j.seq))
                       ORDER BY j.priority DESC, j.run_at, j.seq
                       LIMIT least(slots.free, $2)
                       FOR UPDATE SKIP LOCKED
                  ) c
            ),
            picked AS (
                SELECT id FROM candidates
                 ORDER BY turn, priority DESC, last_started ASC NULLS FIRST, run_at, seq
                 LIMIT $2
            )
            UPDATE kernel.jobs j
               SET state = 'running',
                   attempts = j.attempts + 1,
                   locked_by = $4,
                   locked_until = now() + make_interval(secs => $5),
                   started_at = now(),
                   updated_at = now()
              FROM picked
             WHERE j.id = picked.id
            RETURNING j.id AS "id: JobId", j.kind, j.queue, j.payload, j.workspace_id,
                      j.event_id AS "event_id: EventId", j.attempts, j.max_attempts,
                      j.trace_context,
                      extract(epoch FROM now() - j.run_at)::float8 AS "waited_seconds!"
            "#,
            queue,
            limit,
            i64::from(self.config.per_workspace),
            self.id,
            self.lease().as_secs_f64(),
        )
        .fetch_all(&self.state.db)
        .await?;

        bump(&self.counters.claimed, rows.len() as u64);
        let metrics = &self.state.metrics;
        Ok(rows
            .into_iter()
            .inspect(|row| metrics.job_claimed(&row.kind, &row.queue, row.waited_seconds))
            .map(|row| Claimed {
                info: JobInfo {
                    id: row.id,
                    kind: row.kind,
                    queue: row.queue,
                    attempt: u32::try_from(row.attempts).unwrap_or(0),
                    max_attempts: u32::try_from(row.max_attempts).unwrap_or(1),
                    workspace: row.workspace_id,
                    event_id: row.event_id,
                },
                payload: row.payload,
                trace_context: row.trace_context,
            })
            .collect())
    }

    /// A task ended: forget it, and record a panic as a failed attempt.
    async fn finished(&self, tasks: &mut Tasks, done: Result<(task::Id, ()), JoinError>) {
        match done {
            Ok((id, ())) => {
                tasks.jobs.remove(&id);
            }
            Err(e) => {
                let Some((info, claimed_at)) = tasks.jobs.remove(&e.id()) else {
                    return;
                };
                if e.is_panic() {
                    tracing::error!(job_id = %info.id, kind = %info.kind, "job handler panicked");
                    let outcome = self
                        .fail(&info, &JobError::retry("the handler panicked"))
                        .await;
                    self.record(&info, outcome, claimed_at.elapsed());
                } else {
                    self.release(&[info]).await;
                }
            }
        }
    }

    /// Stop: let running jobs finish within the grace period, then abort the
    /// rest and hand them back to the queue.
    async fn drain(&self, mut tasks: Tasks) {
        if tasks.set.is_empty() {
            return;
        }
        tracing::info!(
            running = tasks.set.len(),
            grace_seconds = self.config.shutdown_grace_seconds,
            "job worker stopping: waiting for running jobs"
        );
        let grace = tokio::time::sleep(Duration::from_secs(self.config.shutdown_grace_seconds));
        tokio::pin!(grace);
        loop {
            tokio::select! {
                done = tasks.set.join_next_with_id() => match done {
                    Some(done) => self.finished(&mut tasks, done).await,
                    None => return,
                },
                () = &mut grace => break,
            }
        }
        tasks.set.abort_all();
        while let Some(done) = tasks.set.join_next_with_id().await {
            self.finished(&mut tasks, done).await;
        }
    }

    /// Reclaim expired leases, then enqueue due schedules.
    async fn upkeep(&self) {
        if let Err(e) = self.reap().await {
            tracing::warn!(error = %e, "could not reclaim expired job leases");
        }
        for schedule in self.registry.schedules() {
            if let Err(e) = self.tick(schedule).await {
                tracing::warn!(schedule = schedule.name, error = %e, "could not run schedule");
            }
        }
    }

    /// Jobs whose worker stopped renewing their lease go back to pending
    /// (or to failed, if that was their last attempt).
    async fn reap(&self) -> Result<(), sqlx::Error> {
        let reclaimed = sqlx::query!(
            r#"
            WITH expired AS (
                SELECT id FROM kernel.jobs
                 WHERE state = 'running' AND locked_until < now()
                 ORDER BY locked_until
                 LIMIT $1
                 FOR UPDATE SKIP LOCKED
            )
            UPDATE kernel.jobs j
               SET state = CASE WHEN j.attempts >= j.max_attempts THEN 'failed' ELSE 'pending' END,
                   finished_at = CASE WHEN j.attempts >= j.max_attempts THEN now() END,
                   run_at = now(),
                   locked_by = NULL,
                   locked_until = NULL,
                   last_error = 'lease expired: its worker stopped or lost the database',
                   updated_at = now()
              FROM expired
             WHERE j.id = expired.id
            RETURNING j.id AS "id: JobId", j.kind, j.state
            "#,
            REAP_BATCH,
        )
        .fetch_all(&self.state.db)
        .await?;
        for job in reclaimed {
            tracing::warn!(job_id = %job.id, kind = %job.kind, state = %job.state,
                "reclaimed a job whose lease expired");
        }
        Ok(())
    }

    /// Enqueue `schedule`'s job if this tick has not been enqueued yet.
    async fn tick(&self, schedule: &super::registry::ScheduleInfo) -> Result<(), sqlx::Error> {
        let mut tx = self.state.db.begin().await?;
        let tick = sqlx::query_scalar!(
            r#"
            INSERT INTO kernel.schedules (name, last_tick)
            VALUES ($1, floor(extract(epoch FROM now())::float8 / $2::float8)::bigint)
            ON CONFLICT (name) DO UPDATE
               SET last_tick = excluded.last_tick, updated_at = now()
             WHERE kernel.schedules.last_tick < excluded.last_tick
            RETURNING last_tick
            "#,
            schedule.name,
            schedule.every.as_secs_f64(),
        )
        .fetch_optional(&mut *tx)
        .await?;
        let Some(tick) = tick else {
            return Ok(());
        };
        let enqueued = super::insert(
            &mut tx,
            NewJob {
                kind: schedule.name,
                queue: schedule.queue,
                max_attempts: schedule.max_attempts,
                payload: schedule.payload.clone(),
            },
            EnqueueOptions::default().unique(format!("schedule:{}", schedule.name)),
        )
        .await?;
        tx.commit().await?;
        tracing::debug!(schedule = schedule.name, tick, ?enqueued, "schedule ticked");
        Ok(())
    }

    /// Record success. With an open job transaction, the success is written
    /// in it and committed with the handler's work, provided this worker
    /// still holds the job.
    async fn complete(
        &self,
        info: &JobInfo,
        tx: Option<sqlx::Transaction<'static, sqlx::Postgres>>,
        elapsed: Duration,
    ) -> Option<JobOutcome> {
        let held = match tx {
            Some(mut tx) => match mark_succeeded(&mut *tx, &self.id, info.id, info.attempt).await {
                Ok(true) => tx.commit().await.map(|()| true),
                Ok(false) => tx.rollback().await.map(|()| false),
                Err(e) => Err(e),
            },
            None => mark_succeeded(&self.state.db, &self.id, info.id, info.attempt).await,
        };
        match held {
            Ok(true) => {
                bump(&self.counters.succeeded, 1);
                tracing::info!(elapsed_ms = elapsed.as_millis(), "job succeeded");
                Some(JobOutcome::Succeeded)
            }
            Ok(false) => {
                bump(&self.counters.lost, 1);
                tracing::warn!(
                    elapsed_ms = elapsed.as_millis(),
                    "job finished after its lease was taken over; its work was rolled back"
                );
                Some(JobOutcome::Lost)
            }
            Err(e) => {
                self.fail(info, &JobError::retry(format!("could not commit: {e}")))
                    .await
            }
        }
    }

    /// Record a failed attempt: retry later, or mark the job failed (dead).
    /// `None` when the failure could not be recorded.
    async fn fail(&self, info: &JobInfo, error: &JobError) -> Option<JobOutcome> {
        let (fatal, after) = match error {
            JobError::Fatal(_) => (true, None),
            JobError::Retry { after, .. } => (false, after.map(|d| d.as_secs_f64())),
        };
        let message = truncate_error(&error.to_string());
        #[expect(
            clippy::cast_precision_loss,
            reason = "retry delays are far below 2^52 ms; f64 seconds are exact enough"
        )]
        let (base, max) = (
            self.config.retry_base_ms as f64 / 1_000.0,
            self.config.retry_max_seconds as f64,
        );
        let outcome = sqlx::query!(
            r#"
            UPDATE kernel.jobs
               SET state = CASE WHEN $4 OR attempts >= max_attempts THEN 'failed' ELSE 'pending' END,
                   finished_at = CASE WHEN $4 OR attempts >= max_attempts THEN now() END,
                   run_at = CASE
                       WHEN $4 OR attempts >= max_attempts THEN run_at
                       WHEN $5::float8 IS NOT NULL THEN now() + make_interval(secs => $5)
                       ELSE now() + make_interval(secs =>
                           least($7::float8, $6::float8 * power(2.0::float8, (attempts - 1)::float8))
                           * (0.5 + random() * 0.5))
                   END,
                   last_error = $8,
                   locked_by = NULL,
                   locked_until = NULL,
                   updated_at = now()
             WHERE id = $1 AND state = 'running' AND locked_by = $2 AND attempts = $3
            RETURNING state, run_at
            "#,
            info.id as JobId,
            self.id,
            super::attempts_to_db(info.attempt),
            fatal,
            after,
            base,
            max,
            message,
        )
        .fetch_optional(&self.state.db)
        .await;
        match outcome {
            Ok(Some(row)) if row.state == "failed" => {
                bump(&self.counters.failed, 1);
                tracing::error!(error = %message, "job failed for good");
                Some(JobOutcome::Failed)
            }
            Ok(Some(row)) => {
                bump(&self.counters.retried, 1);
                tracing::warn!(error = %message, retry_at = %row.run_at, "job failed; will retry");
                Some(JobOutcome::Retried)
            }
            Ok(None) => {
                bump(&self.counters.lost, 1);
                tracing::warn!(error = %message, "job failed after its lease was taken over");
                Some(JobOutcome::Lost)
            }
            Err(e) => {
                tracing::error!(
                    error = %message,
                    db_error = %e,
                    "could not record a job failure; it runs again when its lease expires"
                );
                None
            }
        }
    }

    /// Record how an attempt ended on its span and in the metrics.
    fn record(&self, info: &JobInfo, outcome: Option<JobOutcome>, elapsed: Duration) {
        let Some(outcome) = outcome else {
            return;
        };
        let span = tracing::Span::current();
        span.record("job.outcome", outcome.as_str());
        if matches!(outcome, JobOutcome::Retried | JobOutcome::Failed) {
            span.record("otel.status_code", "error");
        }
        self.state
            .metrics
            .job_finished(&info.kind, &info.queue, outcome, elapsed);
    }

    /// Record queue depth and the age of the oldest ready job, per queue,
    /// for every queue this worker serves (0 when empty).
    async fn sample_queues(&self) {
        let rows = sqlx::query!(
            r#"
            SELECT queue AS "queue!",
                   count(*) FILTER (WHERE state = 'pending' AND run_at <= now()) AS "ready!",
                   count(*) FILTER (WHERE state = 'pending' AND run_at > now()) AS "scheduled!",
                   count(*) FILTER (WHERE state = 'running') AS "running!",
                   count(*) FILTER (WHERE state = 'failed') AS "failed!",
                   extract(epoch FROM now() - min(run_at)
                       FILTER (WHERE state = 'pending' AND run_at <= now()))::float8 AS oldest
              FROM kernel.jobs
             WHERE state IN ('pending', 'running', 'failed')
             GROUP BY queue
            "#
        )
        .fetch_all(&self.state.db)
        .await;
        let rows = match rows {
            Ok(rows) => rows,
            Err(e) => {
                tracing::warn!(error = %e, "could not sample job queues");
                return;
            }
        };
        let metrics = &self.state.metrics;
        for queue in &self.queues {
            let (counts, oldest) =
                rows.iter()
                    .find(|row| row.queue == *queue)
                    .map_or(([0; 4], None), |row| {
                        (
                            [row.ready, row.scheduled, row.running, row.failed],
                            row.oldest,
                        )
                    });
            for (state, count) in ["ready", "scheduled", "running", "failed"]
                .into_iter()
                .zip(counts)
            {
                metrics.queue_depth(queue, state, u64::try_from(count).unwrap_or(0));
            }
            metrics.oldest_ready(queue, oldest.unwrap_or(0.0));
        }
    }

    /// Renew the lease of a running job.
    async fn heartbeat(&self, info: &JobInfo) {
        let renewed = sqlx::query!(
            r#"
            UPDATE kernel.jobs
               SET locked_until = now() + make_interval(secs => $4), updated_at = now()
             WHERE id = $1 AND state = 'running' AND locked_by = $2 AND attempts = $3
            "#,
            info.id as JobId,
            self.id,
            super::attempts_to_db(info.attempt),
            self.lease().as_secs_f64(),
        )
        .execute(&self.state.db)
        .await;
        match renewed {
            Ok(done) if done.rows_affected() == 0 => {
                tracing::warn!("lost the job's lease; its work will be rolled back");
            }
            Ok(_) => {}
            Err(e) => tracing::warn!(error = %e, "could not renew the job's lease"),
        }
    }

    /// Hand unfinished jobs back to the queue; the interrupted attempt does
    /// not count.
    async fn release(&self, jobs: &[JobInfo]) {
        if jobs.is_empty() {
            return;
        }
        let ids: Vec<JobId> = jobs.iter().map(|j| j.id).collect();
        let released = sqlx::query!(
            r#"
            UPDATE kernel.jobs
               SET state = 'pending',
                   attempts = greatest(attempts - 1, 0),
                   run_at = now(),
                   locked_by = NULL,
                   locked_until = NULL,
                   updated_at = now()
             WHERE id = ANY($1) AND state = 'running' AND locked_by = $2
            "#,
            &ids as &[JobId],
            self.id,
        )
        .execute(&self.state.db)
        .await;
        match released {
            Ok(done) => {
                bump(&self.counters.released, done.rows_affected());
                tracing::info!(
                    released = done.rows_affected(),
                    "handed unfinished jobs back to the queue"
                );
            }
            Err(e) => tracing::warn!(
                error = %e,
                "could not hand jobs back; they run again when their leases expire"
            ),
        }
    }
}

/// Run one claimed job in its tracing span, renewing its lease meanwhile.
/// The span links to the span that enqueued the job.
async fn execute(shared: Arc<Shared>, job: Claimed) {
    let info = job.info;
    let span = tracing::info_span!(
        "job",
        otel.name = %format_args!("job {}", info.kind),
        otel.kind = "consumer",
        otel.status_code = tracing::field::Empty,
        job.id = %info.id,
        job.kind = %info.kind,
        job.queue = %info.queue,
        job.attempt = info.attempt,
        job.outcome = tracing::field::Empty,
        workspace.id = tracing::field::Empty,
        trace_id = tracing::field::Empty,
    );
    if let Some(workspace) = &info.workspace {
        span.record(telemetry::field::WORKSPACE_ID, workspace.as_str());
    }
    if let Some(traceparent) = &job.trace_context {
        telemetry::link_to(&span, traceparent);
    }
    telemetry::record_trace_id(&span);
    async move {
        let started = Instant::now();
        let Some(kind) = shared.registry.kind(&info.kind) else {
            let error = JobError::fatal(format!("no handler is registered for `{}`", info.kind));
            let outcome = shared.fail(&info, &error).await;
            shared.record(&info, outcome, started.elapsed());
            return;
        };
        let mut ctx = JobContext::new(shared.state.clone(), info.clone());
        let result = with_heartbeat(&shared, &info, (kind.run)(&mut ctx, job.payload)).await;
        let tx = ctx.take_tx();
        let outcome = match result {
            Ok(()) => shared.complete(&info, tx, started.elapsed()).await,
            Err(error) => {
                if let Some(tx) = tx
                    && let Err(e) = tx.rollback().await
                {
                    tracing::warn!(error = %e, "could not roll back the job's transaction");
                }
                shared.fail(&info, &error).await
            }
        };
        shared.record(&info, outcome, started.elapsed());
    }
    .instrument(span)
    .await;
}

/// Drive `work` to completion, renewing the job's lease every third of it.
async fn with_heartbeat<F: Future>(shared: &Shared, info: &JobInfo, work: F) -> F::Output {
    tokio::pin!(work);
    let every = shared.lease() / 3;
    let mut beat = tokio::time::interval_at(Instant::now() + every, every);
    loop {
        tokio::select! {
            out = &mut work => return out,
            _ = beat.tick() => shared.heartbeat(info).await,
        }
    }
}

/// Mark a job this worker holds succeeded. `false` when it no longer holds it.
async fn mark_succeeded<'e>(
    db: impl PgExecutor<'e>,
    worker: &str,
    id: JobId,
    attempt: u32,
) -> Result<bool, sqlx::Error> {
    let done = sqlx::query!(
        r#"
        UPDATE kernel.jobs
           SET state = 'succeeded', finished_at = now(), updated_at = now(),
               locked_by = NULL, locked_until = NULL
         WHERE id = $1 AND state = 'running' AND locked_by = $2 AND attempts = $3
        "#,
        id as JobId,
        worker,
        super::attempts_to_db(attempt),
    )
    .execute(db)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// Turn enqueue notifications into wake-ups, reconnecting as needed.
async fn listen(db: PgPool, wake: Arc<Notify>) {
    loop {
        let mut listener = match PgListener::connect_with(&db).await {
            Ok(listener) => listener,
            Err(e) => {
                tracing::warn!(error = %e, "cannot listen for job notifications; polling");
                tokio::time::sleep(LISTEN_RETRY).await;
                continue;
            }
        };
        if let Err(e) = listener.listen(NOTIFY_CHANNEL).await {
            tracing::warn!(error = %e, "cannot listen for job notifications; polling");
            tokio::time::sleep(LISTEN_RETRY).await;
            continue;
        }
        loop {
            match listener.recv().await {
                Ok(_) => wake.notify_one(),
                Err(e) => {
                    tracing::warn!(error = %e, "job notifications interrupted; polling");
                    tokio::time::sleep(LISTEN_RETRY).await;
                    break;
                }
            }
        }
    }
}

fn log_stats(shared: &Shared, message: &'static str) {
    let s = shared.counters.snapshot();
    tracing::info!(
        worker = %shared.id,
        claimed = s.claimed,
        succeeded = s.succeeded,
        retried = s.retried,
        failed = s.failed,
        released = s.released,
        lost = s.lost,
        "{message}"
    );
}
