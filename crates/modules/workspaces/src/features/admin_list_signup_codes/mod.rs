//! `GET /admin/v1/workspaces/signup-codes` — the single-use signup codes
//! administrators minted, newest first. Never the codes themselves: only
//! their digests are stored.
//!
//! An admin operation (ADR 0015): requires `signups.review`.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Problem, Result,
    admin::{Admin, AdminOperation, permission::SignupsReview},
    extract,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::domain::{
    model::SignupCodeId,
    page::{Cursor, limit},
    signup::{CodeRow, SignupCodeView},
};

/// Which codes to return.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct Page {
    /// Where the previous page ended (its `next`).
    pub after: Option<String>,
    /// How many: 1 to 200, default 50.
    pub limit: Option<u16>,
}

/// A page of signup codes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SignupCodeList {
    /// The codes, newest first.
    pub codes: Vec<SignupCodeView>,
    /// Pass as `after` for the next page; `null` on the last one.
    pub next: Option<String>,
}

/// List signup codes.
#[utoipa::path(
    get,
    path = "/admin/v1/workspaces/signup-codes",
    operation_id = "workspaces.admin_list_signup_codes",
    tag = "workspaces",
    security(("admin" = ["admin:access"])),
    extensions(("x-campbooks-admin" = json!(AdminOperation::query::<SignupsReview>()))),
    params(Page),
    responses(
        (status = OK, description = "A page of codes.", body = SignupCodeList),
        (status = BAD_REQUEST, description = "Invalid limit or cursor.", body = Problem,
            content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "`admin_reauth_required`: sign in again.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "Not an administrator with `signups.review` (the same \
            answer as an unknown route).", body = Problem,
            content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    _admin: Admin<SignupsReview>,
    State(state): State<AppState>,
    extract::Query(page): extract::Query<Page>,
) -> Result<Json<SignupCodeList>> {
    let size = limit(page.limit)?;
    let after = page
        .after
        .as_deref()
        .map(Cursor::<SignupCodeId>::decode)
        .transpose()?;
    // Codes are instance policy, not workspace data: no row-level security.
    let rows = sqlx::query_as!(
        CodeRow,
        r#"SELECT id AS "id: SignupCodeId", label, max_uses, uses, expires_at,
                  created_by, created_at, revoked_at
             FROM workspaces.signup_codes
            WHERE $1::timestamptz IS NULL
               OR created_at < $1
               OR (created_at = $1 AND id < $2)
            ORDER BY created_at DESC, id DESC
            LIMIT $3"#,
        after.map(|cursor| cursor.at) as Option<DateTime<Utc>>,
        after.map(|cursor| cursor.id) as Option<SignupCodeId>,
        i64::from(size) + 1,
    )
    .fetch_all(&state.db)
    .await?;
    let now = Utc::now();
    let more = rows.len() > usize::from(size);
    let codes: Vec<SignupCodeView> = rows
        .iter()
        .take(usize::from(size))
        .map(|row| SignupCodeView::at(row, now))
        .collect();
    let next = if more {
        codes.last().map(|code| {
            Cursor {
                rank: 0,
                at: code.created_at,
                id: code.id,
            }
            .encode()
        })
    } else {
        None
    };
    Ok(Json(SignupCodeList { codes, next }))
}
