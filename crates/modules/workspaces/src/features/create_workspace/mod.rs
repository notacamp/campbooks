//! `POST /api/v1/workspaces` — create a workspace; its creator becomes its
//! owner. A user with no workspace yet is subject to the instance's signup
//! policy (`CAMPBOOKS_SIGNUP__POLICY`).

use axum::{Json, extract::State, http::StatusCode};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    auth::{Principal, Scoped},
    db::BeginFor as _,
    extract,
    tenancy::{Role, UserId, WorkspaceId},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    domain::{
        model::{Accent, Kind, WorkspaceView, workspace_name},
        security::{self, SecurityKind},
        signup::{self, Admission},
        users::ensure_user,
    },
    events::{MemberJoined, WorkspaceCreated},
    scopes::WorkspacesWrite,
};

/// A new workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateWorkspace {
    /// Its name, 1 to 100 characters (trimmed).
    #[schema(example = "Household")]
    pub name: String,
    /// What it is for.
    pub kind: Kind,
    /// Its colour; defaults to the kind's preset.
    #[serde(default)]
    pub accent: Option<Accent>,
    /// The signup code, when the instance's policy is `code` and this is the
    /// caller's first workspace.
    #[serde(default)]
    pub signup_code: Option<String>,
}

/// Create a workspace.
#[utoipa::path(
    post,
    path = "/api/v1/workspaces",
    operation_id = "workspaces.create",
    tag = "workspaces",
    security(("bearer" = ["workspaces:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).records(["workspaces.workspace", "workspaces.member"])))),
    request_body = CreateWorkspace,
    responses(
        (status = CREATED, description = "Created; you are its owner.", body = WorkspaceView),
        (status = BAD_REQUEST, description = "Invalid name or body.", body = Problem,
            content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, a client token, or refused by the \
            signup policy (`signup_invite_only`, `signup_requires_approval`, \
            `signup_rejected`, `signup_code_invalid`).", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    Scoped { principal, .. }: Scoped<WorkspacesWrite>,
    extract::Json(input): extract::Json<CreateWorkspace>,
) -> Result<(StatusCode, Json<WorkspaceView>)> {
    let view = execute(&state, &principal, input).await?;
    Ok((StatusCode::CREATED, Json(view)))
}

/// Create a workspace owned by `principal`.
pub async fn execute(
    state: &AppState,
    principal: &Principal,
    input: CreateWorkspace,
) -> Result<WorkspaceView> {
    // An app allowed in one workspace can't make others.
    if principal.bound_workspace().is_some() {
        return Err(Error::NotFound);
    }
    let name = workspace_name(&input.name)?;
    let accent = input.accent.unwrap_or(input.kind.default_accent());
    let user = ensure_user(&state.db, principal).await?;
    let admission = signup::admit(state, user, input.signup_code.as_deref()).await?;

    let id = WorkspaceId::new();
    let mut tx = state.db.begin_for(id).await?;
    // A minted code is spent in this transaction: if anything below fails
    // the use is given back, and two attempts at its last use cannot both
    // succeed.
    let redeemed = match &admission {
        Admission::Admitted => None,
        Admission::Redeem(digest) => Some(signup::redeem(&mut tx, digest).await?),
    };
    let created_at = sqlx::query_scalar!(
        r#"INSERT INTO workspaces.workspaces (id, name, kind, accent)
           VALUES ($1, $2, $3, $4)
           RETURNING created_at AS "created_at: DateTime<Utc>""#,
        id as WorkspaceId,
        name,
        input.kind.as_str(),
        accent.as_str(),
    )
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query!(
        "INSERT INTO workspaces.members (workspace_id, user_id, role) VALUES ($1, $2, 'owner')",
        id as WorkspaceId,
        user as UserId,
    )
    .execute(&mut *tx)
    .await?;
    security::record(
        &mut tx,
        Some(id),
        Some(user),
        SecurityKind::WorkspaceCreated,
        serde_json::json!({ "kind": input.kind }),
    )
    .await?;
    if let Some(code) = redeemed {
        security::record(
            &mut tx,
            Some(id),
            Some(user),
            SecurityKind::SignupCodeRedeemed,
            serde_json::json!({ "code_id": code }),
        )
        .await?;
    }
    let workspace = id.to_string();
    state
        .events
        .publish(
            &mut tx,
            &WorkspaceCreated {
                workspace: workspace.clone(),
                owner: user,
                kind: input.kind,
            },
        )
        .await?;
    state
        .events
        .publish(
            &mut tx,
            &MemberJoined {
                workspace,
                user,
                role: Role::Owner,
            },
        )
        .await?;
    tx.commit().await?;
    tracing::info!(workspace = %id, user = %user, "workspace created");

    Ok(WorkspaceView {
        id,
        name,
        kind: input.kind,
        accent,
        created_at,
        role: Some(Role::Owner),
    })
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use campbooks_kernel::config::SignupPolicy;
    use sqlx::PgPool;

    use crate::testkit::Kit;

    #[sqlx::test(migrations = "../../../migrations")]
    async fn creates_with_the_creator_as_owner_and_publishes(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let response = kit
            .post(
                &ana,
                "/api/v1/workspaces",
                serde_json::json!({ "name": "  Household ", "kind": "personal" }),
            )
            .await;
        assert_eq!(response.status, StatusCode::CREATED, "{:?}", response.json);
        assert_eq!(response.json["name"], "Household");
        assert_eq!(response.json["kind"], "personal");
        assert_eq!(response.json["accent"], "personal");
        assert_eq!(response.json["role"], "owner");

        let workspace = response.json["id"].as_str().unwrap().to_owned();
        assert_eq!(
            kit.events_of(&workspace).await,
            ["workspaces.workspace_created", "workspaces.member_joined"]
        );
        assert_eq!(kit.security_kinds(&workspace).await, ["workspace_created"]);
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn validates_the_input(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        for body in [
            serde_json::json!({ "name": "   ", "kind": "personal" }),
            serde_json::json!({ "name": "x".repeat(101), "kind": "personal" }),
            serde_json::json!({ "name": "Shop", "kind": "charity" }),
            serde_json::json!({ "name": "Shop", "kind": "business", "accent": "pink" }),
            serde_json::json!({ "name": "Shop", "kind": "business", "owner": "me" }),
        ] {
            let response = kit.post(&ana, "/api/v1/workspaces", body.clone()).await;
            assert_eq!(response.status, StatusCode::BAD_REQUEST, "{body}");
        }
        let forest = kit
            .post(
                &ana,
                "/api/v1/workspaces",
                serde_json::json!({ "name": "Shop", "kind": "business", "accent": "forest" }),
            )
            .await;
        assert_eq!(forest.json["accent"], "forest");
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn clients_cannot_create_workspaces(db: PgPool) {
        let kit = Kit::new(db).await;
        let client = kit.client(None, "workspaces:write");
        let response = kit
            .post(
                &client,
                "/api/v1/workspaces",
                serde_json::json!({ "name": "Bot", "kind": "business" }),
            )
            .await;
        assert_eq!(response.status, StatusCode::FORBIDDEN);
    }

    async fn second_user_tries(kit: &Kit, body: serde_json::Value) -> (StatusCode, String) {
        // The instance's first user bootstraps it under any policy.
        let first = kit.user("first");
        let created = kit
            .post(
                &first,
                "/api/v1/workspaces",
                serde_json::json!({ "name": "Ops", "kind": "business" }),
            )
            .await;
        assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.json);
        let response = kit
            .post(&kit.user("second"), "/api/v1/workspaces", body)
            .await;
        (
            response.status,
            response.json["code"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
        )
    }

    fn body() -> serde_json::Value {
        serde_json::json!({ "name": "Mine", "kind": "personal" })
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn invite_only_refuses_new_users(db: PgPool) {
        let kit = Kit::with_signup(db, SignupPolicy::InviteOnly, None).await;
        let (status, detail) = second_user_tries(&kit, body()).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(detail, "signup_invite_only");
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn members_may_create_more_under_any_policy(db: PgPool) {
        let kit = Kit::with_signup(db, SignupPolicy::InviteOnly, None).await;
        let first = kit.user("first");
        let ops = kit.create_workspace(&first, "Ops").await;
        let bea = kit.join(&first, ops, "bea", "member").await;
        let response = kit.post(&bea, "/api/v1/workspaces", body()).await;
        assert_eq!(response.status, StatusCode::CREATED, "{:?}", response.json);
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn approval_records_the_request(db: PgPool) {
        let kit = Kit::with_signup(db, SignupPolicy::Approval, None).await;
        let (status, detail) = second_user_tries(&kit, body()).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(detail, "signup_requires_approval");
        assert_eq!(
            kit.instance_security_kinds("second").await,
            ["signed_in", "signup_requested"]
        );
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn code_policy_needs_a_configured_code(db: PgPool) {
        let kit = Kit::with_signup(db, SignupPolicy::Code, Some("early-bird, friends")).await;
        let (status, detail) = second_user_tries(
            &kit,
            serde_json::json!({ "name": "Mine", "kind": "personal", "signup_code": "nope" }),
        )
        .await;
        assert_eq!(
            (status, detail.as_str()),
            (StatusCode::FORBIDDEN, "signup_code_invalid")
        );
        let missing = kit
            .post(&kit.user("third"), "/api/v1/workspaces", body())
            .await;
        assert_eq!(missing.json["code"], "signup_code_invalid");

        let ok = kit
            .post(
                &kit.user("second"),
                "/api/v1/workspaces",
                serde_json::json!({ "name": "Mine", "kind": "personal", "signup_code": "friends" }),
            )
            .await;
        assert_eq!(ok.status, StatusCode::CREATED, "{:?}", ok.json);
    }
}
