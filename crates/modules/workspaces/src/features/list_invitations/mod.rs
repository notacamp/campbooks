//! `GET /api/v1/w/{workspace_id}/invitations` — pending invitations, newest
//! first. Owners and admins.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Operation, Problem, Result,
    db::BeginFor as _,
    tenancy::{Access, Role, UserId, Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    domain::{access::acting_manager, invitation::InvitationView, model::InvitationId},
    scopes::WorkspacesAdmin,
};

/// Pending (not yet accepted) invitations, expired ones included.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct InvitationList {
    /// The invitations.
    pub invitations: Vec<InvitationView>,
}

/// List pending invitations.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/invitations",
    operation_id = "workspaces.list_invitations",
    tag = "workspaces",
    security(("bearer" = ["workspaces:admin"])),
    extensions(("x-campbooks" = json!(Operation::query().records(["workspaces.invitation"])))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    responses(
        (status = OK, description = "Pending invitations.", body = InvitationList),
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
) -> Result<Json<InvitationList>> {
    execute(&state, &workspace.access).await.map(Json)
}

/// The pending invitations of the workspace of `access`.
pub async fn execute(state: &AppState, access: &Access) -> Result<InvitationList> {
    acting_manager(access)?;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let rows = sqlx::query!(
        r#"SELECT id AS "id: InvitationId", email, role, invited_by AS "invited_by: UserId",
                  created_at, expires_at
             FROM workspaces.invitations
            WHERE workspace_id = $1 AND accepted_at IS NULL
            ORDER BY created_at DESC, id DESC"#,
        access.workspace as WorkspaceId,
    )
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    let invitations = rows
        .into_iter()
        .map(|row| {
            Ok(InvitationView {
                id: row.id,
                email: row.email,
                role: row.role.parse::<Role>()?,
                invited_by: row.invited_by,
                created_at: row.created_at,
                expires_at: row.expires_at,
            })
        })
        .collect::<Result<_>>()?;
    Ok(InvitationList { invitations })
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use sqlx::PgPool;

    use crate::testkit::Kit;

    #[sqlx::test(migrations = "../../../migrations")]
    async fn lists_pending_invitations_to_managers_only(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let ws = kit.create_workspace(&ana, "Shop").await;
        kit.invite(&ana, ws, "x@example.com", "member").await;
        let cid = kit.join(&ana, ws, "cid", "member").await;
        let uri = format!("/api/v1/w/{ws}/invitations");

        let response = kit.get(&ana, &uri).await;
        assert_eq!(response.status, StatusCode::OK);
        let emails: Vec<_> = response.json["invitations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["email"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(emails, ["x@example.com"]); // cid's was accepted
        assert_eq!(kit.get(&cid, &uri).await.status, StatusCode::FORBIDDEN);
    }
}
