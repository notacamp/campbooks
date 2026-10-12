//! Shared application state handed to every handler.

use std::sync::Arc;

use sqlx::PgPool;

use crate::{auth::TokenVerifier, config::Config, events::EventBus};

/// What every handler can reach. Cheap to clone.
#[derive(Debug, Clone)]
pub struct AppState {
    /// Postgres pool. Each module only touches its own schema.
    pub db: PgPool,
    /// Loaded configuration.
    pub config: Arc<Config>,
    /// In-process domain event bus.
    pub events: EventBus,
    /// Validates access tokens for [`Scoped`](crate::auth::Scoped) handlers.
    pub auth: TokenVerifier,
}

impl AppState {
    /// State with a fresh event bus and a token verifier for `config.auth`.
    pub fn new(db: PgPool, config: Config) -> Self {
        Self {
            db,
            auth: TokenVerifier::new(&config.auth),
            config: Arc::new(config),
            events: EventBus::default(),
        }
    }
}
