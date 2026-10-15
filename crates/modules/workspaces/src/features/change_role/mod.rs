//! `PATCH /api/v1/w/{workspace_id}/members/{user_id}` — change a member's
//! role. Owners for anything; admins for admins and members only. A
//! workspace always keeps an owner.

use axum::{Json, extract::State};
use campbooks_kernel::authz::Action;
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    db::BeginFor as _,
    extract,
    tenancy::{Access, Role, UserId, Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    domain::{
        access::{acting_user, authorize, keep_an_owner, lock_workspace, member_resource, role_of},
        model::{MemberPath, MemberView},
        security::{self, SecurityKind},
    },
    events::MemberRoleChanged,
    features::list_members,
    scopes::WorkspacesAdmin,
};

/// The new role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ChangeRole {
    /// The member's new role.
    pub role: Role,
}

/// Change a member's role.
#[utoipa::path(
    patch,
    path = "/api/v1/w/{workspace_id}/members/{user_id}",
    operation_id = "workspaces.change_role",
    tag = "workspaces",
    security(("bearer" = ["workspaces:admin"])),
    // Changes someone's permissions: never on a rule or the assistant's initiative.
    extensions(("x-campbooks" = json!(Operation::command(Risk::High).records(["workspaces.member"]).access("workspaces.member", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("user_id" = UserId, Path, description = "The member."),
    ),
    request_body = ChangeRole,
    responses(
        (status = OK, description = "The member with their new role.", body = MemberView),
        (status = BAD_REQUEST, description = "Invalid body.", body = Problem,
            content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, not an owner or admin, or an admin \
            acting on the owner role.", body = Problem, content_type = "application/problem+json"),
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
    extract::Json(input): extract::Json<ChangeRole>,
) -> Result<Json<MemberView>> {
    execute(&state, &workspace.access, path.user_id, input)
        .await
        .map(Json)
}

/// Give `member` the role in `input`.
pub async fn execute(
    state: &AppState,
    access: &Access,
    member: UserId,
    input: ChangeRole,
) -> Result<MemberView> {
    let (actor, actor_role) = acting_user(access)?;
    let id = access.workspace;
    let mut tx = state.db.begin_for(id).await?;
    authorize(
        state,
        &mut tx,
        access,
        Action::Write,
        &member_resource(member),
    )
    .await?;
    lock_workspace(&mut tx, id).await?;
    let from = role_of(&mut tx, id, member).await?.ok_or(Error::NotFound)?;
    let to = input.role;
    if actor_role != Role::Owner && (from == Role::Owner || to == Role::Owner) {
        return Err(Error::Forbidden);
    }
    if from != to {
        if to != Role::Owner {
            keep_an_owner(&mut tx, id, from).await?;
        }
        sqlx::query!(
            "UPDATE workspaces.members SET role = $3 WHERE workspace_id = $1 AND user_id = $2",
            id as WorkspaceId,
            member as UserId,
            to.as_str(),
        )
        .execute(&mut *tx)
        .await?;
        security::record(
            &mut tx,
            Some(id),
            Some(actor),
            SecurityKind::RoleChanged,
            serde_json::json!({ "member": member, "from": from, "to": to }),
        )
        .await?;
        state
            .events
            .publish(
                &mut tx,
                &MemberRoleChanged {
                    workspace: id.to_string(),
                    user: member,
                    from,
                    to,
                    changed_by: actor,
                },
            )
            .await?;
    }
    let view = list_members::load(&mut tx, id, Some(member), access.user)
        .await?
        .pop()
        .ok_or(Error::NotFound)?;
    tx.commit().await?;
    Ok(view)
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use sqlx::PgPool;

    use crate::testkit::Kit;

    #[sqlx::test(migrations = "../../../migrations")]
    async fn owner_promotes_and_it_is_published(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let ws = kit.create_workspace(&ana, "Shop").await;
        kit.join(&ana, ws, "bea", "member").await;
        let bea = kit.user_id("bea").await;

        let response = kit
            .patch(
                &ana,
                &format!("/api/v1/w/{ws}/members/{bea}"),
                serde_json::json!({ "role": "admin" }),
            )
            .await;
        assert_eq!(response.status, StatusCode::OK, "{:?}", response.json);
        assert_eq!(response.json["role"], "admin");
        let events = kit.events_of(&ws.to_string()).await;
        assert_eq!(events.last().unwrap(), "workspaces.member_role_changed");
        let payload = kit.last_event_payload(&ws.to_string()).await;
        assert_eq!(
            (payload["from"].as_str(), payload["to"].as_str()),
            (Some("member"), Some("admin"))
        );
        assert_eq!(
            kit.security_kinds(&ws.to_string()).await.last().unwrap(),
            "role_changed"
        );
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn the_last_owner_stays_an_owner(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let ws = kit.create_workspace(&ana, "Shop").await;
        let ana_id = kit.user_id("ana").await;
        let uri = format!("/api/v1/w/{ws}/members/{ana_id}");

        let response = kit
            .patch(&ana, &uri, serde_json::json!({ "role": "admin" }))
            .await;
        assert_eq!(response.status, StatusCode::CONFLICT);
        assert_eq!(response.json["code"], "last_owner");

        // With a second owner, the first may step down.
        kit.join(&ana, ws, "bea", "admin").await;
        let bea_id = kit.user_id("bea").await;
        let promoted = kit
            .patch(
                &ana,
                &format!("/api/v1/w/{ws}/members/{bea_id}"),
                serde_json::json!({ "role": "owner" }),
            )
            .await;
        assert_eq!(promoted.json["role"], "owner");
        let stepped_down = kit
            .patch(&ana, &uri, serde_json::json!({ "role": "member" }))
            .await;
        assert_eq!(
            stepped_down.status,
            StatusCode::OK,
            "{:?}",
            stepped_down.json
        );
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn admins_cannot_touch_owners_and_members_cannot_manage(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let ws = kit.create_workspace(&ana, "Shop").await;
        let bea = kit.join(&ana, ws, "bea", "admin").await;
        let cid = kit.join(&ana, ws, "cid", "member").await;
        let (ana_id, cid_id) = (kit.user_id("ana").await, kit.user_id("cid").await);

        let demote_owner = kit
            .patch(
                &bea,
                &format!("/api/v1/w/{ws}/members/{ana_id}"),
                serde_json::json!({ "role": "member" }),
            )
            .await;
        assert_eq!(demote_owner.status, StatusCode::FORBIDDEN);
        let make_owner = kit
            .patch(
                &bea,
                &format!("/api/v1/w/{ws}/members/{cid_id}"),
                serde_json::json!({ "role": "owner" }),
            )
            .await;
        assert_eq!(make_owner.status, StatusCode::FORBIDDEN);
        let admin_ok = kit
            .patch(
                &bea,
                &format!("/api/v1/w/{ws}/members/{cid_id}"),
                serde_json::json!({ "role": "admin" }),
            )
            .await;
        assert_eq!(admin_ok.status, StatusCode::OK);
        let member_tries = kit
            .patch(
                &cid,
                &format!("/api/v1/w/{ws}/members/{ana_id}"),
                serde_json::json!({ "role": "member" }),
            )
            .await;
        // cid is an admin now, but still may not touch the owner.
        assert_eq!(member_tries.status, StatusCode::FORBIDDEN);
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn unknown_members_are_404(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let ws = kit.create_workspace(&ana, "Shop").await;
        let stranger = campbooks_kernel::tenancy::UserId::new();
        for uri in [
            format!("/api/v1/w/{ws}/members/{stranger}"),
            format!("/api/v1/w/{ws}/members/not-a-uuid"),
        ] {
            let response = kit
                .patch(&ana, &uri, serde_json::json!({ "role": "admin" }))
                .await;
            assert_eq!(response.status, StatusCode::NOT_FOUND, "{uri}");
        }
    }
}
