//! `POST /api/v1/w/{workspace_id}/connections/authorizations/complete` —
//! finish a consent: called by the web app's callback page, as the member
//! who started it, with the `state` and `code` Google redirected with.
//!
//! The authorization is single use and bound to that member and workspace:
//! anyone else gets 404 and the code is never exchanged, so a consent can
//! never land in someone else's workspace. Completing exchanges the code
//! (PKCE), reads the account, stores the credentials sealed, creates the
//! streams of newly granted capabilities and starts syncing them.

use axum::{Extension, Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    blobs::{BlobOwner, BlobRef},
    crypto,
    db::BeginFor as _,
    tenancy::{Access, UserId, Workspace, WorkspaceId},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Transaction};
use utoipa::ToSchema;

use crate::{
    MESSAGES,
    deps::Deps,
    domain::{
        connections::{self, parse_capabilities},
        credentials,
        model::{
            AuthorizationId, Capability, ConnectionId, ConnectionView, InboxId, Phase, Provider,
            StreamId,
        },
        removal::CONNECTION_OWNER,
        schedule,
    },
    events::{ACCOUNT_CONTENT_TYPE, AccountDocument, CapabilityGranted, ConnectionAdded},
    features::start_connection::{acting_member, open_verifier},
    google::{self, Account, GoogleError, TokenGrant},
    scopes::ConnectorsWrite,
};

/// What Google redirected the browser with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CompleteConnection {
    /// The `state` query parameter.
    pub state: String,
    /// The `code` query parameter.
    pub code: String,
}

/// Complete a connection.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/connections/authorizations/complete",
    operation_id = "connectors.complete_connection",
    tag = "connectors",
    security(("bearer" = ["connectors:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).records(["connectors.connection"])))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    request_body = CompleteConnection,
    responses(
        (status = OK, description = "The connection, syncing.", body = ConnectionView),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope; a client token \
            (`connection_needs_user`); no capability granted (`consent_incomplete`); or no \
            offline access (`no_offline_access`).", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such started connection of yours in this \
            workspace, or it expired.", body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "The code was refused (`authorization_code_invalid`), \
            another account consented (`account_mismatch`), another member connected this \
            account (`account_connected_by_another_member`), or the provider is not \
            configured.", body = Problem, content_type = "application/problem+json"),
        (status = SERVICE_UNAVAILABLE, description = "The provider could not be reached \
            (`provider_unavailable`).", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    Extension(deps): Extension<Deps>,
    workspace: Workspace<ConnectorsWrite>,
    Json(input): Json<CompleteConnection>,
) -> Result<Json<ConnectionView>> {
    execute(&state, &deps, &workspace.access, input)
        .await
        .map(Json)
}

/// A started consent, taken (single use).
struct Started {
    id: AuthorizationId,
    provider: Provider,
    capabilities: Vec<Capability>,
    connection: Option<ConnectionId>,
    verifier: Vec<u8>,
}

/// Complete the consent `input` for the member of `access`.
pub async fn execute(
    state: &AppState,
    deps: &Deps,
    access: &Access,
    input: CompleteConnection,
) -> Result<ConnectionView> {
    let user = acting_member(access)?;
    let (Some(google), Some(cipher)) = (&deps.google, &deps.cipher) else {
        return Err(Error::Conflict(MESSAGES.reason("provider_not_configured")));
    };
    let now = deps.clock.now();
    let started = take(state, access.workspace, user, &input.state, now)
        .await?
        .ok_or(Error::NotFound)?;
    let verifier = open_verifier(cipher, started.id, &started.verifier).ok_or(Error::NotFound)?;
    let grant = google
        .exchange(&input.code, &verifier)
        .await
        .map_err(|err| match err {
            GoogleError::InvalidGrant => {
                Error::Conflict(MESSAGES.reason("authorization_code_invalid"))
            }
            other => provider_error(&other),
        })?;
    let account = google
        .account(&grant.access_token)
        .await
        .map_err(|err| provider_error(&err))?;
    let granted: Vec<Capability> = google::granted_capabilities(&grant.scopes())
        .into_iter()
        .filter(|c| started.capabilities.contains(c))
        .collect();
    if granted.is_empty() {
        return Err(Error::Denied(MESSAGES.reason("consent_incomplete")));
    }
    let address = account.email.trim().to_lowercase();
    let document = AccountDocument {
        version: 1,
        provider: started.provider,
        address: address.clone(),
        display_name: account.name.clone(),
    };
    let account_blob = state
        .blobs
        .put(
            &access.workspace.to_string(),
            serde_json::to_vec(&document).map_err(Error::internal)?,
            ACCOUNT_CONTENT_TYPE,
        )
        .await?;

    let mut tx = state.db.begin_for(access.workspace).await?;
    let id = save(
        &mut tx,
        state,
        deps,
        &Saving {
            workspace: access.workspace,
            user,
            started: &started,
            account: &account,
            address: &address,
            grant: &grant,
            granted: &granted,
            account_blob: &account_blob,
            now,
        },
    )
    .await?;
    let connection = connections::load(&mut tx, id)
        .await?
        .ok_or(Error::NotFound)?;
    let view = connections::view(&mut tx, connection).await?;
    tx.commit().await?;
    tracing::info!(workspace = %access.workspace, %user, connection = %id, "connection completed");
    Ok(view)
}

/// A provider failure, as the member sees it.
fn provider_error(err: &GoogleError) -> Error {
    tracing::warn!(error = %err, "completing a connection failed at the provider");
    Error::Unavailable(MESSAGES.reason("provider_unavailable"))
}

/// Take the caller's unexpired authorization with `state`, consuming it.
async fn take(
    state: &AppState,
    workspace: WorkspaceId,
    user: UserId,
    state_token: &str,
    now: DateTime<Utc>,
) -> Result<Option<Started>> {
    let mut tx = state.db.begin_for(workspace).await?;
    let row = sqlx::query!(
        r#"DELETE FROM connectors.authorizations
            WHERE state_hash = $1 AND user_id = $2
        RETURNING id AS "id: AuthorizationId", provider, capabilities,
                  connection_id AS "connection: ConnectionId", pkce_verifier, expires_at"#,
        crypto::hash(state_token),
        user as UserId,
    )
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    let Some(row) = row.filter(|row| row.expires_at > now) else {
        return Ok(None);
    };
    Ok(Some(Started {
        id: row.id,
        provider: Provider::parse(&row.provider)?,
        capabilities: parse_capabilities(&row.capabilities)?,
        connection: row.connection,
        verifier: row.pkce_verifier,
    }))
}

/// What [`save`] stores.
struct Saving<'a> {
    workspace: WorkspaceId,
    user: UserId,
    started: &'a Started,
    account: &'a Account,
    address: &'a str,
    grant: &'a TokenGrant,
    granted: &'a [Capability],
    account_blob: &'a BlobRef,
    now: DateTime<Utc>,
}

/// Create or extend the connection, store its credentials, create the
/// streams of new capabilities and start them. Returns the connection.
async fn save(
    tx: &mut Transaction<'static, Postgres>,
    state: &AppState,
    deps: &Deps,
    s: &Saving<'_>,
) -> Result<ConnectionId> {
    let cipher = deps
        .cipher
        .as_deref()
        .ok_or_else(|| Error::Conflict(MESSAGES.reason("provider_not_configured")))?;
    let workspace_key = s.workspace.to_string();
    let existing = existing(tx, s).await?;
    let had: Vec<Capability> = existing
        .as_ref()
        .map(|(_, had)| had.clone())
        .unwrap_or_default();
    let id = existing
        .as_ref()
        .map_or_else(ConnectionId::new, |(id, _)| *id);
    let mut capabilities = had.clone();
    capabilities.extend_from_slice(s.granted);
    capabilities.sort_unstable();
    capabilities.dedup();
    let capability_names: Vec<String> =
        capabilities.iter().map(|c| c.as_str().to_owned()).collect();
    sqlx::query!(
        r#"INSERT INTO connectors.connections
               (id, workspace_id, provider, account_subject, address, display_name, granted_by,
                capabilities, scopes, health, health_reason, health_changed_at, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'ok', NULL, $10, $10, $10)
           ON CONFLICT (id) DO UPDATE
              SET address = $5, display_name = $6, capabilities = $8, scopes = $9,
                  health = 'ok', health_reason = NULL,
                  health_changed_at = CASE WHEN connectors.connections.health = 'ok'
                                           THEN connectors.connections.health_changed_at ELSE $10 END,
                  updated_at = $10"#,
        id as ConnectionId,
        s.workspace as WorkspaceId,
        s.started.provider.as_str(),
        s.account.sub,
        s.address,
        s.account.name,
        s.user as UserId,
        &capability_names,
        s.grant.scope.clone().unwrap_or_default(),
        s.now,
    )
    .execute(&mut **tx)
    .await?;
    credentials::store(
        tx,
        cipher,
        s.workspace,
        id,
        s.grant.refresh_token.as_deref(),
        s.grant,
        s.now,
    )
    .await?;
    state
        .blobs
        .attach(
            tx,
            &workspace_key,
            s.account_blob,
            &BlobOwner::new(CONNECTION_OWNER, id),
        )
        .await?;
    if existing.is_none() {
        state
            .events
            .publish(
                tx,
                &ConnectionAdded {
                    workspace: workspace_key.clone(),
                    connection: id,
                    provider: s.started.provider,
                    granted_by: s.user,
                },
            )
            .await?;
    }
    start_streams(tx, state, deps, s, id, &had).await?;
    Ok(id)
}

/// The workspace's connection of this account (its id and capabilities),
/// locked, after checking the consent may land on it.
async fn existing(
    tx: &mut Transaction<'static, Postgres>,
    s: &Saving<'_>,
) -> Result<Option<(ConnectionId, Vec<Capability>)>> {
    let existing = sqlx::query!(
        r#"SELECT id AS "id: ConnectionId", granted_by AS "granted_by: UserId", capabilities
             FROM connectors.connections
            WHERE workspace_id = $1 AND provider = $2 AND account_subject = $3
              FOR UPDATE"#,
        s.workspace as WorkspaceId,
        s.started.provider.as_str(),
        s.account.sub,
    )
    .fetch_optional(&mut **tx)
    .await?;
    if let Some(wanted) = s.started.connection
        && existing.as_ref().is_none_or(|e| e.id != wanted)
    {
        return Err(Error::Conflict(MESSAGES.reason("account_mismatch")));
    }
    if existing.as_ref().is_some_and(|e| e.granted_by != s.user) {
        return Err(Error::Conflict(
            MESSAGES.reason("account_connected_by_another_member"),
        ));
    }
    if existing.is_none() && s.grant.refresh_token.is_none() {
        return Err(Error::Denied(MESSAGES.reason("no_offline_access")));
    }
    existing
        .map(|e| Ok((e.id, parse_capabilities(&e.capabilities)?)))
        .transpose()
}

/// Resume a reconnected connection's paused streams, and create, announce
/// and start the streams of capabilities it did not have.
async fn start_streams(
    tx: &mut Transaction<'static, Postgres>,
    state: &AppState,
    deps: &Deps,
    s: &Saving<'_>,
    id: ConnectionId,
    had: &[Capability],
) -> Result<()> {
    let workspace_key = s.workspace.to_string();
    // A reconnect resumes the paused streams.
    let resumed = sqlx::query_scalar!(
        r#"UPDATE connectors.streams SET paused = false, throttled_until = NULL,
                  consecutive_failures = 0
            WHERE connection_id = $1
        RETURNING id AS "id: StreamId""#,
        id as ConnectionId
    )
    .fetch_all(&mut **tx)
    .await?;
    for stream in resumed {
        schedule::run_now(tx, s.workspace, stream, s.now).await?;
    }
    let min_interval = i32::try_from(deps.config.poll.min_seconds).unwrap_or(i32::MAX);
    for &capability in s.granted.iter().filter(|c| !had.contains(c)) {
        let stream = StreamId::new();
        let inbox = (capability == Capability::Messages).then(InboxId::new);
        sqlx::query!(
            r#"INSERT INTO connectors.streams
                   (id, workspace_id, connection_id, capability, inbox_id, phase, interval_seconds, created_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
               ON CONFLICT (connection_id, capability) DO NOTHING"#,
            stream as StreamId,
            s.workspace as WorkspaceId,
            id as ConnectionId,
            capability.as_str(),
            inbox as Option<InboxId>,
            Phase::Backfill.as_str(),
            min_interval,
            s.now,
        )
        .execute(&mut **tx)
        .await?;
        // The inbox is announced before any batch can name it: the first
        // run cannot commit before this transaction does.
        state
            .events
            .publish(
                tx,
                &CapabilityGranted {
                    workspace: workspace_key.clone(),
                    connection: id,
                    capability,
                    granted_by: s.user,
                    inbox_id: inbox,
                    account: s.account_blob.clone(),
                },
            )
            .await?;
        schedule::run_now(tx, s.workspace, stream, s.now).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
