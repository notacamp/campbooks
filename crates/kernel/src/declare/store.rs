//! The action registry's records in schema `kernel`: idempotency keys and
//! undo tokens, and the job that forgets expired ones.

use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgConnection;

use crate::{
    auth::{Principal, PrincipalKind},
    jobs::{Job, JobContext, JobError},
};

/// Rows deleted per statement, so one prune never holds a long lock.
const PRUNE_BATCH: i64 = 5_000;

/// Who made a call, as stored: `user:<sub>` or `client:<client id>`.
pub(crate) fn principal_key(principal: &Principal) -> String {
    let kind = match principal.kind {
        PrincipalKind::User => "user",
        PrincipalKind::Client => "client",
    };
    format!("{kind}:{}", principal.subject)
}

/// What [`claim_key`] found.
#[derive(Debug, PartialEq)]
pub(crate) enum Claim {
    /// The key is new: this request runs, and must [`complete_key`].
    Owned,
    /// The key answered this very request before: send this body again.
    Replay(Value),
    /// The key was used with another action, input or workspace.
    Mismatch,
}

/// The caller's identity in the key tables.
pub(crate) struct Caller<'a> {
    pub principal: &'a str,
    pub workspace: Option<&'a str>,
}

/// Claim idempotency `key` for `action` with `input` (already normalized),
/// inside the action's transaction. A concurrent request holding the same
/// key makes this wait until that request commits (then it replays) or rolls
/// back (then this one owns the key).
pub(crate) async fn claim_key(
    conn: &mut PgConnection,
    caller: &Caller<'_>,
    key: &str,
    action: &str,
    input: &Value,
    ttl: Duration,
) -> Result<Claim, sqlx::Error> {
    let input = input.to_string();
    sqlx::query!(
        "DELETE FROM kernel.idempotency_keys
          WHERE principal = $1 AND key = $2 AND expires_at <= now()",
        caller.principal,
        key,
    )
    .execute(&mut *conn)
    .await?;
    let claimed = sqlx::query_scalar!(
        r#"
        INSERT INTO kernel.idempotency_keys
            (principal, key, action, workspace_id, request_hash, expires_at)
        VALUES ($1, $2, $3, $4, sha256(convert_to($5, 'UTF8')),
                now() + make_interval(secs => $6))
        ON CONFLICT (principal, key) DO NOTHING
        RETURNING true AS "claimed!"
        "#,
        caller.principal,
        key,
        action,
        caller.workspace,
        input,
        ttl.as_secs_f64(),
    )
    .fetch_optional(&mut *conn)
    .await?;
    if claimed.is_some() {
        return Ok(Claim::Owned);
    }
    let previous = sqlx::query!(
        r#"
        SELECT action = $3
                   AND workspace_id IS NOT DISTINCT FROM $4
                   AND request_hash = sha256(convert_to($5, 'UTF8')) AS "same!",
               response
          FROM kernel.idempotency_keys
         WHERE principal = $1 AND key = $2
        "#,
        caller.principal,
        key,
        action,
        caller.workspace,
        input,
    )
    .fetch_one(&mut *conn)
    .await?;
    Ok(match (previous.same, previous.response) {
        (true, Some(body)) => Claim::Replay(body),
        _ => Claim::Mismatch,
    })
}

/// Store the response of the request that owns `key`.
pub(crate) async fn complete_key(
    conn: &mut PgConnection,
    caller: &Caller<'_>,
    key: &str,
    response: &Value,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE kernel.idempotency_keys SET response = $3 WHERE principal = $1 AND key = $2",
        caller.principal,
        key,
        response,
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Hand out a new undo token for `action`, valid for `window`. Only its hash
/// is stored.
pub(crate) async fn issue_undo(
    conn: &mut PgConnection,
    caller: &Caller<'_>,
    action: &str,
    payload: &Value,
    window: Duration,
) -> Result<(String, DateTime<Utc>), sqlx::Error> {
    let row = sqlx::query!(
        r#"
        WITH token AS (SELECT gen_random_uuid()::text AS value),
        issued AS (
            INSERT INTO kernel.undo_tokens
                (token_hash, action, principal, workspace_id, payload, expires_at)
            SELECT sha256(convert_to(token.value, 'UTF8')), $1, $2, $3, $4,
                   now() + make_interval(secs => $5)
              FROM token
            RETURNING expires_at
        )
        SELECT token.value AS "token!", issued.expires_at AS "expires_at!"
          FROM token, issued
        "#,
        action,
        caller.principal,
        caller.workspace,
        payload,
        window.as_secs_f64(),
    )
    .fetch_one(conn)
    .await?;
    Ok((row.token, row.expires_at))
}

/// An undo token as stored.
#[derive(Debug)]
pub(crate) struct UndoRecord {
    pub action: String,
    pub principal: String,
    pub workspace: Option<String>,
    pub payload: Value,
    pub expired: bool,
    pub redeemed: bool,
}

/// The undo token `token`, locked until the transaction ends.
pub(crate) async fn find_undo(
    conn: &mut PgConnection,
    token: &str,
) -> Result<Option<UndoRecord>, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT action, principal, workspace_id, payload,
               expires_at <= now() AS "expired!",
               redeemed_at IS NOT NULL AS "redeemed!"
          FROM kernel.undo_tokens
         WHERE token_hash = sha256(convert_to($1, 'UTF8'))
           FOR UPDATE
        "#,
        token,
    )
    .fetch_optional(conn)
    .await?;
    Ok(row.map(|row| UndoRecord {
        action: row.action,
        principal: row.principal,
        workspace: row.workspace_id,
        payload: row.payload,
        expired: row.expired,
        redeemed: row.redeemed,
    }))
}

/// Mark `token` used.
pub(crate) async fn redeem_undo(conn: &mut PgConnection, token: &str) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE kernel.undo_tokens SET redeemed_at = now()
          WHERE token_hash = sha256(convert_to($1, 'UTF8'))",
        token,
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Forget expired idempotency keys and undo tokens. Scheduled hourly by the
/// kernel.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct PruneActions;

impl Job for PruneActions {
    const NAME: &'static str = "kernel.prune_actions";
    const QUEUE: &'static str = "maintenance";
    const MAX_ATTEMPTS: u32 = 3;

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        let mut keys = 0_u64;
        loop {
            let deleted = sqlx::query!(
                "DELETE FROM kernel.idempotency_keys
                  WHERE (principal, key) IN (
                      SELECT principal, key FROM kernel.idempotency_keys
                       WHERE expires_at <= now() LIMIT $1)",
                PRUNE_BATCH,
            )
            .execute(ctx.pool())
            .await?
            .rows_affected();
            keys += deleted;
            if deleted < PRUNE_BATCH.unsigned_abs() {
                break;
            }
        }
        let mut tokens = 0_u64;
        loop {
            let deleted = sqlx::query!(
                "DELETE FROM kernel.undo_tokens
                  WHERE token_hash IN (
                      SELECT token_hash FROM kernel.undo_tokens
                       WHERE expires_at <= now() LIMIT $1)",
                PRUNE_BATCH,
            )
            .execute(ctx.pool())
            .await?
            .rows_affected();
            tokens += deleted;
            if deleted < PRUNE_BATCH.unsigned_abs() {
                break;
            }
        }
        tracing::info!(
            idempotency_keys = keys,
            undo_tokens = tokens,
            "pruned expired action records"
        );
        Ok(())
    }
}
