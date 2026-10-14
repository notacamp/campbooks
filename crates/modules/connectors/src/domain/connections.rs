//! Reading connections, who may see them, and how the API shows them.

use campbooks_kernel::{
    Result,
    tenancy::{Access, UserId},
};
use chrono::{DateTime, Utc};
use sqlx::PgConnection;

use super::model::{
    Capability, ConnectionId, ConnectionView, Health, InboxId, Phase, Provider, StreamId,
    StreamView,
};

/// A connection row.
#[derive(Debug, Clone)]
pub struct Connection {
    pub id: ConnectionId,
    pub provider: Provider,
    pub address: String,
    pub address_verified: bool,
    pub display_name: Option<String>,
    pub granted_by: UserId,
    pub capabilities: Vec<Capability>,
    pub health: Health,
    pub health_reason: Option<String>,
    pub health_changed_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub grantor_signs_in: bool,
}

/// Whether the caller of `access` may see a connection granted by
/// `granted_by`: its grantor, owners and admins, and client tokens of the
/// workspace.
pub fn visible(access: &Access, granted_by: UserId) -> bool {
    match (access.user, access.role) {
        (None, _) => true,
        (Some(user), role) => {
            user == granted_by || role.is_some_and(campbooks_kernel::tenancy::Role::can_manage)
        }
    }
}

/// The connection `id` of the transaction's workspace.
pub async fn load(conn: &mut PgConnection, id: ConnectionId) -> Result<Option<Connection>> {
    Ok(select(conn, Some(id)).await?.into_iter().next())
}

/// Every connection of the transaction's workspace, oldest first.
pub async fn list(conn: &mut PgConnection) -> Result<Vec<Connection>> {
    select(conn, None).await
}

/// Connections of the transaction's workspace: `id`, or all.
async fn select(conn: &mut PgConnection, id: Option<ConnectionId>) -> Result<Vec<Connection>> {
    let rows = sqlx::query!(
        r#"SELECT id AS "id: ConnectionId", provider, address, address_verified, display_name,
                  granted_by AS "granted_by: UserId", capabilities, health, health_reason,
                  health_changed_at, created_at, grantor_signs_in
             FROM connectors.connections
            WHERE $1::uuid IS NULL OR id = $1
            ORDER BY created_at, id"#,
        id as Option<ConnectionId>,
    )
    .fetch_all(conn)
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(Connection {
                id: row.id,
                provider: Provider::parse(&row.provider)?,
                address: row.address,
                address_verified: row.address_verified,
                display_name: row.display_name,
                granted_by: row.granted_by,
                capabilities: parse_capabilities(&row.capabilities)?,
                health: Health::parse(&row.health)?,
                health_reason: row.health_reason,
                health_changed_at: row.health_changed_at,
                created_at: row.created_at,
                grantor_signs_in: row.grantor_signs_in,
            })
        })
        .collect()
}

/// `connection` as the API shows it to `caller`, with its streams.
pub async fn view(
    conn: &mut PgConnection,
    connection: Connection,
    caller: Option<UserId>,
) -> Result<ConnectionView> {
    let rows = sqlx::query!(
        r#"SELECT s.id AS "id: StreamId", s.capability, s.inbox_id AS "inbox_id: InboxId",
                  s.address, s.phase, s.paused, s.last_synced_at, s.last_change_at,
                  d.due_at AS "due_at?"
             FROM connectors.streams s
             LEFT JOIN connectors.sync_due d ON d.stream_id = s.id
            WHERE s.connection_id = $1
            ORDER BY s.capability, s.created_at, s.id"#,
        connection.id as ConnectionId,
    )
    .fetch_all(conn)
    .await?;
    let streams = rows
        .into_iter()
        .map(|row| {
            Ok(StreamView {
                capability: Capability::parse(&row.capability)?,
                inbox_id: row.inbox_id,
                address: row.address,
                phase: Phase::parse(&row.phase)?,
                paused: row.paused,
                last_synced_at: row.last_synced_at,
                last_change_at: row.last_change_at,
                next_sync_at: row.due_at,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(ConnectionView {
        id: connection.id,
        provider: connection.provider,
        address: connection.address,
        address_verified: connection.address_verified,
        display_name: connection.display_name,
        granted_by: connection.granted_by,
        granted_by_you: caller == Some(connection.granted_by),
        capabilities: connection.capabilities,
        health: connection.health,
        health_reason: connection.health_reason,
        health_changed_at: connection.health_changed_at,
        grantor_signs_in: connection.grantor_signs_in,
        streams,
        created_at: connection.created_at,
    })
}

/// The capabilities stored as `text[]`.
pub fn parse_capabilities(values: &[String]) -> Result<Vec<Capability>> {
    let mut capabilities = values
        .iter()
        .map(|value| Capability::parse(value))
        .collect::<Result<Vec<_>>>()?;
    capabilities.sort_unstable();
    capabilities.dedup();
    Ok(capabilities)
}

#[cfg(test)]
mod tests {
    use campbooks_kernel::tenancy::{Role, WorkspaceId};

    use super::*;

    #[test]
    fn grantor_admins_and_clients_see_a_connection() {
        let grantor = UserId::new();
        let other = UserId::new();
        let access = |user: Option<UserId>, role: Option<Role>| Access {
            workspace: WorkspaceId::new(),
            user,
            role,
        };
        assert!(visible(&access(Some(grantor), Some(Role::Member)), grantor));
        assert!(!visible(&access(Some(other), Some(Role::Member)), grantor));
        assert!(visible(&access(Some(other), Some(Role::Admin)), grantor));
        assert!(visible(&access(Some(other), Some(Role::Owner)), grantor));
        assert!(visible(&access(None, None), grantor));
    }
}
