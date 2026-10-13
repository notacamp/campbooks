//! What runs in the background, declared per module and collected at startup.

use std::{
    collections::{BTreeMap, BTreeSet},
    pin::Pin,
    time::Duration,
};

use serde_json::Value;

use super::{Job, JobContext, JobError, PruneJobs};
use crate::{
    Module,
    events::{DomainEvent, Event, EventHandler, EventId},
};

pub(crate) type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Runs one job of a registered kind: decodes the payload and calls the
/// handler.
pub(crate) type RunFn =
    for<'a> fn(&'a mut JobContext, Value) -> BoxFuture<'a, Result<(), JobError>>;

/// A module's background work: the jobs it runs, the events it consumes and
/// its recurring schedules. Part of its [`Module`] descriptor.
///
/// ```ignore
/// Background::new()
///     .job::<SyncMailbox>()
///     .on_event::<TrackSender>()
///     .schedule::<RefreshTokens>(Duration::from_secs(15 * 60))
/// ```
#[derive(Default)]
pub struct Background {
    kinds: Vec<KindSpec>,
    schedules: Vec<ScheduleSpec>,
}

impl std::fmt::Debug for Background {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Background")
            .field(
                "kinds",
                &self.kinds.iter().map(|k| k.name).collect::<Vec<_>>(),
            )
            .field(
                "schedules",
                &self.schedules.iter().map(|s| s.name).collect::<Vec<_>>(),
            )
            .finish()
    }
}

#[derive(Clone)]
struct KindSpec {
    name: &'static str,
    queue: &'static str,
    max_attempts: u32,
    run: RunFn,
    /// For event handlers: the event name and version they consume.
    consumes: Option<(&'static str, u16)>,
}

struct ScheduleSpec {
    name: &'static str,
    every: Duration,
    payload: Result<Value, String>,
}

impl Background {
    /// Nothing in the background.
    pub fn new() -> Self {
        Self::default()
    }

    /// Run jobs of type `J` (enqueued with [`enqueue`](super::enqueue)).
    #[must_use]
    pub fn job<J: Job>(mut self) -> Self {
        self.kinds.push(KindSpec {
            name: J::NAME,
            queue: J::QUEUE,
            max_attempts: J::MAX_ATTEMPTS,
            run: run_job::<J>,
            consumes: None,
        });
        self
    }

    /// Deliver every published `H::Event` to `H`.
    #[must_use]
    pub fn on_event<H: EventHandler>(mut self) -> Self {
        self.kinds.push(KindSpec {
            name: H::NAME,
            queue: H::QUEUE,
            max_attempts: H::MAX_ATTEMPTS,
            run: run_handler::<H>,
            consumes: Some((H::Event::NAME, H::Event::VERSION)),
        });
        self
    }

    /// Run `J::default()` every `every`, on ticks aligned to the Unix epoch
    /// (every hour runs on the hour). Registers `J` too. However many
    /// workers run, each tick is enqueued once, and a tick is skipped while
    /// the previous run is still pending or running.
    #[must_use]
    pub fn schedule<J: Job + Default>(self, every: Duration) -> Self {
        let mut this = self.job::<J>();
        this.schedules.push(ScheduleSpec {
            name: J::NAME,
            every,
            payload: serde_json::to_value(J::default()).map_err(|e| e.to_string()),
        });
        this
    }
}

fn run_job<J: Job>(ctx: &mut JobContext, payload: Value) -> BoxFuture<'_, Result<(), JobError>> {
    Box::pin(async move {
        let job: J = serde_json::from_value(payload).map_err(|e| {
            JobError::fatal(format!("payload does not decode as `{}`: {e}", J::NAME))
        })?;
        job.run(ctx).await
    })
}

fn run_handler<H: EventHandler>(
    ctx: &mut JobContext,
    _payload: Value,
) -> BoxFuture<'_, Result<(), JobError>> {
    Box::pin(async move {
        let event = load_event::<H::Event>(ctx).await?;
        H::handle(ctx, event).await
    })
}

async fn load_event<E: DomainEvent>(ctx: &JobContext) -> Result<Event<E>, JobError> {
    let id = ctx
        .info()
        .event_id
        .ok_or_else(|| JobError::fatal("event delivery without an event id"))?;
    let row = sqlx::query!(
        r#"
        SELECT name, version, workspace_id, payload, occurred_at
          FROM kernel.events WHERE id = $1
        "#,
        id as EventId
    )
    .fetch_optional(ctx.pool())
    .await?
    .ok_or_else(|| JobError::fatal(format!("event {id} is not in the log")))?;
    if row.name != E::NAME {
        return Err(JobError::fatal(format!(
            "event {id} is a `{}`, not a `{}`",
            row.name,
            E::NAME
        )));
    }
    if row.version != i32::from(E::VERSION) {
        return Err(JobError::fatal(format!(
            "event {id} has version {}, the handler reads version {}",
            row.version,
            E::VERSION
        )));
    }
    let payload = serde_json::from_value(row.payload).map_err(|e| {
        JobError::fatal(format!("event {id} does not decode as `{}`: {e}", E::NAME))
    })?;
    Ok(Event {
        id,
        version: E::VERSION,
        workspace: row.workspace_id,
        occurred_at: row.occurred_at,
        payload,
    })
}

/// A registered job kind.
#[derive(Clone, Copy)]
pub(crate) struct KindInfo {
    pub queue: &'static str,
    pub max_attempts: u32,
    pub run: RunFn,
}

/// A registered event consumer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConsumerInfo {
    /// Consumer name (`EventHandler::NAME`), the kind of its deliveries.
    pub name: &'static str,
    /// The event it consumes (`DomainEvent::NAME`).
    pub event: &'static str,
    /// The queue its deliveries run on.
    pub queue: &'static str,
    /// Attempts per delivery.
    pub max_attempts: u32,
}

/// A registered recurring schedule.
#[derive(Debug, Clone)]
pub(crate) struct ScheduleInfo {
    pub name: &'static str,
    pub queue: &'static str,
    pub max_attempts: u32,
    pub every: Duration,
    pub payload: Value,
}

/// Every job kind, event consumer and schedule of the running system, built
/// once at startup from the modules (plus the kernel's own maintenance).
#[derive(Default)]
pub struct Registry {
    kinds: BTreeMap<&'static str, KindInfo>,
    consumers: Vec<ConsumerInfo>,
    schedules: Vec<ScheduleInfo>,
}

impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Registry")
            .field("kinds", &self.kinds.keys().collect::<Vec<_>>())
            .field("consumers", &self.consumers)
            .field("schedules", &self.schedules)
            .finish()
    }
}

/// The registrations break a rule; each message names the offender.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid background registrations:\n  - {}", .0.join("\n  - "))]
pub struct RegistryError(pub Vec<String>);

/// Owner name of the kernel's own background work.
pub const KERNEL_OWNER: &str = "kernel";

/// How often the kernel prunes succeeded jobs.
pub const PRUNE_EVERY: Duration = Duration::from_hours(1);

impl Registry {
    /// Collect `modules`' registrations and the kernel's maintenance. Names
    /// must be `<owner>.<name>` (lower-case snake case) of the declaring
    /// module and unique; queues lower-case snake case; attempts at least
    /// one; schedules at least a second apart.
    pub fn build(modules: &[Module]) -> Result<Self, RegistryError> {
        Self::build_with(modules, &[])
    }

    /// Like [`build`](Self::build), plus the background work of parts of the
    /// server that are not modules, each `(owner, work)` named
    /// `<owner>.<name>` (e.g. the web BFF's `bff.prune_sessions`).
    pub fn build_with(
        modules: &[Module],
        others: &[(&'static str, &Background)],
    ) -> Result<Self, RegistryError> {
        let kernel = Background::new().schedule::<PruneJobs>(PRUNE_EVERY);
        let owners = std::iter::once((KERNEL_OWNER, &kernel))
            .chain(modules.iter().map(|m| (m.name, &m.background)))
            .chain(others.iter().copied());

        let mut registry = Self::default();
        let mut errors = Vec::new();
        for (owner, background) in owners {
            for kind in &background.kinds {
                errors.extend(registry.add_kind(owner, kind));
            }
            for schedule in &background.schedules {
                errors.extend(registry.add_schedule(owner, schedule));
            }
        }
        if errors.is_empty() {
            Ok(registry)
        } else {
            Err(RegistryError(errors))
        }
    }

    fn add_kind(&mut self, owner: &str, kind: &KindSpec) -> Vec<String> {
        let mut errors = Vec::new();
        let what = if kind.consumes.is_some() {
            "event handler"
        } else {
            "job"
        };
        if let Err(e) = check_owned_name(owner, kind.name) {
            errors.push(format!("{what} `{}`: {e}", kind.name));
        }
        if !is_snake(kind.queue) {
            errors.push(format!(
                "{what} `{}`: queue `{}` must be lower-case snake case",
                kind.name, kind.queue
            ));
        }
        if kind.max_attempts == 0 {
            errors.push(format!(
                "{what} `{}`: MAX_ATTEMPTS must be at least 1",
                kind.name
            ));
        }
        if let Some((event, version)) = kind.consumes {
            if !is_dotted(event) {
                errors.push(format!(
                    "{what} `{}` consumes `{event}`, which is not `<module>.<event>`",
                    kind.name
                ));
            }
            if version == 0 {
                errors.push(format!(
                    "event `{event}`: VERSION must be at least 1 (consumed by `{}`)",
                    kind.name
                ));
            }
        }
        if self.kinds.contains_key(kind.name) {
            errors.push(format!(
                "{what} `{}` is registered twice (a scheduled job is registered by \
                 `.schedule` already)",
                kind.name
            ));
        } else {
            self.kinds.insert(
                kind.name,
                KindInfo {
                    queue: kind.queue,
                    max_attempts: kind.max_attempts,
                    run: kind.run,
                },
            );
            if let Some((event, _)) = kind.consumes {
                self.consumers.push(ConsumerInfo {
                    name: kind.name,
                    event,
                    queue: kind.queue,
                    max_attempts: kind.max_attempts,
                });
            }
        }
        errors
    }

    fn add_schedule(&mut self, owner: &str, schedule: &ScheduleSpec) -> Vec<String> {
        let mut errors = Vec::new();
        if schedule.every < Duration::from_secs(1) {
            errors.push(format!(
                "schedule `{}`: must run at most once per second",
                schedule.name
            ));
        }
        if self.schedules.iter().any(|s| s.name == schedule.name) {
            errors.push(format!(
                "schedule `{}` is declared twice ({owner})",
                schedule.name
            ));
        }
        let payload = match &schedule.payload {
            Ok(payload) => payload.clone(),
            Err(e) => {
                errors.push(format!(
                    "schedule `{}`: its job does not serialize: {e}",
                    schedule.name
                ));
                Value::Null
            }
        };
        if errors.is_empty() {
            // `schedule::<J>` always registered `J` first, under the same owner.
            if let Some(kind) = self.kinds.get(schedule.name) {
                self.schedules.push(ScheduleInfo {
                    name: schedule.name,
                    queue: kind.queue,
                    max_attempts: kind.max_attempts,
                    every: schedule.every,
                    payload,
                });
            }
        }
        errors
    }

    /// The consumers of the event named `event`.
    pub fn consumers_of(&self, event: &str) -> Vec<ConsumerInfo> {
        self.consumers
            .iter()
            .filter(|c| c.event == event)
            .copied()
            .collect()
    }

    /// The consumer named `name`.
    pub fn consumer(&self, name: &str) -> Option<ConsumerInfo> {
        self.consumers.iter().find(|c| c.name == name).copied()
    }

    /// Every consumer, by name.
    pub fn consumers(&self) -> &[ConsumerInfo] {
        &self.consumers
    }

    /// Every job kind (jobs and event handlers), sorted.
    pub fn kind_names(&self) -> Vec<&'static str> {
        self.kinds.keys().copied().collect()
    }

    /// Every schedule's name.
    pub fn schedule_names(&self) -> Vec<&'static str> {
        self.schedules.iter().map(|s| s.name).collect()
    }

    /// Every queue some kind runs on, sorted.
    pub(crate) fn queues(&self) -> Vec<&'static str> {
        let queues: BTreeSet<&'static str> = self.kinds.values().map(|k| k.queue).collect();
        queues.into_iter().collect()
    }

    pub(crate) fn kind(&self, name: &str) -> Option<KindInfo> {
        self.kinds.get(name).copied()
    }

    pub(crate) fn schedules(&self) -> &[ScheduleInfo] {
        &self.schedules
    }
}

fn check_owned_name(owner: &str, name: &str) -> Result<(), String> {
    match name.split_once('.') {
        Some((prefix, rest)) if prefix == owner && is_snake(rest) => Ok(()),
        _ => Err(format!(
            "must be `{owner}.<name>` (lower-case snake case), named after its module"
        )),
    }
}

fn is_dotted(name: &str) -> bool {
    name.split_once('.')
        .is_some_and(|(a, b)| is_snake(a) && is_snake(b))
}

fn is_snake(s: &str) -> bool {
    s.starts_with(|c: char| c.is_ascii_lowercase())
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}
