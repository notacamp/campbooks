//! Usage metering: whose a model call is, whether it may be made, and what
//! it cost (campbooks-cloud#152).
//!
//! A gateway composed with a [`Meter`] ([`Gateway::with_meter`]) asks it
//! before every model call ([`Gateway::chat`], [`Gateway::embed`],
//! [`Gateway::decide`]) and tells it afterwards how the call went. The meter
//! keeps the per-workspace ledger and the daily cap; the server installs the
//! `system` module's.
//!
//! A metered call must say whose it is: the caller runs it inside a
//! [`Scope`] naming the workspace and the feature,
//!
//! ```no_run
//! # async fn demo(
//! #     gateway: campbooks_ai::Gateway,
//! #     db: sqlx::PgPool,
//! #     workspace: campbooks_kernel::tenancy::WorkspaceId,
//! # ) {
//! use campbooks_ai::usage::Scope;
//!
//! let vectors = Scope::workspace(db, workspace, "search.index")
//!     .run(gateway.embed(&["a text".to_owned()]))
//!     .await;
//! # }
//! ```
//!
//! and a call outside any scope is refused ([`Error::Unattributed`]), so a
//! new call path cannot skip the meter. The scope is a task-local value: it
//! covers the future it runs, not tasks that future spawns.
//!
//! Over the cap the call is refused ([`Error::OverDailyCap`]) and nothing is
//! sent. When the meter cannot count a call it is not made either
//! ([`Error::Metering`]): a broken ledger never means no cap.
//!
//! The gateway's own answers check ([`crate::decide::check`]) runs under the
//! instance scope: fictional mail, bounded by its schedule, counted against
//! no workspace.

use std::{fmt, future::Future, sync::Arc};

use campbooks_kernel::tenancy::WorkspaceId;
use chrono::NaiveDate;
use serde::{Deserialize, Deserializer};
use sqlx::PgPool;

use crate::{AiConfig, Error, Gateway, Role, Usage, decide::strategy::BoxFuture};

/// Model calls a workspace may make per UTC day when
/// `CAMPBOOKS_AI__DAILY_CALLS_PER_WORKSPACE` is unset. About five times a
/// busy small business's bad day of mail at one call per message, and a
/// first sync of two busy mailboxes (see the `ai-usage-caps` design).
pub const DEFAULT_DAILY_CALLS_PER_WORKSPACE: u32 = 5_000;

tokio::task_local! {
    static SCOPE: Scope;
}

/// Whose the model calls made inside [`Scope::run`] are, and for which
/// feature.
#[derive(Clone)]
pub struct Scope {
    whose: Whose,
    feature: &'static str,
    interactive: bool,
}

#[derive(Clone)]
enum Whose {
    Workspace { db: PgPool, workspace: WorkspaceId },
    Instance,
}

impl fmt::Debug for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = f.debug_struct("Scope");
        match &self.whose {
            Whose::Workspace { workspace, .. } => s.field("workspace", workspace),
            Whose::Instance => s.field("workspace", &"instance"),
        };
        s.field("feature", &self.feature)
            .field("interactive", &self.interactive)
            .finish()
    }
}

impl Scope {
    /// Calls made for `workspace` by `feature` (`<module>.<purpose>`, such as
    /// `conversations.triage`), counted in the ledger reached through `db`.
    pub fn workspace(db: PgPool, workspace: WorkspaceId, feature: &'static str) -> Self {
        Self {
            whose: Whose::Workspace { db, workspace },
            feature,
            interactive: false,
        }
    }

    /// The same scope for calls a person is waiting on (the assistant
    /// answering them): they may use the share of the cap that background
    /// work leaves for people, so a mail flood cannot silence the
    /// assistant.
    #[must_use]
    pub fn interactive(mut self) -> Self {
        self.interactive = true;
        self
    }

    /// The instance's own calls (the answers check): no workspace pays for
    /// them and no cap applies. Only the gateway itself makes them.
    pub(crate) fn instance(feature: &'static str) -> Self {
        Self {
            whose: Whose::Instance,
            feature,
            interactive: false,
        }
    }

    /// Run `future` with its model calls attributed to this scope.
    pub async fn run<F: Future>(self, future: F) -> F::Output {
        SCOPE.scope(self, future).await
    }

    /// The feature the calls are for.
    pub fn feature(&self) -> &'static str {
        self.feature
    }

    /// The scope the current task runs in, if any.
    fn current() -> Option<Self> {
        SCOPE.try_with(Clone::clone).ok()
    }
}

/// One metered call: whose, which role, which feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeteredCall<'a> {
    /// The workspace it is for.
    pub workspace: WorkspaceId,
    /// The role it calls.
    pub role: Role,
    /// The feature that asked (`conversations.triage`).
    pub feature: &'a str,
    /// A person is waiting on it (`Scope::interactive`): it may use the
    /// reserve background work leaves.
    pub interactive: bool,
}

/// Whether a call may be made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    /// Counted against `day` (UTC): make it.
    Admitted {
        /// The day it counts against; its outcome is settled on that day.
        day: NaiveDate,
    },
    /// The workspace reached its cap for today: don't.
    Refused {
        /// The cap.
        cap: u32,
    },
}

/// How an admitted call went.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Outcome {
    /// The provider did not answer, or answered something unusable.
    pub failed: bool,
    /// Prompt (input) tokens, when the provider reported them.
    pub prompt_tokens: u64,
    /// Completion (output) tokens, when the provider reported them.
    pub completion_tokens: u64,
}

impl Outcome {
    /// Nothing to add to the ledger: it succeeded and no tokens were
    /// reported (Laya reports none).
    pub fn is_empty(&self) -> bool {
        !self.failed && self.prompt_tokens == 0 && self.completion_tokens == 0
    }
}

/// Why a meter could not count a call (a database error). Never carries
/// content.
pub type MeterError = Box<dyn std::error::Error + Send + Sync>;

/// The ledger and the cap, as the gateway sees them. The server installs
/// the `system` module's implementation.
pub trait Meter: fmt::Debug + Send + Sync + 'static {
    /// Count `call` before it is made, through `db`, or refuse it over the
    /// cap. Admission is atomic: concurrent callers never go past the cap.
    fn admit<'a>(
        &'a self,
        db: &'a PgPool,
        call: MeteredCall<'a>,
    ) -> BoxFuture<'a, Result<Admission, MeterError>>;

    /// Add how an admitted `call` went (failure, tokens) to `day`, the day
    /// it was admitted on. Called only when the outcome is not empty.
    fn settle<'a>(
        &'a self,
        db: &'a PgPool,
        call: MeteredCall<'a>,
        day: NaiveDate,
        outcome: Outcome,
    ) -> BoxFuture<'a, Result<(), MeterError>>;
}

/// An admitted call, to settle once it is done.
pub(crate) struct Ticket {
    meter: Arc<dyn Meter>,
    db: PgPool,
    workspace: WorkspaceId,
    role: Role,
    feature: &'static str,
    interactive: bool,
    day: NaiveDate,
}

impl Gateway {
    /// Count a call of `role` before it is made: nothing to do without a
    /// meter or in the instance scope; refused outside any scope, over the
    /// cap, or when the meter cannot count it.
    pub(crate) async fn admit(&self, role: Role) -> Result<Option<Ticket>, Error> {
        let Some(meter) = self.meter() else {
            return Ok(None);
        };
        let scope = Scope::current().ok_or(Error::Unattributed)?;
        let Whose::Workspace { db, workspace } = scope.whose else {
            return Ok(None);
        };
        let call = MeteredCall {
            workspace,
            role,
            feature: scope.feature,
            interactive: scope.interactive,
        };
        match meter.admit(&db, call).await {
            Ok(Admission::Admitted { day }) => Ok(Some(Ticket {
                meter: meter.clone(),
                db,
                workspace,
                role,
                feature: scope.feature,
                interactive: scope.interactive,
                day,
            })),
            Ok(Admission::Refused { cap }) => Err(Error::OverDailyCap { cap }),
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    role = %role,
                    feature = scope.feature,
                    "an AI call was not made: its usage could not be counted"
                );
                Err(Error::Metering)
            }
        }
    }
}

impl Ticket {
    /// Record how the call went. A failure here loses only the tokens: the
    /// call was counted when it was admitted.
    pub(crate) async fn settle<T>(
        self,
        result: &Result<T, Error>,
        usage: impl FnOnce(&T) -> Option<Usage>,
    ) {
        match result {
            Ok(value) => self.settle_as(false, usage(value)).await,
            Err(_) => self.settle_as(true, None).await,
        }
    }

    /// Record how the call went: `failed`, or the tokens of `usage`.
    pub(crate) async fn settle_as(self, failed: bool, usage: Option<Usage>) {
        let outcome = match (failed, usage) {
            (true, _) => Outcome {
                failed: true,
                ..Outcome::default()
            },
            (false, usage) => usage.map_or_else(Outcome::default, |u| Outcome {
                failed: false,
                prompt_tokens: u64::from(u.prompt_tokens),
                completion_tokens: u64::from(u.completion_tokens),
            }),
        };
        if outcome.is_empty() {
            return;
        }
        let call = MeteredCall {
            workspace: self.workspace,
            role: self.role,
            feature: self.feature,
            interactive: self.interactive,
        };
        if let Err(error) = self.meter.settle(&self.db, call, self.day, outcome).await {
            tracing::warn!(
                error = %error,
                role = %self.role,
                feature = self.feature,
                "an AI call's tokens could not be recorded"
            );
        }
    }
}

/// Settle `ticket`, when there is one.
pub(crate) async fn settle<T>(
    ticket: Option<Ticket>,
    result: &Result<T, Error>,
    usage: impl FnOnce(&T) -> Option<Usage>,
) {
    if let Some(ticket) = ticket {
        ticket.settle(result, usage).await;
    }
}

impl AiConfig {
    /// Model calls a workspace may make per UTC day
    /// (`CAMPBOOKS_AI__DAILY_CALLS_PER_WORKSPACE`): unset is
    /// [`DEFAULT_DAILY_CALLS_PER_WORKSPACE`], `0` is no cap (`None`).
    pub fn daily_call_cap(&self) -> Option<u32> {
        match self.daily_calls_per_workspace {
            None => Some(DEFAULT_DAILY_CALLS_PER_WORKSPACE),
            Some(0) => None,
            Some(cap) => Some(cap),
        }
    }
}

/// `CAMPBOOKS_AI__DAILY_CALLS_PER_WORKSPACE`: a whole number; blank (left
/// empty in `.env`) is unset.
pub(crate) fn daily_calls<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u32>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw {
        Number(u32),
        Text(String),
    }
    match Option::<Raw>::deserialize(d)? {
        None => Ok(None),
        Some(Raw::Number(n)) => Ok(Some(n)),
        Some(Raw::Text(text)) if text.trim().is_empty() => Ok(None),
        Some(Raw::Text(text)) => text.trim().parse().map(Some).map_err(|_| {
            serde::de::Error::custom(format!(
                "DAILY_CALLS_PER_WORKSPACE must be a whole number (0 for no cap), not `{text}`"
            ))
        }),
    }
}

#[cfg(test)]
mod tests {
    use figment::{Figment, providers::Serialized};

    use super::*;

    fn cap(value: &serde_json::Value) -> Result<Option<u32>, Error> {
        let figment = Figment::from(Serialized::defaults(serde_json::json!({
            "ai": {"daily_calls_per_workspace": value}
        })));
        AiConfig::from_figment(&figment).map(|c| c.daily_call_cap())
    }

    #[test]
    fn the_daily_cap_defaults_and_zero_means_none() {
        assert_eq!(
            AiConfig::default().daily_call_cap(),
            Some(DEFAULT_DAILY_CALLS_PER_WORKSPACE)
        );
        assert_eq!(cap(&serde_json::json!(25)).unwrap(), Some(25));
        assert_eq!(cap(&serde_json::json!("40")).unwrap(), Some(40));
        assert_eq!(cap(&serde_json::json!(0)).unwrap(), None);
        assert_eq!(
            cap(&serde_json::json!("")).unwrap(),
            Some(DEFAULT_DAILY_CALLS_PER_WORKSPACE),
            "left empty in .env"
        );
        let err = cap(&serde_json::json!("lots")).unwrap_err().to_string();
        assert!(err.contains("DAILY_CALLS_PER_WORKSPACE"), "{err}");
        assert!(cap(&serde_json::json!(-1)).is_err());
    }

    #[test]
    fn a_scope_shows_whose_it_is_and_no_pool() {
        let scope = Scope::instance("ai.answers_check");
        assert_eq!(
            format!("{scope:?}"),
            r#"Scope { workspace: "instance", feature: "ai.answers_check", interactive: false }"#
        );
    }

    #[test]
    fn an_outcome_with_nothing_to_add_is_empty() {
        assert!(Outcome::default().is_empty());
        assert!(
            !Outcome {
                failed: true,
                ..Outcome::default()
            }
            .is_empty()
        );
        assert!(
            !Outcome {
                prompt_tokens: 3,
                ..Outcome::default()
            }
            .is_empty()
        );
    }
}
