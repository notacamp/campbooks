//! `GET /admin/v1/connectors/workspaces/{workspace_id}/connections` — the
//! connection health shown on Admin → Workspaces → one workspace: each
//! connected account's provider, address, capabilities and health, and how
//! its streams are syncing.
//!
//! An admin operation (ADR 0015). Requires `workspaces.read` (or `super`).
//! Never a credential, a cursor, or anything read from the mailbox.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Problem, Result,
    admin::{Admin, AdminOperation, permission::WorkspacesRead},
    db, extract,
    tenancy::{UserId, WorkspaceId},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use utoipa::ToSchema;

use crate::domain::{
    connections::parse_capabilities,
    model::{Capability, ConnectionId, Health, Phase, Provider},
};

/// A stream's sync state, as an administrator sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct AdminStream {
    /// The capability it reads.
    pub capability: Capability,
    /// Where it is in its reading.
    pub phase: Phase,
    /// Paused until the member connects again.
    pub paused: bool,
    /// When a run last finished.
    pub last_synced_at: Option<DateTime<Utc>>,
    /// How many runs in a row failed.
    pub consecutive_failures: i32,
}

/// A connection, as an administrator sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct AdminConnection {
    /// The connection.
    pub id: ConnectionId,
    /// The provider.
    pub provider: Provider,
    /// The connected account's address.
    pub address: String,
    /// The member who connected it.
    pub granted_by: UserId,
    /// What it lets Campbooks do.
    pub capabilities: Vec<Capability>,
    /// How it is doing.
    pub health: Health,
    /// Why, when it is not `ok` (`grant_revoked`, `rate_limited`, …).
    pub health_reason: Option<String>,
    /// Since when it has had this health.
    pub health_changed_at: DateTime<Utc>,
    /// When it was connected.
    pub created_at: DateTime<Utc>,
    /// Its change streams.
    pub streams: Vec<AdminStream>,
}

/// A workspace's connections.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct AdminConnectionList {
    /// Oldest first.
    pub connections: Vec<AdminConnection>,
}

/// The workspace a path names.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct WorkspacePath {
    /// The workspace.
    pub workspace_id: WorkspaceId,
}

/// See a workspace's connections and how they sync.
#[utoipa::path(
    get,
    path = "/admin/v1/connectors/workspaces/{workspace_id}/connections",
    operation_id = "connectors.admin_list_connections",
    tag = "connectors",
    security(("admin" = ["admin:access"])),
    extensions(("x-campbooks-admin" = json!(AdminOperation::query::<WorkspacesRead>()))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    responses(
        (status = OK, description = "The workspace's connections (none for an unknown \
            workspace).", body = AdminConnectionList),
        (status = UNAUTHORIZED, description = "`admin_reauth_required`: sign in again.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "Not an administrator with `workspaces.read` (the \
            same answer as an unknown route).", body = Problem,
            content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    admin: Admin<WorkspacesRead>,
    State(state): State<AppState>,
    extract::Path(path): extract::Path<WorkspacePath>,
) -> Result<Json<AdminConnectionList>> {
    let mut tx = db::begin_admin(&state.db, &admin.grant).await?;
    let list = execute(&mut tx, path.workspace_id).await?;
    tx.commit().await?;
    Ok(Json(list))
}

/// The connections of `workspace` and their streams, read in an admin
/// transaction.
pub async fn execute(
    conn: &mut PgConnection,
    workspace: WorkspaceId,
) -> Result<AdminConnectionList> {
    let rows = sqlx::query!(
        r#"SELECT id AS "id: ConnectionId", provider, address,
                  granted_by AS "granted_by: UserId", capabilities, health, health_reason,
                  health_changed_at, created_at
             FROM connectors.connections
            WHERE workspace_id = $1
            ORDER BY created_at, id"#,
        workspace as WorkspaceId,
    )
    .fetch_all(&mut *conn)
    .await?;
    let ids: Vec<ConnectionId> = rows.iter().map(|row| row.id).collect();
    let streams = sqlx::query!(
        r#"SELECT connection_id AS "connection_id: ConnectionId", capability, phase, paused,
                  last_synced_at, consecutive_failures
             FROM connectors.streams
            WHERE connection_id = ANY($1)
            ORDER BY capability"#,
        &ids as &[ConnectionId],
    )
    .fetch_all(&mut *conn)
    .await?;
    let connections = rows
        .into_iter()
        .map(|row| {
            let streams = streams
                .iter()
                .filter(|stream| stream.connection_id == row.id)
                .map(|stream| {
                    Ok(AdminStream {
                        capability: Capability::parse(&stream.capability)?,
                        phase: Phase::parse(&stream.phase)?,
                        paused: stream.paused,
                        last_synced_at: stream.last_synced_at,
                        consecutive_failures: stream.consecutive_failures,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(AdminConnection {
                id: row.id,
                provider: Provider::parse(&row.provider)?,
                address: row.address,
                granted_by: row.granted_by,
                capabilities: parse_capabilities(&row.capabilities)?,
                health: Health::parse(&row.health)?,
                health_reason: row.health_reason,
                health_changed_at: row.health_changed_at,
                created_at: row.created_at,
                streams,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(AdminConnectionList { connections })
}

#[cfg(test)]
mod tests;
