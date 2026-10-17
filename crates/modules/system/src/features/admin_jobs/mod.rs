//! `GET /admin/v1/system/jobs` and `GET /admin/v1/system/jobs/failed` — the
//! Jobs panel's reads (ADR 0015): the queue's counts by queue and kind (with
//! the dead jobs of the kinds an operator must hear about, by what they are
//! for), and the dead jobs with their last error.
//!
//! Metadata only: a job's payload is never selected, so it cannot be
//! returned. Requires `jobs.read` (or `super`).

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Result,
    admin::{Admin, AdminOperation, permission::JobsRead},
    extract::Query,
    jobs::admin::{self, FailedFilter, FailedPage, QueueSummary},
};
use serde::Deserialize;
use utoipa::IntoParams;

/// The queue at a glance, by queue and kind.
#[utoipa::path(
    get,
    path = "/admin/v1/system/jobs",
    operation_id = "system.admin_jobs",
    tag = "system",
    security(("admin" = ["admin:access"])),
    extensions(("x-campbooks-admin" = json!(AdminOperation::query::<JobsRead>()))),
    responses(
        (status = OK, description = "Counts of waiting, scheduled, running and failed jobs.",
            body = QueueSummary),
        (status = UNAUTHORIZED, description = "`admin_reauth_required`: sign in again.",
            body = campbooks_kernel::Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "Not an administrator with `jobs.read`.",
            body = campbooks_kernel::Problem, content_type = "application/problem+json"),
    )
)]
pub async fn summary(
    admin: Admin<JobsRead>,
    State(state): State<AppState>,
) -> Result<Json<QueueSummary>> {
    Ok(Json(
        admin::summary(&state.db, state.events.registry(), &admin.grant).await?,
    ))
}

/// Which dead jobs to list.
#[derive(Debug, Deserialize, IntoParams)]
#[serde(deny_unknown_fields)]
pub struct FailedQuery {
    /// Only this kind (`<module>.<job>`).
    kind: Option<String>,
    /// Only this queue.
    queue: Option<String>,
    /// The `next` of the previous page.
    after: Option<String>,
    /// Page size, 1 to 200 (default 50).
    limit: Option<i64>,
}

/// The dead jobs, newest first: what failed and the last error, never the
/// job's payload.
#[utoipa::path(
    get,
    path = "/admin/v1/system/jobs/failed",
    operation_id = "system.admin_failed_jobs",
    tag = "system",
    params(FailedQuery),
    security(("admin" = ["admin:access"])),
    extensions(("x-campbooks-admin" = json!(AdminOperation::query::<JobsRead>()))),
    responses(
        (status = OK, description = "A page of dead jobs.", body = FailedPage),
        (status = BAD_REQUEST, description = "Malformed query or cursor.",
            body = campbooks_kernel::Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "`admin_reauth_required`: sign in again.",
            body = campbooks_kernel::Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "Not an administrator with `jobs.read`.",
            body = campbooks_kernel::Problem, content_type = "application/problem+json"),
    )
)]
pub async fn failed(
    admin: Admin<JobsRead>,
    State(state): State<AppState>,
    Query(query): Query<FailedQuery>,
) -> Result<Json<FailedPage>> {
    let filter = FailedFilter {
        kind: query.kind,
        queue: query.queue,
    };
    Ok(Json(
        admin::failed(
            &state.db,
            &admin.grant,
            &filter,
            query.after.as_deref(),
            query.limit,
        )
        .await?,
    ))
}

#[cfg(test)]
mod tests;
