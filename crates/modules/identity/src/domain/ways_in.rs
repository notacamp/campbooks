//! How many ways in an account has: its provider identities and its
//! passkeys. The last one can never be removed (ADR 0014).

use campbooks_kernel::Result;
use sqlx::PgConnection;
use uuid::Uuid;

/// Lock account `account` for the rest of the transaction (so two removals
/// cannot both see "one more left") and count its ways in. 0 when the
/// account does not exist.
pub async fn lock_and_count(conn: &mut PgConnection, account: Uuid) -> Result<i64> {
    let locked: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM identity.accounts WHERE id = $1 FOR UPDATE")
            .bind(account)
            .fetch_optional(&mut *conn)
            .await?;
    if locked.is_none() {
        return Ok(0);
    }
    Ok(sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM identity.provider_identities WHERE account_id = $1)
              + (SELECT count(*) FROM identity.passkeys WHERE account_id = $1)",
    )
    .bind(account)
    .fetch_one(conn)
    .await?)
}
