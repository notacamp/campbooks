//! `GET /admin/v1/system/ai-usage` — a day's AI usage per workspace and the
//! daily cap (campbooks-cloud#152), from the ledger the
//! [`AiUsageMeter`](crate::AiUsageMeter) keeps.
//!
//! Ids and counts only: workspace names belong to the `workspaces` module,
//! and the ledger holds no content. Requires `system.read` (or `super`).

use axum::{Extension, Json, extract::State};
use campbooks_kernel::{
    AppState, Result,
    admin::{Admin, AdminOperation, permission::SystemRead},
    db,
    extract::Query,
    tenancy::WorkspaceId,
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{AiUsageMeter, Deps, features::admin_overview::AiRole};

/// Workspaces listed when no `limit` is given.
const DEFAULT_LIMIT: i64 = 100;
/// The most workspaces one read lists.
const MAX_LIMIT: i64 = 500;

/// Which day to read.
#[derive(Debug, Deserialize, IntoParams)]
#[serde(deny_unknown_fields)]
pub struct UsageQuery {
    /// The UTC day (`YYYY-MM-DD`); today when absent.
    day: Option<NaiveDate>,
    /// Workspaces to list, 1 to 500 (default 100).
    limit: Option<i64>,
}

/// A day's AI usage across the instance.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AiUsageDay {
    /// The UTC day.
    pub day: NaiveDate,
    /// When that day's counts start again: 00:00 UTC the next day.
    pub resets_at: DateTime<Utc>,
    /// Model calls a workspace may make per UTC day
    /// (`CAMPBOOKS_AI__DAILY_CALLS_PER_WORKSPACE`); `null` when there is no
    /// cap.
    pub cap: Option<u32>,
    /// The workspaces that used AI that day: those the cap refused first,
    /// then by calls.
    pub workspaces: Vec<WorkspaceAiUsage>,
}

/// One workspace's AI usage on the day.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct WorkspaceAiUsage {
    /// The workspace.
    pub workspace_id: WorkspaceId,
    /// Model calls made (admitted).
    pub calls: i64,
    /// Calls refused over the cap.
    pub refused: i64,
    /// Whether the cap refused any of its calls that day.
    pub capped: bool,
    /// The first refusal, when there was one.
    pub capped_at: Option<DateTime<Utc>>,
    /// Calls left today; `null` for another day or without a cap.
    pub remaining: Option<i64>,
    /// Its calls by role and feature.
    pub uses: Vec<AiUse>,
}

/// Calls of one role for one feature.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AiUse {
    /// The AI role called.
    pub role: AiRole,
    /// What asked (`conversations.triage`).
    #[schema(example = "conversations.triage")]
    pub feature: String,
    /// Calls made.
    pub calls: i64,
    /// Of which failed (no answer, or an unusable one).
    pub failed: i64,
    /// Prompt tokens the providers reported (0 when they report none).
    pub prompt_tokens: i64,
    /// Completion tokens the providers reported.
    pub completion_tokens: i64,
}

/// A day's AI usage per workspace, workspaces the cap refused first.
#[utoipa::path(
    get,
    path = "/admin/v1/system/ai-usage",
    operation_id = "system.admin_ai_usage",
    tag = "system",
    params(UsageQuery),
    security(("admin" = ["admin:access"])),
    extensions(("x-campbooks-admin" = json!(AdminOperation::query::<SystemRead>()))),
    responses(
        (status = OK, description = "The day's usage and the cap.", body = AiUsageDay),
        (status = BAD_REQUEST, description = "Malformed query.",
            body = campbooks_kernel::Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "`admin_reauth_required`: sign in again.",
            body = campbooks_kernel::Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "Not an administrator with `system.read`.",
            body = campbooks_kernel::Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    admin: Admin<SystemRead>,
    State(state): State<AppState>,
    Extension(deps): Extension<Deps>,
    Query(query): Query<UsageQuery>,
) -> Result<Json<AiUsageDay>> {
    let meter = &deps.ai_usage;
    let today = meter.now().date_naive();
    let day = query.day.unwrap_or(today);
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let mut tx = db::begin_admin(&state.db, &admin.grant).await?;
    let days = sqlx::query!(
        r#"SELECT workspace_id AS "workspace_id: WorkspaceId", calls, refused, capped_at
             FROM system.ai_usage_days
            WHERE day = $1
            ORDER BY refused > 0 DESC, calls DESC, workspace_id
            LIMIT $2"#,
        day,
        limit,
    )
    .fetch_all(&mut *tx)
    .await?;
    let ids: Vec<sqlx::types::Uuid> = days.iter().map(|d| *d.workspace_id.as_uuid()).collect();
    let uses = sqlx::query!(
        r#"SELECT workspace_id AS "workspace_id: WorkspaceId", role, feature, calls, failed,
                  prompt_tokens, completion_tokens
             FROM system.ai_usage
            WHERE day = $1 AND workspace_id = ANY($2)
            ORDER BY workspace_id, calls DESC, role, feature"#,
        day,
        &ids,
    )
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;

    let cap = meter.cap();
    let workspaces = days
        .into_iter()
        .map(|d| WorkspaceAiUsage {
            workspace_id: d.workspace_id,
            calls: i64::from(d.calls),
            refused: i64::from(d.refused),
            capped: d.refused > 0,
            capped_at: d.capped_at,
            remaining: cap
                .filter(|_| day == today)
                .map(|cap| (i64::from(cap) - i64::from(d.calls)).max(0)),
            uses: uses
                .iter()
                .filter(|u| u.workspace_id == d.workspace_id)
                .filter_map(|u| {
                    Some(AiUse {
                        role: role(&u.role)?,
                        feature: u.feature.clone(),
                        calls: i64::from(u.calls),
                        failed: i64::from(u.failed),
                        prompt_tokens: u.prompt_tokens,
                        completion_tokens: u.completion_tokens,
                    })
                })
                .collect(),
        })
        .collect();
    Ok(Json(AiUsageDay {
        day,
        resets_at: AiUsageMeter::resets_at(day),
        cap,
        workspaces,
    }))
}

/// The role a ledger row names (the table's check allows these only).
fn role(name: &str) -> Option<AiRole> {
    match name {
        "decide" => Some(AiRole::Decide),
        "embed" => Some(AiRole::Embed),
        "complete" => Some(AiRole::Complete),
        "vision" => Some(AiRole::Vision),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
