//! `GET /api/v1/w/{workspace_id}/members` — who belongs to a workspace.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Operation, Problem, Result,
    db::BeginFor as _,
    tenancy::{Access, Role, UserId, Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use utoipa::ToSchema;

use crate::{domain::model::MemberView, scopes::WorkspacesRead};

/// The members, in the order they joined.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MemberList {
    /// The members.
    pub members: Vec<MemberView>,
}

/// List a workspace's members.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/members",
    operation_id = "workspaces.list_members",
    tag = "workspaces",
    security(("bearer" = ["workspaces:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant().records(["workspaces.member"])))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    responses(
        (status = OK, description = "The members.", body = MemberList),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such workspace, or you are not a member.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<WorkspacesRead>,
) -> Result<Json<MemberList>> {
    execute(&state, &workspace.access).await.map(Json)
}

/// The members of the workspace of `access`.
pub async fn execute(state: &AppState, access: &Access) -> Result<MemberList> {
    let mut tx = state.db.begin_for(access.workspace).await?;
    let members = load(&mut tx, access.workspace, None).await?;
    tx.commit().await?;
    Ok(MemberList { members })
}

/// The members of `workspace` (only `user`, when given).
pub async fn load(
    conn: &mut PgConnection,
    workspace: WorkspaceId,
    user: Option<UserId>,
) -> Result<Vec<MemberView>> {
    sqlx::query!(
        r#"
        SELECT m.user_id AS "user_id: UserId", u.display_name,
               CASE WHEN u.email_verified THEN u.email END AS email, m.role, m.joined_at
          FROM workspaces.members m
          JOIN workspaces.users u ON u.id = m.user_id
         WHERE m.workspace_id = $1 AND ($2::uuid IS NULL OR m.user_id = $2)
         ORDER BY m.joined_at, m.user_id
        "#,
        workspace as WorkspaceId,
        user as Option<UserId>,
    )
    .fetch_all(conn)
    .await?
    .into_iter()
    .map(|row| {
        Ok(MemberView {
            user_id: row.user_id,
            display_name: row.display_name,
            email: row.email,
            role: row.role.parse::<Role>()?,
            joined_at: row.joined_at,
        })
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use sqlx::PgPool;

    use crate::testkit::Kit;

    #[sqlx::test(migrations = "../../../migrations")]
    async fn lists_members_in_join_order(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let ws = kit.create_workspace(&ana, "Shop").await;
        let bea = kit.join(&ana, ws, "bea", "admin").await;
        kit.create_workspace(&kit.user("eve"), "Elsewhere").await;

        let response = kit.get(&bea, &format!("/api/v1/w/{ws}/members")).await;
        assert_eq!(response.status, StatusCode::OK);
        let members = response.json["members"].as_array().unwrap();
        let summary: Vec<_> = members
            .iter()
            .map(|m| (m["email"].as_str().unwrap(), m["role"].as_str().unwrap()))
            .collect();
        assert_eq!(
            summary,
            [("ana@example.com", "owner"), ("bea@example.com", "admin")]
        );
        assert_eq!(members[0]["display_name"], "ana");
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn an_address_nobody_verified_is_never_shown(db: PgPool) {
        let kit = Kit::new(db).await;
        let claimed = kit.user_with("dan", "ceo@example.com", false);
        let ws = kit.create_workspace(&claimed, "Shop").await;
        let path = format!("/api/v1/w/{ws}/members");
        let listed = kit.get(&claimed, &path).await;
        assert_eq!(listed.status, StatusCode::OK);
        assert_eq!(listed.json["members"][0]["email"], serde_json::Value::Null);
        assert_eq!(listed.json["members"][0]["display_name"], "dan");

        // The provider verifies it at a later sign-in (the app lists the
        // person's workspaces first): now it shows.
        let verified = kit.user_with("dan", "ceo@example.com", true);
        kit.get(&verified, "/api/v1/workspaces").await;
        let listed = kit.get(&verified, &path).await;
        assert_eq!(listed.json["members"][0]["email"], "ceo@example.com");
    }
}
