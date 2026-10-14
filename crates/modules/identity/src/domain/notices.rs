//! Security notices: whom a notice about an account goes to, and which
//! notices were already sent (so a retried delivery never sends one twice).
//! The ledger keeps the event and which of the account's addresses it went
//! to (`account` or `way_in`), never the address itself.

use campbooks_kernel::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

/// Which of an account's addresses a notice goes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recipient {
    /// The account's primary address.
    Account,
    /// The address of the way in the notice is about.
    WayIn,
}

impl Recipient {
    /// The ledger's name for it.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Account => "account",
            Self::WayIn => "way_in",
        }
    }
}

/// The verified addresses a notice about `account` goes to: its primary
/// address, and the address of provider way in `way_in` when it differs.
/// None for a disabled or deleted account. Unverified addresses never get
/// one: nobody proved they are the account's.
pub async fn recipients(
    db: &PgPool,
    account: Uuid,
    way_in: Option<Uuid>,
) -> Result<Vec<(Recipient, String)>> {
    let primary: Option<Option<String>> = sqlx::query_scalar(
        "SELECT CASE WHEN primary_email_verified THEN primary_email END
           FROM identity.accounts WHERE id = $1 AND disabled_at IS NULL",
    )
    .bind(account)
    .fetch_optional(db)
    .await?;
    let Some(primary) = primary else {
        return Ok(Vec::new());
    };
    let mut to: Vec<(Recipient, String)> = primary
        .filter(|address| !address.trim().is_empty())
        .map(|address| (Recipient::Account, address))
        .into_iter()
        .collect();
    if let Some(way_in) = way_in {
        let linked: Option<String> = sqlx::query_scalar(
            "SELECT email_at_link FROM identity.provider_identities
              WHERE id = $1 AND account_id = $2 AND email_verified_at_link",
        )
        .bind(way_in)
        .bind(account)
        .fetch_optional(db)
        .await?
        .flatten();
        if let Some(address) = linked.filter(|address| !address.trim().is_empty())
            && !to
                .iter()
                .any(|(_, known)| known.trim().eq_ignore_ascii_case(address.trim()))
        {
            to.push((Recipient::WayIn, address));
        }
    }
    Ok(to)
}

/// Whether notice `event` already went to `recipient`.
pub async fn sent(db: &PgPool, event: Uuid, recipient: Recipient) -> Result<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM identity.notices WHERE event_id = $1 AND recipient = $2)",
    )
    .bind(event)
    .bind(recipient.as_str())
    .fetch_one(db)
    .await?)
}

/// Record that notice `event` went to `recipient` (committed at once, on
/// its own: a later failure of the same delivery must not forget it).
pub async fn record(
    db: &PgPool,
    event: Uuid,
    account: Uuid,
    recipient: Recipient,
    now: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO identity.notices (event_id, recipient, account_id, sent_at)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT DO NOTHING",
    )
    .bind(event)
    .bind(recipient.as_str())
    .bind(account)
    .bind(now)
    .execute(db)
    .await?;
    Ok(())
}
