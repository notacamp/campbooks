//! `GET /admin/v1/identity/accounts?ids=…` — Admin → People: whether each
//! person on a page of the list is suspended, in one request.
//!
//! An admin operation (ADR 0015). Requires `users.read` (or `super`). The
//! People list belongs to the workspaces module and suspension to this one,
//! so the client asks for the accounts of the page it shows (the
//! workspaces module's `account_id`s) and puts the two side by side. Each
//! account is its id and its suspension time: nothing else. Ids that are not
//! accounts here (another identity provider's subjects, deleted accounts)
//! are left out.

use std::collections::BTreeSet;

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Problem, Result,
    admin::{Admin, AdminOperation, permission::UsersRead},
    db, extract,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::MESSAGES;

/// Most accounts one request may ask about: a page of the People list.
pub const MAX_IDS: usize = 200;

/// The accounts asked about.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct AccountIds {
    /// Up to 200 account ids (the `sub` of their tokens), comma-separated.
    #[serde(default)]
    pub ids: String,
}

/// An account's standing, as the People list shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct AdminAccountStanding {
    /// The account (the `sub` of its tokens).
    pub id: Uuid,
    /// When an administrator suspended it; absent unless suspended.
    pub suspended_at: Option<DateTime<Utc>>,
}

/// The accounts asked about that exist here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct AdminAccountStandings {
    /// In no particular order.
    pub accounts: Vec<AdminAccountStanding>,
}

/// See whether accounts are suspended.
#[utoipa::path(
    get,
    path = "/admin/v1/identity/accounts",
    operation_id = "identity.admin_list_accounts",
    tag = "identity",
    security(("admin" = ["admin:access"])),
    extensions(("x-campbooks-admin" = json!(AdminOperation::query::<UsersRead>()))),
    params(AccountIds),
    responses(
        (status = OK, description = "The accounts asked about that exist here.",
            body = AdminAccountStandings),
        (status = BAD_REQUEST, description = "`too_many_accounts`: more than 200 ids.",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "`admin_reauth_required`: sign in again.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "Not an administrator with `users.read` (the same \
            answer as an unknown route).", body = Problem,
            content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    admin: Admin<UsersRead>,
    State(state): State<AppState>,
    extract::Query(query): extract::Query<AccountIds>,
) -> Result<Json<AdminAccountStandings>> {
    let ids = parse(&query.ids)?;
    if ids.is_empty() {
        return Ok(Json(AdminAccountStandings {
            accounts: Vec::new(),
        }));
    }
    let mut tx = db::begin_admin(&state.db, &admin.grant).await?;
    let accounts = sqlx::query_as::<_, (Uuid, Option<DateTime<Utc>>)>(
        "SELECT id, disabled_at FROM identity.accounts WHERE id = ANY($1)",
    )
    .bind(ids)
    .fetch_all(&mut *tx)
    .await?
    .into_iter()
    .map(|(id, suspended_at)| AdminAccountStanding { id, suspended_at })
    .collect();
    tx.commit().await?;
    Ok(Json(AdminAccountStandings { accounts }))
}

/// The distinct account ids in `ids`, skipping what is not a UUID (no
/// account here has such an id). 400 `too_many_accounts` beyond
/// [`MAX_IDS`].
fn parse(ids: &str) -> Result<Vec<Uuid>> {
    let parts: Vec<&str> = ids
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    if parts.len() > MAX_IDS {
        return Err(Error::BadRequest(
            MESSAGES.reason("too_many_accounts").arg("max", MAX_IDS),
        ));
    }
    let unique: BTreeSet<Uuid> = parts
        .into_iter()
        .filter_map(|part| part.parse().ok())
        .collect();
    Ok(unique.into_iter().collect())
}

#[cfg(test)]
mod tests;
