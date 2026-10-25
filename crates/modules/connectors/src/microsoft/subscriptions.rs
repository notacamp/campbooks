//! Graph change notifications for a Microsoft connection
//! (campbooks-cloud#316): the subscription a stream's run makes sure of
//! before it reads, kept in `connectors.microsoft_subscriptions`.
//!
//! A subscription lasts about three days ([`LIFETIME`]) and is renewed by
//! the first run within a day of its end ([`RENEW_BEFORE`]). Its
//! `clientState` is 256 random bits, given to Graph once and kept only as
//! its keyed hash under the instance key ([`CLIENT_STATE_CONTEXT`]): what
//! the callback (`features::microsoft_notifications`) checks a notification
//! against, in constant time. A creation Graph refuses (its check of the
//! notification URL failing, say) is tried again no sooner than
//! [`RETRY_AFTER_FAILURE`], and never fails the run: the stream is polled
//! meanwhile. Nothing here is ever logged with the `clientState`.

use campbooks_kernel::{crypto::Signer, db::BeginFor as _, tenancy::WorkspaceId};
use chrono::{DateTime, TimeDelta, Utc};
use sqlx::PgPool;

use super::Microsoft;
use crate::domain::model::{ConnectionId, StreamId};

/// How long a subscription is asked for: under Graph's limit for mail
/// without resource data.
pub const LIFETIME: TimeDelta = TimeDelta::minutes(4230);

/// A subscription ending within this is renewed.
pub const RENEW_BEFORE: TimeDelta = TimeDelta::hours(24);

/// After a creation Graph refused, the next try waits this long.
pub const RETRY_AFTER_FAILURE: TimeDelta = TimeDelta::hours(1);

/// What a `clientState`'s hash is signed for: no other value verifies.
pub const CLIENT_STATE_CONTEXT: &[u8] = b"connectors.microsoft.client_state";

/// What the instance key's fingerprint is signed for ([`key_tag`]).
pub const KEY_CONTEXT: &[u8] = b"connectors.microsoft.key";

/// The connection a subscription is for.
#[derive(Debug, Clone, Copy)]
pub struct For {
    pub workspace: WorkspaceId,
    pub connection: ConnectionId,
    pub stream: StreamId,
}

/// A connection's subscription row.
#[derive(Debug, sqlx::FromRow)]
struct Row {
    subscription_id: Option<String>,
    expires_at: Option<DateTime<Utc>>,
    retry_at: Option<DateTime<Utc>>,
    key_tag: Option<Vec<u8>>,
}

/// What [`ensure`] did: whether a subscription is live after, and how many
/// calls it made to Graph (charged to the connection's budget).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ensured {
    pub live: bool,
    pub calls: u32,
}

/// The instance key's fingerprint now: a subscription whose `clientState`
/// was hashed under another key (rotated since) is made again, before the
/// old key retires and its notifications stop verifying.
pub fn key_tag(signer: &Signer) -> Vec<u8> {
    signer.sign(KEY_CONTEXT, b"")
}

/// Make sure `at`'s connection has a live subscription, with `token`, when
/// `microsoft` takes notifications; forget it when it does not. A
/// subscription ending within [`RENEW_BEFORE`] is renewed; one whose
/// `clientState` was hashed under another instance key is made again with a
/// fresh one.
pub async fn ensure(
    pool: &PgPool,
    signer: &Signer,
    microsoft: &Microsoft,
    token: &str,
    at: For,
    now: DateTime<Utc>,
) -> Result<Ensured, sqlx::Error> {
    let idle = Ensured {
        live: false,
        calls: 0,
    };
    if microsoft.notification_url().is_none() {
        // Notifications turned off: Graph's subscription lapses on its own,
        // its notifications answered 404.
        forget(pool, at).await?;
        return Ok(idle);
    }
    let mut tx = pool.begin_for(at.workspace).await?;
    let row: Option<Row> = sqlx::query_as(
        "SELECT subscription_id, expires_at, retry_at, key_tag
           FROM connectors.microsoft_subscriptions WHERE connection_id = $1",
    )
    .bind(at.connection)
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    let current = key_tag(signer);
    match row {
        Some(Row {
            subscription_id: Some(_),
            key_tag: Some(tag),
            ..
        }) if tag != current => {
            tracing::info!(
                connection = %at.connection,
                "the instance key changed: a new Microsoft subscription"
            );
            create(pool, signer, microsoft, token, at, now, 0).await
        }
        Some(Row {
            subscription_id: Some(id),
            expires_at: Some(expires),
            ..
        }) => {
            if expires - now > RENEW_BEFORE {
                return Ok(Ensured {
                    live: true,
                    calls: 0,
                });
            }
            match microsoft.renew(token, &id, now + LIFETIME).await {
                Ok(Some(renewed)) => {
                    let mut tx = pool.begin_for(at.workspace).await?;
                    sqlx::query(
                        "UPDATE connectors.microsoft_subscriptions
                            SET expires_at = $2, updated_at = $3
                          WHERE connection_id = $1 AND subscription_id = $4",
                    )
                    .bind(at.connection)
                    .bind(renewed.expires)
                    .bind(now)
                    .bind(&id)
                    .execute(&mut *tx)
                    .await?;
                    tx.commit().await?;
                    tracing::debug!(connection = %at.connection, "Microsoft subscription renewed");
                    Ok(Ensured {
                        live: renewed.expires > now,
                        calls: 1,
                    })
                }
                // Graph no longer has it: a new one.
                Ok(None) => create(pool, signer, microsoft, token, at, now, 1).await,
                Err(err) => {
                    tracing::warn!(
                        connection = %at.connection, error = %err,
                        "could not renew a Microsoft subscription"
                    );
                    Ok(Ensured {
                        live: expires > now,
                        calls: 1,
                    })
                }
            }
        }
        Some(Row {
            retry_at: Some(retry),
            ..
        }) if retry > now => Ok(idle),
        _ => create(pool, signer, microsoft, token, at, now, 0).await,
    }
}

/// Subscribe `at`'s connection with a fresh `clientState`, and keep the
/// subscription (or the failure, to try again later). `before` is the calls
/// already made.
async fn create(
    pool: &PgPool,
    signer: &Signer,
    microsoft: &Microsoft,
    token: &str,
    at: For,
    now: DateTime<Utc>,
    before: u32,
) -> Result<Ensured, sqlx::Error> {
    let client_state = campbooks_kernel::crypto::random_token();
    let subscribed = microsoft
        .subscribe(token, &client_state, now + LIFETIME)
        .await;
    let (id, tag, key, expires, retry) = match &subscribed {
        Ok(subscription) => (
            Some(subscription.id.clone()),
            Some(signer.sign(CLIENT_STATE_CONTEXT, client_state.as_bytes())),
            Some(key_tag(signer)),
            Some(subscription.expires),
            None,
        ),
        Err(err) => {
            tracing::warn!(
                connection = %at.connection, error = %err,
                "Microsoft refused a subscription; the mailbox is polled"
            );
            (None, None, None, None, Some(now + RETRY_AFTER_FAILURE))
        }
    };
    let mut tx = pool.begin_for(at.workspace).await?;
    sqlx::query(
        "INSERT INTO connectors.microsoft_subscriptions
             (connection_id, workspace_id, stream_id, subscription_id, client_state_tag,
              key_tag, expires_at, retry_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         ON CONFLICT (connection_id) DO UPDATE
            SET stream_id = EXCLUDED.stream_id, subscription_id = EXCLUDED.subscription_id,
                client_state_tag = EXCLUDED.client_state_tag, key_tag = EXCLUDED.key_tag,
                expires_at = EXCLUDED.expires_at, retry_at = EXCLUDED.retry_at,
                updated_at = EXCLUDED.updated_at",
    )
    .bind(at.connection)
    .bind(at.workspace)
    .bind(at.stream)
    .bind(id)
    .bind(tag)
    .bind(key)
    .bind(expires)
    .bind(retry)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Ensured {
        live: subscribed.is_ok_and(|subscription| subscription.expires > now),
        calls: before.saturating_add(1),
    })
}

/// Forget `at`'s connection's subscription, if it has one.
async fn forget(pool: &PgPool, at: For) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin_for(at.workspace).await?;
    sqlx::query("DELETE FROM connectors.microsoft_subscriptions WHERE connection_id = $1")
        .bind(at.connection)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
