//! The AI usage ledger and the daily cap (campbooks-cloud#152): the
//! `campbooks_ai::usage::Meter` the server composes its gateway with.
//!
//! - **Admission** counts a call against its workspace's UTC day with one
//!   conditional upsert on `system.ai_usage_days` (`calls < cap`), in the
//!   workspace's transaction, together with `+1` on its role and feature in
//!   `system.ai_usage`. Postgres serializes the upsert on the day's row, so
//!   any number of workers admit at most the cap. Over the cap the call is
//!   refused and counted as refused; the first refusal of the day logs one
//!   line.
//! - **Settling** adds a call's failure and tokens to the day it was
//!   admitted on (the gateway skips it when there is nothing to add).
//!
//! The day comes from a [`Clock`], so tests move it without sleeping.

use std::{fmt, sync::Arc};

use campbooks_ai::{
    decide::strategy::BoxFuture,
    usage::{Admission, Meter, MeterError, MeteredCall, Outcome},
};
use campbooks_kernel::{
    clock::{Clock, SystemClock},
    db::BeginFor as _,
};
use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use sqlx::PgPool;

/// Counts AI calls per workspace and UTC day and refuses them over the cap.
/// Cheap to clone; the server gives the same one to the gateway and to the
/// admin read.
#[derive(Clone)]
pub struct AiUsageMeter {
    cap: Option<u32>,
    clock: Arc<dyn Clock>,
}

impl fmt::Debug for AiUsageMeter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AiUsageMeter")
            .field("cap", &self.cap)
            .finish_non_exhaustive()
    }
}

impl Default for AiUsageMeter {
    /// The default cap (`campbooks_ai::usage::DEFAULT_DAILY_CALLS_PER_WORKSPACE`).
    fn default() -> Self {
        Self::new(Some(campbooks_ai::usage::DEFAULT_DAILY_CALLS_PER_WORKSPACE))
    }
}

/// How an admission went, for the log and the tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Admitted {
    /// Counted: the workspace's `calls`-th call of `day`.
    Yes { day: NaiveDate, calls: i32 },
    /// Refused over `cap`; `first` when it is the day's first refusal.
    No { cap: u32, first: bool },
}

impl AiUsageMeter {
    /// A meter admitting `cap` calls per workspace per UTC day; `None` (or
    /// `Some(0)`) counts without a cap.
    pub fn new(cap: Option<u32>) -> Self {
        Self {
            cap: cap.filter(|cap| *cap > 0),
            clock: Arc::new(SystemClock),
        }
    }

    /// The same meter reading the time from `clock`.
    #[must_use]
    pub fn with_clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = clock;
        self
    }

    /// Calls a workspace may make per UTC day; `None` is no cap.
    pub fn cap(&self) -> Option<u32> {
        self.cap
    }

    /// Now, by the meter's clock.
    pub(crate) fn now(&self) -> DateTime<Utc> {
        self.clock.now()
    }

    /// The start of the UTC day after `day`, when its count starts again.
    pub(crate) fn resets_at(day: NaiveDate) -> DateTime<Utc> {
        (day + TimeDelta::days(1))
            .and_hms_opt(0, 0, 0)
            .unwrap_or_default()
            .and_utc()
    }

    pub(crate) async fn admit_call(
        &self,
        db: &PgPool,
        call: MeteredCall<'_>,
    ) -> Result<Admitted, sqlx::Error> {
        let now = self.clock.now();
        let day = now.date_naive();
        // A cap past `i32::MAX` is no cap in practice.
        let cap = self.cap.and_then(|cap| i32::try_from(cap).ok());
        let workspace = *call.workspace.as_uuid();
        let mut tx = db.begin_for(call.workspace).await?;
        let calls = sqlx::query_scalar!(
            r#"INSERT INTO system.ai_usage_days AS d (workspace_id, day, calls)
               VALUES ($1, $2, 1)
               ON CONFLICT (workspace_id, day) DO UPDATE SET calls = d.calls + 1
                WHERE $3::int IS NULL OR d.calls < $3
               RETURNING d.calls"#,
            workspace,
            day,
            cap,
        )
        .fetch_optional(&mut *tx)
        .await?;
        let admitted = if let Some(calls) = calls {
            sqlx::query!(
                r#"INSERT INTO system.ai_usage AS u (workspace_id, day, role, feature, calls)
                   VALUES ($1, $2, $3, $4, 1)
                   ON CONFLICT (workspace_id, day, role, feature)
                   DO UPDATE SET calls = u.calls + 1"#,
                workspace,
                day,
                call.role.as_str(),
                call.feature,
            )
            .execute(&mut *tx)
            .await?;
            Admitted::Yes { day, calls }
        } else {
            let refused = sqlx::query_scalar!(
                r#"UPDATE system.ai_usage_days
                      SET refused = refused + 1, capped_at = coalesce(capped_at, $3)
                    WHERE workspace_id = $1 AND day = $2
                RETURNING refused"#,
                workspace,
                day,
                now,
            )
            .fetch_one(&mut *tx)
            .await?;
            Admitted::No {
                cap: self.cap.unwrap_or(u32::MAX),
                first: refused == 1,
            }
        };
        tx.commit().await?;
        if let Admitted::No { cap, first: true } = admitted {
            tracing::warn!(
                workspace = %call.workspace,
                cap,
                "a workspace reached its daily AI limit; its new mail is sorted by rules only \
                 until 00:00 UTC"
            );
        }
        Ok(admitted)
    }

    pub(crate) async fn settle_call(
        db: &PgPool,
        call: MeteredCall<'_>,
        day: NaiveDate,
        outcome: Outcome,
    ) -> Result<(), sqlx::Error> {
        let tokens = |n: u64| i64::try_from(n).unwrap_or(i64::MAX);
        let mut tx = db.begin_for(call.workspace).await?;
        sqlx::query!(
            r#"UPDATE system.ai_usage
                  SET failed = failed + $5,
                      prompt_tokens = prompt_tokens + $6,
                      completion_tokens = completion_tokens + $7
                WHERE workspace_id = $1 AND day = $2 AND role = $3 AND feature = $4"#,
            *call.workspace.as_uuid(),
            day,
            call.role.as_str(),
            call.feature,
            i32::from(outcome.failed),
            tokens(outcome.prompt_tokens),
            tokens(outcome.completion_tokens),
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await
    }
}

impl Meter for AiUsageMeter {
    fn admit<'a>(
        &'a self,
        db: &'a PgPool,
        call: MeteredCall<'a>,
    ) -> BoxFuture<'a, Result<Admission, MeterError>> {
        Box::pin(async move {
            Ok(match self.admit_call(db, call).await? {
                Admitted::Yes { day, .. } => Admission::Admitted { day },
                Admitted::No { cap, .. } => Admission::Refused { cap },
            })
        })
    }

    fn settle<'a>(
        &'a self,
        db: &'a PgPool,
        call: MeteredCall<'a>,
        day: NaiveDate,
        outcome: Outcome,
    ) -> BoxFuture<'a, Result<(), MeterError>> {
        Box::pin(async move { Ok(Self::settle_call(db, call, day, outcome).await?) })
    }
}

#[cfg(test)]
mod tests;
