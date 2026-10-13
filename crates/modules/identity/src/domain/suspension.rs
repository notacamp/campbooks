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
//! BFF drops the account's web sessions. Access tokens already issued live
//! out their ten minutes.

use campbooks_kernel::{EventBus, Result};
use chrono::{DateTime, Utc};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use super::accounts;
use crate::events::AccountSuspended;

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
    let Some(was_suspended) = lock(tx, account).await? else {
        return Ok(Outcome::Missing);
    };
    if was_suspended {
        return Ok(Outcome::Unchanged);
    }
    sqlx::query("UPDATE identity.accounts SET disabled_at = $2 WHERE id = $1")
        .bind(account)
        .bind(now)
        .execute(&mut **tx)
        .await?;
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
    events.publish(tx, &AccountSuspended { account }).await?;
    Ok(Outcome::Changed)
}

/// Lift the suspension of `account`, on behalf of administrator `by`. The
/// person signs in again: nothing that was ended comes back.
pub async fn unsuspend(
    tx: &mut Transaction<'_, Postgres>,
    account: Uuid,
    by: &str,
    now: DateTime<Utc>,
) -> Result<Outcome> {
    let Some(was_suspended) = lock(tx, account).await? else {
        return Ok(Outcome::Missing);
    };
    if !was_suspended {
        return Ok(Outcome::Unchanged);
    }
    sqlx::query("UPDATE identity.accounts SET disabled_at = NULL WHERE id = $1")
        .bind(account)
        .execute(&mut **tx)
        .await?;
    accounts::audit(
        tx,
        Some(account),
        "account_unsuspended",
        serde_json::json!({ "by": by }),
        now,
    )
    .await?;
    Ok(Outcome::Changed)
}

/// Lock `account`'s row; whether it is suspended, or `None` when there is no
/// such account.
async fn lock(tx: &mut Transaction<'_, Postgres>, account: Uuid) -> Result<Option<bool>> {
    Ok(sqlx::query_scalar(
        "SELECT disabled_at IS NOT NULL FROM identity.accounts WHERE id = $1 FOR UPDATE",
    )
    .bind(account)
    .fetch_optional(&mut **tx)
    .await?)
}
