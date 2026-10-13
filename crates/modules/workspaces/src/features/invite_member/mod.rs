//! `POST /api/v1/w/{workspace_id}/invitations` — invite someone by email.
//! Owners and admins; as `admin` or `member`.
//!
//! Nothing emails invitations yet (the `notifications` module will), so the
//! inviter gets the accept link once, in this response, to pass on. The link
//! carries the token: it is never stored in clear, logged, published in an
//! event or returned by any other endpoint. Inviting the same address again
//! replaces the invitation and returns a fresh link.

use axum::{Json, extract::State, http::StatusCode};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    db::BeginFor as _,
    extract,
    tenancy::{Access, Role, UserId, Workspace, WorkspaceId},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::MESSAGES;

use crate::{
    domain::{
        access::{acting_manager, lock_workspace},
        invitation::{self, InvitationView, VALIDITY},
        model::InvitationId,
        security::{self, SecurityKind},
    },
    scopes::WorkspacesAdmin,
};

/// Whom to invite.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct InviteMember {
    /// Their email address; the invitation can only be accepted by a user
    /// whose verified email is this one.
    #[schema(example = "someone@example.com")]
    pub email: String,
    /// The role they get: `admin` or `member` (the default). Owners are made
    /// by promotion, never by invitation.
    #[serde(default = "member")]
    pub role: Role,
}

fn member() -> Role {
    Role::Member
}

/// An invitation and its token, which only the server ever sees.
#[derive(Debug, Clone)]
pub struct Issued {
    /// The invitation.
    pub invitation: InvitationView,
    /// The token to send to the invitee. Returned to the inviter once, inside
    /// the accept link; never logged.
    pub token: String,
}

/// A new invitation, as the inviter sees it once: with the link to pass on.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CreatedInvitation {
    /// The invitation.
    #[serde(flatten)]
    pub invitation: InvitationView,
    /// The link that accepts it (it carries the token). Returned only here,
    /// once: no other endpoint, log or event ever shows it.
    #[schema(example = "https://campbooks.example.com/invitations/accept?token=…")]
    pub accept_url: String,
}

/// The accept link for `token` on this instance's public origin.
fn accept_url(public_url: &str, token: &str) -> String {
    format!(
        "{}/invitations/accept?token={token}",
        public_url.trim_end_matches('/')
    )
}

/// Invite someone.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/invitations",
    operation_id = "workspaces.invite_member",
    tag = "workspaces",
    security(("bearer" = ["workspaces:admin"])),
    // Emails another person and grants access once accepted.
    extensions(("x-campbooks" = json!(Operation::command(Risk::High).records(["workspaces.invitation"])))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    request_body = InviteMember,
    responses(
        (status = CREATED, description = "Invited, with the accept link to pass on (shown only \
            in this response). A previous pending invitation to the same address is replaced.",
            body = CreatedInvitation),
        (status = BAD_REQUEST, description = "Invalid email, role or body.", body = Problem,
            content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or not an owner or admin.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such workspace, or you are not a member.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "`already_a_member`.", body = Problem,
            content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<WorkspacesAdmin>,
    extract::Json(input): extract::Json<InviteMember>,
) -> Result<(StatusCode, Json<CreatedInvitation>)> {
    let issued = execute(&state, &workspace.access, input).await?;
    let accept_url = accept_url(&state.config.public_url, &issued.token);
    Ok((
        StatusCode::CREATED,
        Json(CreatedInvitation {
            invitation: issued.invitation,
            accept_url,
        }),
    ))
}

/// Invite `input.email` to the workspace of `access`.
pub async fn execute(state: &AppState, access: &Access, input: InviteMember) -> Result<Issued> {
    let (actor, _) = acting_manager(access)?;
    if input.role == Role::Owner {
        return Err(Error::BadRequest(MESSAGES.reason("invalid_role")));
    }
    let email = invitation::normalize_email(&input.email)?;
    let id = access.workspace;
    let mut tx = state.db.begin_for(id).await?;
    lock_workspace(&mut tx, id).await?;
    let already_member = sqlx::query_scalar!(
        r#"SELECT EXISTS (
               SELECT 1 FROM workspaces.members m
                 JOIN workspaces.users u ON u.id = m.user_id
                WHERE m.workspace_id = $1 AND lower(u.email) = $2
           ) AS "exists!""#,
        id as WorkspaceId,
        email,
    )
    .fetch_one(&mut *tx)
    .await?;
    if already_member {
        return Err(Error::Conflict(MESSAGES.reason("already_a_member")));
    }
    sqlx::query!(
        "DELETE FROM workspaces.invitations
          WHERE workspace_id = $1 AND email = $2 AND accepted_at IS NULL",
        id as WorkspaceId,
        email,
    )
    .execute(&mut *tx)
    .await?;
    let (token, token_hash) = invitation::new_token(id)?;
    let invitation_id = InvitationId::new();
    let row = sqlx::query!(
        r#"INSERT INTO workspaces.invitations
               (id, workspace_id, email, role, token_hash, expires_at, invited_by)
           VALUES ($1, $2, $3, $4, $5, now() + make_interval(days => $6), $7)
           RETURNING created_at AS "created_at: DateTime<Utc>",
                     expires_at AS "expires_at: DateTime<Utc>""#,
        invitation_id as InvitationId,
        id as WorkspaceId,
        email,
        input.role.as_str(),
        token_hash,
        i32::try_from(VALIDITY.num_days()).unwrap_or(i32::MAX),
        actor as UserId,
    )
    .fetch_one(&mut *tx)
    .await?;
    security::record(
        &mut tx,
        Some(id),
        Some(actor),
        SecurityKind::InvitationCreated,
        serde_json::json!({ "invitation": invitation_id, "role": input.role }),
    )
    .await?;
    tx.commit().await?;
    Ok(Issued {
        invitation: InvitationView {
            id: invitation_id,
            email,
            role: input.role,
            invited_by: actor,
            created_at: row.created_at,
            expires_at: row.expires_at,
        },
        token,
    })
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use sqlx::PgPool;

    use crate::testkit::Kit;

    #[sqlx::test(migrations = "../../../migrations")]
    async fn returns_the_accept_link_once_and_never_lists_it(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let ws = kit.create_workspace(&ana, "Shop").await;
        let uri = format!("/api/v1/w/{ws}/invitations");
        let created = kit
            .post(
                &ana,
                &uri,
                serde_json::json!({ "email": "bea@example.com" }),
            )
            .await;
        assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.json);
        let link = created.json["accept_url"].as_str().unwrap().to_owned();
        let token = link
            .strip_prefix(&format!(
                "{}/invitations/accept?token=",
                kit.state.config.public_url.trim_end_matches('/')
            ))
            .expect("a link on the public origin")
            .to_owned();
        assert!(token.len() >= 32, "{token}");

        // Listing invitations never shows the link or the token.
        let list = kit.get(&ana, &uri).await;
        let listed = list.json.to_string();
        assert!(!listed.contains(&token), "{listed}");
        assert!(
            !listed.contains("accept_url") && !listed.contains("token"),
            "{listed}"
        );

        // Inviting again returns a fresh link; only the new one works.
        let again = kit
            .post(
                &ana,
                &uri,
                serde_json::json!({ "email": "bea@example.com" }),
            )
            .await;
        let fresh = again.json["accept_url"].as_str().unwrap().to_owned();
        assert_ne!(fresh, link);
        let bea = kit.user("bea");
        let stale = kit
            .post(
                &bea,
                "/api/v1/workspaces/invitations/accept",
                serde_json::json!({ "token": token }),
            )
            .await;
        assert_eq!(stale.status, StatusCode::NOT_FOUND);
        let fresh_token = fresh.rsplit_once("token=").unwrap().1;
        let accepted = kit
            .post(
                &bea,
                "/api/v1/workspaces/invitations/accept",
                serde_json::json!({ "token": fresh_token }),
            )
            .await;
        assert_eq!(accepted.status, StatusCode::OK, "{:?}", accepted.json);
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn invites_without_exposing_the_token_outside_the_link(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let ws = kit.create_workspace(&ana, "Shop").await;
        let response = kit
            .post(
                &ana,
                &format!("/api/v1/w/{ws}/invitations"),
                serde_json::json!({ "email": " Bea@Example.com " }),
            )
            .await;
        assert_eq!(response.status, StatusCode::CREATED, "{:?}", response.json);
        assert_eq!(response.json["email"], "bea@example.com");
        assert_eq!(response.json["role"], "member");
        let fields: Vec<_> = response.json.as_object().unwrap().keys().cloned().collect();
        assert!(!fields.iter().any(|f| f.contains("token")), "{fields:?}");
        assert!(response.json["accept_url"].is_string());
        assert_eq!(
            kit.security_kinds(&ws.to_string()).await.last().unwrap(),
            "invitation_created"
        );
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn reinviting_replaces_the_pending_invitation(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let ws = kit.create_workspace(&ana, "Shop").await;
        let first = kit.invite(&ana, ws, "bea@example.com", "member").await;
        kit.invite(&ana, ws, "bea@example.com", "admin").await;
        let list = kit.get(&ana, &format!("/api/v1/w/{ws}/invitations")).await;
        let invitations = list.json["invitations"].as_array().unwrap();
        assert_eq!(invitations.len(), 1);
        assert_eq!(invitations[0]["role"], "admin");
        // The replaced token no longer works.
        let response = kit
            .post(
                &kit.user("bea"),
                "/api/v1/workspaces/invitations/accept",
                serde_json::json!({ "token": first }),
            )
            .await;
        assert_eq!(response.status, StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn refuses_owners_bad_emails_members_and_non_managers(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let ws = kit.create_workspace(&ana, "Shop").await;
        let cid = kit.join(&ana, ws, "cid", "member").await;
        let uri = format!("/api/v1/w/{ws}/invitations");

        let owner = kit
            .post(
                &ana,
                &uri,
                serde_json::json!({ "email": "x@example.com", "role": "owner" }),
            )
            .await;
        assert_eq!(owner.status, StatusCode::BAD_REQUEST);
        let bad = kit
            .post(&ana, &uri, serde_json::json!({ "email": "nope" }))
            .await;
        assert_eq!(bad.status, StatusCode::BAD_REQUEST);
        let member = kit
            .post(
                &ana,
                &uri,
                serde_json::json!({ "email": "CID@example.com" }),
            )
            .await;
        assert_eq!(member.status, StatusCode::CONFLICT);
        assert_eq!(member.json["code"], "already_a_member");
        let not_manager = kit
            .post(
                &cid,
                &uri,
                serde_json::json!({ "email": "dan@example.com" }),
            )
            .await;
        assert_eq!(not_manager.status, StatusCode::FORBIDDEN);
    }
}
