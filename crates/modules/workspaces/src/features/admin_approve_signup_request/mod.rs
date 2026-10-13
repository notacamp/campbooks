//! `POST /admin/v1/workspaces/signup-requests/{user_id}/approve` — let a
//! person who asked create their first workspace. Idempotent.
//!
//! An admin operation (ADR 0015): requires `signups.review`.

use crate::domain::signup::{RequestStatus, SignupRequestPath, SignupRequestView, decide};
use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Problem, Result,
    admin::{Admin, AdminOperation, permission::SignupsReview},
    extract,
    tenancy::UserId,
};

/// Approve a signup request.
#[utoipa::path(
    post,
    path = "/admin/v1/workspaces/signup-requests/{user_id}/approve",
    operation_id = "workspaces.admin_approve_signup_request",
    tag = "workspaces",
    security(("admin" = ["admin:access"])),
    extensions(("x-campbooks-admin" = json!(AdminOperation::command::<SignupsReview>()))),
    params(("user_id" = UserId, Path, description = "The requesting user.")),
    responses(
        (status = OK, description = "The request, approved (also when it already was).",
            body = SignupRequestView),
        (status = UNAUTHORIZED, description = "`admin_reauth_required`: sign in again.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such request, or not an administrator with \
            `signups.review`.", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    admin: Admin<SignupsReview>,
    State(state): State<AppState>,
    extract::Path(path): extract::Path<SignupRequestPath>,
) -> Result<Json<SignupRequestView>> {
    decide(
        &state,
        &admin.grant,
        path.user_id,
        RequestStatus::Approved,
        None,
    )
    .await
    .map(Json)
}
