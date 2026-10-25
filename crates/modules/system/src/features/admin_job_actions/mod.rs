//! Retry and discard dead jobs (ADR 0015): one job, or every dead job of a
//! kind; and discard the pending or stuck row of a schedule
//! (campbooks-cloud#207). Requires `jobs.retry` (or `super`). Only dead jobs
//! and a schedule's blocking row are ever touched, and repeating a call is a
//! no-op success: the answer says whether anything changed. What changed is
//! tracked by the kernel, in its transaction (`kernel.job_retried`,
//! `kernel.jobs_retried`, `kernel.job_discarded`,
//! `kernel.schedule_run_discarded`).

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Result,
    admin::{Admin, AdminOperation, permission::JobsRetry},
    extract::{Json as JsonBody, Path},
    jobs::{
        JobId,
        admin::{self, Discard, DiscardSchedule, Retry},
    },
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// The `{id}` of a job route. A struct, not a bare id: behind the BFF's
/// `/bff/admin/{*path}` the request also carries a `path` parameter, which a
/// single-value `Path<JobId>` would refuse.
#[derive(Debug, Deserialize)]
pub struct JobPath {
    id: JobId,
}

/// What a retry or discard did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// The job was dead; it is now pending again (retry) or gone (discard).
    Applied,
    /// The job was not dead (already retried, running or done) or is gone:
    /// nothing changed.
    Unchanged,
    /// Another unfinished job of the same kind holds this job's unique key:
    /// retrying would duplicate that work, so nothing changed.
    Superseded,
}

/// The result of acting on one job.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct JobAction {
    /// What happened.
    pub outcome: Outcome,
}

/// Retry one dead job.
#[utoipa::path(
    post,
    path = "/admin/v1/system/jobs/{id}/retry",
    operation_id = "system.admin_retry_job",
    tag = "system",
    params(("id" = JobId, Path, description = "The job.")),
    security(("admin" = ["admin:access"])),
    extensions(("x-campbooks-admin" = json!(AdminOperation::command::<JobsRetry>()))),
    responses(
        (status = OK, description = "Requeued, or nothing to do.", body = JobAction),
        (status = UNAUTHORIZED, description = "`admin_reauth_required`: sign in again.",
            body = campbooks_kernel::Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such job, or not an administrator with \
            `jobs.retry`.", body = campbooks_kernel::Problem,
            content_type = "application/problem+json"),
    )
)]
pub async fn retry(
    admin: Admin<JobsRetry>,
    State(state): State<AppState>,
    Path(JobPath { id }): Path<JobPath>,
) -> Result<Json<JobAction>> {
    let outcome = match admin::retry(&state.db, &state.tracker, &admin.grant, id).await? {
        Retry::Requeued => Outcome::Applied,
        Retry::Unchanged => Outcome::Unchanged,
        Retry::Superseded => Outcome::Superseded,
        Retry::Missing => return Err(Error::NotFound),
    };
    Ok(Json(JobAction { outcome }))
}

/// Delete one dead job.
#[utoipa::path(
    delete,
    path = "/admin/v1/system/jobs/{id}",
    operation_id = "system.admin_discard_job",
    tag = "system",
    params(("id" = JobId, Path, description = "The job.")),
    security(("admin" = ["admin:access"])),
    extensions(("x-campbooks-admin" = json!(AdminOperation::command::<JobsRetry>()))),
    responses(
        (status = OK, description = "Deleted, or nothing to do.", body = JobAction),
        (status = UNAUTHORIZED, description = "`admin_reauth_required`: sign in again.",
            body = campbooks_kernel::Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "Not an administrator with `jobs.retry`.",
            body = campbooks_kernel::Problem, content_type = "application/problem+json"),
    )
)]
pub async fn discard(
    admin: Admin<JobsRetry>,
    State(state): State<AppState>,
    Path(JobPath { id }): Path<JobPath>,
) -> Result<Json<JobAction>> {
    let outcome = match admin::discard(&state.db, &state.tracker, &admin.grant, id).await? {
        Discard::Discarded => Outcome::Applied,
        Discard::Unchanged | Discard::Missing => Outcome::Unchanged,
    };
    Ok(Json(JobAction { outcome }))
}

/// The `{name}` of a schedule route (a struct for the same reason as
/// [`JobPath`]).
#[derive(Debug, Deserialize)]
pub struct SchedulePath {
    name: String,
}

/// Discard a schedule's pending or stuck run, so its next tick enqueues
/// afresh. A run a live worker holds is kept.
#[utoipa::path(
    delete,
    path = "/admin/v1/system/schedules/{name}/run",
    operation_id = "system.admin_discard_schedule_run",
    tag = "system",
    params(("name" = String, Path, description = "The schedule (`<module>.<job>`).")),
    security(("admin" = ["admin:access"])),
    extensions(("x-campbooks-admin" = json!(AdminOperation::command::<JobsRetry>()))),
    responses(
        (status = OK, description = "Deleted, or nothing to do.", body = JobAction),
        (status = UNAUTHORIZED, description = "`admin_reauth_required`: sign in again.",
            body = campbooks_kernel::Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such schedule, or not an administrator with \
            `jobs.retry`.", body = campbooks_kernel::Problem,
            content_type = "application/problem+json"),
    )
)]
pub async fn discard_schedule_run(
    admin: Admin<JobsRetry>,
    State(state): State<AppState>,
    Path(SchedulePath { name }): Path<SchedulePath>,
) -> Result<Json<JobAction>> {
    let outcome = match admin::discard_schedule(&state, &admin.grant, &name).await? {
        DiscardSchedule::Discarded => Outcome::Applied,
        DiscardSchedule::Unchanged => Outcome::Unchanged,
        DiscardSchedule::Unknown => return Err(Error::NotFound),
    };
    Ok(Json(JobAction { outcome }))
}

/// Which dead jobs to retry.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RetryKind {
    /// The kind (`<module>.<job>`).
    pub kind: String,
    /// Only on this queue.
    pub queue: Option<String>,
}

/// How many jobs a bulk retry requeued.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RetryKindResult {
    /// Jobs put back on their queue.
    pub requeued: u64,
    /// Whether the batch limit was reached, so more dead jobs of the kind
    /// may remain: call again.
    pub more: bool,
}

/// Retry the dead jobs of one kind, oldest first, up to 1000 per call.
#[utoipa::path(
    post,
    path = "/admin/v1/system/jobs/retry",
    operation_id = "system.admin_retry_jobs",
    tag = "system",
    request_body = RetryKind,
    security(("admin" = ["admin:access"])),
    extensions(("x-campbooks-admin" = json!(AdminOperation::command::<JobsRetry>()))),
    responses(
        (status = OK, description = "How many were requeued.", body = RetryKindResult),
        (status = BAD_REQUEST, description = "Malformed body.",
            body = campbooks_kernel::Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "`admin_reauth_required`: sign in again.",
            body = campbooks_kernel::Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "Not an administrator with `jobs.retry`.",
            body = campbooks_kernel::Problem, content_type = "application/problem+json"),
    )
)]
pub async fn retry_kind(
    admin: Admin<JobsRetry>,
    State(state): State<AppState>,
    JsonBody(body): JsonBody<RetryKind>,
) -> Result<Json<RetryKindResult>> {
    let requeued =
        admin::retry_kind(
        &state.db,
        &state.tracker,
        &admin.grant,
        &body.kind,
        body.queue.as_deref(),
    ).await?;
    Ok(Json(RetryKindResult {
        requeued,
        more: i64::try_from(requeued).is_ok_and(|n| n >= admin::RETRY_BATCH),
    }))
}

#[cfg(test)]
mod tests;
