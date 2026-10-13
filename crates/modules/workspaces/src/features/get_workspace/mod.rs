//! `GET /api/v1/w/{workspace_id}` — one workspace, with the caller's role.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result,
    db::BeginFor as _,
    tenancy::{Access, Workspace, WorkspaceId},
};

use crate::{domain::model::WorkspaceView, scopes::WorkspacesRead};

/// Read a workspace.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}",
    operation_id = "workspaces.get",
    tag = "workspaces",
    security(("bearer" = ["workspaces:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant().records(["workspaces.workspace"])))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    responses(
        (status = OK, description = "The workspace.", body = WorkspaceView),
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
) -> Result<Json<WorkspaceView>> {
    execute(&state, &workspace.access).await.map(Json)
}

/// The workspace `access` is for.
pub async fn execute(state: &AppState, access: &Access) -> Result<WorkspaceView> {
    let mut tx = state.db.begin_for(access.workspace).await?;
    let view = load(&mut tx, access).await?;
    tx.commit().await?;
    Ok(view)
}

/// Read the live workspace of `access` in an open transaction.
pub async fn load(conn: &mut sqlx::PgConnection, access: &Access) -> Result<WorkspaceView> {
    let row = sqlx::query!(
        r#"SELECT id AS "id: WorkspaceId", name, kind, accent, created_at
             FROM workspaces.workspaces
            WHERE id = $1 AND deleted_at IS NULL"#,
        access.workspace as WorkspaceId,
    )
    .fetch_optional(conn)
    .await?
    .ok_or(Error::NotFound)?;
    WorkspaceView::from_row(
        row.id,
        row.name,
        &row.kind,
        &row.accent,
        row.created_at,
        access.role,
    )
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use sqlx::PgPool;

    use crate::testkit::Kit;

    #[sqlx::test(migrations = "../../../migrations")]
    async fn members_read_it_with_their_role(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let ws = kit.create_workspace(&ana, "Shop").await;
        let bea = kit.join(&ana, ws, "bea", "member").await;

        let response = kit.get(&bea, &format!("/api/v1/w/{ws}")).await;
        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(response.json["name"], "Shop");
        assert_eq!(response.json["role"], "member");
    }

    /// The assistant invoking `workspaces.get` by id goes through the same
    /// `Workspace<S>` extractor: a member reads it, anyone else gets 404.
    #[sqlx::test(migrations = "../../../migrations")]
    async fn invoked_by_id_with_the_same_membership_check(db: PgPool) {
        use campbooks_kernel::{
            auth::{Principal, PrincipalKind, Scope},
            operations::{self, Caller, Invocation, InvokeError},
        };

        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let ws = kit.create_workspace(&ana, "Shop").await;
        let eve = kit.user("eve");
        kit.get(&eve, "/api/v1/workspaces").await; // eve exists, belongs nowhere
        let state = operations::compose(crate::module().router, kit.state.clone()).state;
        let principal = |subject: &str| Principal {
            subject: subject.to_owned(),
            kind: PrincipalKind::User,
            client_id: None,
            scopes: [Scope::new("workspaces:read")].into(),
            workspace: None,
            email: Some(format!("{subject}@example.com")),
            email_verified: true,
            name: None,
            expires_at: None,
            auth_time: None,
        };
        let get = |subject: &str| {
            Invocation::new(
                Caller::Assistant,
                principal(subject),
                "workspaces.get",
                serde_json::Value::Null,
            )
            .in_workspace(ws.to_string())
        };

        let output = operations::invoke(&state, get("ana")).await.unwrap();
        assert_eq!(output.body["name"], "Shop");
        assert_eq!(output.body["role"], "owner");

        let refused = operations::invoke(&state, get("eve")).await.unwrap_err();
        assert!(
            matches!(
                refused,
                InvokeError::Failed {
                    status: StatusCode::NOT_FOUND,
                    ..
                }
            ),
            "{refused:?}"
        );
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn non_members_and_bad_ids_get_404(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let ws = kit.create_workspace(&ana, "Shop").await;
        let eve = kit.user("eve");
        kit.get(&eve, "/api/v1/workspaces").await; // eve exists, belongs nowhere

        for uri in [
            format!("/api/v1/w/{ws}"),
            format!(
                "/api/v1/w/{}",
                campbooks_kernel::tenancy::WorkspaceId::new()
            ),
            "/api/v1/w/not-a-uuid".to_owned(),
        ] {
            let response = kit.get(&eve, &uri).await;
            assert_eq!(response.status, StatusCode::NOT_FOUND, "{uri}");
            assert_eq!(
                response.content_type.as_deref(),
                Some("application/problem+json")
            );
        }
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn token_and_scope_come_first(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let ws = kit.create_workspace(&ana, "Shop").await;
        let anonymous = kit.get("", &format!("/api/v1/w/{ws}")).await;
        assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);
        let unscoped = kit.user_scopes("ana", "workspaces:write");
        let response = kit.get(&unscoped, &format!("/api/v1/w/{ws}")).await;
        assert_eq!(response.status, StatusCode::FORBIDDEN);
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn client_tokens_are_bound_to_their_workspace(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let a = kit.create_workspace(&ana, "A").await;
        let b = kit.create_workspace(&ana, "B").await;
        let client = kit.client(Some(a), "workspaces:read");

        let own = kit.get(&client, &format!("/api/v1/w/{a}")).await;
        assert_eq!(own.status, StatusCode::OK);
        assert!(own.json.get("role").is_none(), "{:?}", own.json);
        let other = kit.get(&client, &format!("/api/v1/w/{b}")).await;
        assert_eq!(other.status, StatusCode::NOT_FOUND);
        let unbound = kit.client(None, "workspaces:read");
        assert_eq!(
            kit.get(&unbound, &format!("/api/v1/w/{a}")).await.status,
            StatusCode::NOT_FOUND
        );
    }
}
