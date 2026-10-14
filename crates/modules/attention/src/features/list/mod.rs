//! `GET /api/v1/w/{workspace_id}/attention` — what needs the caller, in the
//! order Home shows it; or, with `state=cleared`, what left their list today
//! (Done today).

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use chrono::{TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{
    domain::{ItemId, ItemRow, ItemView, OVERDUE_DAYS, Reason, visible_containers},
    scopes::AttentionRead,
};

/// Open items, or what left the list today.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ListState {
    /// What needs the caller now (the default).
    #[default]
    Open,
    /// What was cleared or put off since the start of the caller's day.
    Cleared,
}

/// Which items.
#[derive(Debug, Clone, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListQuery {
    /// `open` (default) or `cleared`.
    #[serde(default)]
    #[param(inline)]
    pub state: ListState,
    /// The caller's offset from UTC, in minutes (-840 to 840; default 0,
    /// UTC): where their day starts, for Done today and for what is overdue.
    #[serde(default)]
    pub utc_offset: i32,
}

/// The widest offset from UTC a time zone has (UTC+14:00), in minutes.
const MAX_OFFSET_MINUTES: i32 = 14 * 60;

/// How many open items there are for one reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ReasonCount {
    /// The reason.
    pub reason: Reason,
    /// Open items with it.
    pub count: u32,
}

/// The caller's items.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct AttentionList {
    /// Open: by reason (review, reply, decide, do, pay), then overdue, by
    /// due date, urgent first, oldest first. Cleared: newest first.
    pub items: Vec<ItemView>,
    /// Open items per reason, every reason listed (open lists only; empty
    /// for cleared).
    pub counts: Vec<ReasonCount>,
}

/// List what needs you.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/attention",
    operation_id = "attention.list",
    tag = "attention",
    security(("bearer" = ["attention:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant().records(["attention.item"])))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace."), ListQuery),
    responses(
        (status = OK, description = "The caller's items.", body = AttentionList),
        (status = BAD_REQUEST, description = "Not a state, or an offset beyond 14 hours (`invalid_offset`).", body = Problem,
            content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such workspace, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<AttentionRead>,
    extract::Query(query): extract::Query<ListQuery>,
) -> Result<Json<AttentionList>> {
    if query.utc_offset.abs() > MAX_OFFSET_MINUTES {
        return Err(Error::BadRequest(crate::MESSAGES.reason("invalid_offset")));
    }
    let access = &workspace.access;
    let now = Utc::now();
    let offset = TimeDelta::minutes(i64::from(query.utc_offset));
    // The caller's own today.
    let today = (now + offset).date_naive();
    let mut tx = state.db.begin_for(access.workspace).await?;
    let visible = visible_containers(&state, &mut tx, access).await?;
    let user = access.user.map(|u| *u.as_uuid());
    let rows = match query.state {
        ListState::Open => {
            sqlx::query_as!(
                ItemRow,
                r#"
                SELECT i.id AS "id: ItemId", i.record_type, i.record_id, i.container_type,
                       i.container_id, i.cause_type, i.cause_id, i.reason AS "reason!", i.trust AS "trust!", i.due_on,
                       i.urgent, i.since, i.state, i.cleared_because,
                       i.cleared_by AS "cleared_by: _", i.cleared_at,
                       NULL::timestamptz AS "snoozed_until?"
                  FROM attention.items i
                 WHERE i.workspace_id = $1 AND i.state = 'open'
                   AND i.container_type || ':' || i.container_id = ANY($2)
                   AND (i.due_on IS NULL OR i.due_on >= $3)
                   AND NOT EXISTS (SELECT 1 FROM attention.snoozes s
                                    WHERE s.item_id = i.id AND s.user_id = $4 AND s.until > $5)
                 ORDER BY CASE i.reason WHEN 'review' THEN 0 WHEN 'reply' THEN 1
                                        WHEN 'decide' THEN 2 WHEN 'do' THEN 3 ELSE 4 END,
                          (i.due_on IS NOT NULL AND i.due_on < $6) DESC,
                          i.due_on ASC NULLS LAST, i.urgent DESC, i.since ASC, i.id
                "#,
                access.workspace.as_uuid(),
                &visible,
                today - TimeDelta::days(OVERDUE_DAYS),
                user,
                now,
                today,
            )
            .fetch_all(&mut *tx)
            .await?
        }
        ListState::Cleared => {
            // Midnight where the caller is, as an instant.
            let start = today.and_hms_opt(0, 0, 0).unwrap_or_default().and_utc() - offset;
            sqlx::query_as!(
                ItemRow,
                r#"
                SELECT i.id AS "id: ItemId", i.record_type, i.record_id, i.container_type,
                       i.container_id, i.cause_type, i.cause_id, i.reason AS "reason!", i.trust AS "trust!", i.due_on,
                       i.urgent, i.since, i.state, i.cleared_because,
                       i.cleared_by AS "cleared_by: _", i.cleared_at,
                       s.until AS "snoozed_until?"
                  FROM attention.items i
                  LEFT JOIN attention.snoozes s
                         ON s.item_id = i.id AND s.user_id = $3 AND s.until > $4
                 WHERE i.workspace_id = $1
                   AND i.container_type || ':' || i.container_id = ANY($2)
                   AND i.reason IS NOT NULL
                   AND ((i.state = 'cleared' AND i.cleared_at >= $5
                         AND i.cleared_because IS DISTINCT FROM 'superseded')
                        OR (i.state = 'open' AND s.created_at >= $5))
                 ORDER BY greatest(i.cleared_at, s.created_at) DESC, i.id
                "#,
                access.workspace.as_uuid(),
                &visible,
                user,
                now,
                start,
            )
            .fetch_all(&mut *tx)
            .await?
        }
    };
    tx.commit().await?;
    let items = rows
        .into_iter()
        .map(|row| row.view(today))
        .collect::<Result<Vec<_>>>()?;
    let counts = match query.state {
        ListState::Open => Reason::ALL
            .into_iter()
            .map(|reason| ReasonCount {
                reason,
                count: u32::try_from(items.iter().filter(|i| i.reason == reason).count())
                    .unwrap_or(u32::MAX),
            })
            .collect(),
        ListState::Cleared => Vec::new(),
    };
    Ok(Json(AttentionList { items, counts }))
}

#[cfg(test)]
mod tests;
