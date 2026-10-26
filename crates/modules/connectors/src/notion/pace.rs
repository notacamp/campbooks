//! A Notion connection's request budget, taken one request at a time.
//!
//! Notion allows an integration about three requests a second, and answers
//! more with 429. Every call goes through [`Pace::take`], which takes a unit
//! of the connection's per-second and per-minute budgets (token buckets
//! shared by every process, `connectors.provider_budgets`) and waits for one
//! when there is none, for at most the run's patience: a run that would wait
//! longer gives the page up for later ([`Stop::Waiting`]).

use std::time::Duration;

use campbooks_kernel::Error;
use sqlx::PgPool;

use crate::{
    config::NotionAdapter,
    domain::{
        model::ConnectionId,
        schedule::{self, Rate},
    },
    provider::ProviderError,
};

/// Why a run with Notion stopped.
#[derive(Debug)]
pub enum Stop {
    /// Notion's trouble.
    Provider(ProviderError),
    /// The database failed.
    Database(Error),
    /// The budget would make the run wait longer than it may: look again
    /// after this.
    Waiting(Duration),
}

impl From<ProviderError> for Stop {
    fn from(err: ProviderError) -> Self {
        Self::Provider(err)
    }
}

impl From<Error> for Stop {
    fn from(err: Error) -> Self {
        Self::Database(err)
    }
}

/// One run's use of its connection's budget.
#[derive(Debug)]
pub struct Pace {
    pool: PgPool,
    connection: ConnectionId,
    per_second: u32,
    per_minute: u32,
    waited: Duration,
    patience: Duration,
}

impl Pace {
    /// The pace of a run on `connection`, under `adapter`'s settings.
    pub fn new(pool: PgPool, connection: ConnectionId, adapter: &NotionAdapter) -> Self {
        Self {
            pool,
            connection,
            per_second: adapter.requests_per_second,
            per_minute: adapter.requests_per_minute,
            waited: Duration::ZERO,
            patience: Duration::from_secs(u64::from(adapter.patience_seconds)),
        }
    }

    /// Take the budget for one request, waiting for it within the run's
    /// patience.
    pub async fn take(&mut self) -> Result<(), Stop> {
        let second = schedule::notion_second_budget(self.connection);
        let minute = schedule::notion_budget(self.connection);
        loop {
            let each_second = Rate::per_second(self.per_second);
            if let Err(wait) = schedule::take(&self.pool, &second, 1, each_second).await? {
                self.wait(wait).await?;
                continue;
            }
            let each_minute = Rate::per_minute(self.per_minute);
            if let Err(wait) = schedule::take(&self.pool, &minute, 1, each_minute).await? {
                self.wait(wait).await?;
                continue;
            }
            return Ok(());
        }
    }

    /// Wait `wait`, unless that is more patience than the run has.
    async fn wait(&mut self, wait: Duration) -> Result<(), Stop> {
        self.waited = self.waited.saturating_add(wait);
        if self.waited > self.patience {
            return Err(Stop::Waiting(wait));
        }
        tokio::time::sleep(wait).await;
        Ok(())
    }

    /// How long the run has waited for its budget so far.
    pub fn waited(&self) -> Duration {
        self.waited
    }
}
