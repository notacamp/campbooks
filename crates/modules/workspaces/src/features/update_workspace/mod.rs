//! `PATCH /api/v1/w/{workspace_id}` — rename a workspace or change its
//! accent. Owners and admins.

use axum::{Json, extract::State};
use campbooks_kernel::authz::Action;
use campbooks_kernel::{
    AppState, Operation, Problem, Result, Risk,
    db::BeginFor as _,
    extract,
    tenancy::{Access, Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    domain::{
        access::{acting_user, authorize, lock_workspace, workspace_resource},
        model::{Accent, WorkspaceView, workspace_name},
    },
    events::WorkspaceUpdated,
    features::get_workspace,
    scopes::WorkspacesAdmin,
};

/// The settings to change; absent fields stay as they are.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateWorkspace {
    /// A new name, 1 to 100 characters (trimmed).
    #[serde(default)]
    pub name: Option<String>,
    /// A new colour.
    #[serde(default)]
    pub accent: Option<Accent>,
}

/// Update a workspace.
#[utoipa::path(
    patch,
    path = "/api/v1/w/{workspace_id}",
    operation_id = "workspaces.update",
    tag = "workspaces",
    security(("bearer" = ["workspaces:admin"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).records(["workspaces.workspace"]).access("workspaces.workspace", campbooks_kernel::authz::Action::Write)))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    request_body = UpdateWorkspace,
    responses(
        (status = OK, description = "The updated workspace.", body = WorkspaceView),
        (status = BAD_REQUEST, description = "Invalid name or body.", body = Problem,
            content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or not an owner or admin.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such workspace, or you are not a member.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<WorkspacesAdmin>,
    extract::Json(input): extract::Json<UpdateWorkspace>,
) -> Result<Json<WorkspaceView>> {
    execute(&state, &workspace.access, input).await.map(Json)
}

/// Apply `input` to the workspace of `access`.
pub async fn execute(
    state: &AppState,
    access: &Access,
    input: UpdateWorkspace,
) -> Result<WorkspaceView> {
    let (actor, _) = acting_user(access)?;
    let name = input.name.as_deref().map(workspace_name).transpose()?;
    let mut tx = state.db.begin_for(access.workspace).await?;
    authorize(
        state,
        &mut tx,
        access,
        Action::Write,
        &workspace_resource(access),
    )
    .await?;
    lock_workspace(&mut tx, access.workspace).await?;
    let changed = sqlx::query!(
        r#"WITH old AS (SELECT name, accent FROM workspaces.workspaces WHERE id = $1)
           UPDATE workspaces.workspaces w
              SET name = coalesce($2, w.name), accent = coalesce($3, w.accent)
             FROM old
            WHERE w.id = $1
        RETURNING w.name IS DISTINCT FROM old.name AS "name!",
                  w.accent IS DISTINCT FROM old.accent AS "accent!""#,
        access.workspace as WorkspaceId,
        name,
        input.accent.map(Accent::as_str),
    )
    .fetch_one(&mut *tx)
    .await?;
    let fields: Vec<String> = [("name", changed.name), ("accent", changed.accent)]
        .into_iter()
        .filter(|(_, changed)| *changed)
        .map(|(field, _)| field.to_owned())
        .collect();
    if !fields.is_empty() {
        state
            .events
            .publish(
                &mut tx,
                &WorkspaceUpdated {
                    workspace: access.workspace.to_string(),
                    fields,
                    updated_by: actor,
                },
            )
            .await?;
    }
    let view = get_workspace::load(&mut tx, access).await?;
    tx.commit().await?;
    Ok(view)
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use sqlx::PgPool;

    use crate::testkit::Kit;

    #[sqlx::test(migrations = "../../../migrations")]
    async fn admins_rename_and_recolour(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let ws = kit.create_workspace(&ana, "Shop").await;
        let bea = kit.join(&ana, ws, "bea", "admin").await;
        let uri = format!("/api/v1/w/{ws}");

        let response = kit
            .patch(
                &bea,
                &uri,
                serde_json::json!({ "name": " Corner shop ", "accent": "forest" }),
            )
            .await;
        assert_eq!(response.status, StatusCode::OK, "{:?}", response.json);
        assert_eq!(response.json["name"], "Corner shop");
        assert_eq!(response.json["accent"], "forest");
        assert_eq!(response.json["role"], "admin");

        let only_accent = kit
            .patch(&ana, &uri, serde_json::json!({ "accent": "business" }))
            .await;
        assert_eq!(only_accent.json["name"], "Corner shop");
        let bad = kit
            .patch(&ana, &uri, serde_json::json!({ "name": "" }))
            .await;
        assert_eq!(bad.status, StatusCode::BAD_REQUEST);

        // Two real changes, one event each, naming the settings, not values.
        let updates: Vec<String> = kit
            .events_of(&ws.to_string())
            .await
            .into_iter()
            .filter(|name| name == "workspaces.workspace_updated")
            .collect();
        assert_eq!(updates.len(), 2);
        let payload = kit.last_event_payload(&ws.to_string()).await;
        assert_eq!(payload["fields"], serde_json::json!(["accent"]));
        assert!(!payload.to_string().contains("Corner shop"));

        // Setting what is already there publishes nothing.
        kit.patch(&ana, &uri, serde_json::json!({ "accent": "business" }))
            .await;
        let after: usize = kit
            .events_of(&ws.to_string())
            .await
            .iter()
            .filter(|name| *name == "workspaces.workspace_updated")
            .count();
        assert_eq!(after, 2);
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn members_and_clients_may_not(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let ws = kit.create_workspace(&ana, "Shop").await;
        let cid = kit.join(&ana, ws, "cid", "member").await;
        let uri = format!("/api/v1/w/{ws}");
        let body = serde_json::json!({ "name": "Mine now" });
        assert_eq!(
            kit.patch(&cid, &uri, body.clone()).await.status,
            StatusCode::FORBIDDEN
        );
        let client = kit.client(Some(ws), "workspaces:admin");
        assert_eq!(
            kit.patch(&client, &uri, body).await.status,
            StatusCode::FORBIDDEN
        );
    }
}
