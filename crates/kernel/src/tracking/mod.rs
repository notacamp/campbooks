//! Tracked events (ADR 0020, `docs/tracking.md`): the record of every action
//! — who did it, in what context, and what the records it touched looked
//! like at that moment.
//!
//! Tracked events are not domain events. Domain events
//! ([`events`](crate::events)) are contracts between modules; tracked
//! events are the product's record, defined once in the tracking plan
//! (`tracking/tracking-plan.xlsx`) and generated into typed structs, one
//! `tracking.gen.rs` per owning crate. Only those generated types implement
//! [`TrackedEvent`] (the plan check fails on any other).
//!
//! A module fills a generated event from its own rows, in the transaction
//! of the change, and tracks it there:
//!
//! ```ignore
//! let mut tx = state.db.begin_for(workspace).await?;
//! /* archive the conversation, read it back */
//! state.tracker.track(&mut tx, &tracking::ConversationArchived {
//!     workspace,
//!     conversation: snapshot,
//! }).await?;
//! tx.commit().await?;
//! ```
//!
//! The kernel adds the envelope — id, name, version, time, workspace, actor
//! and on behalf of, operation, correlation and causation, role, app and
//! plan version — from the ambient [`TrackingContext`], never from the
//! caller. Events go to the outbox `kernel.tracked_events`
//! (`CAMPBOOKS_TRACKING__SINK=outbox` or `amqp`) or nowhere (`none`, the
//! default). With `amqp`, the worker's [`Relay`] publishes the outbox to
//! RabbitMQ (campbooks-cloud#321), and [`PruneTrackedEvents`] deletes the
//! rows it published once their retention has passed.

mod context;
pub mod http;
mod prune;
pub mod relay;
mod tracker;

#[cfg(test)]
mod relay_tests;
#[cfg(test)]
mod tests;

#[path = "tracking.gen.rs"]
mod generated;

pub use context::{
    Actor, ActorType, Causation, CausationKind, JobOrigin, MAX_ORIGIN_BYTES, TrackingContext,
    current, current_or_outside, new_correlation, scope, set_command,
};
pub(crate) use context::{administrator, authenticated, for_job, origin};
#[allow(
    clippy::wildcard_imports,
    reason = "the generated file's items are this module's: the plan version and the kernel's \
              and the server's events, whatever the plan holds"
)]
pub use generated::*;
pub use prune::PruneTrackedEvents;
pub use relay::Relay;
pub use tracker::Tracker;

/// A tracked event: a type generated from the tracking plan. Never
/// implemented by hand (the plan check rejects it outside tests).
pub trait TrackedEvent: serde::Serialize + Send + Sync {
    /// `<module>.<object>_<past verb>`.
    const NAME: &'static str;
    /// The plan's version of the event, from 1.
    const VERSION: u16;
    /// The owning module.
    const MODULE: &'static str;
    /// Whether every event of the type belongs to a workspace.
    const WORKSPACE_SCOPED: bool;

    /// The workspace it happened in; `None` for an instance-wide event.
    fn workspace(&self) -> Option<types::WorkspaceId>;
}

/// The types generated events are built from, so a module's generated code
/// needs no dependency beyond the kernel and `serde`.
pub mod types {
    pub use super::TrackedEvent;
    pub use crate::records::RecordRef;
    pub use crate::tenancy::WorkspaceId;
    pub use chrono::{DateTime, NaiveDate, Utc};
    pub use uuid::Uuid;
}
