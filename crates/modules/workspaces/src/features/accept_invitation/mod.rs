//! `POST /api/v1/workspaces/invitations/accept` — join a workspace with an
//! invitation token. The caller's verified email must be the invited one.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    auth::{Principal, Scoped},
    db::BeginFor as _,
    extract,
    tenancy::{Role, UserId, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    domain::{
        db::set_user,
        invitation,
        model::{InvitationId, WorkspaceView},
        security::{self, SecurityKind},
        users::ensure_user,
    },
    events::MemberJoined,
    features::get_workspace,
    scopes::WorkspacesWrite,
};

/// The invitation to accept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AcceptInvitation {
    /// The token from the invitation link.
    pub token: String,
}

/// Accept an invitation.
#[utoipa::path(
    post,
    path = "/api/v1/workspaces/invitations/accept",
    operation_id = "workspaces.accept_invitation",
    tag = "workspaces",
    security(("bearer" = ["workspaces:write"])),
    // Accepting an invitation is the invitee's own act; not for rules or the assistant.
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).records(["workspaces.invitation", "workspaces.member"])))),
    request_body = AcceptInvitation,
    responses(
        (status = OK, description = "You are a member; the workspace with your role.",
            body = WorkspaceView),
        (status = BAD_REQUEST, description = "Invalid body.", body = Problem,
            content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, a client token, or \
            `invitation_email_mismatch`: your verified email is not the invited one.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such invitation.", body = Problem,
            content_type = "application/problem+json"),
        (status = CONFLICT, description = "`invitation_used`, `invitation_expired` or \
            `already_a_member`.", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    Scoped { principal, .. }: Scoped<WorkspacesWrite>,
    extract::Json(input): extract::Json<AcceptInvitation>,
) -> Result<Json<WorkspaceView>> {
    execute(&state, &principal, input).await.map(Json)
}

/// `principal` accepts the invitation `input.token`.
pub async fn execute(
    state: &AppState,
    principal: &Principal,
    input: AcceptInvitation,
) -> Result<WorkspaceView> {
    let token = input.token.trim();
    let workspace = invitation::workspace_of(token).ok_or(Error::NotFound)?;
    let user = ensure_user(&state.db, principal).await?;

    let mut tx = state.db.begin_for(workspace).await?;
    set_user(&mut tx, user).await?;
    let found = sqlx::query!(
        r#"SELECT i.id AS "id: InvitationId", i.email, i.role,
                  i.accepted_at IS NOT NULL AS "used!", i.expires_at <= now() AS "expired!"
             FROM workspaces.invitations i
             JOIN workspaces.workspaces w ON w.id = i.workspace_id
            WHERE i.token_hash = $1 AND i.workspace_id = $2 AND w.deleted_at IS NULL
              FOR UPDATE OF i"#,
        invitation::hash(token),
        workspace as WorkspaceId,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(Error::NotFound)?;
    if found.used {
        return Err(Error::Conflict("invitation_used".to_owned()));
    }
    if found.expired {
        return Err(Error::Conflict("invitation_expired".to_owned()));
    }
    let email_matches = principal.email_verified
        && principal
            .email
            .as_deref()
            .is_some_and(|email| email.trim().to_lowercase() == found.email);
    if !email_matches {
        return Err(Error::Denied("invitation_email_mismatch"));
    }
    let role: Role = found.role.parse()?;
    let joined = sqlx::query!(
        "INSERT INTO workspaces.members (workspace_id, user_id, role) VALUES ($1, $2, $3)
         ON CONFLICT (workspace_id, user_id) DO NOTHING",
        workspace as WorkspaceId,
        user as UserId,
        role.as_str(),
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if joined == 0 {
        return Err(Error::Conflict("already_a_member".to_owned()));
    }
    sqlx::query!(
        "UPDATE workspaces.invitations SET accepted_at = now(), accepted_by = $2 WHERE id = $1",
        found.id as InvitationId,
        user as UserId,
    )
    .execute(&mut *tx)
    .await?;
    security::record(
        &mut tx,
        Some(workspace),
        Some(user),
        SecurityKind::InvitationAccepted,
        serde_json::json!({ "invitation": found.id, "role": role }),
    )
    .await?;
    state
        .events
        .publish(
            &mut tx,
            &MemberJoined {
                workspace: workspace.to_string(),
                user,
                role,
            },
        )
        .await?;
    let access = campbooks_kernel::tenancy::Access {
        workspace,
        user: Some(user),
        role: Some(role),
    };
    let view = get_workspace::load(&mut tx, &access).await?;
    tx.commit().await?;
    tracing::info!(workspace = %workspace, user = %user, "invitation accepted");
    Ok(view)
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use sqlx::PgPool;

    use crate::testkit::Kit;

    const ACCEPT: &str = "/api/v1/workspaces/invitations/accept";

    #[sqlx::test(migrations = "../../../migrations")]
    async fn the_invitee_joins_once(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let ws = kit.create_workspace(&ana, "Shop").await;
        let token = kit.invite(&ana, ws, "Bea@Example.com", "admin").await;
        let bea = kit.user("bea");

        let response = kit
            .post(&bea, ACCEPT, serde_json::json!({ "token": token }))
            .await;
        assert_eq!(response.status, StatusCode::OK, "{:?}", response.json);
        assert_eq!(response.json["id"], ws.to_string());
        assert_eq!(response.json["role"], "admin");
        assert_eq!(
            kit.get(&bea, &format!("/api/v1/w/{ws}")).await.status,
            StatusCode::OK
        );
        assert_eq!(
            kit.events_of(&ws.to_string()).await.last().unwrap(),
            "workspaces.member_joined"
        );
        assert_eq!(
            kit.security_kinds(&ws.to_string()).await.last().unwrap(),
            "invitation_accepted"
        );

        let again = kit
            .post(&bea, ACCEPT, serde_json::json!({ "token": token }))
            .await;
        assert_eq!(again.status, StatusCode::CONFLICT);
        assert_eq!(again.json["detail"], "invitation_used");
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn expired_invitations_do_not_work(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let ws = kit.create_workspace(&ana, "Shop").await;
        let token = kit.invite(&ana, ws, "bea@example.com", "member").await;
        sqlx::query!("UPDATE workspaces.invitations SET expires_at = now() - interval '1 minute'")
            .execute(&kit.db)
            .await
            .unwrap();

        let bea = kit.user("bea");
        let response = kit
            .post(&bea, ACCEPT, serde_json::json!({ "token": token }))
            .await;
        assert_eq!(response.status, StatusCode::CONFLICT);
        assert_eq!(response.json["detail"], "invitation_expired");
        assert_eq!(
            kit.get(&bea, &format!("/api/v1/w/{ws}")).await.status,
            StatusCode::NOT_FOUND
        );
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn only_the_invited_verified_address_may_accept(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let ws = kit.create_workspace(&ana, "Shop").await;
        let token = kit.invite(&ana, ws, "bea@example.com", "member").await;

        let eve = kit.user("eve");
        let unverified = kit.user_with("bea2", "bea@example.com", false);
        for caller in [&eve, &unverified] {
            let response = kit
                .post(caller, ACCEPT, serde_json::json!({ "token": token }))
                .await;
            assert_eq!(response.status, StatusCode::FORBIDDEN);
            assert_eq!(response.json["detail"], "invitation_email_mismatch");
        }
        // Still usable by the right person.
        let bea = kit.user("bea");
        let response = kit
            .post(&bea, ACCEPT, serde_json::json!({ "token": token }))
            .await;
        assert_eq!(response.status, StatusCode::OK);
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn unknown_or_forged_tokens_are_404(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let ws = kit.create_workspace(&ana, "Shop").await;
        let real = kit.invite(&ana, ws, "bea@example.com", "member").await;
        let other = kit.create_workspace(&ana, "Other").await;
        // A real secret moved to another workspace's prefix finds nothing.
        let (_, secret) = real.split_once('.').unwrap();
        let bea = kit.user("bea");
        let forged = [
            "nonsense".to_owned(),
            format!("{ws}.AAAA"),
            format!("{other}.{secret}"),
        ];
        for (case, token) in forged.iter().enumerate() {
            let response = kit
                .post(&bea, ACCEPT, serde_json::json!({ "token": token }))
                .await;
            assert_eq!(
                response.status,
                StatusCode::NOT_FOUND,
                "forged token #{case}"
            );
        }
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn members_and_clients_cannot_accept(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let ws = kit.create_workspace(&ana, "Shop").await;
        let bea = kit.join(&ana, ws, "bea", "member").await;
        // bea's stored address is stale, so inviting her current one passes.
        sqlx::query!(
            "UPDATE workspaces.users SET email = 'old@example.com' WHERE idp_subject = 'bea'"
        )
        .execute(&kit.db)
        .await
        .unwrap();
        let token = kit.invite(&ana, ws, "bea@example.com", "admin").await;

        let response = kit
            .post(&bea, ACCEPT, serde_json::json!({ "token": token }))
            .await;
        assert_eq!(response.status, StatusCode::CONFLICT);
        assert_eq!(response.json["detail"], "already_a_member");

        let client = kit.client(Some(ws), "workspaces:write");
        let response = kit
            .post(&client, ACCEPT, serde_json::json!({ "token": token }))
            .await;
        assert_eq!(response.status, StatusCode::FORBIDDEN);
    }
}
