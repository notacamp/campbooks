//! `campbooks-kernel` — the domain-agnostic core every crate builds on.
//!
//! It depends on no other Campbooks crate and knows nothing about mail,
//! calendars or money (enforced by `cargo xtask arch`).

#![forbid(unsafe_code)]

pub mod auth;
pub mod blobs;
pub mod clock;
pub mod config;
pub mod db;
pub mod error;
pub mod events;
pub mod extract;
pub mod idempotency;
pub mod ids;
pub mod jobs;
pub mod module;
pub mod operations;
pub mod records;
pub mod state;
pub mod tenancy;
#[cfg(any(test, feature = "test-util"))]
pub mod testing;

pub use config::Config;
pub use error::{Error, Problem, Result};
pub use events::{DomainEvent, Event, EventBus, EventHandler};
pub use jobs::{Background, Job, JobContext, JobError};
pub use module::Module;
pub use operations::{Operation, Risk};
pub use records::RecordRef;
pub use state::AppState;
