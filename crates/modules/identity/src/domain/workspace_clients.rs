//! A workspace's own clients, as its owners and admins manage them
//! (Settings → Apps and integrations).
//!
//! - **Confidential** (server-to-server): `client_credentials` with a secret,
//!   tokens bound to the workspace.
//! - **Public** (a sign-in app): code + PKCE with exact `https` or loopback
//!   redirect URIs; each person allows it on the consent screen.
//!
//! Secrets are 256-bit random values, stored as their SHA-256 only, shown
//! once when created or rotated.

use campbooks_kernel::{
    Error, Result,
    crypto::{hash, random_token},
    tenancy::Access,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::MESSAGES;

/// How a workspace client signs in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ClientType {
    /// Server to server, with a secret (`client_credentials`).
    Confidential,
    /// A sign-in app people allow (code + PKCE, no secret).
    Public,
}

impl ClientType {
    /// The wire name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Confidential => "confidential",
            Self::Public => "public",
        }
    }
}

/// A workspace client, as settings show it. Never its secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct WorkspaceClient {
    /// Its client id.
    pub client_id: String,
    /// Its name.
    pub name: String,
    /// Confidential or public.
    #[serde(rename = "type")]
    pub client_type: ClientType,
    /// The scopes it may ask for.
    pub scopes: Vec<String>,
    /// A public client's redirect URIs.
    pub redirect_uris: Vec<String>,
    /// When it was registered.
    pub created_at: DateTime<Utc>,
    /// When its secret last changed.
    pub secret_rotated_at: Option<DateTime<Utc>>,
    /// When it last got a token.
    pub last_used_at: Option<DateTime<Utc>>,
}

#[derive(sqlx::FromRow)]
struct Row {
    client_id: String,
    name: String,
    auth_method: String,
    allowed_scopes: Vec<String>,
    redirect_uris: Vec<String>,
    created_at: DateTime<Utc>,
    secret_rotated_at: Option<DateTime<Utc>>,
    last_used_at: Option<DateTime<Utc>>,
}

impl From<Row> for WorkspaceClient {
    fn from(row: Row) -> Self {
        Self {
            client_id: row.client_id,
            name: row.name,
            client_type: if row.auth_method == "none" {
                ClientType::Public
            } else {
                ClientType::Confidential
            },
            scopes: row.allowed_scopes,
            redirect_uris: row.redirect_uris,
            created_at: row.created_at,
            secret_rotated_at: row.secret_rotated_at,
            last_used_at: row.last_used_at,
        }
    }
}

const COLUMNS: &str = "client_id, name, auth_method, allowed_scopes, redirect_uris, created_at, \
     secret_rotated_at, last_used_at";

/// Ask the access catalogue (ADR 0017) whether `access` may take `action`
/// on the workspace's clients: owners and admins only (403
/// `not_workspace_admin`).
pub fn require_admin(
    state: &campbooks_kernel::AppState,
    access: &Access,
    action: campbooks_kernel::authz::Action,
) -> Result<()> {
    campbooks_kernel::authz::require_type(state, access, action, CLIENT_RECORD, || {
        Error::Denied(MESSAGES.reason("not_workspace_admin"))
    })
}

/// The record type of a workspace's client.
pub const CLIENT_RECORD: &str = "identity.client";

/// The workspace's clients, oldest first (the transaction is in the
/// workspace).
pub async fn list(conn: &mut PgConnection) -> Result<Vec<WorkspaceClient>> {
    let rows: Vec<Row> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM identity.clients WHERE kind = 'workspace'
          ORDER BY created_at, client_id"
    )))
    .fetch_all(conn)
    .await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

/// What an admin registers.
#[derive(Debug)]
pub struct NewClient<'a> {
    /// The workspace.
    pub workspace: Uuid,
    /// The admin's account.
    pub created_by: Uuid,
    /// Name.
    pub name: &'a str,
    /// Type.
    pub client_type: ClientType,
    /// Scopes.
    pub scopes: &'a [String],
    /// A public client's redirect URIs.
    pub redirect_uris: &'a [String],
}

/// Register `client`; the client and, for a confidential one, its secret
/// (shown once).
pub async fn insert(
    conn: &mut PgConnection,
    client: &NewClient<'_>,
    now: DateTime<Utc>,
) -> Result<(WorkspaceClient, Option<String>)> {
    let mut id = [0_u8; 16];
    aws_lc_rs::rand::fill(&mut id).map_err(|_| Error::internal("no entropy"))?;
    let client_id = format!(
        "cb_{}",
        base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, id)
    );
    let (secret, auth_method, grant_types): (Option<String>, &str, &[&str]) =
        match client.client_type {
            ClientType::Confidential => (
                Some(random_token()),
                "client_secret_basic",
                &["client_credentials"],
            ),
            ClientType::Public => (None, "none", &["authorization_code", "refresh_token"]),
        };
    let row: Row = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "INSERT INTO identity.clients
             (client_id, kind, workspace_id, name, redirect_uris, grant_types, auth_method,
              secret_hash, allowed_scopes, created_by, created_at)
         VALUES ($1, 'workspace', $2, $3, $4, $5, $6, $7, $8, $9, $10)
         RETURNING {COLUMNS}"
    )))
    .bind(&client_id)
    .bind(client.workspace)
    .bind(client.name)
    .bind(client.redirect_uris)
    .bind(grant_types)
    .bind(auth_method)
    .bind(secret.as_deref().map(hash))
    .bind(client.scopes)
    .bind(client.created_by)
    .bind(now)
    .fetch_one(conn)
    .await?;
    Ok((row.into(), secret))
}

/// A new secret for confidential client `id` (the old one stops working
/// at once); `None` when there is no such client, `Err` conflict for a
/// public one.
pub async fn rotate(
    conn: &mut PgConnection,
    id: &str,
    now: DateTime<Utc>,
) -> Result<Option<(WorkspaceClient, String)>> {
    let secret = random_token();
    let row: Option<Row> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "UPDATE identity.clients SET secret_hash = $2, secret_rotated_at = $3
          WHERE client_id = $1 AND kind = 'workspace' AND auth_method <> 'none'
          RETURNING {COLUMNS}"
    )))
    .bind(id)
    .bind(hash(&secret))
    .bind(now)
    .fetch_optional(&mut *conn)
    .await?;
    if let Some(row) = row {
        return Ok(Some((row.into(), secret)));
    }
    let public: Option<String> = sqlx::query_scalar(
        "SELECT client_id FROM identity.clients WHERE client_id = $1 AND kind = 'workspace'",
    )
    .bind(id)
    .fetch_optional(conn)
    .await?;
    match public {
        Some(_) => Err(Error::Conflict(MESSAGES.reason("client_is_public"))),
        None => Ok(None),
    }
}

/// Remove client `id`; whether it existed (the transaction is in the
/// workspace).
pub async fn remove(conn: &mut PgConnection, id: &str) -> Result<bool> {
    let removed =
        sqlx::query("DELETE FROM identity.clients WHERE client_id = $1 AND kind = 'workspace'")
            .bind(id)
            .execute(conn)
            .await?
            .rows_affected();
    Ok(removed == 1)
}

/// End everything client `id` was given: its consents and every refresh
/// token it holds. The accounts whose consents ended, with their workspace.
pub async fn revoke_everything(
    conn: &mut PgConnection,
    id: &str,
    reason: &str,
    now: DateTime<Utc>,
) -> Result<Vec<(Uuid, Option<Uuid>)>> {
    let ended: Vec<(Uuid, Option<Uuid>)> = sqlx::query_as(
        "UPDATE identity.consents SET revoked_at = $2
          WHERE client_id = $1 AND revoked_at IS NULL
          RETURNING account_id, bound_workspace_id",
    )
    .bind(id)
    .bind(now)
    .fetch_all(&mut *conn)
    .await?;
    sqlx::query(
        "UPDATE identity.refresh_families SET revoked_at = $2, revoke_reason = $3
          WHERE client_id = $1 AND revoked_at IS NULL",
    )
    .bind(id)
    .bind(now)
    .bind(reason)
    .execute(conn)
    .await?;
    Ok(ended)
}
