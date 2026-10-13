//! `DELETE /api/v1/w/{workspace_id}/membership` — leave a workspace. The
//! last owner cannot leave; they delete the workspace or make someone else
//! an owner first.

use axum::{extract::State, http::StatusCode};
use campbooks_kernel::{
    AppState, Operation, Problem, Result, Risk,
    tenancy::{Access, Workspace, WorkspaceId},
};
use campbooks_kernel::authz::Action;
use campbooks_kernel::db::BeginFor as _;

use crate::{
    domain::{
        access::{acting_user, authorize, member_resource},
        departure::{Departure, depart},
    },
    scopes::WorkspacesWrite,
};

/// Leave a workspace.
#[utoipa::path(
    delete,
    path = "/api/v1/w/{workspace_id}/membership",
    operation_id = "workspaces.leave",
    tag = "workspaces",
    security(("bearer" = ["workspaces:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::High).records(["workspaces.member"]).access("workspaces.member", campbooks_kernel::authz::Action::Delete)))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    responses(
        (status = NO_CONTENT, description = "You are no longer a member."),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or a client token.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such workspace, or you are not a member.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "`last_owner`: you are its only owner.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<WorkspacesWrite>,
) -> Result<StatusCode> {
    execute(&state, &workspace.access).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// The user of `access` leaves its workspace.
pub async fn execute(state: &AppState, access: &Access) -> Result<()> {
    let (user, _) = acting_user(access)?;
    let mut tx = state.db.begin_for(access.workspace).await?;
    authorize(state, &mut tx, access, Action::Delete, &member_resource(user)).await?;
    tx.commit().await?;
    depart(state, access.workspace, user, Departure::Leave).await
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use sqlx::PgPool;

    use crate::testkit::Kit;

    #[sqlx::test(migrations = "../../../migrations")]
    async fn members_leave_but_the_last_owner_cannot(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let ws = kit.create_workspace(&ana, "Shop").await;
        let bea = kit.join(&ana, ws, "bea", "member").await;
        let uri = format!("/api/v1/w/{ws}/membership");

        let left = kit.delete(&bea, &uri).await;
        assert_eq!(left.status, StatusCode::NO_CONTENT);
        assert_eq!(
            kit.get(&bea, &format!("/api/v1/w/{ws}")).await.status,
            StatusCode::NOT_FOUND
        );
        let payload = kit.last_event_payload(&ws.to_string()).await;
        assert_eq!(payload["removed_by"], serde_json::Value::Null);
        assert_eq!(payload["mail"], "delete");
        assert_eq!(
            kit.security_kinds(&ws.to_string()).await.last().unwrap(),
            "member_left"
        );

        let last = kit.delete(&ana, &uri).await;
        assert_eq!(last.status, StatusCode::CONFLICT);
        assert_eq!(last.json["code"], "last_owner");
        assert_eq!(last.json["title"], "Conflict");

        // Once Ana saves a language, her errors speak it; the code stays.
        sqlx::query("UPDATE workspaces.users SET locale = 'pt-PT' WHERE idp_subject = 'ana'")
            .execute(&kit.db)
            .await
            .unwrap();
        let last = kit.delete(&ana, &uri).await;
        assert_eq!(last.json["code"], "last_owner");
        assert_eq!(last.json["title"], "Conflito");
        assert_eq!(
            last.json["detail"],
            crate::MESSAGES.t(
                campbooks_kernel::i18n::Locale::PtPt,
                "errors.last_owner",
                &[]
            )
        );
    }
}
