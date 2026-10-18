//! Credentials at rest: sealed with the module's [`Cipher`], bound to table,
//! column and connection, and never returned, logged or put in a payload.

use campbooks_kernel::{Result, crypto::Cipher, db::BeginFor as _, tenancy::WorkspaceId};
use chrono::{DateTime, TimeDelta, Utc};
use sqlx::{PgConnection, PgPool};

use super::model::{ConnectionId, Provider};
use crate::{
    deps::Deps,
    provider::{HTTP_TIMEOUT, ProviderError, TokenGrant},
};

/// An access token is refreshed this long before it expires.
const EXPIRY_MARGIN: TimeDelta = TimeDelta::minutes(2);

/// How often a run waiting for another run's refresh looks again.
const REFRESH_POLL: std::time::Duration = std::time::Duration::from_millis(100);

/// How long a run waits for another run's refresh: that run's call to the
/// provider is bounded by [`HTTP_TIMEOUT`], storing its result takes a
/// moment more.
const REFRESH_WAIT: std::time::Duration =
    HTTP_TIMEOUT.saturating_add(std::time::Duration::from_secs(5));

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
    Provider(ProviderError),
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
    let Some(cipher) = &deps.cipher else {
        return Err(TokenError::Provider(ProviderError::Unexpected(
            "the provider is not configured".into(),
        )));
    };
    let now = deps.clock.now();
    let mut tx = pool.begin_for(workspace).await?;
    let current = stored_access(&mut tx, cipher, id, now).await?;
    tx.commit().await?;
    if let Some(token) = current {
        return Ok(token);
    }
    // Refresh under a per-connection lock, so concurrent runs (a sync and
    // a send, two workers) refresh once and share the result. The lock is
    // only ever tried, never waited on: the run that holds it calls the
    // provider (up to `HTTP_TIMEOUT`), longer than a lock wait may last
    // (`lock_timeout`), so the others look for its token until it is stored
    // instead (campbooks-cloud#254).
    let waited_until = tokio::time::Instant::now() + REFRESH_WAIT;
    loop {
        let mut tx = pool.begin_for(workspace).await?;
        let locked: bool = sqlx::query_scalar(
            "SELECT pg_try_advisory_xact_lock(hashtextextended('connectors.credentials:' || $1::text, 0))",
        )
        .bind(id.to_string())
        .fetch_one(&mut *tx)
        .await?;
        if locked {
            return refresh_locked(tx, cipher, deps, workspace, id, now).await;
        }
        // Another run is refreshing: its token, once it is stored.
        let stored = stored_access(&mut tx, cipher, id, now).await?;
        tx.commit().await?;
        if let Some(token) = stored {
            return Ok(token);
        }
        if tokio::time::Instant::now() >= waited_until {
            return Err(TokenError::Provider(ProviderError::Unavailable(
                "another run's token refresh did not finish".into(),
            )));
        }
        tokio::time::sleep(REFRESH_POLL).await;
    }
}

/// Refresh connection `id`'s access token in `tx`, which holds the
/// connection's refresh lock: the stored token if a run stored one
/// meanwhile, else the provider's, stored back.
async fn refresh_locked(
    mut tx: sqlx::Transaction<'static, sqlx::Postgres>,
    cipher: &Cipher,
    deps: &Deps,
    workspace: WorkspaceId,
    id: ConnectionId,
    now: DateTime<Utc>,
) -> Result<String, TokenError> {
    if let Some(token) = stored_access(&mut tx, cipher, id, now).await? {
        tx.commit().await?;
        return Ok(token);
    }
    let sealed = sqlx::query_scalar!(
        "SELECT refresh_token FROM connectors.credentials WHERE connection_id = $1",
        id as ConnectionId
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(TokenError::NeedsReauth("credentials_unreadable"))?;
    let refresh = cipher
        .open(&sealed, &context("refresh_token", id))
        .map_err(|_| TokenError::NeedsReauth("credentials_unreadable"))?;
    let grant = match refresh_at_provider(&mut tx, deps, id, &refresh).await? {
        Ok(grant) => grant,
        Err(ProviderError::InvalidGrant) => return Err(TokenError::NeedsReauth("grant_revoked")),
        Err(other) => return Err(TokenError::Provider(other)),
    };
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

/// Ask connection `id`'s provider for a fresh access token: Google, or the
/// accounts server of the Zoho grant's data centre.
async fn refresh_at_provider(
    conn: &mut PgConnection,
    deps: &Deps,
    id: ConnectionId,
    refresh: &str,
) -> Result<Result<TokenGrant, ProviderError>, TokenError> {
    let row = sqlx::query!(
        "SELECT provider, provider_server FROM connectors.connections WHERE id = $1",
        id as ConnectionId
    )
    .fetch_optional(conn)
    .await?
    .ok_or(TokenError::NeedsReauth("credentials_unreadable"))?;
    let not_configured = || {
        TokenError::Provider(ProviderError::Unexpected(
            "the provider is not configured".into(),
        ))
    };
    match Provider::parse(&row.provider).map_err(TokenError::Database)? {
        Provider::Google => {
            let google = deps.google.as_ref().ok_or_else(not_configured)?;
            Ok(google.refresh(refresh).await)
        }
        Provider::Zoho => {
            let zoho = deps.zoho.as_ref().ok_or_else(not_configured)?;
            let centre = row
                .provider_server
                .as_deref()
                .and_then(|server| zoho.centre(server))
                .ok_or(TokenError::NeedsReauth("unknown_data_centre"))?;
            Ok(zoho.refresh(&centre, refresh).await)
        }
    }
}

/// The stored access token of connection `id`, while it is valid.
async fn stored_access(
    conn: &mut PgConnection,
    cipher: &Cipher,
    id: ConnectionId,
    now: DateTime<Utc>,
) -> Result<Option<String>, TokenError> {
    let row = sqlx::query!(
        r#"SELECT access_token, access_expires_at
             FROM connectors.credentials WHERE connection_id = $1"#,
        id as ConnectionId
    )
    .fetch_optional(conn)
    .await?
    .ok_or(TokenError::NeedsReauth("credentials_unreadable"))?;
    Ok(match (row.access_token, row.access_expires_at) {
        (Some(sealed), Some(expires_at)) if expires_at - EXPIRY_MARGIN > now => {
            cipher.open(&sealed, &context("access_token", id)).ok()
        }
        _ => None,
    })
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
