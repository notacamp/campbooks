//! `campbooks-kernel` — the domain-agnostic core every crate builds on.
//!
//! It depends on no other Campbooks crate and knows nothing about mail,
//! calendars or money (enforced by `cargo xtask arch`).

#![forbid(unsafe_code)]

pub mod auth;
pub mod clock;
pub mod config;
pub mod error;
pub mod events;
pub mod ids;
pub mod jobs;
pub mod module;
pub mod state;
#[cfg(any(test, feature = "test-util"))]
pub mod testing;

pub use config::Config;
pub use error::{Error, Problem, Result};
pub use events::{DomainEvent, Event, EventBus, EventHandler};
pub use jobs::{Background, Job, JobContext, JobError};
pub use module::Module;
pub use state::AppState;
