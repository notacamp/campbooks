//! Shared application state handed to every handler.

use std::sync::Arc;

use sqlx::PgPool;

use crate::{auth::TokenVerifier, config::Config, events::EventBus, jobs::Registry};

/// What every handler can reach. Cheap to clone.
#[derive(Debug, Clone)]
pub struct AppState {
    /// Postgres pool. Each module only touches its own schema.
    pub db: PgPool,
    /// Loaded configuration.
    pub config: Arc<Config>,
    /// The durable event bus; publishes events and their deliveries.
    pub events: EventBus,
    /// Validates access tokens for [`Scoped`](crate::auth::Scoped) handlers.
    pub auth: TokenVerifier,
}

impl AppState {
    /// State with an event bus that knows no consumers yet and a token
    /// verifier for `config.auth`. The server adds the modules' registry with
    /// [`with_registry`](Self::with_registry).
    pub fn new(db: PgPool, config: Config) -> Self {
        Self {
            db,
            auth: TokenVerifier::new(&config.auth),
            config: Arc::new(config),
            events: EventBus::default(),
        }
    }

    /// The same state, publishing to (and running the work of) `registry`.
    #[must_use]
    pub fn with_registry(mut self, registry: Registry) -> Self {
        self.events = EventBus::new(Arc::new(registry));
        self
    }
}
