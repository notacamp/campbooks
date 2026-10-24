//! Passkeys as ways in, and the single-use challenges of their ceremonies.
//!
//! A credential id is unique across the instance, so an assertion names at
//! most one account. A challenge is a random 256-bit value stored only as
//! its SHA-256, taken (deleted) by the first attempt that names it from the
//! account or browser that started it, whether that attempt then verifies
//! or not, and gone after
//! [`CEREMONY_TIMEOUT`](crate::webauthn::CEREMONY_TIMEOUT).

use campbooks_kernel::{
    Result,
    crypto::{hash, random_token},
};
use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::webauthn::{self, CEREMONY_TIMEOUT, CredentialDescriptor, NewCredential};

/// Longest passkey name.
pub const MAX_NAME_CHARS: usize = 64;

/// A stored passkey.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Passkey {
    /// Its id (the way in's id).
    pub id: Uuid,
    /// The account it is a way into.
    pub account_id: Uuid,
    /// The WebAuthn credential id.
    pub credential_id: Vec<u8>,
    /// Its COSE_Key.
    pub public_key: Vec<u8>,
    /// Its COSE algorithm.
    pub alg: i32,
    /// The last signature counter seen.
    pub sign_count: i64,
    /// Transports the authenticator reported.
    pub transports: Vec<String>,
    /// BE: it may be synced.
    pub backup_eligible: bool,
    /// BS: it was synced when last used.
    pub backup_state: bool,
    /// What the person calls it.
    pub name: String,
    /// When it was registered.
    pub created_at: DateTime<Utc>,
    /// When it last signed someone in.
    pub last_used_at: Option<DateTime<Utc>>,
    /// It was made with the account (a passkey-only sign-up), not added
    /// later.
    pub signed_up_with: bool,
}

/// The columns of a [`Passkey`].
macro_rules! columns {
    () => {
        "id, account_id, credential_id, public_key, alg, sign_count, transports, \
         backup_eligible, backup_state, name, created_at, last_used_at, \
         added_via = 'sign_up' AS signed_up_with"
    };
}

/// A name, trimmed, with control characters removed and at most
/// [`MAX_NAME_CHARS`] characters; `None` when nothing is left.
pub fn clean_name(name: &str) -> Option<String> {
    let cleaned: String = name
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(MAX_NAME_CHARS)
        .collect();
    let cleaned = cleaned.trim_end().to_owned();
    (!cleaned.is_empty()).then_some(cleaned)
}

/// Create account `id`, named `display_name`, with no provider and no
/// address: its passkey, stored next, is its only way in.
pub async fn create_account(
    conn: &mut PgConnection,
    id: Uuid,
    display_name: &str,
    now: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO identity.accounts (id, display_name, primary_email_verified, created_at)
         VALUES ($1, $2, false, $3)",
    )
    .bind(id)
    .bind(display_name)
    .bind(now)
    .execute(conn)
    .await?;
    Ok(())
}

/// Store `credential` as a way into `account`, named `name`, made with the
/// account when `signed_up_with` (a passkey-only sign-up). `None` when that
/// credential id is already registered (to anyone).
pub async fn insert(
    conn: &mut PgConnection,
    account: Uuid,
    credential: &NewCredential,
    name: &str,
    signed_up_with: bool,
    now: DateTime<Utc>,
) -> Result<Option<Uuid>> {
    let id = Uuid::now_v7();
    let inserted = sqlx::query(
        "INSERT INTO identity.passkeys
             (id, account_id, credential_id, public_key, alg, sign_count, aaguid, transports,
              backup_eligible, backup_state, name, created_at, added_via)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
                 CASE WHEN $13 THEN 'sign_up' ELSE 'passkey' END)
         ON CONFLICT (credential_id) DO NOTHING",
    )
    .bind(id)
    .bind(account)
    .bind(&credential.credential_id)
    .bind(&credential.public_key)
    .bind(i32::try_from(credential.alg).unwrap_or_default())
    .bind(i64::from(credential.sign_count))
    .bind(credential.aaguid)
    .bind(&credential.transports)
    .bind(credential.backup_eligible)
    .bind(credential.backup_state)
    .bind(name)
    .bind(now)
    .bind(signed_up_with)
    .execute(conn)
    .await?
    .rows_affected();
    Ok((inserted == 1).then_some(id))
}

/// The passkeys of `account`, oldest first.
pub async fn list(db: &PgPool, account: Uuid) -> Result<Vec<Passkey>> {
    Ok(sqlx::query_as(concat!(
        "SELECT ",
        columns!(),
        " FROM identity.passkeys WHERE account_id = $1 ORDER BY created_at, id"
    ))
    .bind(account)
    .fetch_all(db)
    .await?)
}

/// The passkeys of `account` as `excludeCredentials` names them.
pub async fn descriptors(db: &PgPool, account: Uuid) -> Result<Vec<CredentialDescriptor>> {
    Ok(list(db, account)
        .await?
        .into_iter()
        .map(|p| webauthn::descriptor(&p.credential_id, p.transports))
        .collect())
}

/// The passkey with credential id `credential_id`, locked for the rest of
/// the transaction. Its account may be suspended: the caller refuses it
/// with [`accounts::is_suspended`](super::accounts::is_suspended) once the
/// assertion verified, so only the passkey's holder learns why.
pub async fn by_credential(
    conn: &mut PgConnection,
    credential_id: &[u8],
) -> Result<Option<Passkey>> {
    Ok(sqlx::query_as(concat!(
        "SELECT ",
        columns!(),
        " FROM identity.passkeys p
          WHERE credential_id = $1
          FOR UPDATE"
    ))
    .bind(credential_id)
    .fetch_optional(conn)
    .await?)
}

/// Record that passkey `id` signed someone in, with the counter and backup
/// state its authenticator reported. The counter only moves forward.
pub async fn record_use(
    conn: &mut PgConnection,
    id: Uuid,
    sign_count: u32,
    backup_state: bool,
    now: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "UPDATE identity.passkeys
            SET sign_count = greatest(sign_count, $2), backup_state = $3, last_used_at = $4
          WHERE id = $1",
    )
    .bind(id)
    .bind(i64::from(sign_count))
    .bind(backup_state)
    .bind(now)
    .execute(conn)
    .await?;
    Ok(())
}

/// Rename `account`'s passkey `id`; the renamed passkey, or `None` when it
/// is not theirs.
pub async fn rename(
    conn: &mut PgConnection,
    id: Uuid,
    account: Uuid,
    name: &str,
) -> Result<Option<Passkey>> {
    Ok(sqlx::query_as(concat!(
        "UPDATE identity.passkeys SET name = $3 WHERE id = $1 AND account_id = $2 RETURNING ",
        columns!()
    ))
    .bind(id)
    .bind(account)
    .bind(name)
    .fetch_optional(conn)
    .await?)
}

/// The name of `account`'s passkey `id`, its row held (`FOR UPDATE`) until
/// the transaction ends; `None` when it is not theirs.
pub async fn name_of(conn: &mut PgConnection, id: Uuid, account: Uuid) -> Result<Option<String>> {
    Ok(sqlx::query_scalar(
        "SELECT name FROM identity.passkeys WHERE id = $1 AND account_id = $2 FOR UPDATE",
    )
    .bind(id)
    .bind(account)
    .fetch_optional(conn)
    .await?)
}

/// Whether `account` has passkey `id`.
pub async fn owned(conn: &mut PgConnection, id: Uuid, account: Uuid) -> Result<bool> {
    let found: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM identity.passkeys WHERE id = $1 AND account_id = $2")
            .bind(id)
            .bind(account)
            .fetch_optional(conn)
            .await?;
    Ok(found.is_some())
}

/// Delete `account`'s passkey `id`; whether it was theirs.
pub async fn delete(conn: &mut PgConnection, id: Uuid, account: Uuid) -> Result<bool> {
    let deleted = sqlx::query("DELETE FROM identity.passkeys WHERE id = $1 AND account_id = $2")
        .bind(id)
        .bind(account)
        .execute(conn)
        .await?
        .rows_affected();
    Ok(deleted == 1)
}

/// What a ceremony is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// A signed-in account adds a passkey.
    Register,
    /// Someone signs in.
    SignIn,
    /// Someone creates an account with a passkey alone.
    SignUp,
}

impl Purpose {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Register => "register",
            Self::SignIn => "sign_in",
            Self::SignUp => "sign_up",
        }
    }
}

/// A ceremony waiting for the browser.
#[derive(Debug, Clone, PartialEq, Eq, Default, sqlx::FromRow)]
pub struct Ceremony {
    /// The account adding a passkey (`register`).
    pub account_id: Option<Uuid>,
    /// The new account's id (`sign_up`).
    pub user_handle: Option<Uuid>,
    /// The new account's name (`sign_up`).
    pub display_name: Option<String>,
    /// The authorization request to complete afterwards.
    pub request_id: Option<String>,
    /// Where to land afterwards (`sign_in`, `sign_up`).
    pub return_to: Option<String>,
}

/// Who may finish a ceremony: the account that started it, or the browser
/// holding the ceremony cookie.
#[derive(Debug, Clone, Copy)]
pub enum Binding<'a> {
    /// A signed-in account (`register`).
    Account(Uuid),
    /// A browser, by its ceremony cookie (`sign_in`, `sign_up`).
    Browser(&'a str),
}

/// Start a `purpose` ceremony for `binding`: the challenge to hand to the
/// browser.
pub async fn start(
    db: &PgPool,
    purpose: Purpose,
    binding: Binding<'_>,
    ceremony: &Ceremony,
    now: DateTime<Utc>,
) -> Result<String> {
    let challenge = random_token();
    let (account, browser) = match binding {
        Binding::Account(account) => (Some(account), None),
        Binding::Browser(cookie) => (None, Some(hash(cookie))),
    };
    sqlx::query(
        "INSERT INTO identity.webauthn_challenges
             (challenge_hash, purpose, account_id, browser_hash, user_handle, display_name,
              request_id, return_to, created_at, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(hash(&challenge))
    .bind(purpose.as_str())
    .bind(account.or(ceremony.account_id))
    .bind(browser)
    .bind(ceremony.user_handle)
    .bind(&ceremony.display_name)
    .bind(&ceremony.request_id)
    .bind(&ceremony.return_to)
    .bind(now)
    .bind(now + CEREMONY_TIMEOUT)
    .execute(db)
    .await?;
    Ok(challenge)
}

/// Take the `purpose` ceremony of `challenge` for `binding`: deleted
/// whatever happens next, so it is used at most once. `None` when there is
/// none (unknown, expired, already used, another purpose, or started by
/// another account or browser).
pub async fn take(
    db: &PgPool,
    challenge: &str,
    purpose: Purpose,
    binding: Binding<'_>,
    now: DateTime<Utc>,
) -> Result<Option<Ceremony>> {
    let (account, browser) = match binding {
        Binding::Account(account) => (Some(account), None),
        Binding::Browser(cookie) => (None, Some(hash(cookie))),
    };
    Ok(sqlx::query_as(
        "DELETE FROM identity.webauthn_challenges
          WHERE challenge_hash = $1 AND purpose = $2 AND expires_at > $5
            AND (($3::uuid IS NOT NULL AND account_id = $3)
              OR ($4::bytea IS NOT NULL AND browser_hash = $4))
         RETURNING account_id, user_handle, display_name, request_id, return_to",
    )
    .bind(hash(challenge))
    .bind(purpose.as_str())
    .bind(account)
    .bind(browser)
    .bind(now)
    .fetch_optional(db)
    .await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_trimmed_bounded_and_printable() {
        assert_eq!(clean_name("  MacBook\n "), Some("MacBook".to_owned()));
        assert_eq!(clean_name("\u{7}\t "), None);
        assert_eq!(
            clean_name(&"é".repeat(80)).map(|n| n.chars().count()),
            Some(64)
        );
    }
}
