//! `GET` and `PUT /api/v1/w/{workspace_id}/assistant/profile` — the
//! caller's own profile: how Scout's own proposals start, and when the
//! questions were answered or put off. Private to its member: every query
//! is keyed by the caller's own id and no operation takes another's.
//!
//! Also the kernel's `MemberPreferences` port: other modules ask it whether
//! a member lets Scout act on what it is sure of and tell them afterwards.
//!
//! Tracked (ADR 0020): `assistant.profile_updated`, naming the settings that
//! changed; saving the values the profile already has tracks nothing.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Operation, Problem, Result, Risk,
    authz::Action,
    db::BeginFor as _,
    extract,
    preferences::{MemberPreferences, PreferenceFuture},
    tenancy::{UserId, Workspace, WorkspaceId},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use utoipa::ToSchema;

use crate::{
    domain::{Proactivity, resources},
    scopes::{AssistantRead, AssistantWrite},
    tracking,
};

/// What the member told Scout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ProfileView {
    /// How rules Scout proposes from your corrections start.
    pub proactivity: Proactivity,
    /// When you answered or put off the questions; `None` until then.
    pub asked_at: Option<DateTime<Utc>>,
    /// Grows by one on every change; 0 before the first.
    pub revision: u32,
}

/// The profile to keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ProfileUpdate {
    /// How rules Scout proposes from your corrections start.
    pub proactivity: Proactivity,
    /// Whether you answered or put off the questions (keeps the first time).
    pub asked: bool,
}

/// Your profile, or the defaults when you never answered.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/assistant/profile",
    operation_id = "assistant.get_profile",
    tag = "assistant",
    security(("bearer" = ["assistant:read"])),
    extensions(("x-campbooks" = json!(Operation::query().records(["assistant.profile"]).access("assistant.profile", campbooks_kernel::authz::Action::Read)))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    responses(
        (status = OK, description = "Your profile.", body = ProfileView),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or not a member (`profile_needs_member`).",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such workspace, or you are not a member.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn get(
    State(state): State<AppState>,
    workspace: Workspace<AssistantRead>,
) -> Result<Json<ProfileView>> {
    let mut tx = state.db.begin_for(workspace.access.workspace).await?;
    let user = resources::require_own(&state, &mut tx, &workspace.access, Action::Read).await?;
    let view = read(&mut tx, workspace.access.workspace, user).await?;
    tx.commit().await?;
    Ok(Json(view))
}

/// Keep your profile.
#[utoipa::path(
    put,
    path = "/api/v1/w/{workspace_id}/assistant/profile",
    operation_id = "assistant.update_profile",
    tag = "assistant",
    security(("bearer" = ["assistant:write"])),
    // Undo is another update with the profile it answered before.
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .records(["assistant.profile"]).access("assistant.profile", campbooks_kernel::authz::Action::Write)))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    request_body = ProfileUpdate,
    responses(
        (status = OK, description = "Your profile afterwards.", body = ProfileView),
        (status = BAD_REQUEST, description = "A value outside the allowed set.", body = Problem,
            content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or not a member (`profile_needs_member`).",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such workspace, or you are not a member.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn update(
    State(state): State<AppState>,
    workspace: Workspace<AssistantWrite>,
    extract::Json(body): extract::Json<ProfileUpdate>,
) -> Result<Json<ProfileView>> {
    let id = workspace.access.workspace;
    let mut tx = state.db.begin_for(id).await?;
    let user = resources::require_own(&state, &mut tx, &workspace.access, Action::Write).await?;
    let before = read(&mut tx, id, user).await?;
    sqlx::query!(
        r#"
        INSERT INTO assistant.profiles (workspace_id, user_id, proactivity, asked_at)
        VALUES ($1, $2, $3, CASE WHEN $4 THEN now() END)
        ON CONFLICT (workspace_id, user_id) DO UPDATE
           SET proactivity = EXCLUDED.proactivity,
               asked_at = CASE WHEN $4 THEN coalesce(assistant.profiles.asked_at, now()) END,
               revision = assistant.profiles.revision + 1,
               updated_at = now()
        "#,
        id.as_uuid(),
        user.as_uuid(),
        body.proactivity.as_str(),
        body.asked,
    )
    .execute(&mut *tx)
    .await?;
    let view = read(&mut tx, id, user).await?;
    let mut fields = Vec::new();
    if view.proactivity != before.proactivity {
        fields.push(tracking::ProfileSetting::Proactivity);
    }
    if view.asked_at.is_some() != before.asked_at.is_some() {
        fields.push(tracking::ProfileSetting::Asked);
    }
    if !fields.is_empty() {
        let event = tracking::ProfileUpdated {
            workspace: id,
            fields,
            profile: tracking::ProfileSnapshot {
                user_id: *user.as_uuid(),
                proactivity: match view.proactivity {
                    Proactivity::AskFirst => tracking::Proactivity::AskFirst,
                    Proactivity::ActThenTell => tracking::Proactivity::ActThenTell,
                },
                asked: view.asked_at.is_some(),
                asked_at: view.asked_at,
                revision: i64::from(view.revision),
            },
        };
        state.tracker.track(&mut tx, &event).await?;
    }
    tx.commit().await?;
    Ok(Json(view))
}

async fn read(
    conn: &mut PgConnection,
    workspace: WorkspaceId,
    user: UserId,
) -> Result<ProfileView> {
    let row = sqlx::query!(
        r#"SELECT proactivity, asked_at, revision FROM assistant.profiles
            WHERE workspace_id = $1 AND user_id = $2"#,
        workspace.as_uuid(),
        user.as_uuid(),
    )
    .fetch_optional(conn)
    .await?;
    Ok(row.map_or(
        ProfileView {
            proactivity: Proactivity::default(),
            asked_at: None,
            revision: 0,
        },
        |row| ProfileView {
            proactivity: Proactivity::from_db(&row.proactivity),
            asked_at: row.asked_at,
            revision: u32::try_from(row.revision).unwrap_or_default(),
        },
    ))
}

/// The `MemberPreferences` port over members' profiles.
#[derive(Debug, Clone, Copy, Default)]
pub struct Preferences;

impl MemberPreferences for Preferences {
    fn acts_without_asking<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        workspace: WorkspaceId,
        member: UserId,
    ) -> PreferenceFuture<'a> {
        Box::pin(async move {
            let view = read(tx, workspace, member).await?;
            Ok(view.proactivity == Proactivity::ActThenTell)
        })
    }
}

#[cfg(test)]
mod tests;
