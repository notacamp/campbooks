//! `POST /admin/v1/identity/accounts/{account_id}/suspend` — Admin →
//! People → Suspend: the person can no longer sign in by any way, and is
//! signed out everywhere (see [`suspension`](crate::domain::suspension)).
//!
//! An admin operation (ADR 0015). Requires `users.suspend` (or `super`).
//! Idempotent: suspending a suspended account changes nothing and answers
//! the account as it is. An account with an admin profile is refused with
//! 409 `account_is_admin`: the operator revokes the profile with the
//! server's CLI first (profiles are read through the kernel's
//! `AdminProfiles` port, never from here).

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Problem, Result,
    admin::{Admin, AdminOperation, permission::UsersSuspend},
    extract,
};
use chrono::Utc;

use crate::{
    MESSAGES,
    domain::suspension::{self, Outcome},
    features::admin_get_account::{self, AccountPath, AdminAccount},
};

/// Suspend an account.
#[utoipa::path(
    post,
    path = "/admin/v1/identity/accounts/{account_id}/suspend",
    operation_id = "identity.admin_suspend_account",
    tag = "identity",
    security(("admin" = ["admin:access"])),
    extensions(("x-campbooks-admin" = json!(AdminOperation::command::<UsersSuspend>()))),
    params(("account_id" = Uuid, Path, description = "The account (a token `sub`).")),
    responses(
        (status = OK, description = "The account, suspended (now or before).",
            body = AdminAccount),
        (status = UNAUTHORIZED, description = "`admin_reauth_required`: sign in again.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such account, or not an administrator with \
            `users.suspend`.", body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "`account_is_admin`: the account has an admin \
            profile; revoke it with `campbooks-server admin revoke` first.", body = Problem,
            content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    admin: Admin<UsersSuspend>,
    State(state): State<AppState>,
    extract::Path(path): extract::Path<AccountPath>,
) -> Result<Json<AdminAccount>> {
    let now = Utc::now();
    let mut tx = state.db.begin().await?;
    if admin_get_account::load(&mut tx, path.account_id, now)
        .await?
        .is_none()
    {
        return Err(Error::NotFound);
    }
    let profile = state
        .admin_profiles
        .permissions_of(&state.db, &path.account_id.to_string())
        .await?;
    if !profile.is_empty() {
        return Err(Error::Conflict(MESSAGES.reason("account_is_admin")));
    }
    let outcome = suspension::suspend(
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
    let account = admin_get_account::load(&mut tx, path.account_id, now)
        .await?
        .ok_or(Error::NotFound)?;
    tx.commit().await?;
    if outcome == Outcome::Changed {
        tracing::info!("an administrator suspended an account");
    }
    Ok(Json(account))
}

#[cfg(test)]
mod tests;
