//! The verification token of Notion's webhook subscription
//! (campbooks-cloud#376).
//!
//! After an operator creates a subscription, Notion posts
//! `{"verification_token": "…"}` once to `POST /hooks/notion`, and the
//! operator pastes that token back into Notion's screen, where it becomes
//! the key the events are signed with. So the endpoint exists before the
//! secret is configured, and the operator needs a safe way to read the
//! token. While no secret is configured the hook keeps the token here,
//! **sealed** with the instance key, as the instance's one row (a newer
//! one replaces it). Only `campbooks-server admin notion-webhook-token`
//! reads it back, on the migration login; the runtime role can replace the
//! row and nothing more. No request, event or log ever carries it.

use campbooks_kernel::{
    Error, Result,
    crypto::{Cipher, SecretKey},
};
use chrono::{DateTime, Utc};
use sqlx::PgConnection;

/// The purpose of the cipher that seals the token (the HKDF label: never
/// change it, or what it sealed cannot be opened).
pub const PURPOSE: &[u8] = b"campbooks connectors notion webhook v1";

/// What a sealed token is bound to.
const CONTEXT: &[u8] = b"connectors.notion_webhook_token";

/// The longest token kept, in characters.
pub const MAX_TOKEN_CHARS: usize = 200;

/// The cipher for the token under the instance secret.
pub fn cipher(secret: &SecretKey) -> Cipher {
    Cipher::new(secret, PURPOSE)
}

/// Whether `token` is one worth keeping: 1 to [`MAX_TOKEN_CHARS`] printable
/// ASCII characters, no spaces.
pub fn acceptable(token: &str) -> bool {
    !token.is_empty()
        && token.len() <= MAX_TOKEN_CHARS
        && token.bytes().all(|b| b.is_ascii_graphic())
}

/// Keep `token`, sealed, as the instance's one: any earlier one goes.
pub async fn store(
    conn: &mut PgConnection,
    cipher: &Cipher,
    token: &str,
    now: DateTime<Utc>,
) -> Result<()> {
    let sealed = cipher.seal(token, CONTEXT);
    sqlx::query("DELETE FROM connectors.notion_webhook_token")
        .execute(&mut *conn)
        .await?;
    sqlx::query("INSERT INTO connectors.notion_webhook_token (token, received_at) VALUES ($1, $2)")
        .bind(sealed)
        .bind(now)
        .execute(conn)
        .await?;
    Ok(())
}

/// The token kept, opened.
#[derive(Clone)]
pub struct Stored {
    /// The token itself.
    pub token: String,
    /// When it arrived.
    pub received_at: DateTime<Utc>,
}

impl std::fmt::Debug for Stored {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stored")
            .field("received_at", &self.received_at)
            .finish_non_exhaustive()
    }
}

/// The token kept, if any. Only the operator's CLI calls this (the runtime
/// role has no `SELECT` on the table).
pub async fn read(conn: &mut PgConnection, cipher: &Cipher) -> Result<Option<Stored>> {
    let row: Option<(Vec<u8>, DateTime<Utc>)> =
        sqlx::query_as("SELECT token, received_at FROM connectors.notion_webhook_token")
            .fetch_optional(conn)
            .await?;
    row.map(|(sealed, received_at)| {
        cipher
            .open(&sealed, CONTEXT)
            .map(|token| Stored { token, received_at })
            .map_err(Error::internal)
    })
    .transpose()
}

/// Forget the token; how many rows went.
pub async fn clear(conn: &mut PgConnection) -> Result<u64> {
    Ok(sqlx::query("DELETE FROM connectors.notion_webhook_token")
        .execute(conn)
        .await?
        .rows_affected())
}
