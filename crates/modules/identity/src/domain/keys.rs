//! The issuer's signing keys: Ed25519, generated here, sealed at rest,
//! rotated on a schedule, published as a JWKS.
//!
//! A key is **published** (in the JWKS) from `publish_at`, **signs** from
//! `activate_at` until `retire_at`, and stays published until
//! `delete_after`, so every token it signed can still be verified:
//!
//! - one key signs at a time, for [`ACTIVE_FOR`];
//! - its successor is published [`PUBLISH_AHEAD`] before it takes over
//!   (longer than any verifier caches a JWKS: 24 h for ours);
//! - a retired key stays published [`KEEP_RETIRED`] (longer than any token
//!   lives).
//!
//! The schedule is kept on demand, not by a job: whenever the issuer signs
//! or publishes its keys and its view is more than [`RECHECK_EVERY`] old, it
//! checks (under an advisory lock, so concurrent processes create one key)
//! and creates or deletes what the schedule says. An idle instance issues no
//! tokens, so it needs no rotation either.
//!
//! If the signing key cannot be opened, the instance secret key is not the
//! one the keys were sealed with (a replica with the wrong
//! `CAMPBOOKS_SECRET_KEY`, or a changed one). The issuer then fails closed: it
//! signs nothing and deletes nothing, and the server refuses to start
//! ([`Keys::check`]). Restoring the right key fixes it; starting over is a
//! deliberate operator step (deleting `identity.signing_keys`), which signs
//! everyone out.

use std::sync::{Arc, Mutex, PoisonError};

use aws_lc_rs::signature::Ed25519KeyPair;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use campbooks_kernel::{
    Error, Result,
    clock::Clock,
    crypto::{Cipher, random_token},
};
use chrono::{DateTime, TimeDelta, Utc};
use jsonwebtoken::{
    Algorithm, EncodingKey,
    jwk::{Jwk, KeyAlgorithm, PublicKeyUse},
};
use sqlx::PgPool;

/// How long a key signs.
pub const ACTIVE_FOR: TimeDelta = TimeDelta::days(30);
/// How long before it signs a key is published.
pub const PUBLISH_AHEAD: TimeDelta = TimeDelta::days(7);
/// How long a retired key stays published.
pub const KEEP_RETIRED: TimeDelta = TimeDelta::days(2);
/// How often the schedule is checked while the issuer is busy.
pub const RECHECK_EVERY: TimeDelta = TimeDelta::minutes(1);

/// What is wrong, and what to do, when the signing key does not open.
pub const KEY_MISMATCH: &str = "CAMPBOOKS_SECRET_KEY does not match the stored signing keys: \
    restore the secret key they were sealed with. (To start over instead, delete the rows of \
    identity.signing_keys; everyone has to sign in again.)";

/// Advisory lock id serializing key creation across processes
/// (`hashtext('identity.signing_keys')` would vary by version; a constant
/// does not).
const LOCK_ID: i64 = 0x6964_656e_7469_7479; // "identity"

/// A key that signs.
pub struct Signer {
    /// The key id (`kid`).
    pub kid: String,
    /// The private key.
    pub key: EncodingKey,
}

impl std::fmt::Debug for Signer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Signer")
            .field("kid", &self.kid)
            .finish_non_exhaustive()
    }
}

struct View {
    checked_at: DateTime<Utc>,
    signer: Arc<Signer>,
    jwks: serde_json::Value,
}

/// The signing keys of one issuer.
pub struct Keys {
    cipher: Cipher,
    clock: Arc<dyn Clock>,
    view: Mutex<Option<Arc<View>>>,
    refresh: tokio::sync::Mutex<()>,
}

impl std::fmt::Debug for Keys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Keys").finish_non_exhaustive()
    }
}

#[derive(sqlx::FromRow)]
struct KeyRow {
    kid: String,
    public_jwk: serde_json::Value,
    private_key: Vec<u8>,
    activate_at: DateTime<Utc>,
    retire_at: DateTime<Utc>,
}

impl Keys {
    /// Keys sealed with `cipher`, scheduled by `clock`.
    pub fn new(cipher: Cipher, clock: Arc<dyn Clock>) -> Self {
        Self {
            cipher,
            clock,
            view: Mutex::new(None),
            refresh: tokio::sync::Mutex::new(()),
        }
    }

    /// The key that signs now.
    pub async fn signer(&self, db: &PgPool) -> Result<Arc<Signer>> {
        Ok(Arc::clone(&self.view(db).await?.signer))
    }

    /// The JWKS document: every published key's public half.
    pub async fn jwks(&self, db: &PgPool) -> Result<serde_json::Value> {
        Ok(self.view(db).await?.jwks.clone())
    }

    async fn view(&self, db: &PgPool) -> Result<Arc<View>> {
        if let Some(view) = self.fresh() {
            return Ok(view);
        }
        let _single_flight = self.refresh.lock().await;
        if let Some(view) = self.fresh() {
            return Ok(view);
        }
        let view = Arc::new(self.load(db).await?);
        *self.view.lock().unwrap_or_else(PoisonError::into_inner) = Some(Arc::clone(&view));
        Ok(view)
    }

    fn fresh(&self) -> Option<Arc<View>> {
        let now = self.clock.now();
        self.view
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .filter(|view| (TimeDelta::zero()..RECHECK_EVERY).contains(&(now - view.checked_at)))
            .cloned()
    }

    /// Startup check: the signing keys exist and open with this instance's
    /// secret key. The error says what to do.
    pub async fn check(&self, db: &PgPool) -> std::result::Result<(), String> {
        self.signer(db)
            .await
            .map(|_| ())
            .map_err(|_| KEY_MISMATCH.to_owned())
    }

    async fn load(&self, db: &PgPool) -> Result<View> {
        let now = self.clock.now();
        self.keep_schedule(db, now).await?;
        let rows: Vec<KeyRow> = sqlx::query_as(
            "SELECT kid, public_jwk, private_key, activate_at, retire_at
               FROM identity.signing_keys
              WHERE publish_at <= $1 AND delete_after > $1
              ORDER BY activate_at",
        )
        .bind(now)
        .fetch_all(db)
        .await?;
        self.seal_again(db, &rows).await?;
        let active = rows
            .iter()
            .filter(|row| row.activate_at <= now && now < row.retire_at)
            .max_by_key(|row| row.activate_at)
            .ok_or_else(|| Error::internal("no active signing key after scheduling"))?;
        let signer = self.open(active).map_err(|()| {
            tracing::error!(kid = %active.kid, "{KEY_MISMATCH}");
            Error::internal(KEY_MISMATCH)
        })?;
        let jwks = serde_json::json!({
            "keys": rows.iter().map(|row| row.public_jwk.clone()).collect::<Vec<_>>(),
        });
        Ok(View {
            checked_at: now,
            signer: Arc::new(signer),
            jwks,
        })
    }

    /// Seal again, with the current instance secret, every key the
    /// previous one sealed (campbooks-cloud#191), so the keys still open
    /// once the previous key is gone. A key that opens with neither is left
    /// as it is: [`check`](Self::check) refuses it.
    async fn seal_again(&self, db: &PgPool, rows: &[KeyRow]) -> Result<()> {
        for row in rows {
            let context = context(&row.kid);
            let Ok(opened) = self.cipher.open_checked(&row.private_key, &context) else {
                continue;
            };
            if !opened.by_previous {
                continue;
            }
            sqlx::query("UPDATE identity.signing_keys SET private_key = $2 WHERE kid = $1")
                .bind(&row.kid)
                .bind(self.cipher.seal(&opened.value, &context))
                .execute(db)
                .await?;
            tracing::info!(kid = %row.kid, "sealed a signing key again with the current secret key");
        }
        Ok(())
    }

    fn open(&self, row: &KeyRow) -> Result<Signer, ()> {
        let pkcs8 = self
            .cipher
            .open(&row.private_key, &context(&row.kid))
            .map_err(|_| ())?;
        let der = URL_SAFE_NO_PAD.decode(pkcs8).map_err(|_| ())?;
        Ok(Signer {
            kid: row.kid.clone(),
            key: EncodingKey::from_ed_der(&der),
        })
    }

    /// Delete keys past `delete_after`; create the active key when there is
    /// none, and its successor when it retires within [`PUBLISH_AHEAD`].
    async fn keep_schedule(&self, db: &PgPool, now: DateTime<Utc>) -> Result<()> {
        let mut tx = db.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(LOCK_ID)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM identity.signing_keys WHERE delete_after <= $1")
            .bind(now)
            .execute(&mut *tx)
            .await?;
        let latest: Option<(DateTime<Utc>, DateTime<Utc>)> = sqlx::query_as(
            "SELECT activate_at, retire_at FROM identity.signing_keys
              WHERE retire_at > $1 ORDER BY activate_at DESC LIMIT 1",
        )
        .bind(now)
        .fetch_optional(&mut *tx)
        .await?;
        match latest {
            None => self.insert(&mut tx, now, now).await?,
            Some((activate_at, retire_at))
                if activate_at <= now && retire_at - now <= PUBLISH_AHEAD =>
            {
                self.insert(&mut tx, now, retire_at).await?;
            }
            Some(_) => {}
        }
        tx.commit().await?;
        Ok(())
    }

    async fn insert(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        now: DateTime<Utc>,
        activate_at: DateTime<Utc>,
    ) -> Result<()> {
        let kid = random_token();
        let pkcs8 = Ed25519KeyPair::generate()
            .and_then(|pair| pair.to_pkcs8v1())
            .map_err(|e| Error::internal(format!("generating a signing key: {e}")))?;
        let encoding = EncodingKey::from_ed_der(pkcs8.as_ref());
        let mut jwk = Jwk::from_encoding_key(&encoding, Algorithm::EdDSA)
            .map_err(|e| Error::internal(format!("deriving a signing key's JWK: {e}")))?;
        jwk.common.key_id = Some(kid.clone());
        jwk.common.public_key_use = Some(PublicKeyUse::Signature);
        jwk.common.key_algorithm = Some(KeyAlgorithm::EdDSA);
        let sealed = self
            .cipher
            .seal(&URL_SAFE_NO_PAD.encode(pkcs8.as_ref()), &context(&kid));
        let retire_at = activate_at + ACTIVE_FOR;
        sqlx::query(
            "INSERT INTO identity.signing_keys
                 (kid, alg, public_jwk, private_key, created_at, publish_at, activate_at,
                  retire_at, delete_after)
             VALUES ($1, 'EdDSA', $2, $3, $4, $4, $5, $6, $7)",
        )
        .bind(&kid)
        .bind(serde_json::to_value(&jwk).map_err(Error::internal)?)
        .bind(sealed)
        .bind(now)
        .bind(activate_at)
        .bind(retire_at)
        .bind(retire_at + KEEP_RETIRED)
        .execute(&mut **tx)
        .await?;
        tracing::info!(%kid, activate_at = %activate_at, "created a signing key");
        Ok(())
    }
}

/// What a sealed private key is bound to.
fn context(kid: &str) -> Vec<u8> {
    format!("identity.signing_keys:{kid}:private_key").into_bytes()
}

/// The issuer's keys as the kernel's token verifier reads them in-process.
#[derive(Debug, Clone)]
pub struct LocalKeys {
    /// The database.
    pub db: PgPool,
    /// The keys.
    pub keys: Arc<Keys>,
}

impl campbooks_kernel::auth::KeySource for LocalKeys {
    fn jwks(&self) -> campbooks_kernel::auth::KeySourceFuture<'_> {
        Box::pin(async move {
            self.keys
                .jwks(&self.db)
                .await
                .map_err(|e| format!("reading the signing keys: {e}"))
        })
    }
}

#[cfg(test)]
mod tests {
    use campbooks_kernel::{clock::FixedClock, crypto::SecretKey, testing};
    use chrono::{TimeZone as _, Utc};
    use sqlx::PgPool;

    use super::*;

    fn keys(clock: &Arc<FixedClock>, secret: u8) -> Keys {
        Keys::new(
            Cipher::new(
                &SecretKey::from_bytes([secret; 32]),
                b"campbooks identity test v1",
            ),
            clock.clone(),
        )
    }

    fn kids(jwks: &serde_json::Value) -> Vec<String> {
        jwks["keys"]
            .as_array()
            .unwrap()
            .iter()
            .map(|k| k["kid"].as_str().unwrap().to_owned())
            .collect()
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn keys_rotate_on_schedule_with_overlap(db: PgPool) {
        let db = testing::app_role_pool(&db).await;
        let clock = Arc::new(FixedClock::new(
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        ));
        let keys = keys(&clock, 1);

        let first = keys.signer(&db).await.unwrap().kid.clone();
        assert_eq!(
            kids(&keys.jwks(&db).await.unwrap()),
            std::slice::from_ref(&first)
        );

        // Within a week of retirement the successor is published, not used.
        clock.advance(TimeDelta::days(24));
        assert_eq!(keys.signer(&db).await.unwrap().kid, first);
        let published = kids(&keys.jwks(&db).await.unwrap());
        assert_eq!(published.len(), 2);
        assert_eq!(published[0], first);
        let second = published[1].clone();

        // At 30 days the successor signs; the first stays published.
        clock.advance(TimeDelta::days(6) + TimeDelta::minutes(1));
        assert_eq!(keys.signer(&db).await.unwrap().kid, second);
        assert!(kids(&keys.jwks(&db).await.unwrap()).contains(&first));

        // Two days after retiring, the first is gone.
        clock.advance(TimeDelta::days(2));
        assert_eq!(kids(&keys.jwks(&db).await.unwrap()), [second]);
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn concurrent_first_starts_make_one_key(db: PgPool) {
        let db = testing::app_role_pool(&db).await;
        let clock = Arc::new(FixedClock::new(Utc::now()));
        let processes: Vec<Keys> = (0..3).map(|_| keys(&clock, 1)).collect();
        let signers = futures_join(&processes, &db).await;
        assert!(signers.windows(2).all(|w| w[0] == w[1]), "{signers:?}");
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM identity.signing_keys")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }

    async fn futures_join(processes: &[Keys], db: &PgPool) -> Vec<String> {
        let (a, b, c) = tokio::join!(
            processes[0].signer(db),
            processes[1].signer(db),
            processes[2].signer(db)
        );
        [a, b, c]
            .into_iter()
            .map(|s| s.unwrap().kid.clone())
            .collect()
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn another_secret_key_fails_closed_and_deletes_nothing(db: PgPool) {
        let db = testing::app_role_pool(&db).await;
        let clock = Arc::new(FixedClock::new(Utc::now()));
        let kid = keys(&clock, 1).signer(&db).await.unwrap().kid.clone();

        let wrong = keys(&clock, 2);
        assert!(wrong.signer(&db).await.is_err());
        let refused = wrong.check(&db).await.unwrap_err();
        assert!(refused.contains("CAMPBOOKS_SECRET_KEY"), "{refused}");
        let kept: Vec<String> = sqlx::query_scalar("SELECT kid FROM identity.signing_keys")
            .fetch_all(&db)
            .await
            .unwrap();
        assert_eq!(kept, [kid]);
        assert!(keys(&clock, 1).check(&db).await.is_ok());
    }

    /// After a rotation of the instance secret, with the previous key set,
    /// the signing keys still open and are sealed again with the new one:
    /// once the previous key is gone they open with the new key alone. A
    /// wrong pair fails closed (campbooks-cloud#191).
    #[sqlx::test(migrations = "../../../migrations")]
    async fn a_rotated_secret_key_opens_and_seals_the_keys_again(db: PgPool) {
        let db = testing::app_role_pool(&db).await;
        let clock = Arc::new(FixedClock::new(Utc::now()));
        let kid = keys(&clock, 1).signer(&db).await.unwrap().kid.clone();
        let sealed_before: Vec<u8> =
            sqlx::query_scalar("SELECT private_key FROM identity.signing_keys")
                .fetch_one(&db)
                .await
                .unwrap();

        let rotated = |current: u8, previous: u8| {
            Keys::new(
                Cipher::new(
                    &SecretKey::from_bytes([current; 32])
                        .with_previous(&SecretKey::from_bytes([previous; 32])),
                    b"campbooks identity test v1",
                ),
                clock.clone(),
            )
        };
        // A wrong pair: neither key sealed it.
        assert!(rotated(3, 2).check(&db).await.is_err());
        // The new key, with the old one as previous: opens, sealed again.
        assert_eq!(rotated(2, 1).signer(&db).await.unwrap().kid, kid);
        let sealed_after: Vec<u8> =
            sqlx::query_scalar("SELECT private_key FROM identity.signing_keys")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_ne!(sealed_after, sealed_before);
        // The previous key gone: the new key alone opens it; the old alone
        // no longer does.
        assert!(keys(&clock, 2).check(&db).await.is_ok());
        assert_eq!(keys(&clock, 2).signer(&db).await.unwrap().kid, kid);
        assert!(keys(&clock, 1).check(&db).await.is_err());
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn private_keys_are_sealed_at_rest(db: PgPool) {
        let db = testing::app_role_pool(&db).await;
        let clock = Arc::new(FixedClock::new(Utc::now()));
        keys(&clock, 1).signer(&db).await.unwrap();
        let (jwk, sealed): (serde_json::Value, Vec<u8>) =
            sqlx::query_as("SELECT public_jwk, private_key FROM identity.signing_keys")
                .fetch_one(&db)
                .await
                .unwrap();
        assert!(jwk.get("d").is_none());
        assert_eq!(sealed[0], 1, "the kernel's sealed format, version 1");
    }
}
