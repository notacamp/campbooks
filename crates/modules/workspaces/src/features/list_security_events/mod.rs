//! `GET /api/v1/w/{workspace_id}/security-events` — the workspace's security
//! log, newest first. Owners and admins.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Operation, Problem, Result,
    db::BeginFor as _,
    extract,
    tenancy::{Access, UserId, Workspace, WorkspaceId},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::MESSAGES;

use crate::{
    domain::{access::acting_manager, model::SecurityEventId},
    scopes::WorkspacesAdmin,
};

/// Most events one call returns.
pub const MAX_LIMIT: u16 = 200;

/// Which events to return.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct Page {
    /// How many, newest first: 1 to 200, default 50.
    pub limit: Option<u16>,
}

/// One entry of the security log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct SecurityEventView {
    /// Its id.
    pub id: SecurityEventId,
    /// What happened: `workspace_created`, `role_changed`, `invitation_created`,
    /// `invitation_accepted`, `member_removed`, `member_left`, `workspace_deleted`.
    #[schema(example = "role_changed")]
    pub kind: String,
    /// Who did it.
    pub user_id: Option<UserId>,
    /// When.
    pub at: DateTime<Utc>,
    /// Ids and roles involved; never secrets.
    pub metadata: serde_json::Value,
}

/// The log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct SecurityEventList {
    /// The events, newest first.
    pub events: Vec<SecurityEventView>,
}

/// Read the security log.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/security-events",
    operation_id = "workspaces.list_security_events",
    tag = "workspaces",
    security(("bearer" = ["workspaces:admin"])),
    // The audit log stays out of the assistant's context.
    extensions(("x-campbooks" = json!(Operation::query().records(["workspaces.security_event"])))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace."), Page),
    responses(
        (status = OK, description = "The newest events.", body = SecurityEventList),
        (status = BAD_REQUEST, description = "Invalid limit.", body = Problem,
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
    extract::Query(page): extract::Query<Page>,
) -> Result<Json<SecurityEventList>> {
    execute(&state, &workspace.access, page).await.map(Json)
}

/// The newest security events of the workspace of `access`.
pub async fn execute(state: &AppState, access: &Access, page: Page) -> Result<SecurityEventList> {
    acting_manager(access)?;
    let limit = page.limit.unwrap_or(50);
    if limit == 0 || limit > MAX_LIMIT {
        return Err(campbooks_kernel::Error::BadRequest(
            MESSAGES.reason("invalid_limit").arg("max", MAX_LIMIT),
        ));
    }
    let mut tx = state.db.begin_for(access.workspace).await?;
    let events = sqlx::query!(
        r#"SELECT id AS "id: SecurityEventId", kind, user_id AS "user_id: UserId", at, metadata
             FROM workspaces.security_events
            WHERE workspace_id = $1
            ORDER BY at DESC, id DESC
            LIMIT $2"#,
        access.workspace as WorkspaceId,
        i64::from(limit),
    )
    .fetch_all(&mut *tx)
    .await?
    .into_iter()
    .map(|row| SecurityEventView {
        id: row.id,
        kind: row.kind,
        user_id: row.user_id,
        at: row.at,
        metadata: row.metadata,
    })
    .collect();
    tx.commit().await?;
    Ok(SecurityEventList { events })
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use sqlx::PgPool;

    use crate::testkit::Kit;

    #[sqlx::test(migrations = "../../../migrations")]
    async fn managers_read_the_log_newest_first_without_secrets(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let ws = kit.create_workspace(&ana, "Shop").await;
        let token = kit.invite(&ana, ws, "bea@example.com", "member").await;
        let bea = kit.user("bea");
        kit.post(
            &bea,
            "/api/v1/workspaces/invitations/accept",
            serde_json::json!({ "token": token }),
        )
        .await;
        let bea_id = kit.user_id("bea").await;
        kit.patch(
            &ana,
            &format!("/api/v1/w/{ws}/members/{bea_id}"),
            serde_json::json!({ "role": "admin" }),
        )
        .await;

        let response = kit
            .get(&bea, &format!("/api/v1/w/{ws}/security-events?limit=10"))
            .await;
        assert_eq!(response.status, StatusCode::OK, "{:?}", response.json);
        let kinds: Vec<_> = response.json["events"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["kind"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(
            kinds,
            [
                "role_changed",
                "invitation_accepted",
                "invitation_created",
                "workspace_created"
            ]
        );
        let role_change = &response.json["events"][0]["metadata"];
        assert_eq!(
            (role_change["from"].as_str(), role_change["to"].as_str()),
            (Some("member"), Some("admin"))
        );
        let raw = response.json.to_string();
        assert!(!raw.contains(token.split_once('.').unwrap().1));
        assert!(!raw.contains("@example.com"), "{raw}");

        let limited = kit
            .get(&bea, &format!("/api/v1/w/{ws}/security-events?limit=1"))
            .await;
        assert_eq!(limited.json["events"].as_array().unwrap().len(), 1);
        for bad in ["0", "201", "x"] {
            let response = kit
                .get(&bea, &format!("/api/v1/w/{ws}/security-events?limit={bad}"))
                .await;
            assert_eq!(response.status, StatusCode::BAD_REQUEST, "{bad}");
        }
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn members_may_not_read_it(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let ws = kit.create_workspace(&ana, "Shop").await;
        let cid = kit.join(&ana, ws, "cid", "member").await;
        let response = kit
            .get(&cid, &format!("/api/v1/w/{ws}/security-events"))
            .await;
        assert_eq!(response.status, StatusCode::FORBIDDEN);
    }
}
