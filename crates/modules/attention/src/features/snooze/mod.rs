//! `POST …/attention/{item_id}/snooze` and `/unsnooze` — "Later": a member
//! puts an item off until a time, for themselves only. A new cause (the
//! sender writes again) brings it back sooner.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, RecordRef, Result, Risk,
    authz::{self, Action},
    db::BeginFor as _,
    extract,
    tenancy::{Access, Workspace, WorkspaceId},
};
use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Transaction};
use utoipa::ToSchema;

use crate::{
    domain::{ItemId, ItemRow, ItemView, key, resources::ITEM_RECORD, visible_containers},
    events::ItemChanged,
    scopes::AttentionWrite,
};

/// `/api/v1/w/{workspace_id}/attention/{item_id}…`
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct ItemPath {
    /// The item.
    pub item_id: ItemId,
}

/// Until when.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Snooze {
    /// Later than now and less than a year away.
    pub until: DateTime<Utc>,
}

/// The longest snooze.
const LONGEST: TimeDelta = TimeDelta::days(366);

/// Put an item off until later, for yourself.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/attention/{item_id}/snooze",
    operation_id = "attention.snooze",
    tag = "attention",
    security(("bearer" = ["attention:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("attention.unsnooze")
        .automation()
        .assistant()
        .records(["attention.item"]).access("attention.item", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("item_id" = ItemId, Path, description = "The item."),
    ),
    request_body = Snooze,
    responses(
        (status = OK, description = "The item afterwards.", body = ItemView),
        (status = BAD_REQUEST, description = "A time in the past or more than a year away (`invalid_until`).",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or not a member (`snooze_needs_member`).",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such item, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn snooze(
    State(state): State<AppState>,
    workspace: Workspace<AttentionWrite>,
    extract::Path(path): extract::Path<ItemPath>,
    extract::Json(body): extract::Json<Snooze>,
) -> Result<Json<ItemView>> {
    let now = Utc::now();
    if body.until <= now || body.until > now + LONGEST {
        return Err(Error::BadRequest(crate::MESSAGES.reason("invalid_until")));
    }
    change(&state, &workspace.access, path.item_id, Some(body.until))
        .await
        .map(Json)
}

/// Bring an item you put off back now.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/attention/{item_id}/unsnooze",
    operation_id = "attention.unsnooze",
    tag = "attention",
    security(("bearer" = ["attention:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .automation()
        .assistant()
        .records(["attention.item"]).access("attention.item", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("item_id" = ItemId, Path, description = "The item."),
    ),
    responses(
        (status = OK, description = "The item afterwards.", body = ItemView),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or not a member (`snooze_needs_member`).",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such item, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn unsnooze(
    State(state): State<AppState>,
    workspace: Workspace<AttentionWrite>,
    extract::Path(path): extract::Path<ItemPath>,
) -> Result<Json<ItemView>> {
    change(&state, &workspace.access, path.item_id, None)
        .await
        .map(Json)
}

/// Snooze item `id` for the caller until `until`, or end their snooze.
async fn change(
    state: &AppState,
    access: &Access,
    id: ItemId,
    until: Option<DateTime<Utc>>,
) -> Result<ItemView> {
    let Some(user) = access.user else {
        return Err(Error::Denied(crate::MESSAGES.reason("snooze_needs_member")));
    };
    let mut tx = state.db.begin_for(access.workspace).await?;
    authz::require(
        state,
        &mut tx,
        access,
        Action::Write,
        &RecordRef::new(ITEM_RECORD, &id),
        || Error::Denied(crate::MESSAGES.reason("snooze_needs_member")),
    )
    .await?;
    let visible = visible_containers(state, &mut tx, access).await?;
    let item = sqlx::query!(
        r#"SELECT container_type, container_id FROM attention.items
            WHERE id = $1 AND state = 'open' FOR UPDATE"#,
        id as ItemId,
    )
    .fetch_optional(&mut *tx)
    .await?
    .filter(|item| {
        visible.contains(&key(&RecordRef::new(
            item.container_type.clone(),
            &item.container_id,
        )))
    })
    .ok_or(Error::NotFound)?;
    match until {
        Some(until) => {
            sqlx::query!(
                r#"
                INSERT INTO attention.snoozes (workspace_id, item_id, user_id, until)
                VALUES ($1, $2, $3, $4)
                ON CONFLICT (item_id, user_id) DO UPDATE
                   SET until = EXCLUDED.until, created_at = now()
                "#,
                access.workspace.as_uuid(),
                id as ItemId,
                user.as_uuid(),
                until,
            )
            .execute(&mut *tx)
            .await?;
        }
        None => {
            sqlx::query!(
                "DELETE FROM attention.snoozes WHERE item_id = $1 AND user_id = $2",
                id as ItemId,
                user.as_uuid(),
            )
            .execute(&mut *tx)
            .await?;
        }
    }
    let container = RecordRef::new(item.container_type, &item.container_id);
    announce(state, &mut tx, access, id, container).await?;
    let view = read(&mut tx, id, *user.as_uuid()).await?;
    tx.commit().await?;
    Ok(view)
}

async fn announce(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    access: &Access,
    item: ItemId,
    container: RecordRef,
) -> Result<()> {
    state
        .events
        .publish(
            tx,
            &ItemChanged {
                workspace: access.workspace.to_string(),
                item,
                container,
            },
        )
        .await?;
    Ok(())
}

async fn read(
    tx: &mut Transaction<'_, Postgres>,
    id: ItemId,
    user: sqlx::types::Uuid,
) -> Result<ItemView> {
    let row = sqlx::query_as!(
        ItemRow,
        r#"
        SELECT i.id AS "id: ItemId", i.record_type, i.record_id, i.container_type,
               i.container_id, i.cause_type, i.cause_id, i.reason AS "reason!", i.trust AS "trust!", i.due_on, i.urgent,
               i.since, i.state, i.cleared_because, i.cleared_by AS "cleared_by: _",
               i.cleared_at, s.until AS "snoozed_until?"
          FROM attention.items i
          LEFT JOIN attention.snoozes s ON s.item_id = i.id AND s.user_id = $2
         WHERE i.id = $1
        "#,
        id as ItemId,
        user,
    )
    .fetch_one(&mut **tx)
    .await?;
    row.view(Utc::now().date_naive())
}

#[cfg(test)]
mod tests;
