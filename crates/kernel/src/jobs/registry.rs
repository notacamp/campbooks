//! What runs in the background, declared per module and collected at startup.

use std::{
    collections::{BTreeMap, BTreeSet},
    pin::Pin,
    sync::Arc,
    time::Duration,
};

use serde_json::Value;

use super::{Job, JobContext, JobError, JobHandler, PruneJobs};
use crate::{
    Module,
    blobs::{SWEEP_EVERY, SweepBlobs},
    changes::{PruneChanges, SequenceChanges},
    events::{DomainEvent, Event, EventConsumer, EventHandler, EventId, PublishedEvent},
    idempotency::PruneIdempotency,
    rate_limit::PruneRateLimits,
};

pub(crate) type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Runs one job of a registered kind: decodes the payload and calls the
/// handler. Shared, because a [`JobHandler`] or [`EventConsumer`] owns the
/// dependencies its module was composed with.
pub(crate) type RunFn = Arc<
    dyn for<'a> Fn(&'a mut JobContext, Value) -> BoxFuture<'a, Result<(), JobError>> + Send + Sync,
>;

/// Fixes a closure's signature to the higher-ranked one [`RunFn`] needs.
fn run_fn<F>(f: F) -> RunFn
where
    F: for<'a> Fn(&'a mut JobContext, Value) -> BoxFuture<'a, Result<(), JobError>>
        + Send
        + Sync
        + 'static,
{
    Arc::new(f)
}

/// A module's background work: the events it publishes, the jobs it runs,
/// the events it consumes and its recurring schedules. Part of its
/// [`Module`] descriptor.
///
/// ```ignore
/// Background::new()
///     .publishes::<MessageReceived>()
///     .job::<SyncMailbox>()
///     .on_event::<TrackSender>()
///     .schedule::<RefreshTokens>(Duration::from_secs(15 * 60))
///     // Handlers that own dependencies the module was composed with:
///     .handler(ApplyChanges { rules })
///     .consumer(DecideMessage { ai })
/// ```
#[derive(Default)]
pub struct Background {
    kinds: Vec<KindSpec>,
    schedules: Vec<ScheduleSpec>,
    publishes: Vec<EventSpec>,
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
            .field(
                "publishes",
                &self.publishes.iter().map(|e| e.name).collect::<Vec<_>>(),
            )
            .finish()
    }
}

#[derive(Clone, Copy)]
struct EventSpec {
    name: &'static str,
    version: u16,
    workspace_scoped: bool,
    schema: fn() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema>,
    schemas: fn(
        &mut Vec<(
            String,
            utoipa::openapi::RefOr<utoipa::openapi::schema::Schema>,
        )>,
    ),
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

    /// Declare that the module publishes events of type `E`: they are listed,
    /// with their payload schema, in the events catalogue
    /// (`openapi/events.json`), and other modules may consume them. `E`'s
    /// name must be `<module>.<event>` of this module.
    #[must_use]
    pub fn publishes<E: DomainEvent + utoipa::ToSchema>(mut self) -> Self {
        self.publishes.push(EventSpec {
            name: E::NAME,
            version: E::VERSION,
            workspace_scoped: E::WORKSPACE_SCOPED,
            schema: <E as utoipa::PartialSchema>::schema,
            schemas: <E as utoipa::ToSchema>::schemas,
        });
        self
    }

    /// Run jobs of type `J` (enqueued with [`enqueue`](super::enqueue)).
    #[must_use]
    pub fn job<J: Job>(mut self) -> Self {
        self.kinds.push(KindSpec {
            name: J::NAME,
            queue: J::QUEUE,
            max_attempts: J::MAX_ATTEMPTS,
            run: run_fn(run_job::<J>),
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
            run: run_fn(run_handler::<H>),
            consumes: Some((H::Event::NAME, H::Event::VERSION)),
        });
        self
    }

    /// Run jobs of kind `H::NAME` with `handler`, a value that owns the
    /// dependencies the module was composed with (constructor injection).
    /// Enqueue them with [`enqueue_to::<H>`](super::enqueue_to).
    #[must_use]
    pub fn handler<H: JobHandler>(mut self, handler: H) -> Self {
        let handler = Arc::new(handler);
        self.kinds.push(KindSpec {
            name: H::NAME,
            queue: H::QUEUE,
            max_attempts: H::MAX_ATTEMPTS,
            run: run_fn(move |ctx, payload| {
                let handler = Arc::clone(&handler);
                Box::pin(async move {
                    let payload: H::Payload = serde_json::from_value(payload).map_err(|e| {
                        JobError::fatal(format!("payload does not decode as `{}`: {e}", H::NAME))
                    })?;
                    handler.run(ctx, payload).await
                })
            }),
            consumes: None,
        });
        self
    }

    /// Deliver every published `C::Event` to `consumer`, a value that owns
    /// the dependencies the module was composed with (constructor injection).
    #[must_use]
    pub fn consumer<C: EventConsumer>(mut self, consumer: C) -> Self {
        let consumer = Arc::new(consumer);
        self.kinds.push(KindSpec {
            name: C::NAME,
            queue: C::QUEUE,
            max_attempts: C::MAX_ATTEMPTS,
            run: run_fn(move |ctx, _payload| {
                let consumer = Arc::clone(&consumer);
                Box::pin(async move {
                    let event = load_event::<C::Event>(ctx).await?;
                    consumer.handle(ctx, event).await
                })
            }),
            consumes: Some((C::Event::NAME, C::Event::VERSION)),
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
#[derive(Clone)]
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
    /// The payload version it reads (`DomainEvent::VERSION`).
    pub event_version: u16,
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
    published: Vec<PublishedEvent>,
    /// Every declared resource type and its facts (ADR 0017), installed in
    /// the state with the registry.
    resources: crate::authz::Catalogue,
    /// The declarations behind [`resources`](Self::resources), by owner.
    declared: Vec<(&'static str, crate::authz::Resources)>,
}

impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Registry")
            .field("kinds", &self.kinds.keys().collect::<Vec<_>>())
            .field("consumers", &self.consumers)
            .field("schedules", &self.schedules)
            .field(
                "published",
                &self.published.iter().map(|e| e.name).collect::<Vec<_>>(),
            )
            .finish()
    }
}

/// The registrations break a rule; each message names the offender.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid background registrations:\n  - {}", .0.join("\n  - "))]
pub struct RegistryError(pub Vec<String>);

/// Owner name of the kernel's own background work.
pub const KERNEL_OWNER: &str = "kernel";

/// How often the kernel prunes succeeded jobs and expired idempotency keys.
pub const PRUNE_EVERY: Duration = Duration::from_hours(1);

impl Registry {
    /// Collect `modules`' registrations and the kernel's maintenance. Names
    /// (of jobs, handlers and published events) must be `<owner>.<name>`
    /// (lower-case snake case) of the declaring module and unique; queues
    /// lower-case snake case; attempts at least one; event versions at least
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
        let kernel = Background::new()
            .schedule::<PruneJobs>(PRUNE_EVERY)
            .schedule::<SweepBlobs>(SWEEP_EVERY)
            .schedule::<PruneIdempotency>(PRUNE_EVERY)
            .schedule::<PruneRateLimits>(PRUNE_EVERY)
            .job::<SequenceChanges>()
            .schedule::<PruneChanges>(PRUNE_EVERY);
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
            for event in &background.publishes {
                errors.extend(registry.add_published(owner, event));
            }
        }
        for module in modules {
            errors.extend(registry.add_resources(module.name, &module.resources));
        }
        match crate::authz::Catalogue::new(registry.declared.iter().map(|(_, r)| r)) {
            Ok(resources) => registry.resources = resources,
            Err(e) => errors.push(e),
        }
        if errors.is_empty() {
            Ok(registry)
        } else {
            Err(RegistryError(errors))
        }
    }

    /// The same registry, with the resource types of parts of the server
    /// that are not modules, each `(owner, resources)` (the change feed,
    /// the web BFF).
    ///
    /// # Errors
    /// A misnamed or duplicated resource type.
    pub fn with_resources(
        mut self,
        others: &[(&'static str, crate::authz::Resources)],
    ) -> Result<Self, RegistryError> {
        let mut errors = Vec::new();
        for (owner, resources) in others {
            errors.extend(self.add_resources(owner, resources));
        }
        match crate::authz::Catalogue::new(self.declared.iter().map(|(_, r)| r)) {
            Ok(resources) => self.resources = resources,
            Err(e) => errors.push(e),
        }
        if errors.is_empty() {
            Ok(self)
        } else {
            Err(RegistryError(errors))
        }
    }

    fn add_resources(
        &mut self,
        owner: &'static str,
        resources: &crate::authz::Resources,
    ) -> Vec<String> {
        let errors = resources
            .types
            .iter()
            .filter_map(|declared| {
                check_owned_name(owner, declared.name)
                    .err()
                    .map(|e| format!("resource type `{}`: {e}", declared.name))
            })
            .collect();
        self.declared.push((owner, resources.clone()));
        errors
    }

    /// The access catalogue: every declared resource type (ADR 0017).
    pub fn resources(&self) -> &crate::authz::Catalogue {
        &self.resources
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
                    run: Arc::clone(&kind.run),
                },
            );
            if let Some((event, _)) = kind.consumes {
                self.consumers.push(ConsumerInfo {
                    name: kind.name,
                    event,
                    event_version: kind.consumes.map_or(0, |(_, version)| version),
                    queue: kind.queue,
                    max_attempts: kind.max_attempts,
                });
            }
        }
        errors
    }

    fn add_published(&mut self, owner: &'static str, event: &EventSpec) -> Vec<String> {
        let mut errors = Vec::new();
        if let Err(e) = check_owned_name(owner, event.name) {
            errors.push(format!("published event `{}`: {e}", event.name));
        }
        if event.version == 0 {
            errors.push(format!(
                "published event `{}`: VERSION must be at least 1",
                event.name
            ));
        }
        if self.published.iter().any(|e| e.name == event.name) {
            errors.push(format!(
                "published event `{}` is declared twice ({owner})",
                event.name
            ));
        }
        if errors.is_empty() {
            self.published.push(PublishedEvent {
                module: owner,
                name: event.name,
                version: event.version,
                workspace_scoped: event.workspace_scoped,
                schema: event.schema,
                schemas: event.schemas,
            });
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

    /// Every event type a module declared it publishes.
    pub fn published(&self) -> &[PublishedEvent] {
        &self.published
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
        self.kinds.get(name).cloned()
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

/// `<snake>.<snake>`: a module-qualified name.
pub(crate) fn is_dotted(name: &str) -> bool {
    name.split_once('.')
        .is_some_and(|(a, b)| is_snake(a) && is_snake(b))
}

fn is_snake(s: &str) -> bool {
    s.starts_with(|c: char| c.is_ascii_lowercase())
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}
