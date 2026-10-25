//! `POST /admin/v1/identity/accounts/{account_id}/unsuspend` — Admin →
//! People → Unsuspend: the person may sign in again. Nothing that was
//! ended comes back; they sign in afresh.
//!
//! An admin operation (ADR 0015). Requires `users.suspend` (or `super`).
//! Idempotent: unsuspending an account that is not suspended changes
//! nothing and answers the account as it is.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Problem, Result,
    admin::{Admin, AdminOperation, permission::UsersSuspend},
    extract,
};
use chrono::Utc;

use crate::{
    domain::{
        suspension::{self, Outcome},
        tracked,
    },
    features::admin_get_account::{self, AccountPath, AdminAccount},
    tracking,
};

/// Unsuspend an account.
#[utoipa::path(
    post,
    path = "/admin/v1/identity/accounts/{account_id}/unsuspend",
    operation_id = "identity.admin_unsuspend_account",
    tag = "identity",
    security(("admin" = ["admin:access"])),
    extensions(("x-campbooks-admin" = json!(AdminOperation::command::<UsersSuspend>()))),
    params(("account_id" = Uuid, Path, description = "The account (a token `sub`).")),
    responses(
        (status = OK, description = "The account, not suspended (now or before).",
            body = AdminAccount),
        (status = UNAUTHORIZED, description = "`admin_reauth_required`: sign in again.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such account, or not an administrator with \
            `users.suspend`.", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    admin: Admin<UsersSuspend>,
    State(state): State<AppState>,
    extract::Path(path): extract::Path<AccountPath>,
) -> Result<Json<AdminAccount>> {
    let now = Utc::now();
    let mut tx = state.db.begin().await?;
    let outcome = suspension::unsuspend(
        &mut tx,
        &state.events,
        path.account_id,
        admin.grant.account(),
        now,
    )
    .await?;
    if outcome == Outcome::Missing {
        return Err(Error::NotFound);
    }
    // A retry changes nothing, and tracks nothing.
    if outcome == Outcome::Changed {
        let account = tracked::account(&mut tx, path.account_id).await?;
        state
            .tracker
            .track(&mut tx, &tracking::AccountUnsuspended { account })
            .await?;
    }
    let account = admin_get_account::load(&mut tx, path.account_id, now)
        .await?
        .ok_or(Error::NotFound)?;
    tx.commit().await?;
    if outcome == Outcome::Changed {
        tracing::info!("an administrator unsuspended an account");
    }
    Ok(Json(account))
}
