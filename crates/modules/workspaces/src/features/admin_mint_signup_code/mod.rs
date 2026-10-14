//! `POST /admin/v1/workspaces/signup-codes` — mint a signup code. The code
//! is in the response and nowhere else: only its SHA-256 is stored, so it
//! cannot be shown again.
//!
//! An admin operation (ADR 0015): requires `signups.review`.

use axum::{Json, extract::State, http::StatusCode};
use campbooks_kernel::{
    AppState, Error, Problem, Result,
    admin::{Admin, AdminOperation, permission::SignupsReview},
    extract,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    MESSAGES,
    domain::{
        model::SignupCodeId,
        signup::{CodeRow, SignupCodeView, new_code},
    },
};

/// Longest label.
const MAX_LABEL: usize = 100;
/// Most uses one code can allow.
const MAX_USES: i32 = 100_000;

/// A new signup code.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MintSignupCode {
    /// What it is for (who it goes to), 1 to 100 characters.
    #[schema(example = "Beta tester, Ana")]
    pub label: String,
    /// How many times it can be redeemed: 1 to 100000, default 1.
    #[serde(default)]
    pub max_uses: Option<i32>,
    /// After this it cannot be redeemed; must be in the future. Never
    /// expires when absent.
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
}

/// A freshly minted code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MintedSignupCode {
    /// The code to hand over, like `K7QM-2XWP-9DHE-RT4A`. Shown here once.
    #[schema(example = "K7QM-2XWP-9DHE-RT4A")]
    pub code: String,
    /// The stored code (without the code itself).
    pub entry: SignupCodeView,
}

/// Mint a signup code.
#[utoipa::path(
    post,
    path = "/admin/v1/workspaces/signup-codes",
    operation_id = "workspaces.admin_mint_signup_code",
    tag = "workspaces",
    security(("admin" = ["admin:access"])),
    extensions(("x-campbooks-admin" = json!(AdminOperation::command::<SignupsReview>()))),
    request_body = MintSignupCode,
    responses(
        (status = CREATED, description = "Minted. The response is the only place the code \
            appears.", body = MintedSignupCode),
        (status = BAD_REQUEST, description = "Invalid label, number of uses or expiry.",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "`admin_reauth_required`: sign in again.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "Not an administrator with `signups.review` (the same \
            answer as an unknown route).", body = Problem,
            content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    admin: Admin<SignupsReview>,
    State(state): State<AppState>,
    extract::Json(input): extract::Json<MintSignupCode>,
) -> Result<(StatusCode, Json<MintedSignupCode>)> {
    let label = input.label.trim();
    if label.is_empty() || label.chars().count() > MAX_LABEL {
        return Err(Error::BadRequest(
            MESSAGES.reason("invalid_label").arg("max", MAX_LABEL),
        ));
    }
    let max_uses = input.max_uses.unwrap_or(1);
    if !(1..=MAX_USES).contains(&max_uses) {
        return Err(Error::BadRequest(
            MESSAGES.reason("invalid_max_uses").arg("max", MAX_USES),
        ));
    }
    let now = Utc::now();
    if input.expires_at.is_some_and(|at| at <= now) {
        return Err(Error::BadRequest(MESSAGES.reason("invalid_expiry")));
    }
    let (code, digest) = new_code()?;
    let id = SignupCodeId::new();
    let row = sqlx::query_as!(
        CodeRow,
        r#"INSERT INTO workspaces.signup_codes
               (id, digest, label, max_uses, expires_at, created_by)
           VALUES ($1, $2, $3, $4, $5, $6)
           RETURNING id AS "id: SignupCodeId", label, max_uses, uses, expires_at,
                     created_by, created_at, revoked_at"#,
        id as SignupCodeId,
        digest,
        label,
        max_uses,
        input.expires_at,
        admin.grant.account(),
    )
    .fetch_one(&state.db)
    .await?;
    tracing::info!(code = %id, "signup code minted");
    Ok((
        StatusCode::CREATED,
        Json(MintedSignupCode {
            code,
            entry: SignupCodeView::at(&row, now),
        }),
    ))
}
