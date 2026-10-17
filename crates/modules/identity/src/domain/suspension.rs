//! Suspending an account (ADR 0015, Admin → People): it can no longer get
//! in by any way, and whoever is signed in as it is signed out.
//!
//! A suspended account (`identity.accounts.disabled_at`) is refused at every
//! door the issuer has: a provider sign-in answers `account_suspended`, its
//! issuer sessions no longer count (no single sign-on), its codes are not
//! redeemed and its refresh tokens are not rotated. Suspending ends every
//! live session and revokes every refresh-token family at once, records it
//! in the account's audit trail and publishes
//! [`AccountSuspended`](crate::events::AccountSuspended), on which the web
//! BFF drops the account's web sessions and the rules its member taught
//! pause. The access tokens already issued
//! stop at once: the account's revocation epoch moves to now
//! ([`accounts::revoke_tokens`]), and the API checks it on every request.
//! Unsuspending leaves the epoch: those tokens never come back. It publishes
//! [`AccountUnsuspended`](crate::events::AccountUnsuspended), on which the
//! paused rules act again.
//!
//! Both events carry `seq`, the account's count of suspensions and lifts
//! (`identity.accounts.suspension_seq`), bumped by one under the account's
//! row lock: of two changes to one account the later always has the higher
//! `seq`, so consumers order them by it whatever order they arrive in and
//! whatever the clock did between them (campbooks-cloud#197). They also
//! carry `at`, the moment the change took effect, read under the same lock;
//! consumers order by it only the events published before `seq` existed.

use campbooks_kernel::{EventBus, Result};
use chrono::{DateTime, Utc};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use super::accounts;
use crate::events::{AccountSuspended, AccountUnsuspended};

/// What suspending or unsuspending changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The account changed state.
    Changed,
    /// It already was in that state: nothing changed (a retry).
    Unchanged,
    /// No such account.
    Missing,
}

/// Suspend `account` on behalf of administrator `by` (an account id).
pub async fn suspend(
    tx: &mut Transaction<'_, Postgres>,
    events: &EventBus,
    account: Uuid,
    by: &str,
    now: DateTime<Utc>,
) -> Result<Outcome> {
    let Some(was_suspended) = accounts::lock(tx, account).await? else {
        return Ok(Outcome::Missing);
    };
    if was_suspended {
        return Ok(Outcome::Unchanged);
    }
    let seq: i64 = sqlx::query_scalar(
        "UPDATE identity.accounts SET disabled_at = $2, suspension_seq = suspension_seq + 1
          WHERE id = $1 RETURNING suspension_seq",
    )
    .bind(account)
    .bind(now)
    .fetch_one(&mut **tx)
    .await?;
    accounts::revoke_tokens(tx, account, now).await?;
    let sessions = sqlx::query(
        "UPDATE identity.sessions SET ended_at = $2 WHERE account_id = $1 AND ended_at IS NULL",
    )
    .bind(account)
    .bind(now)
    .execute(&mut **tx)
    .await?
    .rows_affected();
    let families = sqlx::query(
        "UPDATE identity.refresh_families SET revoked_at = $2, revoke_reason = 'suspended'
          WHERE account_id = $1 AND revoked_at IS NULL",
    )
    .bind(account)
    .bind(now)
    .execute(&mut **tx)
    .await?
    .rows_affected();
    accounts::audit(
        tx,
        Some(account),
        "account_suspended",
        serde_json::json!({
            "by": by,
            "sessions_ended": sessions,
            "refresh_families_revoked": families,
        }),
        now,
    )
    .await?;
    let at = changed_at(tx).await?;
    events
        .publish(
            tx,
            &AccountSuspended {
                account,
                at: Some(at),
                seq: Some(seq),
            },
        )
        .await?;
    Ok(Outcome::Changed)
}

/// Lift the suspension of `account`, on behalf of administrator `by`. The
/// person signs in again: nothing that was ended comes back, the access
/// tokens issued before the suspension included (the epoch stays).
pub async fn unsuspend(
    tx: &mut Transaction<'_, Postgres>,
    events: &EventBus,
    account: Uuid,
    by: &str,
    now: DateTime<Utc>,
) -> Result<Outcome> {
    let Some(was_suspended) = accounts::lock(tx, account).await? else {
        return Ok(Outcome::Missing);
    };
    if !was_suspended {
        return Ok(Outcome::Unchanged);
    }
    let seq: i64 = sqlx::query_scalar(
        "UPDATE identity.accounts SET disabled_at = NULL, suspension_seq = suspension_seq + 1
          WHERE id = $1 RETURNING suspension_seq",
    )
    .bind(account)
    .fetch_one(&mut **tx)
    .await?;
    accounts::audit(
        tx,
        Some(account),
        "account_unsuspended",
        serde_json::json!({ "by": by }),
        now,
    )
    .await?;
    let at = changed_at(tx).await?;
    events
        .publish(
            tx,
            &AccountUnsuspended {
                account,
                at,
                seq: Some(seq),
            },
        )
        .await?;
    Ok(Outcome::Changed)
}

/// The moment a suspension or lift takes effect: the database's clock now,
/// while the account's row is locked (not the transaction's start, which may
/// precede an earlier change's commit). Ordering goes by `seq`; this orders
/// only events published before it existed.
async fn changed_at(tx: &mut Transaction<'_, Postgres>) -> Result<DateTime<Utc>> {
    Ok(sqlx::query_scalar("SELECT clock_timestamp()")
        .fetch_one(&mut **tx)
        .await?)
}
