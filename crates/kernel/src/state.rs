//! Shared application state handed to every handler.

use std::sync::Arc;

use sqlx::PgPool;

use crate::{
    auth::TokenVerifier,
    blobs::BlobStore,
    config::Config,
    events::EventBus,
    jobs::Registry,
    operations::Operations,
    tenancy::{MembershipPort, NoMembership},
};

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
    /// Answers whether a caller belongs to a workspace, for
    /// [`Workspace`](crate::tenancy::Workspace) handlers. The server
    /// registers the `workspaces` module's implementation; until then nobody
    /// belongs anywhere.
    pub membership: MembershipPort,
    /// Message bodies, attachments and files (ADR 0011). Not configured
    /// until the server attaches one with [`with_blobs`](Self::with_blobs).
    pub blobs: BlobStore,
    /// The composed API, for running operations by id
    /// ([`invoke`](crate::operations::invoke)). Installed by the server.
    pub operations: Operations,
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
            membership: Arc::new(NoMembership),
            blobs: BlobStore::default(),
            operations: Operations::default(),
        }
    }

    /// The same state, resolving workspace membership through `membership`.
    #[must_use]
    pub fn with_membership(mut self, membership: MembershipPort) -> Self {
        self.membership = membership;
        self
    }

    /// The same state, publishing to (and running the work of) `registry`.
    #[must_use]
    pub fn with_registry(mut self, registry: Registry) -> Self {
        self.events = EventBus::new(Arc::new(registry));
        self
    }

    /// The same state, storing blobs in `blobs`.
    #[must_use]
    pub fn with_blobs(mut self, blobs: BlobStore) -> Self {
        self.blobs = blobs;
        self
    }

    /// The same state, able to [`invoke`](crate::operations::invoke) the
    /// operations of `operations`.
    #[must_use]
    pub fn with_operations(mut self, operations: Operations) -> Self {
        self.operations = operations;
        self
    }
}
