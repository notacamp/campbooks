//! The kernel's `TokenRevocation` port: where a person's access token
//! stands. Its account: suspended or not, the instant of its last
//! revocation. Its own sign-in (campbooks-cloud#143): its issuer session
//! (`sid`) signed out or ended with the way in it used, its refresh family
//! (`fam`) revoked, or the consent of its app revoked since it was issued.
//! Registered by the server for the in-process issuer's verifier, which
//! asks on every verification of a person's token.
//!
//! One statement per verification, uncached, as before: a primary-key read
//! of `identity.accounts` with three indexed lookups beside it (a session
//! and a family by primary key, the app's consents by account and client).
//! No cache, so no staleness to bound: a revocation takes effect on the next
//! request in every process, for the same single round trip a request
//! already paid for the account's standing.

use std::sync::Arc;

use campbooks_kernel::auth::{
    Principal, StandingFuture, TokenRevocation, TokenRevocationPort, TokenStanding,
};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

/// The port, for the server to register.
pub fn token_revocation() -> TokenRevocationPort {
    Arc::new(Accounts)
}

#[derive(Debug, Clone, Copy)]
struct Accounts;

/// The token's account and whether its own sign-in ended since `$5` (its
/// `iat`): its session ended (signed out, or its way in removed), its refresh
/// family revoked, or a consent of its app (`$4`) live when it was issued
/// revoked since. Ended sessions and revoked families are kept a day and
/// revoked consents thirty, far longer than an access token lives.
const STANDING: &str = "SELECT a.disabled_at IS NOT NULL, a.tokens_not_before,
        EXISTS (SELECT 1 FROM identity.sessions s
                 WHERE s.id = $2 AND s.ended_at IS NOT NULL)
        OR EXISTS (SELECT 1 FROM identity.refresh_families f
                    WHERE f.id = $3 AND f.revoked_at IS NOT NULL)
        OR EXISTS (SELECT 1 FROM identity.consents c
                    WHERE c.account_id = a.id AND c.client_id = $4
                      AND c.revoked_at IS NOT NULL
                      AND c.granted_at < $5 + interval '1 second'
                      AND c.revoked_at >= $5 - interval '1 second')
   FROM identity.accounts a WHERE a.id = $1";

impl TokenRevocation for Accounts {
    fn standing<'a>(&'a self, db: &'a PgPool, principal: &'a Principal) -> StandingFuture<'a> {
        Box::pin(async move {
            // Not an account id at all: no such account.
            let Ok(id) = principal.subject.parse::<Uuid>() else {
                return Ok(None);
            };
            // A sign-in the issuer named with something that is no id at all
            // stands for nothing.
            let sign_in = principal.sign_in.clone().unwrap_or_default();
            let parse = |id: Option<String>| -> Result<Option<Uuid>, ()> {
                id.map(|id| id.parse::<Uuid>().map_err(|_| ())).transpose()
            };
            let (Ok(session), Ok(family)) = (parse(sign_in.session), parse(sign_in.family)) else {
                return Ok(Some(TokenStanding {
                    sign_in_ended: true,
                    ..TokenStanding::default()
                }));
            };
            let row: Option<(bool, Option<DateTime<Utc>>, bool)> = sqlx::query_as(STANDING)
                .bind(id)
                .bind(session)
                .bind(family)
                .bind(principal.client_id.as_deref())
                .bind(principal.issued_at)
                .fetch_optional(db)
                .await?;
            Ok(row.map(
                |(suspended, tokens_not_before, sign_in_ended)| TokenStanding {
                    suspended,
                    tokens_not_before,
                    sign_in_ended,
                },
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use campbooks_kernel::auth::{Principal, PrincipalKind, SignIn, TokenStanding};
    use chrono::{DateTime, TimeDelta, Utc};
    use sqlx::PgPool;
    use uuid::Uuid;

    /// A person's token for `sub`, issued at `iat` to `client` in `sign_in`.
    fn token(
        sub: &str,
        client: Option<&str>,
        iat: DateTime<Utc>,
        sign_in: Option<SignIn>,
    ) -> Principal {
        Principal {
            subject: sub.to_owned(),
            kind: PrincipalKind::User,
            client_id: client.map(ToOwned::to_owned),
            scopes: BTreeSet::new(),
            workspace: None,
            email: None,
            email_verified: false,
            name: None,
            expires_at: None,
            issued_at: Some(iat),
            auth_time: None,
            sign_in,
        }
    }

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
        let standing = |principal: Principal| {
            let port = port.clone();
            let pool = pool.clone();
            async move { port.standing(&pool, &principal).await.unwrap() }
        };
        let now = Utc::now();

        assert_eq!(
            standing(token(&account.to_string(), None, now, None)).await,
            Some(TokenStanding {
                suspended: false,
                tokens_not_before: Some(epoch),
                sign_in_ended: false,
            })
        );
        sqlx::query("UPDATE identity.accounts SET disabled_at = $2 WHERE id = $1")
            .bind(account)
            .bind(Utc::now())
            .execute(&db)
            .await
            .unwrap();
        assert!(
            standing(token(&account.to_string(), None, now, None))
                .await
                .unwrap()
                .suspended
        );
        // Unknown accounts, and subjects that are no account id at all.
        assert_eq!(
            standing(token(&Uuid::now_v7().to_string(), None, now, None)).await,
            None
        );
        assert_eq!(standing(token("acme-sync", None, now, None)).await, None);
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn a_tokens_own_sign_in_ends_it_and_no_other(db: PgPool) {
        // campbooks-cloud#143: its session ended, its refresh family revoked,
        // or its app's consent revoked since it was issued.
        let pool = campbooks_kernel::testing::app_role_pool(&db).await;
        let account = Uuid::now_v7();
        let now = Utc::now();
        let iat = DateTime::from_timestamp(now.timestamp(), 0).unwrap();
        sqlx::query("INSERT INTO identity.accounts (id, created_at) VALUES ($1, now())")
            .bind(account)
            .execute(&db)
            .await
            .unwrap();
        let (ended, live) = (Uuid::now_v7(), Uuid::now_v7());
        for (session, ended_at) in [(ended, Some(now)), (live, None)] {
            sqlx::query(
                "INSERT INTO identity.sessions
                     (id, token_hash, account_id, method, auth_time, created_at, last_seen_at,
                      idle_expires_at, expires_at, ended_at)
                 VALUES ($1, $2, $3, 'provider', $4, $4, $4, $4 + interval '1 day',
                         $4 + interval '30 days', $5)",
            )
            .bind(session)
            .bind(session.as_bytes().to_vec())
            .bind(account)
            .bind(now)
            .bind(ended_at)
            .execute(&db)
            .await
            .unwrap();
        }
        let (revoked, kept) = (Uuid::now_v7(), Uuid::now_v7());
        for (family, revoked_at) in [(revoked, Some(now)), (kept, None)] {
            sqlx::query(
                "INSERT INTO identity.refresh_families
                     (id, client_id, account_id, session_id, scope, auth_time, created_at,
                      expires_at, revoked_at)
                 VALUES ($1, 'campbooks-web', $2, $3, 'openid', $4, $4, $4 + interval '30 days', $5)",
            )
            .bind(family)
            .bind(account)
            .bind(live)
            .bind(now)
            .bind(revoked_at)
            .execute(&db)
            .await
            .unwrap();
        }
        // An app whose consent was revoked after the token, and one whose
        // earlier consent was revoked before it was granted again.
        let consent =
            |client: &'static str, granted: DateTime<Utc>, revoked: Option<DateTime<Utc>>| {
                let db = db.clone();
                async move {
                    sqlx::query(
                        "INSERT INTO identity.consents
                         (id, account_id, client_id, scopes, remembered, redirect_uri, granted_at,
                          revoked_at)
                     VALUES ($1, $2, $3, '{}', false, 'http://localhost:1/cb', $4, $5)",
                    )
                    .bind(Uuid::now_v7())
                    .bind(account)
                    .bind(client)
                    .bind(granted)
                    .bind(revoked)
                    .execute(&db)
                    .await
                    .unwrap();
                }
            };
        let hour = TimeDelta::hours(1);
        consent(
            "revoked-app",
            now - hour,
            Some(now + TimeDelta::seconds(30)),
        )
        .await;
        consent("regranted-app", now - hour * 3, Some(now - hour * 2)).await;
        consent("regranted-app", now - hour, None).await;

        let port = super::token_revocation();
        let ended_for = |client: &'static str, sign_in: SignIn| {
            let port = port.clone();
            let pool = pool.clone();
            let principal = token(&account.to_string(), Some(client), iat, Some(sign_in));
            async move {
                port.standing(&pool, &principal)
                    .await
                    .unwrap()
                    .unwrap()
                    .sign_in_ended
            }
        };
        let sign_in = |session: Uuid, family: Option<Uuid>| SignIn {
            session: Some(session.to_string()),
            family: family.map(|f| f.to_string()),
        };

        // Untouched: a live session, a live family, a first-party client.
        assert!(!ended_for("campbooks-web", sign_in(live, Some(kept))).await);
        assert!(
            ended_for("campbooks-web", sign_in(ended, None)).await,
            "session ended"
        );
        assert!(
            ended_for("campbooks-web", sign_in(live, Some(revoked))).await,
            "family revoked"
        );
        assert!(
            ended_for("revoked-app", sign_in(live, None)).await,
            "consent revoked"
        );
        assert!(
            !ended_for("regranted-app", sign_in(live, None)).await,
            "a consent revoked before it, then granted again"
        );
        // A sign-in named with something that is no id stands for nothing.
        let bogus = SignIn {
            session: Some("not-an-id".to_owned()),
            family: None,
        };
        assert!(ended_for("campbooks-web", bogus).await);
    }
}
