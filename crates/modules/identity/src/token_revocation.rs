//! The kernel's `TokenRevocation` port: where an account stands for the
//! access tokens issued to it (suspended or not, the instant of its last
//! revocation). Registered by the server for the in-process issuer's
//! verifier, which asks on every verification of a person's token.
//!
//! One primary-key read of `identity.accounts` (not workspace-owned, no
//! row-level security) per verification, uncached: a suspension or sign
//! out everywhere takes effect on the next request in every process.

use std::sync::Arc;

use campbooks_kernel::auth::{StandingFuture, TokenRevocation, TokenRevocationPort, TokenStanding};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

/// The port, for the server to register.
pub fn token_revocation() -> TokenRevocationPort {
    Arc::new(Accounts)
}

#[derive(Debug, Clone, Copy)]
struct Accounts;

impl TokenRevocation for Accounts {
    fn standing<'a>(&'a self, db: &'a PgPool, account: &'a str) -> StandingFuture<'a> {
        Box::pin(async move {
            // Not an account id at all: no such account.
            let Ok(id) = account.parse::<Uuid>() else {
                return Ok(None);
            };
            let row: Option<(bool, Option<DateTime<Utc>>)> = sqlx::query_as(
                "SELECT disabled_at IS NOT NULL, tokens_not_before
                   FROM identity.accounts WHERE id = $1",
            )
            .bind(id)
            .fetch_optional(db)
            .await?;
            Ok(row.map(|(suspended, tokens_not_before)| TokenStanding {
                suspended,
                tokens_not_before,
            }))
        })
    }
}

#[cfg(test)]
mod tests {
    use campbooks_kernel::auth::TokenStanding;
    use chrono::{DateTime, Utc};
    use sqlx::PgPool;
    use uuid::Uuid;

    #[sqlx::test(migrations = "../../../migrations")]
    async fn reads_an_accounts_standing_with_the_runtime_role(db: PgPool) {
        let pool = campbooks_kernel::testing::app_role_pool(&db).await;
        let account = Uuid::now_v7();
        let epoch = DateTime::from_timestamp(1_700_000_000, 250_000_000).unwrap();
        sqlx::query(
            "INSERT INTO identity.accounts (id, created_at, tokens_not_before)
             VALUES ($1, now(), $2)",
        )
        .bind(account)
        .bind(epoch)
        .execute(&db)
        .await
        .unwrap();
        let port = super::token_revocation();
        let standing = |id: String| {
            let port = port.clone();
            let pool = pool.clone();
            async move { port.standing(&pool, &id).await.unwrap() }
        };

        assert_eq!(
            standing(account.to_string()).await,
            Some(TokenStanding {
                suspended: false,
                tokens_not_before: Some(epoch),
            })
        );
        sqlx::query("UPDATE identity.accounts SET disabled_at = $2 WHERE id = $1")
            .bind(account)
            .bind(Utc::now())
            .execute(&db)
            .await
            .unwrap();
        assert!(standing(account.to_string()).await.unwrap().suspended);
        // Unknown accounts, and subjects that are no account id at all.
        assert_eq!(standing(Uuid::now_v7().to_string()).await, None);
        assert_eq!(standing("acme-sync".to_owned()).await, None);
    }
}
