//! Shared application state handed to every handler.

use std::sync::Arc;

use sqlx::PgPool;

use crate::{
    admin::{AdminAuditPort, AdminProfilesPort, NoAdmins, NoAudit},
    auth::TokenVerifier,
    authz,
    blobs::BlobStore,
    changes::{ChangeHub, NoContainers, VisibilityPort},
    config::Config,
    connections::{ConsentDoorPort, GrantIntakePort, NoConsentDoor, NoGrantIntake},
    crypto::Signer,
    events::EventBus,
    i18n::{LocalePreferencePort, NoLocalePreference},
    jobs::Registry,
    operations::Operations,
    rate_limit::RateLimits,
    search::SearchSources,
    signup::{InvitationsPort, NoInvitations},
    telemetry::{self, Metrics},
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
    /// Answers which language a user chose, for the
    /// [`Locale`](crate::i18n::Locale) of their requests. The server
    /// registers the `workspaces` module's implementation; until then
    /// nobody chose one.
    pub locales: LocalePreferencePort,
    /// Message bodies, attachments and files (ADR 0011). Not configured
    /// until the server attaches one with [`with_blobs`](Self::with_blobs).
    pub blobs: BlobStore,
    /// The composed API, for running operations by id
    /// ([`invoke`](crate::operations::invoke)). Installed by the server.
    pub operations: Operations,
    /// This process's change-feed hub: wakes open change streams.
    pub changes: ChangeHub,
    /// Answers whether a member may see changes inside a container (an
    /// inbox, a calendar). Until a module registers one, such changes are
    /// shown to nobody.
    pub visibility: VisibilityPort,
    /// The modules' records as the `search` module reads them (ADR 0016).
    /// Until the server registers them, search indexes nothing.
    pub search_sources: SearchSources,
    /// Takes mailbox grants obtained at sign-in into connections. The
    /// server registers the `connectors` module's implementation; until
    /// then no mailbox can be connected that way.
    pub grants: GrantIntakePort,
    /// Starts provider consents for other modules (the identity module's
    /// front door). Until the server registers it, none can start.
    pub consent: ConsentDoorPort,
    /// Answers whether an address has a pending invitation. The server
    /// registers the `workspaces` module's implementation; until then
    /// nobody is invited.
    pub invitations: InvitationsPort,
    /// Counts hits of endpoints anyone can call ([`rate_limit`](crate::rate_limit)).
    /// The server installs one keyed by the instance secret, shared by all
    /// its processes; until then it counts per process.
    pub rate_limits: RateLimits,
    /// Signs opaque values the server hands out (cursors). The server
    /// installs one keyed by the instance secret; until then it signs per
    /// process.
    pub signer: Signer,
    /// The request and job instruments: from the meter provider installed
    /// when the state is built (the no-op without one).
    pub metrics: Metrics,
    /// Every module's resource types and facts: what
    /// [`authz::authorize`] decides with (ADR 0017). Empty (everything
    /// denied) until the server registers it.
    pub resources: authz::Catalogue,
    /// Reads admin profiles, for [`Admin`](crate::admin::Admin) handlers.
    /// The server registers the `admin` module's implementation; until then
    /// nobody is an administrator.
    pub admin_profiles: AdminProfilesPort,
    /// Appends to the admin audit trail. The server registers the `admin`
    /// module's implementation; until then no admin request is answered.
    pub admin_audit: AdminAuditPort,
}

impl AppState {
    /// State with an event bus that knows no consumers yet and a token
    /// verifier for `config.auth`. The server adds the modules' registry with
    /// [`with_registry`](Self::with_registry).
    pub fn new(db: PgPool, config: Config) -> Self {
        let metrics = Metrics::new(&telemetry::meter(), config.telemetry.exports_metrics());
        Self {
            changes: ChangeHub::new(db.clone(), &config.changes),
            visibility: Arc::new(NoContainers),
            search_sources: SearchSources::default(),
            metrics,
            db,
            auth: TokenVerifier::new(&config.auth),
            config: Arc::new(config),
            events: EventBus::default(),
            membership: Arc::new(NoMembership),
            grants: Arc::new(NoGrantIntake),
            consent: Arc::new(NoConsentDoor),
            invitations: Arc::new(NoInvitations),
            locales: Arc::new(NoLocalePreference),
            blobs: BlobStore::default(),
            operations: Operations::default(),
            admin_profiles: Arc::new(NoAdmins),
            admin_audit: Arc::new(NoAudit),
            rate_limits: RateLimits::default(),
            signer: Signer::default(),
            resources: authz::Catalogue::default(),
        }
    }

    /// The same state, reading admin profiles through `profiles` and
    /// recording admin requests through `audit`.
    #[must_use]
    pub fn with_admin(mut self, profiles: AdminProfilesPort, audit: AdminAuditPort) -> Self {
        self.admin_profiles = profiles;
        self.admin_audit = audit;
        self
    }

    /// The same state, resolving workspace membership through `membership`.
    #[must_use]
    pub fn with_membership(mut self, membership: MembershipPort) -> Self {
        self.membership = membership;
        self
    }

    /// The same state, taking mailbox grants into connections through
    /// `grants`.
    #[must_use]
    pub fn with_grant_intake(mut self, grants: GrantIntakePort) -> Self {
        self.grants = grants;
        self
    }

    /// The same state, starting provider consents through `consent`.
    #[must_use]
    pub fn with_consent_door(mut self, consent: ConsentDoorPort) -> Self {
        self.consent = consent;
        self
    }

    /// The same state, looking up invitations through `invitations`.
    #[must_use]
    pub fn with_invitations(mut self, invitations: InvitationsPort) -> Self {
        self.invitations = invitations;
        self
    }

    /// The same state, validating access tokens with `auth` (the server
    /// installs the in-process issuer's verifier this way).
    #[must_use]
    pub fn with_auth(mut self, auth: TokenVerifier) -> Self {
        self.auth = auth;
        self
    }

    /// The same state, deciding container visibility in the change feed
    /// through `visibility`.
    #[must_use]
    pub fn with_visibility(mut self, visibility: VisibilityPort) -> Self {
        self.visibility = visibility;
        self
    }

    /// The same state, deciding access with `resources` (ADR 0017).
    #[must_use]
    pub fn with_resources(mut self, resources: authz::Catalogue) -> Self {
        self.resources = resources;
        self
    }

    /// The same state, with the modules' search sources.
    #[must_use]
    pub fn with_search_sources(mut self, sources: SearchSources) -> Self {
        self.search_sources = sources;
        self
    }

    /// The same state, reading users' chosen locales through `locales`.
    #[must_use]
    pub fn with_locales(mut self, locales: LocalePreferencePort) -> Self {
        self.locales = locales;
        self
    }

    /// The same state, counting rate limits with `rate_limits`.
    #[must_use]
    pub fn with_rate_limits(mut self, rate_limits: RateLimits) -> Self {
        self.rate_limits = rate_limits;
        self
    }

    /// The same state, signing with `signer`.
    #[must_use]
    pub fn with_signer(mut self, signer: Signer) -> Self {
        self.signer = signer;
        self
    }

    /// The same state, publishing to (and running the work of) `registry`.
    #[must_use]
    pub fn with_registry(mut self, registry: Registry) -> Self {
        self.resources = registry.resources().clone();
        self.events = EventBus::new(Arc::new(registry));
        self
    }

    /// The same state, storing blobs in `blobs`.
    #[must_use]
    pub fn with_blobs(mut self, blobs: BlobStore) -> Self {
        self.blobs = blobs;
        self
    }

    /// The same state, recording into `metrics`.
    #[must_use]
    pub fn with_metrics(mut self, metrics: Metrics) -> Self {
        self.metrics = metrics;
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
