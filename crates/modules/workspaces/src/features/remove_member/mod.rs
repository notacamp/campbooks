//! `DELETE /api/v1/w/{workspace_id}/members/{user_id}` — remove a member.
//! Owners for anyone; admins for admins and members. A workspace always
//! keeps an owner.

use axum::{extract::State, http::StatusCode};
use campbooks_kernel::{
    AppState, Problem, Result, extract,
    tenancy::{Access, UserId, Workspace, WorkspaceId},
};

use crate::{
    domain::{
        access::acting_manager,
        departure::{Departure, depart},
        model::MemberPath,
    },
    scopes::WorkspacesAdmin,
};

/// Remove a member.
#[utoipa::path(
    delete,
    path = "/api/v1/w/{workspace_id}/members/{user_id}",
    operation_id = "workspaces.remove_member",
    tag = "workspaces",
    security(("bearer" = ["workspaces:admin"])),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("user_id" = UserId, Path, description = "The member."),
    ),
    responses(
        (status = NO_CONTENT, description = "Removed."),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, not an owner or admin, or an admin \
            removing an owner.", body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such workspace or member.", body = Problem,
            content_type = "application/problem+json"),
        (status = CONFLICT, description = "`last_owner`: the workspace would have no owner.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<WorkspacesAdmin>,
    extract::Path(path): extract::Path<MemberPath>,
) -> Result<StatusCode> {
    execute(&state, &workspace.access, path.user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Remove `member` from the workspace of `access`.
pub async fn execute(state: &AppState, access: &Access, member: UserId) -> Result<()> {
    let (actor, role) = acting_manager(access)?;
    let how = if actor == member {
        Departure::Leave
    } else {
        Departure::RemovedBy(actor, role)
    };
    depart(state, access.workspace, member, how).await
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use sqlx::PgPool;

    use crate::testkit::Kit;

    #[sqlx::test(migrations = "../../../migrations")]
    async fn removed_members_lose_access_and_it_is_published(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let ws = kit.create_workspace(&ana, "Shop").await;
        let bea = kit.join(&ana, ws, "bea", "admin").await;
        let cid = kit.join(&ana, ws, "cid", "member").await;
        let cid_id = kit.user_id("cid").await;

        let response = kit
            .delete(&bea, &format!("/api/v1/w/{ws}/members/{cid_id}"))
            .await;
        assert_eq!(response.status, StatusCode::NO_CONTENT);
        assert_eq!(
            kit.get(&cid, &format!("/api/v1/w/{ws}")).await.status,
            StatusCode::NOT_FOUND
        );
        let payload = kit.last_event_payload(&ws.to_string()).await;
        assert_eq!(payload["user"], cid_id.to_string());
        assert_eq!(payload["removed_by"], kit.user_id("bea").await.to_string());
        assert_eq!(
            kit.security_kinds(&ws.to_string()).await.last().unwrap(),
            "member_removed"
        );
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn owners_are_protected(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let ws = kit.create_workspace(&ana, "Shop").await;
        let bea = kit.join(&ana, ws, "bea", "admin").await;
        let ana_id = kit.user_id("ana").await;
        let uri = format!("/api/v1/w/{ws}/members/{ana_id}");

        assert_eq!(kit.delete(&bea, &uri).await.status, StatusCode::FORBIDDEN);
        let last = kit.delete(&ana, &uri).await;
        assert_eq!(last.status, StatusCode::CONFLICT);
        assert_eq!(last.json["detail"], "last_owner");
        let member = kit.join(&ana, ws, "cid", "member").await;
        let bea_id = kit.user_id("bea").await;
        assert_eq!(
            kit.delete(&member, &format!("/api/v1/w/{ws}/members/{bea_id}"))
                .await
                .status,
            StatusCode::FORBIDDEN
        );
    }
}
