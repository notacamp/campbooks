//! `POST /admin/v1/workspaces/signup-codes/{code_id}/revoke` — stop a
//! signup code from being redeemed. Idempotent; people who already used it
//! keep their workspaces.
//!
//! An admin operation (ADR 0015): requires `signups.review`.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Problem, Result,
    admin::{Admin, AdminOperation, permission::SignupsReview},
    extract,
};
use chrono::Utc;
use serde::Deserialize;

use crate::domain::{
    model::SignupCodeId,
    signup::{CodeRow, SignupCodeView},
};

/// The path of one code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct CodePath {
    /// The code.
    pub code_id: SignupCodeId,
}

/// Revoke a signup code.
#[utoipa::path(
    post,
    path = "/admin/v1/workspaces/signup-codes/{code_id}/revoke",
    operation_id = "workspaces.admin_revoke_signup_code",
    tag = "workspaces",
    security(("admin" = ["admin:access"])),
    extensions(("x-campbooks-admin" = json!(AdminOperation::command::<SignupsReview>()))),
    params(("code_id" = SignupCodeId, Path, description = "The code.")),
    responses(
        (status = OK, description = "The code, revoked (also when it already was).",
            body = SignupCodeView),
        (status = UNAUTHORIZED, description = "`admin_reauth_required`: sign in again.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such code, or not an administrator with \
            `signups.review`.", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    _admin: Admin<SignupsReview>,
    State(state): State<AppState>,
    extract::Path(path): extract::Path<CodePath>,
) -> Result<Json<SignupCodeView>> {
    let row = sqlx::query_as!(
        CodeRow,
        r#"UPDATE workspaces.signup_codes
              SET revoked_at = coalesce(revoked_at, now())
            WHERE id = $1
        RETURNING id AS "id: SignupCodeId", label, max_uses, uses, expires_at,
                  created_by, created_at, revoked_at"#,
        path.code_id as SignupCodeId,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(Error::NotFound)?;
    Ok(Json(SignupCodeView::at(&row, Utc::now())))
}
