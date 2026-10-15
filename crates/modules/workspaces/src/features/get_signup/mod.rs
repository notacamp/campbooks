//! `GET /api/v1/signup` — where the caller stands with the instance's
//! signup policy, so the "create a workspace" screen can say "waiting for
//! approval", "approved" or "not approved" without trying to create one.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Operation, Problem, Result,
    auth::{Principal, Scoped},
    config::SignupPolicy,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    domain::{
        db::begin_as_user,
        signup::{RequestStatus, is_exempt, request_status},
        users::ensure_user,
    },
    scopes::WorkspacesRead,
};

/// The caller's standing with the signup policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SignupStatus {
    /// The instance's policy.
    pub policy: SignupPolicy,
    /// Whether the policy holds the caller back from creating a workspace:
    /// `false` for members of a workspace, the instance's first user and
    /// every user of an `open` instance.
    pub restricted: bool,
    /// Their signup request under the `approval` policy: `null` until they
    /// have asked (asking is trying to create a workspace).
    pub request: Option<RequestStatus>,
}

/// See where I stand with the signup policy.
#[utoipa::path(
    get,
    path = "/api/v1/signup",
    operation_id = "workspaces.get_signup",
    tag = "workspaces",
    security(("bearer" = ["workspaces:read"])),
    extensions(("x-campbooks" = json!(Operation::query().access("workspaces.account", campbooks_kernel::authz::Action::Read)))),
    responses(
        (status = OK, description = "The policy and the caller's request.", body = SignupStatus),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or not a user token.", body = Problem,
            content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    Scoped { principal, .. }: Scoped<WorkspacesRead>,
) -> Result<Json<SignupStatus>> {
    campbooks_kernel::authz::require_account(
        &state,
        &principal,
        campbooks_kernel::authz::Action::Read,
        crate::domain::resources::ACCOUNT_RECORD,
        || campbooks_kernel::Error::Forbidden,
    )?;
    execute(&state, &principal).await.map(Json)
}

/// The standing of `principal` (creating their user on first sign-in).
pub async fn execute(state: &AppState, principal: &Principal) -> Result<SignupStatus> {
    let user = ensure_user(&state.db, principal).await?;
    let policy = state.config.signup.policy;
    let mut tx = begin_as_user(&state.db, user).await?;
    let restricted = policy != SignupPolicy::Open && !is_exempt(&mut tx, user).await?;
    let request = request_status(&mut tx, user).await?;
    tx.commit().await?;
    Ok(SignupStatus {
        policy,
        restricted,
        request: request.filter(|_| policy == SignupPolicy::Approval && restricted),
    })
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use campbooks_kernel::config::SignupPolicy;
    use sqlx::PgPool;

    use crate::testkit::Kit;

    #[sqlx::test(migrations = "../../../migrations")]
    async fn shows_the_policy_and_the_request(db: PgPool) {
        let kit = Kit::with_signup(db, SignupPolicy::Approval, None).await;
        let first = kit.user("first");
        // The first user bootstraps the instance: not held back.
        let status = kit.get(&first, "/api/v1/signup").await;
        assert_eq!(status.status, StatusCode::OK);
        assert_eq!(
            status.json,
            serde_json::json!({ "policy": "approval", "restricted": false, "request": null })
        );
        kit.create_workspace(&first, "Ops").await;

        let second = kit.user("second");
        let before = kit.get(&second, "/api/v1/signup").await;
        assert_eq!(before.json["restricted"], true);
        assert_eq!(before.json["request"], serde_json::Value::Null);

        let refused = kit
            .post(
                &second,
                "/api/v1/workspaces",
                serde_json::json!({ "name": "Mine", "kind": "personal" }),
            )
            .await;
        assert_eq!(refused.status, StatusCode::FORBIDDEN);
        let after = kit.get(&second, "/api/v1/signup").await;
        assert_eq!(after.json["request"], "pending");

        // Nobody sees another person's request.
        let third = kit.user("third");
        let other = kit.get(&third, "/api/v1/signup").await;
        assert_eq!(other.json["request"], serde_json::Value::Null);
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn an_open_instance_holds_nobody_back(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let status = kit.get(&ana, "/api/v1/signup").await;
        assert_eq!(status.json["policy"], "open");
        assert_eq!(status.json["restricted"], false);
        assert_eq!(
            kit.get("", "/api/v1/signup").await.status,
            StatusCode::UNAUTHORIZED
        );
    }
}
