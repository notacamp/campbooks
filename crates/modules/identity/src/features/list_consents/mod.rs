//! `GET /api/v1/identity/consents` — the apps the caller let in: which
//! workspace each may use, what it may do, when it was allowed and last
//! used. Settings → Apps and integrations lists them with Revoke.

use axum::{Extension, Json, extract::State};
use campbooks_kernel::{AppState, Operation, Problem, Result, auth::Scoped};
use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    deps::Deps,
    domain::{clients::Kind, consents},
    scopes::IdentityRead,
};

/// An app the caller let in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct ConnectedApp {
    /// The consent (what `identity.revoke_consent` takes).
    pub id: Uuid,
    /// The app's client id.
    pub client_id: String,
    /// Its name; `null` when the app is gone (its tokens no longer work).
    pub client_name: Option<String>,
    /// `first_party`, `workspace`, `dynamic` or `metadata_document`.
    pub client_kind: Option<String>,
    /// Whether Campbooks or a workspace admin registered it.
    pub verified: bool,
    /// The workspace it may use; `null` for Campbooks' own apps (all).
    pub workspace_id: Option<Uuid>,
    /// What it may do.
    pub scopes: Vec<String>,
    /// When it was allowed.
    pub granted_at: DateTime<Utc>,
    /// When it last got a token.
    pub last_used_at: Option<DateTime<Utc>>,
}

/// The apps the caller let in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct ConnectedApps {
    /// Newest first.
    pub items: Vec<ConnectedApp>,
}

/// List the apps you let in.
#[utoipa::path(
    get,
    path = "/api/v1/identity/consents",
    operation_id = "identity.list_consents",
    tag = "identity",
    security(("bearer" = ["identity:read"])),
    extensions(("x-campbooks" = json!(Operation::query().records(["identity.consent"])))),
    responses(
        (status = OK, description = "The caller's connected apps.", body = ConnectedApps),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "The token lacks `identity:read`.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "Another identity provider serves this instance.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    Extension(deps): Extension<Deps>,
    Scoped { principal, .. }: Scoped<IdentityRead>,
) -> Result<Json<ConnectedApps>> {
    let (issuer, account) = super::account(&deps, &principal)?;
    let mut items = Vec::new();
    for consent in consents::of_account(&state.db, account).await? {
        let client = issuer.clients.find(&state.db, &consent.client_id).await?;
        items.push(ConnectedApp {
            id: consent.id,
            client_name: client.as_ref().map(|c| c.name.clone()),
            client_kind: client.as_ref().map(|c| c.kind.as_str().to_owned()),
            verified: client
                .as_ref()
                .is_some_and(|c| matches!(c.kind, Kind::FirstParty | Kind::Workspace)),
            client_id: consent.client_id,
            workspace_id: consent.bound_workspace_id,
            scopes: consent.scopes,
            granted_at: consent.granted_at,
            last_used_at: consent.last_used_at,
        });
    }
    Ok(Json(ConnectedApps { items }))
}
