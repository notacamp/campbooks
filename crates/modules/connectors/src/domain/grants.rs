//! Turning a provider grant that is already in hand into a connection: the
//! one seam every way of obtaining a grant goes through. The connector's own
//! consent flow (`connectors.complete_connection`) calls it today; a sign-in
//! flow that obtains the same grant can call it later without the
//! connector's start/callback.

use campbooks_kernel::{
    AppState, Error, Result,
    blobs::{BlobOwner, BlobRef},
    db::BeginFor as _,
    tenancy::{UserId, WorkspaceId},
};
use chrono::{DateTime, Utc};
use sqlx::{Postgres, Transaction};

use super::{
    connections::{self, parse_capabilities},
    credentials,
    model::{Capability, ConnectionId, ConnectionView, InboxId, Phase, Provider, StreamId},
    removal::CONNECTION_OWNER,
    schedule,
};
use crate::{
    MESSAGES,
    deps::Deps,
    events::{ACCOUNT_CONTENT_TYPE, AccountDocument, CapabilityGranted, ConnectionAdded},
    google::TokenGrant,
};

/// A provider grant already obtained for a member.
#[derive(Debug, Clone)]
pub struct ProviderGrant {
    /// The workspace the connection belongs to.
    pub workspace: WorkspaceId,
    /// The member whose grant it is.
    pub user: UserId,
    /// The provider.
    pub provider: Provider,
    /// The provider's stable account id (the `OpenID` `sub`).
    pub account_subject: String,
    /// The account's verified email address.
    pub address: String,
    /// The account's display name.
    pub display_name: Option<String>,
    /// The capabilities granted (and wanted); never empty.
    pub capabilities: Vec<Capability>,
    /// The tokens: access, refresh (required for a new connection), scopes.
    pub tokens: TokenGrant,
    /// The connection this consent extends, when it was started as one.
    pub extending: Option<ConnectionId>,
}

/// What [`save`] stores.
struct Pending<'a> {
    grant: &'a ProviderGrant,
    account_blob: &'a BlobRef,
    now: DateTime<Utc>,
}

/// Create the connection of `grant`, or refresh the existing one of the
/// same account in the workspace: store its credentials sealed, announce it
/// (`connection_added`, `capability_granted` with the account document),
/// create the streams of new capabilities, resume paused ones, and start
/// syncing. Refuses an account another member connected, or one that does
/// not match the connection being extended.
pub async fn create_or_refresh_connection(
    state: &AppState,
    deps: &Deps,
    grant: &ProviderGrant,
) -> Result<ConnectionView> {
    let document = AccountDocument {
        version: 1,
        provider: grant.provider,
        address: grant.address.trim().to_lowercase(),
        display_name: grant.display_name.clone(),
    };
    let account_blob = state
        .blobs
        .put(
            &grant.workspace.to_string(),
            serde_json::to_vec(&document).map_err(Error::internal)?,
            ACCOUNT_CONTENT_TYPE,
        )
        .await?;
    let mut tx = state.db.begin_for(grant.workspace).await?;
    let pending = Pending {
        grant,
        account_blob: &account_blob,
        now: deps.clock.now(),
    };
    let id = save(&mut tx, state, deps, &pending).await?;
    let connection = connections::load(&mut tx, id)
        .await?
        .ok_or(Error::NotFound)?;
    let view = connections::view(&mut tx, connection).await?;
    tx.commit().await?;
    Ok(view)
}

/// Create or extend the connection, store its credentials, create the
/// streams of new capabilities and start them. Returns the connection.
async fn save(
    tx: &mut Transaction<'static, Postgres>,
    state: &AppState,
    deps: &Deps,
    g: &Pending<'_>,
) -> Result<ConnectionId> {
    let cipher = deps
        .cipher
        .as_deref()
        .ok_or_else(|| Error::Conflict(MESSAGES.reason("provider_not_configured")))?;
    let workspace_key = g.grant.workspace.to_string();
    let existing = existing(tx, g).await?;
    let had: Vec<Capability> = existing
        .as_ref()
        .map(|(_, had)| had.clone())
        .unwrap_or_default();
    let id = existing
        .as_ref()
        .map_or_else(ConnectionId::new, |(id, _)| *id);
    let mut capabilities = had.clone();
    capabilities.extend_from_slice(&g.grant.capabilities);
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
        g.grant.workspace as WorkspaceId,
        g.grant.provider.as_str(),
        g.grant.account_subject,
        g.grant.address,
        g.grant.display_name,
        g.grant.user as UserId,
        &capability_names,
        g.grant.tokens.scope.clone().unwrap_or_default(),
        g.now,
    )
    .execute(&mut **tx)
    .await?;
    credentials::store(
        tx,
        cipher,
        g.grant.workspace,
        id,
        g.grant.tokens.refresh_token.as_deref(),
        &g.grant.tokens,
        g.now,
    )
    .await?;
    state
        .blobs
        .attach(
            tx,
            &workspace_key,
            g.account_blob,
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
                    provider: g.grant.provider,
                    granted_by: g.grant.user,
                },
            )
            .await?;
    }
    start_streams(tx, state, deps, g, id, &had).await?;
    Ok(id)
}

/// The workspace's connection of this account (its id and capabilities),
/// locked, after checking the consent may land on it.
async fn existing(
    tx: &mut Transaction<'static, Postgres>,
    g: &Pending<'_>,
) -> Result<Option<(ConnectionId, Vec<Capability>)>> {
    let existing = sqlx::query!(
        r#"SELECT id AS "id: ConnectionId", granted_by AS "granted_by: UserId", capabilities
             FROM connectors.connections
            WHERE workspace_id = $1 AND provider = $2 AND account_subject = $3
              FOR UPDATE"#,
        g.grant.workspace as WorkspaceId,
        g.grant.provider.as_str(),
        g.grant.account_subject,
    )
    .fetch_optional(&mut **tx)
    .await?;
    if let Some(wanted) = g.grant.extending
        && existing.as_ref().is_none_or(|e| e.id != wanted)
    {
        return Err(Error::Conflict(MESSAGES.reason("account_mismatch")));
    }
    if existing
        .as_ref()
        .is_some_and(|e| e.granted_by != g.grant.user)
    {
        return Err(Error::Conflict(
            MESSAGES.reason("account_connected_by_another_member"),
        ));
    }
    if existing.is_none() && g.grant.tokens.refresh_token.is_none() {
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
    g: &Pending<'_>,
    id: ConnectionId,
    had: &[Capability],
) -> Result<()> {
    let workspace_key = g.grant.workspace.to_string();
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
        schedule::run_now(tx, g.grant.workspace, stream, g.now).await?;
    }
    let min_interval = i32::try_from(deps.config.poll.min_seconds).unwrap_or(i32::MAX);
    for &capability in g.grant.capabilities.iter().filter(|c| !had.contains(c)) {
        let stream = StreamId::new();
        let inbox = (capability == Capability::Messages).then(InboxId::new);
        sqlx::query!(
            r#"INSERT INTO connectors.streams
                   (id, workspace_id, connection_id, capability, inbox_id, phase, interval_seconds, created_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
               ON CONFLICT (connection_id, capability) DO NOTHING"#,
            stream as StreamId,
            g.grant.workspace as WorkspaceId,
            id as ConnectionId,
            capability.as_str(),
            inbox as Option<InboxId>,
            Phase::Backfill.as_str(),
            min_interval,
            g.now,
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
                    granted_by: g.grant.user,
                    inbox_id: inbox,
                    account: g.account_blob.clone(),
                },
            )
            .await?;
        schedule::run_now(tx, g.grant.workspace, stream, g.now).await?;
    }
    Ok(())
}
