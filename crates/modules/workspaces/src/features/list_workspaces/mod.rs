//! `GET /api/v1/workspaces` — the workspaces the signed-in user belongs to.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Operation, Problem, Result,
    auth::{Principal, Scoped},
    tenancy::{Role, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    domain::{db::begin_as_user, model::WorkspaceView, users::ensure_user},
    scopes::WorkspacesRead,
};

/// The caller's workspaces, oldest first, each with their role.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct WorkspaceList {
    /// The workspaces.
    pub workspaces: Vec<WorkspaceView>,
}

/// List my workspaces.
#[utoipa::path(
    get,
    path = "/api/v1/workspaces",
    operation_id = "workspaces.list",
    tag = "workspaces",
    security(("bearer" = ["workspaces:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant().records(["workspaces.workspace"]).access("workspaces.account", campbooks_kernel::authz::Action::Read)))),
    responses(
        (status = OK, description = "The workspaces you belong to.", body = WorkspaceList),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or not a user token.", body = Problem,
            content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    Scoped { principal, .. }: Scoped<WorkspacesRead>,
) -> Result<Json<WorkspaceList>> {
    campbooks_kernel::authz::require_account(
        &state,
        &principal,
        campbooks_kernel::authz::Action::Read,
        crate::domain::resources::ACCOUNT_RECORD,
        || campbooks_kernel::Error::Forbidden,
    )?;
    execute(&state, &principal).await.map(Json)
}

/// The workspaces `principal` belongs to (creating their user on first
/// sign-in).
pub async fn execute(state: &AppState, principal: &Principal) -> Result<WorkspaceList> {
    let bound = principal.bound_workspace();
    let user = ensure_user(&state.db, principal).await?;
    let mut tx = begin_as_user(&state.db, user).await?;
    let rows = sqlx::query!(
        r#"
        SELECT w.id AS "id: WorkspaceId", w.name, w.kind, w.accent, w.created_at, m.role
          FROM workspaces.members m
          JOIN workspaces.workspaces w ON w.id = m.workspace_id
         WHERE m.user_id = $1 AND w.deleted_at IS NULL
         ORDER BY w.created_at, w.id
        "#,
        user as campbooks_kernel::tenancy::UserId,
    )
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    let workspaces = rows
        .into_iter()
        // An app allowed in one workspace sees that one only.
        .filter(|row| bound.is_none_or(|w| w == row.id.to_string()))
        .map(|row| {
            WorkspaceView::from_row(
                row.id,
                row.name,
                &row.kind,
                &row.accent,
                row.created_at,
                Some(row.role.parse::<Role>()?),
            )
        })
        .collect::<Result<_>>()?;
    Ok(WorkspaceList { workspaces })
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use sqlx::PgPool;

    use crate::testkit::Kit;

    #[sqlx::test(migrations = "../../../migrations")]
    async fn first_call_creates_the_user_and_lists_nothing(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let response = kit.get(&ana, "/api/v1/workspaces").await;
        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(response.json, serde_json::json!({ "workspaces": [] }));

        // One user, one `signed_in` event, however often they call.
        kit.get(&ana, "/api/v1/workspaces").await;
        let (users, signed_in) = kit.count_users_and_sign_ins().await;
        assert_eq!((users, signed_in), (1, 1));
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn lists_only_my_live_workspaces_with_my_role(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let bea = kit.user("bea");
        let mine = kit.create_workspace(&ana, "Mine").await;
        kit.create_workspace(&bea, "Hers").await;
        let gone = kit.create_workspace(&ana, "Gone").await;
        kit.delete(&ana, &format!("/api/v1/w/{gone}")).await;

        let response = kit.get(&ana, "/api/v1/workspaces").await;
        let list = response.json["workspaces"].as_array().unwrap();
        assert_eq!(list.len(), 1, "{list:?}");
        assert_eq!(list[0]["id"], mine.to_string());
        assert_eq!(list[0]["role"], "owner");
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn an_app_bound_to_one_workspace_sees_it_and_can_make_or_join_no_other(db: PgPool) {
        use crate::features::{accept_invitation, create_workspace};

        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let mine = kit.create_workspace(&ana, "Mine").await;
        kit.create_workspace(&ana, "Other").await;
        let bound = Kit::bound(&ana, mine);

        let listed = super::execute(&kit.state, &bound).await.unwrap();
        assert_eq!(listed.workspaces.len(), 1);
        assert_eq!(listed.workspaces[0].id, mine);

        let created = create_workspace::execute(
            &kit.state,
            &bound,
            serde_json::from_value(serde_json::json!({ "name": "Sneaky", "kind": "business" }))
                .unwrap(),
        )
        .await;
        assert!(
            matches!(created, Err(campbooks_kernel::Error::NotFound)),
            "{created:?}"
        );
        let accepted = accept_invitation::execute(
            &kit.state,
            &bound,
            serde_json::from_value(serde_json::json!({ "token": "anything" })).unwrap(),
        )
        .await;
        assert!(
            matches!(accepted, Err(campbooks_kernel::Error::NotFound)),
            "{accepted:?}"
        );
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn updates_a_changed_email(db: PgPool) {
        let kit = Kit::new(db).await;
        kit.get(&kit.user("ana"), "/api/v1/workspaces").await;
        let renamed = kit.user_with("ana", "ana.new@example.com", true);
        kit.get(&renamed, "/api/v1/workspaces").await;
        assert_eq!(
            kit.user_email("ana").await.as_deref(),
            Some("ana.new@example.com")
        );
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn client_tokens_have_no_workspace_list(db: PgPool) {
        let kit = Kit::new(db).await;
        let client = kit.client(None, "workspaces:read");
        let response = kit.get(&client, "/api/v1/workspaces").await;
        assert_eq!(response.status, StatusCode::FORBIDDEN);
    }
}
