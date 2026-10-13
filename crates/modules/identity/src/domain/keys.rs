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
//! tokens, so it needs no rotation either. If the instance secret changed and
//! a private key can no longer be opened, that key is dropped and a new one
//! made: everyone signs in again, as when the web sessions' key changes.

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
        f.debug_struct("Signer").field("kid", &self.kid).finish_non_exhaustive()
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

    async fn load(&self, db: &PgPool) -> Result<View> {
        let now = self.clock.now();
        // At most two passes: the second runs after dropping a key that can
        // no longer be opened.
        for _ in 0..2 {
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
            let active = rows
                .iter()
                .filter(|row| row.activate_at <= now && now < row.retire_at)
                .max_by_key(|row| row.activate_at)
                .ok_or_else(|| Error::internal("no active signing key after scheduling"))?;
            let Ok(signer) = self.open(active) else {
                tracing::warn!(
                    kid = %active.kid,
                    "a signing key cannot be opened (the instance secret changed?); replacing it"
                );
                sqlx::query("DELETE FROM identity.signing_keys WHERE kid = $1")
                    .bind(&active.kid)
                    .execute(db)
                    .await?;
                continue;
            };
            let jwks = serde_json::json!({
                "keys": rows.iter().map(|row| row.public_jwk.clone()).collect::<Vec<_>>(),
            });
            return Ok(View {
                checked_at: now,
                signer: Arc::new(signer),
                jwks,
            });
        }
        Err(Error::internal("could not make a usable signing key"))
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
            Some((activate_at, retire_at)) if activate_at <= now && retire_at - now <= PUBLISH_AHEAD => {
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
