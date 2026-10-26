//! The ambient tracking context: who is acting, in which operation, for
//! which request. It is a task-local the kernel sets, never the caller of
//! [`track`](super::Tracker::track):
//!
//! - **HTTP**: the [`http`](super::http) layer opens one per request with the
//!   matched `operationId` and the request's correlation (its trace id), as
//!   `anonymous`; [`Scoped`](crate::auth::Scoped) then names the
//!   authenticated principal (`user` with the Campbooks account id, `client`
//!   with the client id), and [`Admin`](crate::admin::Admin) the
//!   administrator. An operation [`invoke`](crate::operations::invoke)d by
//!   the assistant or an automation is `assistant` / `automation`, on behalf
//!   of the principal it runs for.
//! - **Jobs**: the worker opens one per job from the origin stored on the
//!   job row: `system` (the job's kind), on behalf of whoever caused it, with
//!   that request's correlation, caused by the job (or, for a delivery, the
//!   event).
//! - **Outside any**: `system` with the command's name ([`set_command`]),
//!   and a fresh correlation.

use std::future::Future;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use serde::{Deserialize, Serialize};

use crate::{
    auth::{Principal, PrincipalKind},
    events::EventId,
    jobs::JobId,
};

/// Who acts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActorType {
    /// A person (id: their Campbooks account id).
    User,
    /// An app acting on its own behalf (id: its OAuth client id).
    Client,
    /// The assistant, for a person (`on_behalf_of`).
    Assistant,
    /// A rule or workflow, for a person (`on_behalf_of`).
    Automation,
    /// Background work or a command (id: the job kind or the command).
    System,
    /// An instance administrator (id: their Campbooks account id).
    Admin,
    /// Nobody signed in (no id).
    Anonymous,
}

/// An actor: its type and, where the type has one, its id (an account id, a
/// client id, a job kind; never a name or an address).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Actor {
    /// Its type.
    #[serde(rename = "type")]
    pub kind: ActorType,
    /// Its id, when the type has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

impl Actor {
    fn with_id(kind: ActorType, id: impl Into<String>) -> Self {
        Self {
            kind,
            id: Some(id.into()),
        }
    }

    fn without_id(kind: ActorType) -> Self {
        Self { kind, id: None }
    }

    /// A person, by Campbooks account id.
    pub fn user(account: impl Into<String>) -> Self {
        Self::with_id(ActorType::User, account)
    }

    /// An app on its own behalf, by client id when Campbooks minted it
    /// ([`minted_client_id`]); any other client (one known by its metadata
    /// document, whose id is a URL) is an app with no id.
    pub fn client(client: impl Into<String>) -> Self {
        let client = client.into();
        Self {
            kind: ActorType::Client,
            id: minted_client_id(&client).map(str::to_owned),
        }
    }

    /// The assistant.
    pub fn assistant() -> Self {
        Self::without_id(ActorType::Assistant)
    }

    /// An automation.
    pub fn automation() -> Self {
        Self::without_id(ActorType::Automation)
    }

    /// Background work or a command, by kind or name.
    pub fn system(kind: impl Into<String>) -> Self {
        Self::with_id(ActorType::System, kind)
    }

    /// An instance administrator, by Campbooks account id.
    pub fn admin(account: impl Into<String>) -> Self {
        Self::with_id(ActorType::Admin, account)
    }

    /// Nobody signed in.
    pub fn anonymous() -> Self {
        Self::without_id(ActorType::Anonymous)
    }

    /// The authenticated caller of a request: a person by their account id
    /// (the token's subject, ADR 0014), an app by its client id.
    pub fn of(principal: &Principal) -> Self {
        match principal.kind {
            PrincipalKind::User => Self::user(principal.subject.clone()),
            PrincipalKind::Client => Self::client(
                principal
                    .client_id
                    .clone()
                    .unwrap_or_else(|| principal.subject.clone()),
            ),
        }
    }
}

/// The client ids of Campbooks' own apps: the web app and the desktop and
/// mobile app.
pub const FIRST_PARTY_CLIENT_IDS: [&str; 2] = ["campbooks-web", "campbooks-native"];

/// `id`, when it is a client id Campbooks minted and so may be recorded: one
/// of its own apps ([`FIRST_PARTY_CLIENT_IDS`]), a workspace's app (`cb_`),
/// a registered one (`dcr_`) or an instance app (`app_`, registered by an
/// instance administrator from the server's CLI), each followed by 16 random bytes in
/// unpadded base64url. `None` for any other: a client known by its metadata
/// document has a URL for an id, and another identity provider's ids are
/// its own, so neither goes into a tracked event.
pub fn minted_client_id(id: &str) -> Option<&str> {
    let random = |prefix: &str| {
        id.strip_prefix(prefix).is_some_and(|rest| {
            rest.len() == 22
                && rest
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        })
    };
    (FIRST_PARTY_CLIENT_IDS.contains(&id) || random("cb_") || random("dcr_") || random("app_"))
        .then_some(id)
}

/// What caused a job's events: the job, or the event a delivery delivers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CausationKind {
    /// A job.
    Job,
    /// A domain event (its delivery).
    Event,
}

/// The job or event that caused what is tracked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Causation {
    /// A job or an event.
    #[serde(rename = "type")]
    pub kind: CausationKind,
    /// Its id.
    pub id: String,
}

/// Who acts, in which operation, for which request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackingContext {
    /// Who acts.
    pub actor: Actor,
    /// For whom, when the actor works for someone (the assistant, an
    /// automation, a job a request caused).
    pub on_behalf_of: Option<Actor>,
    /// The `operationId` being served.
    pub operation: Option<String>,
    /// The request's trace id, carried into every job and delivery it causes.
    pub correlation: String,
    /// The job or event that caused it.
    pub causation: Option<Causation>,
}

impl TrackingContext {
    /// `actor` acting on its own, with a new correlation (the current trace
    /// id when there is one).
    pub fn new(actor: Actor) -> Self {
        Self {
            actor,
            on_behalf_of: None,
            operation: None,
            correlation: new_correlation(),
            causation: None,
        }
    }

    /// The same, on behalf of `actor`.
    #[must_use]
    pub fn on_behalf_of(mut self, actor: Actor) -> Self {
        self.on_behalf_of = Some(actor);
        self
    }

    /// The same, inside operation `operation`.
    #[must_use]
    pub fn in_operation(mut self, operation: impl Into<String>) -> Self {
        self.operation = Some(operation.into());
        self
    }

    /// The same, with correlation `correlation`.
    #[must_use]
    pub fn correlated(mut self, correlation: impl Into<String>) -> Self {
        self.correlation = correlation.into();
        self
    }

    /// The context of work done with no request or job around it:
    /// `system` with the command's name ([`set_command`]).
    pub fn outside() -> Self {
        Self::new(Actor::system(command()))
    }
}

tokio::task_local! {
    /// The running request's or job's context. Shared (not copied) with the
    /// extractors that name the actor once they know it.
    static CURRENT: Arc<Mutex<TrackingContext>>;
}

/// Run `work` in `context`.
pub async fn scope<F: Future>(context: TrackingContext, work: F) -> F::Output {
    CURRENT.scope(Arc::new(Mutex::new(context)), work).await
}

/// The running context, if any.
pub fn current() -> Option<TrackingContext> {
    CURRENT
        .try_with(|current| {
            current
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone()
        })
        .ok()
}

/// The running context, or [`TrackingContext::outside`].
pub fn current_or_outside() -> TrackingContext {
    current().unwrap_or_else(TrackingContext::outside)
}

/// Change the running context, if any.
fn update(change: impl FnOnce(&mut TrackingContext)) {
    let _ = CURRENT.try_with(|current| {
        change(&mut current.lock().unwrap_or_else(PoisonError::into_inner));
    });
}

/// The request authenticated `principal`: an anonymous request is now the
/// principal's; the assistant or an automation acts on its behalf.
pub(crate) fn authenticated(principal: &Principal) {
    update(|context| match context.actor.kind {
        ActorType::Anonymous => context.actor = Actor::of(principal),
        ActorType::Assistant | ActorType::Automation if context.on_behalf_of.is_none() => {
            context.on_behalf_of = Some(Actor::of(principal));
        }
        _ => {}
    });
}

/// The request is an instance administrator's (`account`).
pub(crate) fn administrator(account: &str) {
    update(|context| {
        context.actor = Actor::admin(account);
        context.on_behalf_of = None;
    });
}

static COMMAND: OnceLock<String> = OnceLock::new();

/// Name the process's command (`serve`, `admin`), the actor id of what is
/// tracked outside any request or job. The first call wins.
pub fn set_command(name: &str) {
    let _ = COMMAND.set(name.to_owned());
}

fn command() -> String {
    COMMAND
        .get()
        .cloned()
        .unwrap_or_else(|| "process".to_owned())
}

/// A correlation id: the current trace id (32 hex digits), or a new random
/// one of the same shape when nothing is traced.
pub fn new_correlation() -> String {
    crate::telemetry::current_trace_id()
        .unwrap_or_else(|| uuid::Uuid::now_v7().simple().to_string())
}

/// What a job remembers of the context it was enqueued in (or the delivery
/// fanned out in): the person (or app) it works for, and the request's
/// correlation. Ids and enums only, at most [`MAX_ORIGIN_BYTES`]; stored in
/// `kernel.jobs.origin`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobOrigin {
    /// Who the work is for: the context's `on_behalf_of`, else its actor
    /// unless that is `system`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<Actor>,
    /// The request's correlation.
    pub correlation: String,
}

/// Largest origin stored; a larger one (an unusually long client id) is
/// dropped rather than refusing the enqueue.
pub const MAX_ORIGIN_BYTES: usize = 1_024;

/// The origin of a job enqueued now: `None` outside any context.
pub(crate) fn origin() -> Option<serde_json::Value> {
    let context = current()?;
    let actor = context
        .on_behalf_of
        .clone()
        .or_else(|| (context.actor.kind != ActorType::System).then(|| context.actor.clone()));
    let origin = JobOrigin {
        actor,
        correlation: context.correlation,
    };
    let value = serde_json::to_value(origin).ok()?;
    (value.to_string().len() <= MAX_ORIGIN_BYTES).then_some(value)
}

/// The context a job runs in: `system` (its kind), on behalf of its origin's
/// actor, with its origin's correlation, caused by the job or, for a
/// delivery, by the event.
pub(crate) fn for_job(
    kind: &str,
    job: JobId,
    event: Option<EventId>,
    origin: Option<&serde_json::Value>,
) -> TrackingContext {
    let origin: Option<JobOrigin> = origin.and_then(|value| JobOrigin::deserialize(value).ok());
    let causation = match event {
        Some(event) => Causation {
            kind: CausationKind::Event,
            id: event.to_string(),
        },
        None => Causation {
            kind: CausationKind::Job,
            id: job.to_string(),
        },
    };
    let (on_behalf_of, correlation) = match origin {
        Some(origin) => (origin.actor, origin.correlation),
        None => (None, new_correlation()),
    };
    TrackingContext {
        actor: Actor::system(kind),
        on_behalf_of,
        operation: None,
        correlation,
        causation: Some(causation),
    }
}
