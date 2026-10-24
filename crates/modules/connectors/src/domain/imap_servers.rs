//! Where an IMAP connection's mail lives: its IMAP and SMTP servers and the
//! username it signs in with (`connectors.imap_servers`). No secret here:
//! the password is a credential (`credentials::password`).

use campbooks_kernel::{Result, tenancy::WorkspaceId};
use chrono::{DateTime, Utc};
use sqlx::PgConnection;

use super::model::ConnectionId;
use crate::imap::{Security, Server, Servers, Source};

/// An IMAP connection's account: what it signs in with, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImapAccount {
    /// The servers.
    pub servers: Servers,
    /// The username both servers accepted.
    pub username: String,
    /// Where the servers came from.
    pub source: Source,
    /// The provider database's id of the provider, when known.
    pub provider_hint: Option<String>,
}

/// Store `account` as connection `id`'s, replacing what it had.
pub async fn store(
    conn: &mut PgConnection,
    workspace: WorkspaceId,
    id: ConnectionId,
    account: &ImapAccount,
    now: DateTime<Utc>,
) -> Result<()> {
    let Servers { imap, smtp } = &account.servers;
    sqlx::query(
        "INSERT INTO connectors.imap_servers
             (connection_id, workspace_id, username, imap_host, imap_port, imap_security,
              smtp_host, smtp_port, smtp_security, source, provider_hint, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
         ON CONFLICT (connection_id) DO UPDATE
            SET username = $3, imap_host = $4, imap_port = $5, imap_security = $6,
                smtp_host = $7, smtp_port = $8, smtp_security = $9, source = $10,
                provider_hint = $11, updated_at = $12",
    )
    .bind(id)
    .bind(workspace)
    .bind(&account.username)
    .bind(&imap.host)
    .bind(i32::from(imap.port))
    .bind(imap.security.as_str())
    .bind(&smtp.host)
    .bind(i32::from(smtp.port))
    .bind(smtp.security.as_str())
    .bind(account.source.as_str())
    .bind(account.provider_hint.as_deref())
    .bind(now)
    .execute(conn)
    .await?;
    Ok(())
}

/// Connection `id`'s IMAP server and the username it signs in with;
/// `None` when it has none (or one this version cannot read).
pub async fn imap_server(
    conn: &mut PgConnection,
    id: ConnectionId,
) -> Result<Option<(String, Server)>> {
    let row: Option<(String, String, i32, String)> = sqlx::query_as(
        "SELECT username, imap_host, imap_port, imap_security
           FROM connectors.imap_servers WHERE connection_id = $1",
    )
    .bind(id)
    .fetch_optional(conn)
    .await?;
    Ok(row.and_then(|(username, host, port, security)| {
        let security = match security.as_str() {
            "tls" => Security::Tls,
            "starttls" => Security::Starttls,
            _ => return None,
        };
        let server = Server {
            host,
            port: u16::try_from(port).ok()?,
            security,
        }
        .normalized()?;
        Some((username, server))
    }))
}

/// Connection `id`'s SMTP submission server and the username it signs in
/// with; `None` when it has none (or one this version cannot read).
pub async fn smtp_server(
    conn: &mut PgConnection,
    id: ConnectionId,
) -> Result<Option<(String, Server)>> {
    let row: Option<(String, String, i32, String)> = sqlx::query_as(
        "SELECT username, smtp_host, smtp_port, smtp_security
           FROM connectors.imap_servers WHERE connection_id = $1",
    )
    .bind(id)
    .fetch_optional(conn)
    .await?;
    Ok(row.and_then(|(username, host, port, security)| {
        let security = match security.as_str() {
            "tls" => Security::Tls,
            "starttls" => Security::Starttls,
            _ => return None,
        };
        let server = Server {
            host,
            port: u16::try_from(port).ok()?,
            security,
        }
        .normalized()?;
        Some((username, server))
    }))
}
