//! Credentials at rest: sealed with the module's [`Cipher`], bound to table,
//! column and connection, and never returned, logged or put in a payload.

use campbooks_kernel::{Result, crypto::Cipher, db::BeginFor as _, tenancy::WorkspaceId};
use chrono::{DateTime, TimeDelta, Utc};
use sqlx::{PgConnection, PgPool};

use super::model::ConnectionId;
use crate::{
    deps::Deps,
    google::{GoogleError, TokenGrant},
};

/// An access token is refreshed this long before it expires.
const EXPIRY_MARGIN: TimeDelta = TimeDelta::minutes(2);

/// Associated data of a sealed credential column.
fn context(column: &str, id: ConnectionId) -> Vec<u8> {
    format!("connectors.credentials:{column}:{id}").into_bytes()
}

/// Store a connection's tokens, replacing what it had. `refresh_token`
/// `None` keeps the stored one (Google omits it on some re-consents).
pub async fn store(
    conn: &mut PgConnection,
    cipher: &Cipher,
    workspace: WorkspaceId,
    id: ConnectionId,
    refresh_token: Option<&str>,
    grant: &TokenGrant,
    now: DateTime<Utc>,
) -> Result<()> {
    let refresh = refresh_token.map(|token| cipher.seal(token, &context("refresh_token", id)));
    let access = cipher.seal(&grant.access_token, &context("access_token", id));
    let expires_at = expiry(grant, now);
    sqlx::query!(
        r#"INSERT INTO connectors.credentials
               (connection_id, workspace_id, refresh_token, access_token, access_expires_at, updated_at)
           VALUES ($1, $2, coalesce($3, ''::bytea), $4, $5, $6)
           ON CONFLICT (connection_id) DO UPDATE
              SET refresh_token = coalesce($3, connectors.credentials.refresh_token),
                  access_token = $4, access_expires_at = $5, updated_at = $6"#,
        id as ConnectionId,
        workspace as WorkspaceId,
        refresh,
        access,
        expires_at,
        now,
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// When a granted access token expires.
fn expiry(grant: &TokenGrant, now: DateTime<Utc>) -> DateTime<Utc> {
    now + TimeDelta::seconds(grant.expires_in.unwrap_or(3600).clamp(0, 86_400))
}

/// Why no access token could be had.
#[derive(Debug)]
pub enum TokenError {
    /// The grant is gone or unreadable: the member must connect again.
    /// The reason is `grant_revoked` or `credentials_unreadable`.
    NeedsReauth(&'static str),
    /// Talking to the provider failed.
    Provider(GoogleError),
    /// The database failed.
    Database(campbooks_kernel::Error),
}

impl From<sqlx::Error> for TokenError {
    fn from(err: sqlx::Error) -> Self {
        Self::Database(err.into())
    }
}

/// The refresh token of connection `id`, opened.
pub async fn refresh_token(
    conn: &mut PgConnection,
    cipher: &Cipher,
    id: ConnectionId,
) -> Result<Option<String>, TokenError> {
    let sealed = sqlx::query_scalar!(
        "SELECT refresh_token FROM connectors.credentials WHERE connection_id = $1",
        id as ConnectionId
    )
    .fetch_optional(conn)
    .await?;
    sealed
        .map(|sealed| {
            cipher
                .open(&sealed, &context("refresh_token", id))
                .map_err(|_| TokenError::NeedsReauth("credentials_unreadable"))
        })
        .transpose()
}

/// A valid access token for connection `id` of `workspace`: the stored one
/// while it lasts, else a fresh one from the refresh token (stored back).
/// Runs its own short transactions; call it before a job's transaction.
pub async fn access_token(
    pool: &PgPool,
    deps: &Deps,
    workspace: WorkspaceId,
    id: ConnectionId,
) -> Result<String, TokenError> {
    let (Some(cipher), Some(google)) = (&deps.cipher, &deps.google) else {
        return Err(TokenError::Provider(GoogleError::Unexpected(
            "the provider is not configured".into(),
        )));
    };
    let now = deps.clock.now();
    let mut tx = pool.begin_for(workspace).await?;
    let row = sqlx::query!(
        r#"SELECT refresh_token, access_token, access_expires_at
             FROM connectors.credentials WHERE connection_id = $1"#,
        id as ConnectionId
    )
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    let Some(row) = row else {
        return Err(TokenError::NeedsReauth("credentials_unreadable"));
    };
    if let (Some(sealed), Some(expires_at)) = (&row.access_token, row.access_expires_at)
        && expires_at - EXPIRY_MARGIN > now
        && let Ok(token) = cipher.open(sealed, &context("access_token", id))
    {
        return Ok(token);
    }
    let refresh = cipher
        .open(&row.refresh_token, &context("refresh_token", id))
        .map_err(|_| TokenError::NeedsReauth("credentials_unreadable"))?;
    let grant = match google.refresh(&refresh).await {
        Ok(grant) => grant,
        Err(GoogleError::InvalidGrant) => return Err(TokenError::NeedsReauth("grant_revoked")),
        Err(other) => return Err(TokenError::Provider(other)),
    };
    let mut tx = pool.begin_for(workspace).await?;
    store(
        &mut tx,
        cipher,
        workspace,
        id,
        grant.refresh_token.as_deref(),
        &grant,
        now,
    )
    .await
    .map_err(TokenError::Database)?;
    tx.commit().await?;
    Ok(grant.access_token)
}

/// Forget connection `id`'s access token, so the next run refreshes it
/// (after the provider refused it).
pub async fn forget_access_token(conn: &mut PgConnection, id: ConnectionId) -> Result<()> {
    sqlx::query!(
        "UPDATE connectors.credentials SET access_token = NULL, access_expires_at = NULL
          WHERE connection_id = $1",
        id as ConnectionId
    )
    .execute(conn)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use campbooks_kernel::crypto::SecretKey;

    use super::*;

    #[test]
    fn credentials_are_bound_to_their_column_and_connection() {
        let cipher = Cipher::new(
            &SecretKey::from_bytes([3; 32]),
            crate::deps::CREDENTIALS_PURPOSE,
        );
        let a = ConnectionId::new();
        let b = ConnectionId::new();
        let sealed = cipher.seal("a-refresh-token", &context("refresh_token", a));
        assert_eq!(
            cipher
                .open(&sealed, &context("refresh_token", a))
                .as_deref(),
            Ok("a-refresh-token")
        );
        assert!(cipher.open(&sealed, &context("refresh_token", b)).is_err());
        assert!(cipher.open(&sealed, &context("access_token", a)).is_err());
    }
}
