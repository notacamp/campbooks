//! `DELETE /api/v1/w/{workspace_id}/members/{user_id}` — remove a member.
//! Owners for anyone; admins for admins and members. A workspace always
//! keeps an owner. `?mail=keep` keeps the mail of the mailboxes they
//! connected as read-only history; by default it is deleted with their
//! connections. Their inbox memberships and drafts go either way.

use axum::{extract::State, http::StatusCode};
use campbooks_kernel::{
    AppState, Operation, Problem, Result, Risk, extract,
    tenancy::{Access, UserId, Workspace, WorkspaceId},
};
use campbooks_kernel::authz::Action;
use campbooks_kernel::db::BeginFor as _;

use serde::Deserialize;
use utoipa::IntoParams;

use crate::{
    events::MailChoice,
    domain::{
        access::{acting_user, authorize, member_resource},
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
    extensions(("x-campbooks" = json!(Operation::command(Risk::High).records(["workspaces.member"]).access("workspaces.member", campbooks_kernel::authz::Action::Delete)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("user_id" = UserId, Path, description = "The member."),
        RemoveQuery,
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
    extract::Query(query): extract::Query<RemoveQuery>,
) -> Result<StatusCode> {
    execute(
        &state,
        &workspace.access,
        path.user_id,
        query.mail.unwrap_or_default(),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// What to do with the departing member's mail.
#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct RemoveQuery {
    /// `delete` (the default): the mailboxes they connected are
    /// disconnected and their mail deleted. `keep`: their mail stays as
    /// read-only history, no longer synced.
    pub mail: Option<MailChoice>,
}

/// Remove `member` from the workspace of `access`, deleting or keeping the
/// mail of the mailboxes they connected as `mail` says.
pub async fn execute(
    state: &AppState,
    access: &Access,
    member: UserId,
    mail: MailChoice,
) -> Result<()> {
    let (actor, role) = acting_user(access)?;
    let mut tx = state.db.begin_for(access.workspace).await?;
    authorize(state, &mut tx, access, Action::Delete, &member_resource(member)).await?;
    tx.commit().await?;
    let how = if actor == member {
        Departure::Leave
    } else {
        Departure::RemovedBy(actor, role, mail)
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
        assert_eq!(payload["mail"], "delete");
        assert_eq!(
            kit.security_kinds(&ws.to_string()).await.last().unwrap(),
            "member_removed"
        );
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn a_manager_may_keep_the_departing_members_mail(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let ws = kit.create_workspace(&ana, "Shop").await;
        kit.join(&ana, ws, "dee", "member").await;
        let dee_id = kit.user_id("dee").await;
        let response = kit
            .delete(&ana, &format!("/api/v1/w/{ws}/members/{dee_id}?mail=keep"))
            .await;
        assert_eq!(response.status, StatusCode::NO_CONTENT);
        let payload = kit.last_event_payload(&ws.to_string()).await;
        assert_eq!(payload["mail"], "keep");
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
        assert_eq!(last.json["code"], "last_owner");
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
