//! `GET /admin/v1/workspaces/signup-requests` — who asked to create a
//! workspace on an instance that approves signups, pending first, then the
//! decided ones newest first.
//!
//! An admin operation (ADR 0015): requires `signups.review`. Reads across
//! users inside `begin_admin`, on a table with an admin-read policy.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Problem, Result,
    admin::{Admin, AdminOperation, permission::SignupsReview},
    db::begin_admin,
    extract,
    tenancy::UserId,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::domain::{
    page::{Cursor, limit},
    signup::{RequestStatus, SignupRequestView},
};

/// Which requests to return.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct Filter {
    /// Only requests in this state.
    pub status: Option<RequestStatus>,
    /// Where the previous page ended (its `next`).
    pub after: Option<String>,
    /// How many: 1 to 200, default 50.
    pub limit: Option<u16>,
}

/// A page of signup requests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SignupRequestList {
    /// The requests: pending first, then newest first.
    pub requests: Vec<SignupRequestView>,
    /// Pass as `after` for the next page; `null` on the last one.
    pub next: Option<String>,
}

/// List signup requests.
#[utoipa::path(
    get,
    path = "/admin/v1/workspaces/signup-requests",
    operation_id = "workspaces.admin_list_signup_requests",
    tag = "workspaces",
    security(("admin" = ["admin:access"])),
    extensions(("x-campbooks-admin" = json!(AdminOperation::query::<SignupsReview>()))),
    params(Filter),
    responses(
        (status = OK, description = "A page of requests.", body = SignupRequestList),
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
    admin: Admin<SignupsReview>,
    State(state): State<AppState>,
    extract::Query(filter): extract::Query<Filter>,
) -> Result<Json<SignupRequestList>> {
    let size = limit(filter.limit)?;
    let after = filter
        .after
        .as_deref()
        .map(Cursor::<UserId>::decode)
        .transpose()?;
    let status = filter.status.map(RequestStatus::as_str);
    let mut tx = begin_admin(&state.db, &admin.grant).await?;
    let mut rows = sqlx::query!(
        r#"SELECT r.user_id AS "user_id: UserId", u.display_name, u.email, r.status,
                  r.requested_at, r.decided_at, r.decided_by, r.reason,
                  (r.status <> 'pending')::int AS "rank!"
             FROM workspaces.signup_requests r
             JOIN workspaces.users u ON u.id = r.user_id
            WHERE ($1::text IS NULL OR r.status = $1)
              AND ($2::int IS NULL
                   OR (r.status <> 'pending')::int > $2
                   OR ((r.status <> 'pending')::int = $2
                       AND (r.requested_at < $3
                            OR (r.requested_at = $3 AND r.user_id < $4))))
            ORDER BY "rank!", r.requested_at DESC, r.user_id DESC
            LIMIT $5"#,
        status,
        after.map(|cursor| cursor.rank),
        after.map(|cursor| cursor.at) as Option<DateTime<Utc>>,
        after.map(|cursor| cursor.id) as Option<UserId>,
        i64::from(size) + 1,
    )
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    let next = if rows.len() > usize::from(size) {
        rows.truncate(usize::from(size));
        rows.last().map(|row| {
            Cursor {
                rank: row.rank,
                at: row.requested_at,
                id: row.user_id,
            }
            .encode()
        })
    } else {
        None
    };
    let requests = rows
        .into_iter()
        .map(|row| {
            Ok(SignupRequestView {
                user_id: row.user_id,
                display_name: row.display_name,
                email: row.email,
                status: RequestStatus::parse(&row.status)?,
                requested_at: row.requested_at,
                decided_at: row.decided_at,
                decided_by: row.decided_by,
                reason: row.reason,
            })
        })
        .collect::<Result<_>>()?;
    Ok(Json(SignupRequestList { requests, next }))
}
