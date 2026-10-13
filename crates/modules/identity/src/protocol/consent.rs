//! The consent screen's endpoints: what it shows for a waiting request, and
//! the person's answer.
//!
//! A request waits here when a client other than the web app asked and no
//! remembered consent covers it (see [`super::authorize`]). Only the browser
//! signed in as the person it was put to may see or answer it; allowing
//! needs a sign-in in the last ten minutes and, for a client bound to a
//! workspace, a workspace the person belongs to (a workspace's own client:
//! that workspace only).

use std::{collections::BTreeSet, time::Duration};

use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
    response::{IntoResponse as _, Response},
};
use campbooks_kernel::{
    Error, Operation, Result, Risk,
    auth::{Principal, PrincipalKind},
    rate_limit::{ClientIp, Limit, RateKey},
    tenancy::WorkspaceId,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::{Ctx, authorize};
use crate::{
    MESSAGES,
    deps::Issuer,
    domain::{
        accounts,
        clients::{Client, Kind, is_device_redirect, is_https_redirect, redirect_host},
        consents::{self, NewConsent},
        sessions::{self, Session},
        step_up::STEP_UP_WINDOW,
    },
    events::ConsentGranted,
    web,
};

/// Answers one address may send.
const CONSENT_LIMIT: Limit = Limit::new("identity.consent", 30, Duration::from_mins(1));

/// The app asking.
#[derive(Debug, Serialize, ToSchema)]
pub struct ConsentClient {
    /// Its client id.
    pub id: String,
    /// Its name, as it registered it (only trustworthy when `verified`).
    pub name: String,
    /// `first_party`, `workspace`, `dynamic` or `metadata_document`.
    pub kind: String,
    /// Whether it is Campbooks' own app. Every other app's name is its
    /// registrant's word.
    pub verified: bool,
    /// For a workspace's own app, the workspace that registered it (its
    /// name, when the person belongs to it): what the screen says instead of
    /// "verified".
    pub workspace_name: Option<String>,
    /// Its logo (a workspace's own app only).
    pub logo_uri: Option<String>,
    /// Where the browser goes next: the redirect URI's host (`None` for a
    /// private-use scheme).
    pub redirect_host: Option<String>,
    /// For a metadata-document client, the host serving its document.
    pub document_host: Option<String>,
    /// The redirect goes to an app on this device (loopback or a
    /// private-use scheme): it is asked every time, and warned about.
    pub device: bool,
}

/// One thing the app may do.
#[derive(Debug, Serialize, ToSchema)]
pub struct ConsentScope {
    /// The scope's name (`conversations:read`, `openid`…).
    pub name: String,
    /// What it allows, in English (the screen translates by name).
    pub description: String,
}

/// A workspace the person may let the app use.
#[derive(Debug, Serialize, ToSchema)]
pub struct ConsentWorkspace {
    /// The workspace.
    pub id: WorkspaceId,
    /// Its name.
    pub name: String,
    /// The person's role there.
    pub role: campbooks_kernel::tenancy::Role,
}

/// What the consent screen shows.
#[derive(Debug, Serialize, ToSchema)]
pub struct ConsentRequest {
    /// The app.
    pub client: ConsentClient,
    /// What it asks to do.
    pub scopes: Vec<ConsentScope>,
    /// Whether the person picks one workspace for it; `false` for
    /// Campbooks' own apps, which work in all of them.
    pub binds_workspace: bool,
    /// The workspaces it may be allowed in (a workspace's own app: its
    /// workspace only).
    pub workspaces: Vec<ConsentWorkspace>,
    /// The signed-in person's name.
    pub account_name: Option<String>,
    /// Their address.
    pub account_email: Option<String>,
    /// Whether "Allow" will be remembered (an exact `https` redirect).
    pub remembered: bool,
}

/// The descriptions of the scopes that are not the API's.
const OIDC_DESCRIPTIONS: [(&str, &str); 4] = [
    ("openid", "Know who you are on Campbooks."),
    ("profile", "See your name."),
    ("email", "See your email address."),
    (
        "offline_access",
        "Keep access when you are not using it, until you revoke it.",
    ),
];

/// The live session in this browser and the request `id` waiting for its
/// answer, with its client; 404 for anything else.
async fn waiting(
    ctx: &Ctx,
    issuer: &Issuer,
    headers: &HeaderMap,
    id: &str,
) -> Result<(Session, authorize::Request, Client)> {
    let now = issuer.now();
    let token = issuer.session_cookie(headers).ok_or(Error::NotFound)?;
    let session = sessions::current(&ctx.state.db, &token, now)
        .await?
        .ok_or(Error::NotFound)?;
    let request = authorize::load(
        &ctx.state.db,
        id,
        authorize::browser(issuer, headers).as_deref(),
        now,
    )
    .await?
    .filter(|r| r.consent_due && r.consent_account_id == Some(session.account_id))
    .ok_or(Error::NotFound)?;
    let client = issuer
        .clients
        .find(&ctx.state.db, &request.client_id)
        .await?
        .filter(Client::needs_consent)
        .ok_or(Error::NotFound)?;
    Ok((session, request, client))
}

fn principal_of(account: Uuid) -> Principal {
    Principal {
        subject: account.to_string(),
        kind: PrincipalKind::User,
        client_id: None,
        scopes: BTreeSet::new(),
        workspace: None,
        email: None,
        email_verified: false,
        name: None,
        expires_at: None,
        auth_time: None,
    }
}

/// What the consent screen shows for the waiting request `id`.
#[utoipa::path(
    get,
    path = "/auth/consent/{id}",
    tag = "identity",
    operation_id = "identity.get_consent_request",
    extensions(("x-campbooks" = json!(Operation::query().records(["identity.authorize_request"])))),
    params(("id" = String, Path, description = "The `request` the consent screen was given.")),
    security(()),
    responses(
        (status = 200, description = "The request.", body = ConsentRequest),
        (status = 404, description = "No such request waiting for this browser's person, or \
            another identity provider serves this instance."),
    ),
)]
pub async fn get(
    State(ctx): State<Ctx>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response> {
    let issuer = ctx.issuer()?;
    let (session, request, client) = waiting(&ctx, issuer, &headers, &id).await?;
    let mine = ctx
        .state
        .membership
        .workspaces_of(&ctx.state.db, &session.account_id.to_string())
        .await?;
    let workspace_name = client.workspace.and_then(|own| {
        mine.iter()
            .find(|w| *w.id.as_uuid() == own)
            .map(|w| w.name.clone())
    });
    let workspaces = if client.binds_workspace() {
        mine.into_iter()
            .filter(|w| client.workspace.is_none_or(|own| *w.id.as_uuid() == own))
            .map(|w| ConsentWorkspace {
                id: w.id,
                name: w.name,
                role: w.role,
            })
            .collect()
    } else {
        Vec::new()
    };
    let scopes = request
        .scope
        .split_whitespace()
        .map(|name| ConsentScope {
            name: name.to_owned(),
            description: OIDC_DESCRIPTIONS
                .iter()
                .find(|(scope, _)| *scope == name)
                .map(|(_, d)| (*d).to_owned())
                .or_else(|| {
                    issuer
                        .api_scopes
                        .iter()
                        .find(|s| s.name == name)
                        .map(|s| s.description.to_owned())
                })
                .unwrap_or_default(),
        })
        .collect();
    let profile = accounts::profile(&ctx.state.db, session.account_id).await?;
    let binds_workspace = client.binds_workspace();
    let body = ConsentRequest {
        client: ConsentClient {
            name: client.name.clone(),
            kind: client.kind.as_str().to_owned(),
            verified: client.is_first_party(),
            workspace_name,
            logo_uri: client.logo_uri.clone(),
            redirect_host: redirect_host(&request.redirect_uri),
            document_host: (client.kind == Kind::MetadataDocument)
                .then(|| redirect_host(&client.id))
                .flatten(),
            device: is_device_redirect(&request.redirect_uri),
            id: client.id,
        },
        scopes,
        binds_workspace,
        workspaces,
        account_name: profile.as_ref().and_then(|p| p.display_name.clone()),
        account_email: profile.and_then(|p| p.primary_email),
        remembered: is_https_redirect(&request.redirect_uri),
    };
    Ok(web::no_store(Json(body).into_response()))
}

/// Allow or deny.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    /// Let the app in.
    Allow,
    /// Don't.
    Deny,
}

/// The person's answer.
#[derive(Debug, Deserialize, ToSchema)]
pub struct Answer {
    /// Allow or deny.
    pub decision: Decision,
    /// The workspace the app may use (required to allow an app that binds
    /// one).
    pub workspace_id: Option<WorkspaceId>,
}

/// Where to go next.
#[derive(Debug, Serialize, ToSchema)]
pub struct Answered {
    /// Navigate here: the app's redirect URI, with a code or
    /// `error=access_denied`.
    pub redirect: String,
}

/// Answer the waiting request `id`.
#[utoipa::path(
    post,
    path = "/auth/consent/{id}",
    tag = "identity",
    operation_id = "identity.answer_consent",
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).records(["identity.consent", "identity.authorization_code"])))),
    params(
        ("id" = String, Path, description = "The `request` the consent screen was given."),
        ("x-campbooks-csrf" = String, Header, description = "Must be `1`."),
    ),
    request_body = Answer,
    security(()),
    responses(
        (status = 200, description = "Answered.", body = Answered),
        (status = 400, description = "`consent_needs_workspace`.", body = campbooks_kernel::Problem,
            content_type = "application/problem+json"),
        (status = 403, description = "`csrf_check_failed`, `step_up_required` (sign in again: \
            the sign-in is older than ten minutes), or `consent_wrong_workspace`.",
            body = campbooks_kernel::Problem, content_type = "application/problem+json"),
        (status = 404, description = "No such request waiting for this browser's person, or a \
            workspace the person does not belong to."),
        (status = 429, description = "`rate_limited`.", body = campbooks_kernel::Problem,
            content_type = "application/problem+json"),
    ),
)]
pub async fn answer(
    State(ctx): State<Ctx>,
    Path(id): Path<String>,
    ip: ClientIp,
    headers: HeaderMap,
    Json(answer): Json<Answer>,
) -> Result<Response> {
    let issuer = ctx.issuer()?;
    if !web::csrf_passes(&headers, &issuer.url) {
        return Err(Error::Denied(MESSAGES.reason("csrf_check_failed")));
    }
    ctx.state
        .rate_limits
        .check(&ctx.state.db, &CONSENT_LIMIT, &RateKey::ip(ip))
        .await?;
    let (session, _, client) = waiting(&ctx, issuer, &headers, &id).await?;
    let now = issuer.now();
    if answer.decision == Decision::Allow && now - session.auth_time > STEP_UP_WINDOW {
        return Err(Error::Denied(MESSAGES.reason("step_up_required")));
    }
    let (workspace, member) = match (answer.decision, client.binds_workspace()) {
        (Decision::Deny, _) | (Decision::Allow, false) => (None, None),
        (Decision::Allow, true) => {
            let workspace = answer
                .workspace_id
                .ok_or_else(|| Error::BadRequest(MESSAGES.reason("consent_needs_workspace")))?;
            if client
                .workspace
                .is_some_and(|own| own != *workspace.as_uuid())
            {
                return Err(Error::Denied(MESSAGES.reason("consent_wrong_workspace")));
            }
            let access = ctx
                .state
                .membership
                .resolve(&ctx.state.db, &principal_of(session.account_id), workspace)
                .await?
                .filter(|access| access.workspace == workspace)
                .ok_or(Error::NotFound)?;
            (
                Some(*workspace.as_uuid()),
                access.user.map(|user| *user.as_uuid()),
            )
        }
    };
    let mut tx = ctx.state.db.begin().await?;
    let request = authorize::take(
        &mut tx,
        &id,
        authorize::browser(issuer, &headers).as_deref(),
        now,
    )
    .await?
    .filter(|r| r.consent_due && r.consent_account_id == Some(session.account_id))
    .ok_or(Error::NotFound)?;
    if answer.decision == Decision::Deny {
        tx.commit().await?;
        let redirect = request.redirect(&issuer.url, &[("error", "access_denied")]);
        return Ok(web::no_store(Json(Answered { redirect }).into_response()));
    }
    let scopes: Vec<String> = request
        .scope
        .split_whitespace()
        .map(ToOwned::to_owned)
        .collect();
    consents::grant(
        &mut tx,
        &NewConsent {
            account_id: session.account_id,
            client_id: &client.id,
            workspace,
            member,
            scopes: &scopes,
            remembered: is_https_redirect(&request.redirect_uri),
            redirect_uri: &request.redirect_uri,
        },
        now,
    )
    .await?;
    accounts::audit(
        &mut tx,
        Some(session.account_id),
        "consent_granted",
        serde_json::json!({ "client": client.id, "kind": client.kind.as_str(), "workspace": workspace }),
        now,
    )
    .await?;
    ctx.state
        .events
        .publish(
            &mut tx,
            &ConsentGranted {
                account: session.account_id,
                client: client.id.clone(),
                workspace: workspace.map(|w| w.to_string()),
            },
        )
        .await?;
    tx.commit().await?;
    let redirect =
        authorize::issue(&ctx.state.db, issuer, &request, &session, workspace, now).await?;
    Ok(web::no_store(Json(Answered { redirect }).into_response()))
}
