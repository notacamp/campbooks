//! `POST /admin/v1/workspaces/signup-requests/{user_id}/reject` — turn a
//! signup request down, with an optional reason. Idempotent.
//!
//! An admin operation (ADR 0015): requires `signups.review`.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Problem, Result,
    admin::{Admin, AdminOperation, permission::SignupsReview},
    extract,
    tenancy::UserId,
};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::domain::signup::{RequestStatus, SignupRequestPath, SignupRequestView, decide};

/// Why a request is turned down.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RejectSignupRequest {
    /// An optional note for the administrators' records, at most 500
    /// characters. The requester is told only that they were not approved.
    #[serde(default)]
    pub reason: Option<String>,
}

/// Reject a signup request.
#[utoipa::path(
    post,
    path = "/admin/v1/workspaces/signup-requests/{user_id}/reject",
    operation_id = "workspaces.admin_reject_signup_request",
    tag = "workspaces",
    security(("admin" = ["admin:access"])),
    extensions(("x-campbooks-admin" = json!(AdminOperation::command::<SignupsReview>()))),
    params(("user_id" = UserId, Path, description = "The requesting user.")),
    request_body = RejectSignupRequest,
    responses(
        (status = OK, description = "The request, rejected (also when it already was).",
            body = SignupRequestView),
        (status = BAD_REQUEST, description = "The reason is too long.", body = Problem,
            content_type = "application/problem+json"),
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
    extract::Json(input): extract::Json<RejectSignupRequest>,
) -> Result<Json<SignupRequestView>> {
    decide(
        &state,
        &admin.grant,
        path.user_id,
        RequestStatus::Rejected,
        input.reason.as_deref(),
    )
    .await
    .map(Json)
}
