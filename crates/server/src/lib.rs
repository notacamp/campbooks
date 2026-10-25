//! `campbooks-server` — composes the modules into one HTTP API.
//!
//! This is the only crate that knows the full list of modules. Adding a module
//! means adding one line to [`modules`]. Their background work (jobs, event
//! handlers, schedules, published events) is collected by [`registry`]; their
//! operations form the catalogue of [`openapi`] (ADR 0013). Their admin
//! operations form a separate API, [`admin_openapi`], outside that catalogue
//! (ADR 0015).

#![forbid(unsafe_code)]

pub mod bff;
pub mod changes;
pub mod error_reports;
pub mod telemetry;
pub mod web;

#[cfg(test)]
mod access_matrix_tests;
#[cfg(test)]
mod admin_tests;
#[cfg(test)]
mod ai_usage_tests;
#[cfg(test)]
mod assistant_tests;
#[cfg(test)]
mod identity_tests;
#[cfg(test)]
mod rate_limit_tests;
#[cfg(test)]
mod revocation_tests;
#[cfg(test)]
mod search_tests;
#[cfg(test)]
mod tracking_tests;

use std::{fmt::Write as _, sync::Arc, time::Duration};

use axum::{
    Json, Router,
    extract::Request as AxumRequest,
    http::{HeaderName, HeaderValue, header},
    middleware,
    routing::get,
};
use campbooks_kernel::{
    AppState, Error, Module, Problem,
    auth::ScopeInfo,
    events::EventCatalogue,
    i18n,
    jobs::{Registry, RegistryError},
    operations,
    search::SearchSources,
};
use tower::ServiceBuilder;
use tower_http::{
    compression::CompressionLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    set_header::SetResponseHeaderLayer,
};
use utoipa::{
    OpenApi as _,
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
};
use utoipa_axum::router::OpenApiRouter;

/// The server's own messages (`i18n/<locale>.json`): the reasons of the web
/// BFF's errors.
pub static MESSAGES: i18n::Catalogue = campbooks_kernel::catalogue!("server");

/// Longest a request may take before the server answers 503, unless its
/// handler allows itself longer ([`campbooks_kernel::time_limit`]).
pub const REQUEST_TIMEOUT: Duration = campbooks_kernel::time_limit::DEFAULT;

/// Path the OpenAPI document is served at.
pub const OPENAPI_PATH: &str = "/openapi.json";

/// Path the health check also answers at, for load balancers and uptime
/// monitors that probe `/up`. Not an API operation: it is mounted by
/// [`compose`], outside the OpenAPI document.
pub const UP_PATH: &str = "/up";

/// Header carrying the request id, generated when the client sends none.
pub const REQUEST_ID_HEADER: &str = "x-request-id";

/// Security headers set on every response, API and web client alike. A
/// response may only make `Referrer-Policy` stricter (the BFF's sign-in
/// landing page sends `no-referrer`); the others are always these.
pub const SECURITY_HEADERS: [(HeaderName, &str); 3] = [
    (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
    (header::REFERRER_POLICY, "strict-origin-when-cross-origin"),
    (header::X_FRAME_OPTIONS, "DENY"),
];

/// Path prefixes whose responses carry people's data (the API, the web BFF
/// and its proxied API, the admin API): never stored by a browser or a shared
/// cache ([`no_store`]).
pub const NO_STORE_PREFIXES: [&str; 3] = ["/api/", "/bff/", "/admin/"];

/// Exact paths outside those prefixes that are never stored either: the
/// probe alias, whose answer must always be live.
const NO_STORE_PATHS: [&str; 1] = [UP_PATH];

/// `Cache-Control: no-store` and `Pragma: no-cache` on every response under
/// [`NO_STORE_PREFIXES`], whatever the handler set (a handler's own
/// `no-store` variant, such as `private, no-store`, is kept). One layer, so
/// no endpoint can forget it.
async fn no_store(request: AxumRequest, next: middleware::Next) -> axum::response::Response {
    let path = request.uri().path();
    let private = NO_STORE_PREFIXES
        .iter()
        .any(|prefix| path.starts_with(prefix) || path == prefix.trim_end_matches('/'))
        || NO_STORE_PATHS.contains(&path);
    let mut response = next.run(request).await;
    if private {
        let headers = response.headers_mut();
        let stores = headers
            .get(header::CACHE_CONTROL)
            .and_then(|value| value.to_str().ok())
            .is_none_or(|value| !value.contains("no-store"));
        if stores {
            headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        }
        headers.insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    }
    response
}

/// Name of the OpenAPI security scheme every scoped operation references:
/// `security(("bearer" = ["<scope>"]))`.
pub const SECURITY_SCHEME: &str = "bearer";

/// What the modules are composed with (constructor injection), read from
/// the environment at startup. `Default` is inert (no provider configured):
/// what the catalogues (OpenAPI, events, scopes) are built from.
#[derive(Debug, Clone, Default)]
pub struct Composition {
    /// The connectors module's settings, Google client and credentials
    /// cipher.
    pub connectors: campbooks_connectors::Deps,
    /// The conversations module's AI gateway (when an AI role is set),
    /// settings, rules strategy and per-question policy.
    pub conversations: campbooks_conversations::ConversationsDeps,
    /// The identity module's in-process issuer, when it serves this
    /// instance (ADR 0014).
    pub identity: campbooks_identity::Deps,
    /// The system module's AI gateway and the facts of this composition,
    /// for the admin system overview.
    pub system: campbooks_system::Deps,
    /// The assistant module's AI gateway (its `complete` role compiles
    /// members' sentences into rules to check and answers in Scout's chat).
    /// Its handbook is always the one [`handbook`] composes.
    pub assistant: campbooks_assistant::AssistantDeps,
}

impl Composition {
    /// Read every module's dependencies from the environment. A broken
    /// setting is an error naming it. `tracker` is the server's (with its
    /// role): what the AI usage meter tracks the daily limit with.
    pub fn from_env(
        config: &campbooks_kernel::Config,
        tracker: &campbooks_kernel::tracking::Tracker,
    ) -> Result<Self, String> {
        let ai = campbooks_ai::AiConfig::from_env().map_err(|e| e.to_string())?;
        let (gateway, ai_usage) = ai_gateway(ai, tracker.clone())?;
        let connectors = campbooks_connectors::Deps::from_env(config)?;
        let identity = campbooks_identity::Deps::from_env(config, &scope_catalogue())?;
        let assistant = campbooks_assistant::AssistantDeps {
            ai: gateway.clone(),
            handbook: handbook(),
        };
        let system = campbooks_system::Deps {
            ai: gateway.clone(),
            facts: facts(config, &identity, &connectors),
            started_at: chrono::Utc::now(),
            ai_usage,
            screen_views: campbooks_system::ScreenViewRelay::default(),
        };
        Ok(Self {
            connectors,
            identity,
            conversations: campbooks_conversations::ConversationsDeps::new(
                gateway,
                campbooks_conversations::ConversationsConfig::from_env()?,
            ),
            system,
            assistant,
        })
    }
}

/// The AI gateway every module calls models through (none when no role is
/// configured), and the meter it counts every call with, per workspace and
/// UTC day, under `ai`'s daily cap (campbooks-cloud#152), tracking the day's
/// first refusal with `tracker`. The admin AI usage read shows the same
/// meter.
pub fn ai_gateway(
    ai: campbooks_ai::AiConfig,
    tracker: campbooks_kernel::tracking::Tracker,
) -> Result<
    (
        Option<campbooks_ai::Gateway>,
        campbooks_system::AiUsageMeter,
    ),
    String,
> {
    let meter = campbooks_system::AiUsageMeter::new(ai.daily_call_cap()).with_tracker(tracker);
    if ai.roles.is_empty() {
        return Ok((None, meter));
    }
    let gateway = campbooks_ai::Gateway::new(ai)
        .map_err(|e| e.to_string())?
        .with_meter(Arc::new(meter.clone()));
    Ok((Some(gateway), meter))
}

/// Who signs people in and which providers are set up, by name only: what
/// the admin system overview shows.
fn facts(
    config: &campbooks_kernel::Config,
    identity: &campbooks_identity::Deps,
    connectors: &campbooks_connectors::Deps,
) -> campbooks_system::Facts {
    use campbooks_system::IdentityProvider;

    let (identity_provider, sign_in_providers) = match &identity.issuer {
        Some(issuer) => (
            IdentityProvider::BuiltIn,
            issuer
                .upstreams
                .iter()
                .map(|upstream| upstream.provider().as_str().to_owned())
                .collect(),
        ),
        None if config.auth.issuer().is_some() => (IdentityProvider::External, Vec::new()),
        None => (IdentityProvider::None, Vec::new()),
    };
    campbooks_system::Facts {
        identity_provider,
        sign_in_providers,
        mail_providers: connectors
            .google
            .is_some()
            .then(|| "google".to_owned())
            .into_iter()
            .collect(),
    }
}

/// Every module, in mount order, composed inertly (for catalogues).
pub fn modules() -> Vec<Module> {
    modules_with(&Composition::default())
}

/// Every module, in mount order, composed with `composition`.
pub fn modules_with(composition: &Composition) -> Vec<Module> {
    vec![
        campbooks_system::module_with(composition.system.clone()),
        campbooks_workspaces::module(),
        campbooks_connectors::module(composition.connectors.clone()),
        campbooks_conversations::module(composition.conversations.clone()),
        campbooks_identity::module(composition.identity.clone()),
        campbooks_admin::module(),
        campbooks_attention::module(),
        campbooks_search::module(),
        campbooks_assistant::module(campbooks_assistant::AssistantDeps {
            handbook: handbook(),
            ..composition.assistant.clone()
        }),
        campbooks_tasks::module(),
        campbooks_schedule::module(),
        // `cargo xtask new-module` inserts above this line.
    ]
}

/// Scout's handbook (campbooks-cloud#305): the assistant's core section,
/// then each module's own section, in this order. A module Scout should
/// explain adds its `HANDBOOK` here.
pub fn handbook() -> campbooks_assistant::Handbook {
    use campbooks_assistant::Section;
    campbooks_assistant::Handbook::new(vec![
        Section {
            module: campbooks_conversations::NAME,
            text: campbooks_conversations::HANDBOOK,
        },
        Section {
            module: campbooks_attention::NAME,
            text: campbooks_attention::HANDBOOK,
        },
        Section {
            module: campbooks_tasks::NAME,
            text: campbooks_tasks::HANDBOOK,
        },
        Section {
            module: campbooks_schedule::NAME,
            text: campbooks_schedule::HANDBOOK,
        },
    ])
}

/// `state` with the ports modules implement for the kernel registered: the
/// `workspaces` module answers who belongs to which workspace and which
/// language each user chose; the `conversations` module, who may see the
/// change feed's records inside an inbox and search's results, and how
/// search reads its conversations (ADR 0016); the `admin` module, who is an
/// administrator, and keeps the trail of what they do; the `assistant`
/// module, whether a member lets Scout act on its own proposals; the
/// `connectors` module, where each inbox's sends go (campbooks-cloud#242).
pub fn with_ports(state: AppState) -> AppState {
    state
        .with_resources(resource_catalogue())
        .with_admin(campbooks_admin::profiles(), campbooks_admin::audit())
        .with_membership(campbooks_workspaces::membership())
        .with_locales(campbooks_workspaces::locale_preference())
        .with_invitations(campbooks_workspaces::invitations())
        .with_visibility(Arc::new(campbooks_kernel::changes::Visibilities::new(
            vec![
                campbooks_conversations::visibility(),
                campbooks_tasks::visibility(),
            ],
        )))
        .with_open_tasks(campbooks_tasks::open_tasks())
        .with_preferences(campbooks_assistant::preferences())
        .with_kept_mail(campbooks_conversations::kept_mail())
        .with_outbound(campbooks_conversations::outbound_requests())
        .with_destinations(campbooks_connectors::outbound_destinations())
        .with_calendar_requests(campbooks_schedule::calendar_requests())
        .with_time_zones(campbooks_schedule::time_zones())
        .with_availability(campbooks_schedule::availability())
        .with_holds(campbooks_schedule::holds())
        .with_search_sources(SearchSources::new(vec![
            campbooks_conversations::search_source(),
        ]))
}

/// [`with_ports`], plus the ports of the modules composed with
/// `composition`: the connectors module takes mailbox grants obtained at
/// sign-in, and when the issuer runs in this process the API validates
/// tokens with its keys, read in-process (ADR 0014), and refuses a person's
/// token once their account is suspended or signed out everywhere.
pub fn with_composed_ports(state: AppState, composition: &Composition) -> AppState {
    let state = with_ports(state)
        .with_grant_intake(campbooks_connectors::grant_intake(
            composition.connectors.clone(),
        ))
        .with_consent_door(campbooks_identity::consent_door(
            composition.identity.clone(),
        ));
    match &composition.identity.issuer {
        Some(issuer) => {
            // Every client token comes from this issuer: a registered client
            // acts for the member who registered it (ADR 0017).
            let state = state.with_client_directory(campbooks_identity::client_directory());
            let keys = campbooks_identity::LocalKeys {
                db: state.db.clone(),
                keys: Arc::clone(&issuer.keys),
            };
            // Its access tokens are for the API's resource URI (RFC 8707);
            // `CAMPBOOKS_AUTH__AUDIENCE` is what an outside IdP sends.
            let mut auth = state.config.auth.clone();
            auth.audience.clone_from(&issuer.resource);
            // Its subjects are identity accounts: a person's token stops at
            // once when the account is suspended or signs out everywhere.
            let verifier = campbooks_kernel::auth::TokenVerifier::in_process(
                &auth,
                &issuer.url,
                Arc::new(keys),
            )
            .with_revocation(state.db.clone(), campbooks_identity::token_revocation());
            state.with_auth(verifier)
        }
        None => state,
    }
}

/// Every job kind, event handler and schedule of every module, plus the
/// kernel's maintenance and the web BFF's. Fails when a registration breaks
/// a naming rule or is duplicated (also checked by `cargo xtask arch`).
pub fn registry() -> Result<Registry, RegistryError> {
    registry_with(&Composition::default())
}

/// [`registry`] of the modules composed with `composition`.
pub fn registry_with(composition: &Composition) -> Result<Registry, RegistryError> {
    Registry::build_with(
        &modules_with(composition),
        &[(bff::OWNER, &bff::background())],
    )?
    .with_resources(&[
        (changes::OWNER, changes::resources()),
        (bff::OWNER, bff::resources()),
    ])
}

/// The access catalogue (ADR 0017): every resource type the modules, the
/// change feed and the web BFF declare, with the ports answering their
/// facts. (A registration error leaves it empty, so everything is denied;
/// startup reports the error.)
pub fn resource_catalogue() -> campbooks_kernel::authz::Catalogue {
    registry()
        .map(|registry| registry.resources().clone())
        .unwrap_or_default()
}

/// Every event the modules publish, with its payload schema: what
/// `campbooks-server events catalogue` prints and `openapi/events.json` holds.
pub fn events_catalogue() -> Result<EventCatalogue, RegistryError> {
    registry().map(|registry| EventCatalogue::new(&registry))
}

/// Every OAuth scope the API knows, sorted by name: what
/// `campbooks-server scopes` prints and the identity provider mirrors.
pub fn scope_catalogue() -> Vec<ScopeInfo> {
    let mut scopes: Vec<ScopeInfo> = modules()
        .iter()
        .flat_map(|module| module.scopes.iter().copied())
        .collect();
    scopes.sort_unstable();
    scopes
}

/// The `bearer` security scheme, listing the scope catalogue.
fn security_scheme() -> SecurityScheme {
    let scopes = scope_catalogue()
        .iter()
        .fold(String::new(), |mut list, scope| {
            // Writing to a `String` cannot fail.
            let _ = write!(list, "\n- `{}`: {}", scope.name, scope.description);
            list
        });
    let description = format!(
        "An OAuth 2.1 access token (a JWT) issued by this instance's identity provider, \
         the issuer configured as `CAMPBOOKS_AUTH__ISSUER`. Each operation lists the \
         scope it requires; operations with an empty requirement are public. Scopes:\n{scopes}"
    );
    SecurityScheme::Http(
        HttpBuilder::new()
            .scheme(HttpAuthScheme::Bearer)
            .bearer_format("JWT")
            .description(Some(description))
            .build(),
    )
}

#[derive(utoipa::OpenApi)]
#[openapi(
    info(
        title = "Campbooks API",
        description = "The Campbooks HTTP API. Errors are RFC 9457 `application/problem+json`. \
            Operations require an OAuth 2.1 bearer token with the scope they list \
            (401 without a valid token, 403 without the scope) unless they are public. \
            Those operations are rate-limited per caller (a person per app, an app on its own) \
            in classes: reads, writes, and mail sends (`x-campbooks.rate_limit: send`, counted \
            per person across their apps). Over a limit they answer 429 `rate_limited` with \
            `Retry-After`, in seconds. \
            The `/bff/*` operations serve the web app only: they authenticate with its \
            session cookie instead of a token.",
        license(name = "Sustainable Use License", identifier = "LicenseRef-Sustainable-Use"),
    ),
    components(schemas(Problem)),
    tags(
        (name = "system", description = "Instance health and version."),
        (name = "workspaces", description = "Workspaces, members and roles, invitations and the \
            security log. Workspace-owned resources live under `/api/v1/w/{workspace_id}/`; a \
            caller who is not a member (or a client token bound to another workspace) gets 404."),
        (name = "connectors", description = "Connections to outside accounts (Gmail, Zoho Mail): \
            connect with the provider's consent, see their health, sync now, disconnect. \
            Credentials are never returned."),
        (name = "conversations", description = "Inboxes, conversations and messages on any \
            channel (email first), sorted into streams. Workspace-owned: a user sees the inboxes \
            they are a member of; a client token bound to the workspace sees them all."),
        (name = "identity", description = "Accounts and their ways in, and this instance's \
            OAuth 2.1 authorization server: discovery, keys, authorization, tokens, revocation, \
            sign-in with a provider, and the mailboxes a sign-up granted, waiting for a \
            workspace. The protocol endpoints (`/.well-known/*`, `/oauth2/*`, `/auth/*`) serve \
            browsers and OAuth clients; they answer 404 while another identity provider serves \
            the instance."),
        (name = "attention", description = "What needs you: one queue per member of what the \
            records you may open ask of you (conversations today), with Later per member."),
        (name = "search", description = "One search over the records you may open: \
            conversations today. Words in English, Portuguese, Spanish and French with accents \
            ignored, Gmail-style operators, highlights as plain-text segments. Results come only \
            from the containers (inboxes) you may see, checked at query time."),
        (name = "assistant", description = "What you told Scout about how to act, and turning \
            a sentence you type into rules to check. Compiling uses the `complete` AI role only \
            when you ask, and sends it your sentence and your own addresses and label names, never \
            your mail; nothing is saved until you save a rule through the conversations module."),
        (name = "schedule", description = "Your calendars and their events: the provider \
            calendars you connect and your own Campbooks calendar, where Scout puts the dated \
            things it reads in your email. Events have a kind; free time is answered from your \
            working hours."),
        (name = "tasks", description = "What you have to do: your tasks, with a due date, a \
            plan (a day, or Later), a hand-off to one teammate and an undo for every change. \
            Personal: a task is seen by who it is with, and by who handed it off."),
        // `cargo xtask new-module` inserts above this line.
        (name = "changes", description = "The change feed: a Server-Sent Events stream per \
            workspace of the records that changed (ids only), so clients refetch only what \
            they show."),
    ),
)]
struct ApiDoc;

/// The admin API's document (`openapi/admin.json`, ADR 0015).
#[derive(utoipa::OpenApi)]
#[openapi(
    info(
        title = "Campbooks admin API",
        description = "Instance administration. Every operation answers 404 (as for a route that \
            does not exist) unless the caller is a user whose bearer token carries the \
            `admin:access` scope and whose admin profile grants the operation's permission \
            (`x-campbooks-admin.permission`; `super` grants all). Then a token whose last \
            sign-in (`auth_time`) is more than an hour old answers 401 \
            `admin_reauth_required`. Every request past the profile check is recorded in the \
            audit trail. Calls are rate-limited per caller (429 `rate_limited` with \
            `Retry-After`). These operations are not part of the operation catalogue \
            (`/openapi.json`): automation and the assistant cannot call them. Admin profiles \
            are managed only with the server's CLI (`campbooks-server admin`), never over HTTP. \
            The web app reaches this API through `/bff/admin/*`.",
        license(name = "Sustainable Use License", identifier = "LicenseRef-Sustainable-Use"),
    ),
    components(schemas(Problem)),
    tags(
        (name = "system", description = "The instance's version, health and configuration."),
    ),
)]
struct AdminDoc;

fn api() -> OpenApiRouter<AppState> {
    api_with(&Composition::default())
}

/// Every module's admin routes, under `/admin/v1/<module>/`.
fn admin_api_with(composition: &Composition) -> OpenApiRouter<AppState> {
    let mut doc = AdminDoc::openapi();
    env!("CARGO_PKG_VERSION").clone_into(&mut doc.info.version);
    doc.components
        .get_or_insert_with(Default::default)
        .add_security_scheme(
            campbooks_kernel::admin::SECURITY_SCHEME,
            admin_security_scheme(),
        );
    modules_with(composition)
        .into_iter()
        .filter_map(|module| module.admin_router)
        .fold(OpenApiRouter::with_openapi(doc), OpenApiRouter::merge)
}

/// The `admin` security scheme: a user's bearer token carrying
/// `admin:access`.
fn admin_security_scheme() -> SecurityScheme {
    SecurityScheme::Http(
        HttpBuilder::new()
            .scheme(HttpAuthScheme::Bearer)
            .bearer_format("JWT")
            .description(Some(format!(
                "An OAuth 2.1 access token for a person (not a client), issued to one of \
                 Campbooks' own apps with the `{}` scope, whose account has an admin profile.",
                campbooks_kernel::admin::ACCESS_SCOPE
            )))
            .build(),
    )
}

/// The admin API (`/admin/v1/*`) over `state`: not composed into the
/// operation catalogue, so [`invoke`](operations::invoke) and the
/// idempotency layer never see it; every route behind the audit middleware
/// and the admin class of the per-caller rate limits, problems in the
/// caller's language, all inside the request's tracking context (the
/// administrator, by account id, once `Admin` admits them; ADR 0020).
fn admin_routes(state: &AppState, composition: &Composition) -> Router<AppState> {
    let (routes, spec) = admin_api_with(composition).split_for_parts();
    let tracking = campbooks_kernel::tracking::http::HttpTracking::from_spec(&spec);
    let routes = routes
        .route_layer(middleware::from_fn_with_state(
            campbooks_kernel::admin::AuditLayer::new(state, &spec),
            campbooks_kernel::admin::audit,
        ))
        // Counted before the profile check: a non-admin hammering it stops
        // costing a lookup per request, and is answered as an admin would be.
        .route_layer(middleware::from_fn_with_state(
            campbooks_kernel::rate_limit::api::CallerLimits::admin(state),
            campbooks_kernel::rate_limit::api::middleware,
        ))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            i18n::localize,
        ));
    campbooks_kernel::tracking::http::around(routes, tracking)
}

/// The admin API's document: every module's admin operations
/// (`openapi/admin.json`). Never served over HTTP, and never merged into
/// [`openapi`].
pub fn admin_openapi() -> utoipa::openapi::OpenApi {
    admin_api_with(&Composition::default()).into_openapi()
}

fn api_with(composition: &Composition) -> OpenApiRouter<AppState> {
    let mut doc = ApiDoc::openapi();
    env!("CARGO_PKG_VERSION").clone_into(&mut doc.info.version);
    doc.components
        .get_or_insert_with(Default::default)
        .add_security_scheme(SECURITY_SCHEME, security_scheme());
    modules_with(composition)
        .into_iter()
        .fold(OpenApiRouter::with_openapi(doc), |api, module| {
            api.merge(module.router)
        })
        .merge(changes::router())
}

/// The OpenAPI document: every module's routes, the web BFF's and the
/// identity issuer's protocol endpoints.
pub fn openapi() -> utoipa::openapi::OpenApi {
    let mut doc = api().into_openapi();
    doc.merge(bff::openapi());
    doc.merge(identity_protocol_openapi());
    doc
}

/// The identity issuer's protocol endpoints (`/.well-known/*`, `/oauth2/*`,
/// `/auth/*`): owned by the `identity` module, outside its API router.
pub fn identity_protocol_openapi() -> utoipa::openapi::OpenApi {
    campbooks_identity::protocol::openapi()
}

/// The full HTTP application: module routes (behind the idempotency and
/// localization layers), `/openapi.json`, the web BFF (`/bff/*`; `bff` is
/// `None` when web sign-in is off), the web client (when `CAMPBOOKS_WEB_DIR`
/// is set) or else a problem+json 404 fallback, and the middleware stack
/// (security headers, request id, tracing and metrics
/// ([`telemetry::observe`]), timeout, compression, and problem+json rendered
/// in the request's locale).
///
/// The same module routes are installed in the state's
/// [`Operations`](operations::Operations), so automation and the assistant
/// invoke operations by id through exactly what HTTP requests reach. The
/// BFF's own operations are not in that catalogue: they serve browsers only.
pub fn router(state: AppState, bff: Option<bff::Settings>) -> Router {
    router_with(state, bff, &Composition::default())
}

/// [`router`] over the modules composed with `composition`.
pub fn router_with(
    state: AppState,
    bff: Option<bff::Settings>,
    composition: &Composition,
) -> Router {
    let operations::Composed {
        routes,
        mut spec,
        state,
    } = operations::compose(api_with(composition), state);
    spec.merge(bff::openapi());
    spec.merge(campbooks_identity::protocol::openapi());
    let spec = Arc::new(spec);

    // The issuer's protocol endpoints (`/.well-known/*`, `/oauth2/*`,
    // `/auth/*`); the BFF reaches them in-process when the issuer runs here.
    let identity =
        campbooks_identity::protocol::router(state.clone(), composition.identity.clone());

    // What `/bff/api/*` runs requests through: the composed module routes
    // (idempotency included), without the middleware below (the outer
    // request already passed it).
    let api_only: Router = routes
        .clone()
        .fallback(|| async { Error::NotFound })
        .with_state(state.clone());
    let admin_routes = admin_routes(&state, composition);
    let admin_only: Router = admin_routes
        .clone()
        .fallback(|| async { Error::NotFound })
        .with_state(state.clone());
    let bff = bff::Bff::new(bff, &state, api_only, admin_only, identity.clone());
    // Where Microsoft Graph posts change notifications (campbooks-cloud#316):
    // public like the issuer's endpoints, and like them outside the
    // operation catalogue. Empty unless notifications are on.
    let identity = identity.merge(campbooks_connectors::notifications_router(
        state.clone(),
        composition.connectors.clone(),
    ));

    // RFC 9728: every Bearer challenge of the API names its metadata, so an
    // agent that knows only the API's URL finds the issuer.
    let routes = routes.merge(admin_routes);
    let routes = match &composition.identity.issuer {
        Some(issuer) => routes.layer(axum::middleware::from_fn_with_state(
            Arc::<str>::from(
                campbooks_identity::protocol::discovery::resource_metadata_url(issuer),
            ),
            campbooks_kernel::auth::challenge_with_resource_metadata,
        )),
        None => routes,
    };
    compose(state, routes, spec, &bff, identity)
}

/// Assemble the application from the API routes, the OpenAPI document and
/// the BFF.
fn compose(
    state: AppState,
    routes: Router<AppState>,
    spec: Arc<utoipa::openapi::OpenApi>,
    bff: &bff::Bff,
    identity: Router,
) -> Router {
    let request_id = HeaderName::from_static(REQUEST_ID_HEADER);
    let web_client = web::configured_dir(&state.config).map(web::WebClient::new);
    // Innermost of the stack below, so fallback 404s (and the BFF's own
    // problems) are localized before compression wraps the body.
    let localize = axum::middleware::from_fn_with_state(state.clone(), i18n::localize);
    let metrics = state.metrics.clone();
    // The web BFF's and the issuer's own routes (sign-in, sign-up) run in a
    // tracking context too: anonymous until a token names the caller
    // (ADR 0020). The module routes have theirs from `operations::compose`.
    let tracking = campbooks_kernel::tracking::http::HttpTracking::from_spec(&spec);

    let routes = routes
        .route(
            OPENAPI_PATH,
            get(move || async move { Json(spec.as_ref().clone()) }),
        )
        // The health check again, for probes: same handler, so same answer.
        .route(UP_PATH, get(campbooks_system::health))
        .with_state(state)
        .merge(campbooks_kernel::tracking::http::around(
            bff.router(),
            tracking.clone(),
        ))
        .merge(campbooks_kernel::tracking::http::around(identity, tracking));
    let routes = match web_client {
        Some(client) => {
            routes.fallback(move |request: AxumRequest| client.clone().respond(request))
        }
        None => routes.fallback(|| async { Error::NotFound }),
    };

    let [nosniff, referrer, frame] = SECURITY_HEADERS.map(|(name, value)| {
        let value = HeaderValue::from_static(value);
        if name == header::REFERRER_POLICY {
            SetResponseHeaderLayer::if_not_present(name, value)
        } else {
            SetResponseHeaderLayer::overriding(name, value)
        }
    });
    routes.layer(
        ServiceBuilder::new()
            .layer(nosniff)
            .layer(referrer)
            .layer(frame)
            .layer(middleware::from_fn(no_store))
            .layer(SetRequestIdLayer::new(request_id.clone(), MakeRequestUuid))
            .layer(middleware::from_fn_with_state(metrics, telemetry::observe))
            .layer(PropagateRequestIdLayer::new(request_id))
            .layer(CompressionLayer::new())
            .layer(localize)
            // Inside `localize`, so its problem is in the caller's language.
            .layer(middleware::from_fn_with_state(
                REQUEST_TIMEOUT,
                campbooks_kernel::time_limit::middleware,
            )),
    )
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use campbooks_kernel::testing;
    use tower::ServiceExt;

    use super::*;

    fn app() -> Router {
        router(testing::state(testing::unreachable_pool()), None)
    }

    #[test]
    fn identity_and_connectors_know_the_same_zoho_data_centres() {
        // Identity validates the accounts server a Zoho grant comes from;
        // connectors maps that server to the data centre's mail host.
        assert_eq!(
            campbooks_identity::upstream::client::ZOHO_ACCOUNTS_SERVERS,
            campbooks_connectors::ZOHO_ACCOUNTS_SERVERS
        );
    }

    #[test]
    fn openapi_lists_every_module_route() {
        let spec = serde_json::to_value(openapi()).unwrap();
        assert_eq!(spec["info"]["title"], "Campbooks API");
        assert_eq!(spec["info"]["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(
            spec["paths"]["/api/v1/system/health"]["get"]["operationId"],
            "system.health"
        );
        assert_eq!(
            spec["paths"]["/api/v1/system/version"]["get"]["operationId"],
            "system.version"
        );
        assert_eq!(
            spec["paths"]["/api/v1/system/version"]["get"]["x-campbooks"],
            serde_json::json!({
                "mutates": false,
                "automation": false,
                "assistant": false,
                "risk": "low",
                "records": [],
                "access": {"resource": "system.instance", "action": "read"}
            })
        );
        assert!(spec["components"]["schemas"]["Problem"].is_object());
    }

    #[test]
    fn every_handbook_section_fits() {
        let handbook = handbook();
        let modules: Vec<&str> = handbook.sections().iter().map(|s| s.module).collect();
        assert_eq!(
            modules,
            [
                "assistant",
                "conversations",
                "attention",
                "tasks",
                "schedule"
            ]
        );
        for section in handbook.sections() {
            assert!(
                section.text.len() <= campbooks_assistant::handbook::MAX_SECTION_BYTES,
                "{}: {} bytes",
                section.module,
                section.text.len()
            );
            assert!(!section.text.trim().is_empty(), "{}", section.module);
        }
    }

    #[test]
    fn openapi_declares_the_bearer_scheme() {
        let spec = serde_json::to_value(openapi()).unwrap();
        let scheme = &spec["components"]["securitySchemes"][SECURITY_SCHEME];
        assert_eq!(scheme["type"], "http");
        assert_eq!(scheme["scheme"], "bearer");
        assert_eq!(scheme["bearerFormat"], "JWT");
        let description = scheme["description"].as_str().unwrap();
        assert!(description.contains("CAMPBOOKS_AUTH__ISSUER"));
        for scope in scope_catalogue() {
            assert!(description.contains(scope.name), "{}", scope.name);
        }
    }

    #[test]
    fn health_and_version_are_explicitly_public() {
        let spec = serde_json::to_value(openapi()).unwrap();
        for path in ["/api/v1/system/health", "/api/v1/system/version"] {
            assert_eq!(
                spec["paths"][path]["get"]["security"],
                serde_json::json!([{}]),
                "{path}"
            );
        }
    }

    #[test]
    fn scope_catalogue_is_sorted_unique_and_owned_by_modules() {
        let catalogue = scope_catalogue();
        assert!(catalogue.iter().any(|s| s.name == "system:read"));
        let names: Vec<_> = catalogue.iter().map(|s| s.name).collect();
        let mut unique = names.clone();
        unique.dedup();
        assert_eq!(names, unique, "sorted and unique");
        for module in modules() {
            for scope in module.scopes {
                assert!(
                    scope.name.starts_with(&format!("{}:", module.name)),
                    "{} declares {}",
                    module.name,
                    scope.name
                );
            }
        }
    }

    /// Guards the pairing of `security(("bearer" = [..]))` in the OpenAPI doc
    /// with the `Scoped<S>` extractor in the handler: an operation documented
    /// as scoped must actually refuse a request without a token.
    #[tokio::test]
    async fn secured_operations_reject_anonymous_requests() {
        let spec = serde_json::to_value(openapi()).unwrap();
        let app = app();
        for (path, item) in spec["paths"].as_object().unwrap() {
            for (method, operation) in item.as_object().unwrap() {
                let secured = operation["security"]
                    .as_array()
                    .is_some_and(|reqs| reqs.iter().all(|r| r.get(SECURITY_SCHEME).is_some()));
                if !secured {
                    continue;
                }
                let uri = path_with_dummy_params(path);
                let request = Request::builder()
                    .method(method.to_uppercase().as_str())
                    .uri(&uri)
                    .body(Body::empty())
                    .unwrap();
                let response = testing::send(&app, request).await;
                assert_eq!(
                    response.status,
                    StatusCode::UNAUTHORIZED,
                    "{method} {path} is documented as scoped but answered without a token"
                );
            }
        }
    }

    /// `/a/{id}/b` → `/a/00000000-0000-0000-0000-000000000000/b`.
    fn path_with_dummy_params(path: &str) -> String {
        path.split('/')
            .map(|segment| {
                if segment.starts_with('{') {
                    "00000000-0000-0000-0000-000000000000"
                } else {
                    segment
                }
            })
            .collect::<Vec<_>>()
            .join("/")
    }

    /// Row-level security can only protect tables that have it: every table
    /// with a `workspace_id` column (and the workspaces themselves) must
    /// enable and force it and carry a policy, so a new module cannot forget.
    /// The kernel's tables too (the change feed, blobs, the event log and
    /// the queue): a workspace transaction sees only its own rows there.
    #[sqlx::test(migrations = "../../migrations")]
    async fn every_workspace_owned_table_has_row_level_security(db: sqlx::PgPool) {
        let unprotected = sqlx::query_scalar!(
            r#"
            SELECT n.nspname || '.' || c.relname AS "table!"
              FROM pg_class c
              JOIN pg_namespace n ON n.oid = c.relnamespace
             WHERE c.relkind IN ('r', 'p')
               AND n.nspname NOT IN ('pg_catalog', 'information_schema', 'public')
               AND (EXISTS (SELECT 1 FROM pg_attribute a
                             WHERE a.attrelid = c.oid AND a.attname = 'workspace_id'
                               AND NOT a.attisdropped)
                    OR (n.nspname, c.relname) = ('workspaces', 'workspaces'))
               AND NOT (c.relrowsecurity AND c.relforcerowsecurity
                        AND EXISTS (SELECT 1 FROM pg_policy p WHERE p.polrelid = c.oid))
             ORDER BY 1
            "#
        )
        .fetch_all(&db)
        .await
        .unwrap();
        assert!(
            unprotected.is_empty(),
            "no forced row-level security: {unprotected:?}"
        );
    }

    /// Cross-workspace maintenance on a module's table needs the
    /// maintenance scope (the schedulers' dispatchers), and may only read and
    /// lease (or start): its maintenance policies are `FOR SELECT` and `FOR
    /// UPDATE`. No module policy admits the kernel scope (campbooks-cloud#196),
    /// no kernel policy admits the maintenance scope, and none admits a
    /// transaction merely for setting no workspace (campbooks-cloud#129).
    /// Every policy of every schema is read, `USING` and `WITH CHECK` alike,
    /// for any mention of a setting (campbooks-cloud#207).
    #[sqlx::test(migrations = "../../migrations")]
    async fn maintenance_policies_only_read_and_lease(db: sqlx::PgPool) {
        let rows: Vec<(String, String, String, String, String)> = sqlx::query_as(
            "SELECT schemaname::text, tablename::text, policyname::text, cmd::text,
                    coalesce(qual, '') || ' ' || coalesce(with_check, '')
               FROM pg_policies
              WHERE schemaname <> 'pg_catalog'
              ORDER BY 1, 2, 3",
        )
        .fetch_all(&db)
        .await
        .unwrap();
        let unset = "'campbooks.workspace_id'::text, true), ''::text) IS NULL";
        let admitting_unset: Vec<&String> = rows
            .iter()
            .filter(|(.., text)| text.contains(unset))
            .map(|(_, _, name, ..)| name)
            .collect();
        assert!(admitting_unset.is_empty(), "{admitting_unset:?}");
        let mentioning = |setting: &str| {
            let name = format!("'{setting}'");
            rows.iter()
                .filter(|(.., text)| text.contains(&name))
                .map(|(schema, table, policy, cmd, _)| {
                    (format!("{schema}.{table}"), policy.clone(), cmd.clone())
                })
                .collect::<Vec<_>>()
        };
        // The kernel scope: the kernel's own tables, each through its one
        // `workspace_isolation` policy, except the schedules' last ticks and
        // the rate-limit counters (`kernel_scope_only`) and the idempotency
        // keys, of which it may only read and delete the expired
        // (campbooks-cloud#210); and nothing else.
        let kernel = mentioning(testing::kernel_scope_setting());
        assert!(!kernel.is_empty());
        let expected = |table: &str, policy: &str, cmd: &str| match table {
            "kernel.schedules" | "kernel.rate_limits" => {
                (policy, cmd) == ("kernel_scope_only", "ALL")
            }
            "kernel.idempotency" => matches!(
                (policy, cmd),
                ("kernel_scope_reads_expired", "SELECT")
                    | ("kernel_scope_prunes_expired", "DELETE")
            ),
            table if table.starts_with("kernel.") => {
                (policy, cmd) == ("workspace_isolation", "ALL")
            }
            _ => false,
        };
        let elsewhere: Vec<_> = kernel
            .iter()
            .filter(|(table, policy, cmd)| !expected(table.as_str(), policy.as_str(), cmd.as_str()))
            .collect();
        assert!(elsewhere.is_empty(), "{elsewhere:?}");
        for (table, policy) in [
            ("kernel.schedules", "kernel_scope_only"),
            ("kernel.rate_limits", "kernel_scope_only"),
            ("kernel.idempotency", "kernel_scope_reads_expired"),
            ("kernel.idempotency", "kernel_scope_prunes_expired"),
        ] {
            assert!(
                kernel.iter().any(|(t, p, _)| t == table && p == policy),
                "{table} `{policy}` admits the kernel scope: {kernel:?}"
            );
        }
        // The idempotency layer's caller: its keys only, through one policy.
        assert_eq!(
            mentioning(testing::idempotency_caller_setting()),
            [(
                "kernel.idempotency".to_owned(),
                "caller_isolation".to_owned(),
                "ALL".to_owned()
            )]
        );
        // The maintenance scope: these ten, on module tables.
        let policies = mentioning(testing::maintenance_scope_setting());
        let expected = [
            // The chats' expiry (assistant, campbooks-cloud#305): which
            // workspaces hold a chat to expire, read only.
            ("assistant.chats", "maintenance_reads_chats", "SELECT"),
            // The IMAP IDLE pools (connectors, campbooks-cloud#318): claim,
            // renew and release leases (holders and times only, a trigger
            // keeps the rest), never insert or delete.
            (
                "connectors.imap_idle_leases",
                "maintenance_claims_idle_leases",
                "UPDATE",
            ),
            (
                "connectors.imap_idle_leases",
                "maintenance_reads_idle_leases",
                "SELECT",
            ),
            // The Microsoft notification callback (connectors,
            // campbooks-cloud#316): finds a subscription's workspace, read
            // only.
            (
                "connectors.microsoft_subscriptions",
                "maintenance_reads_microsoft_subscriptions",
                "SELECT",
            ),
            // The stream dispatcher (connectors): reads and leases.
            (
                "connectors.sync_due",
                "maintenance_leases_schedule",
                "UPDATE",
            ),
            (
                "connectors.sync_due",
                "maintenance_reads_schedule",
                "SELECT",
            ),
            // The needs' clock (conversations, campbooks-cloud#275): which
            // workspaces have a due row, read only.
            (
                "conversations.need_clocks",
                "maintenance_reads_need_clocks",
                "SELECT",
            ),
            // The needs backfill's dispatcher (conversations): reads ids and
            // states, and may only start a row (`WITH CHECK`).
            (
                "conversations.needs_backfills",
                "maintenance_reads_backfills",
                "SELECT",
            ),
            (
                "conversations.needs_backfills",
                "maintenance_starts_backfills",
                "UPDATE",
            ),
            // The re-sign of the rules kept at the deploy (campbooks-cloud#242):
            // which workspaces still have rows, read only.
            (
                "conversations.rule_resigns",
                "maintenance_reads_resigns",
                "SELECT",
            ),
        ]
        .map(|(table, policy, cmd)| (table.to_owned(), policy.to_owned(), cmd.to_owned()));
        assert_eq!(policies, expected);
    }

    /// What a workspace transaction may add of the instance-wide rows
    /// (`kernel.instance_rows_from_workspaces`, read by row-level security)
    /// is what the registry derives: the events declared
    /// `PUBLISHED_IN_WORKSPACES` and their consumers' deliveries
    /// (campbooks-cloud#207). A new such event, or a new consumer of one,
    /// needs a kernel migration adding it to the list.
    #[sqlx::test(migrations = "../../migrations")]
    async fn the_instance_rows_a_workspace_may_add_match_the_registry(db: sqlx::PgPool) {
        let listed: Vec<(String, String)> = sqlx::query_as(
            "SELECT row_kind, name FROM kernel.instance_rows_from_workspaces ORDER BY 1, 2",
        )
        .fetch_all(&db)
        .await
        .unwrap();
        let derived = registry().unwrap().instance_rows_from_workspaces();
        let mut expected: Vec<(String, String)> = derived
            .events
            .iter()
            .map(|name| ("event".to_owned(), (*name).to_owned()))
            .chain(
                derived
                    .kinds
                    .iter()
                    .map(|name| ("job".to_owned(), (*name).to_owned())),
            )
            .collect();
        expected.sort();
        assert_eq!(
            listed, expected,
            "kernel.instance_rows_from_workspaces must list exactly what the registry \
             derives; add (or remove) the rows in a kernel migration"
        );
        // Today: identity's two account events a workspace announces, with
        // no consumer.
        assert_eq!(
            derived.events,
            ["identity.consent_revoked", "identity.way_in_removed"]
        );
    }

    /// Every table of every module (and the kernel) is usable by the runtime
    /// role the server queries as: each migration grants its own. The
    /// `admin` module's tables are the deliberate exception (ADR 0015): the
    /// runtime role may only read profiles and append to the trail, which
    /// the `admin` crate's own tests pin. So is the list of instance-wide
    /// rows a workspace may add (`kernel.instance_rows_from_workspaces`,
    /// campbooks-cloud#207), which it may only read: it changes by migration;
    /// the rate-limit counters (`kernel.rate_limits`, campbooks-cloud#210),
    /// which it reaches only through the kernel's two functions; and the web
    /// sign-in's tables (schema `bff`, campbooks-cloud#212), which it reaches
    /// only through the BFF's functions.
    #[sqlx::test(migrations = "../../migrations")]
    async fn the_runtime_role_may_use_every_table(db: sqlx::PgPool) {
        let role = campbooks_kernel::db::DEFAULT_RUNTIME_ROLE;
        let missing = sqlx::query_scalar!(
            r#"
            SELECT t.schemaname || '.' || t.tablename AS "table!"
              FROM pg_tables t
             WHERE t.schemaname NOT IN ('pg_catalog', 'information_schema', 'public', $2)
               AND NOT (has_schema_privilege($1, t.schemaname, 'USAGE')
                        AND has_table_privilege($1, format('%I.%I', t.schemaname, t.tablename), 'SELECT')
                        AND has_table_privilege($1, format('%I.%I', t.schemaname, t.tablename), 'INSERT')
                        AND has_table_privilege($1, format('%I.%I', t.schemaname, t.tablename), 'UPDATE')
                        AND has_table_privilege($1, format('%I.%I', t.schemaname, t.tablename), 'DELETE'))
             ORDER BY 1
            "#,
            role,
            campbooks_admin::NAME
        )
        .fetch_all(&db)
        .await
        .unwrap();
        let read_only = "kernel.instance_rows_from_workspaces";
        // The rate-limit counters: no privilege at all; the runtime role
        // counts and prunes through the kernel's functions (campbooks-cloud#210).
        let functions_only = "kernel.rate_limits";
        // The snapshot of the rules kept when rules were bound
        // (campbooks-cloud#242): read only; its rows leave through
        // `conversations.take_rule_resign` alone.
        let snapshot = "conversations.rule_resigns";
        // Where each inbox's sends went, and where each trashed message went
        // back to, when destinations were bound (campbooks-cloud#242): read
        // only, written by that migration alone.
        let at_deploy = [
            "connectors.destinations_at_deploy",
            "connectors.restores_at_deploy",
        ];
        // The tracked events outbox (ADR 0020): append only; nothing in the
        // application reads, changes or deletes a row back.
        let append_only = "kernel.tracked_events";
        let missing: Vec<String> = missing
            .into_iter()
            .filter(|t| {
                t != read_only
                    && t != append_only
                    && t != functions_only
                    && t != snapshot
                    && !at_deploy.contains(&t.as_str())
                    && !BFF_TABLES.contains(&t.as_str())
            })
            .collect();
        assert!(
            missing.is_empty(),
            "`{role}` lacks privileges on {missing:?}"
        );
        let privileges: (bool, bool, bool, bool) = sqlx::query_as(
            "SELECT has_table_privilege($1, $2, 'SELECT'), has_table_privilege($1, $2, 'INSERT'),
                    has_table_privilege($1, $2, 'UPDATE'), has_table_privilege($1, $2, 'DELETE')",
        )
        .bind(role)
        .bind(read_only)
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(
            privileges,
            (true, false, false, false),
            "`{role}` only reads {read_only}"
        );
        let privileges: (bool, bool, bool, bool, bool) = sqlx::query_as(
            "SELECT has_table_privilege($1, $2, 'SELECT'), has_table_privilege($1, $2, 'INSERT'),
                    has_table_privilege($1, $2, 'UPDATE'), has_table_privilege($1, $2, 'DELETE'),
                    has_any_column_privilege($1, $2, 'SELECT, UPDATE')",
        )
        .bind(role)
        .bind(append_only)
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(
            privileges,
            (false, true, false, false, false),
            "`{role}` only appends to {append_only}"
        );
        // The relay, the prune, the backlog sample and the purge reach it
        // through the kernel's functions only (campbooks-cloud#321).
        for function in OUTBOX_FUNCTIONS {
            let granted: bool =
                sqlx::query_scalar("SELECT has_function_privilege($1, $2, 'EXECUTE')")
                    .bind(role)
                    .bind(function)
                    .fetch_one(&db)
                    .await
                    .unwrap();
            assert!(granted, "`{role}` may call {function}");
        }
        for table in at_deploy {
            let privileges: (bool, bool, bool, bool, bool) = sqlx::query_as(
                "SELECT has_table_privilege($1, $2, 'SELECT'), has_table_privilege($1, $2, 'INSERT'),
                        has_table_privilege($1, $2, 'UPDATE'), has_table_privilege($1, $2, 'DELETE'),
                        has_any_column_privilege($1, $2, 'INSERT, UPDATE')",
            )
            .bind(role)
            .bind(table)
            .fetch_one(&db)
            .await
            .unwrap();
            assert_eq!(
                privileges,
                (true, false, false, false, false),
                "`{role}` only reads {table}"
            );
        }
        let privileges: (bool, bool, bool, bool, bool) = sqlx::query_as(
            "SELECT has_table_privilege($1, $2, 'SELECT'), has_table_privilege($1, $2, 'INSERT'),
                    has_table_privilege($1, $2, 'UPDATE'), has_table_privilege($1, $2, 'DELETE'),
                    has_function_privilege($1, 'conversations.take_rule_resign(uuid)', 'EXECUTE')",
        )
        .bind(role)
        .bind(snapshot)
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(
            privileges,
            (true, false, false, false, true),
            "`{role}` only reads {snapshot}, and takes rows out through its function"
        );
        let privileges: (bool, bool, bool, bool, bool, bool, bool) = sqlx::query_as(
            "SELECT has_table_privilege($1, $2, 'SELECT'), has_table_privilege($1, $2, 'INSERT'),
                    has_table_privilege($1, $2, 'UPDATE'), has_table_privilege($1, $2, 'DELETE'),
                    has_any_column_privilege($1, $2, 'SELECT, INSERT, UPDATE, REFERENCES'),
                    has_function_privilege($1,
                        'kernel.take_rate_limit(bytea, double precision, integer)', 'EXECUTE'),
                    has_function_privilege($1, 'kernel.prune_rate_limits(integer)', 'EXECUTE')",
        )
        .bind(role)
        .bind(functions_only)
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(
            privileges,
            (false, false, false, false, false, true, true),
            "`{role}` reaches {functions_only} through the kernel's functions only"
        );
        // The web sign-in's tables: no privilege at all, of any kind, on the
        // table or on any of its columns.
        for table in BFF_TABLES {
            let any: bool = sqlx::query_scalar(
                "SELECT has_table_privilege($1, $2,
                            'SELECT, INSERT, UPDATE, DELETE, TRUNCATE, REFERENCES, TRIGGER')
                     OR has_any_column_privilege($1, $2, 'SELECT, INSERT, UPDATE, REFERENCES')",
            )
            .bind(role)
            .bind(table)
            .fetch_one(&db)
            .await
            .unwrap();
            assert!(!any, "`{role}` holds a privilege on {table}");
        }
        // And exactly these functions to reach them.
        let functions: Vec<String> = sqlx::query_scalar(
            "SELECT p.proname::text
               FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace
              WHERE n.nspname = 'bff' AND has_function_privilege($1, p.oid, 'EXECUTE')
              ORDER BY 1",
        )
        .bind(role)
        .fetch_all(&db)
        .await
        .unwrap();
        assert_eq!(
            functions, BFF_FUNCTIONS,
            "`{role}` reaches schema `bff` through these"
        );
    }

    /// The functions the runtime role reaches the tracked events outbox
    /// through (campbooks-cloud#321).
    const OUTBOX_FUNCTIONS: [&str; 5] = [
        "kernel.claim_tracked_events(integer)",
        "kernel.mark_tracked_events_published(uuid[])",
        "kernel.tracked_events_backlog()",
        "kernel.prune_tracked_events(integer, integer)",
        "kernel.purge_workspace_tracked_events(text, integer)",
    ];

    /// The web sign-in's tables (campbooks-cloud#212).
    const BFF_TABLES: [&str; 3] = [
        "bff.login_transactions",
        "bff.retired_tokens",
        "bff.sessions",
    ];

    /// The functions the BFF reaches them through, in name order: one per
    /// operation it needs, each on the one session (or sign-in, or account)
    /// its arguments name; only the prunes reach across accounts.
    const BFF_FUNCTIONS: [&str; 15] = [
        "create_session",
        "end_account_sessions",
        "end_session",
        "find_session",
        "lock_session_tokens",
        "mark_session_delivered",
        "prune_logins",
        "prune_sessions",
        "reissue_session",
        "retired_session",
        "rotate_session",
        "start_login",
        "store_session_tokens",
        "take_login",
        "touch_session",
    ];

    /// Every `SECURITY DEFINER` function runs with the pinned path
    /// `pg_catalog, pg_temp` (so a caller's objects cannot shadow a name in
    /// its body) and is not executable by `PUBLIC`: the runtime role is
    /// granted each one explicitly (campbooks-cloud#210, #212).
    #[sqlx::test(migrations = "../../migrations")]
    async fn every_definer_function_pins_its_path_and_is_granted(db: sqlx::PgPool) {
        let functions: Vec<(String, Option<Vec<String>>, bool)> = sqlx::query_as(
            "SELECT p.oid::regprocedure::text, p.proconfig,
                    has_function_privilege('public', p.oid, 'EXECUTE')
               FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace
              WHERE p.prosecdef AND n.nspname NOT IN ('pg_catalog', 'information_schema')
              ORDER BY 1",
        )
        .fetch_all(&db)
        .await
        .unwrap();
        assert!(functions.len() >= BFF_FUNCTIONS.len() + 2, "{functions:?}");
        for (function, config, public) in functions {
            assert_eq!(
                config,
                Some(vec!["search_path=pg_catalog, pg_temp".to_owned()]),
                "{function} pins its search_path"
            );
            assert!(!public, "{function} is executable by PUBLIC");
        }
    }

    /// Every table of the kernel's and the BFF's schemas has row-level
    /// security, enabled and forced with a policy, or grants the runtime role
    /// no privilege at all (reached through functions only), or is listed
    /// here as instance-only with the reason (campbooks-cloud#210, #212): a
    /// new table there cannot be left open to every transaction by omission.
    /// A listed table grants the runtime role no write.
    #[sqlx::test(migrations = "../../migrations")]
    async fn every_kernel_table_has_row_level_security_or_a_reason(db: sqlx::PgPool) {
        const INSTANCE_ONLY: [(&str, &str); 1] = [(
            "kernel.instance_rows_from_workspaces",
            "the instance-wide rows a workspace may add, read by the queue's and the event \
             log's policies: written by migrations only, the runtime role may only read it, and \
             it holds nothing of any caller or workspace",
        )];
        let role = campbooks_kernel::db::DEFAULT_RUNTIME_ROLE;
        let tables: Vec<(String, bool)> = sqlx::query_as(KERNEL_TABLES_PROTECTED)
            .bind(role)
            .fetch_all(&db)
            .await
            .unwrap();
        for table in BFF_TABLES {
            assert!(
                tables.iter().any(|(t, _)| t == table),
                "{table} is covered: {tables:?}"
            );
        }
        assert!(tables.len() > INSTANCE_ONLY.len(), "{tables:?}");
        let open: Vec<&str> = tables
            .iter()
            .filter(|(_, protected)| !protected)
            .map(|(table, _)| table.as_str())
            .collect();
        let listed: Vec<&str> = INSTANCE_ONLY.iter().map(|(table, _)| *table).collect();
        assert_eq!(
            open, listed,
            "every kernel and BFF table enables and forces row-level security with a policy, \
             grants the runtime role nothing, or is listed as instance-only with its reason"
        );
        for (table, reason) in INSTANCE_ONLY {
            assert!(!reason.trim().is_empty(), "{table} needs a reason");
            let writes: bool = sqlx::query_scalar(
                "SELECT has_table_privilege($1, $2, 'INSERT')
                     OR has_table_privilege($1, $2, 'UPDATE')
                     OR has_table_privilege($1, $2, 'DELETE')
                     OR has_table_privilege($1, $2, 'TRUNCATE')
                     OR has_any_column_privilege($1, $2, 'INSERT, UPDATE')",
            )
            .bind(role)
            .bind(table)
            .fetch_one(&db)
            .await
            .unwrap();
            assert!(
                !writes,
                "`{role}` may write {table}, which has no row-level security"
            );
        }
    }

    /// Whether each table, view and materialized view of the kernel's and
    /// the BFF's schemas is protected from the runtime role (`$1`): a table
    /// with row-level security enabled and forced, a policy, and no
    /// permissive policy for that role (or everyone) that admits every row,
    /// to read or to write: one with no expression, or an expression that
    /// does not depend on the row (reads no column, calls no function, holds
    /// no subquery or parameter: `USING (true)`, `USING (1 = 1)`) and is not
    /// a lone `false`, on either side (campbooks-cloud#260); or no privilege
    /// of any
    /// kind on the relation or on any of its columns (`has_table_privilege`
    /// does not see a column's grant). Row-level security does not apply
    /// through a view, so a view is protected by having no grant
    /// (campbooks-cloud#213, item 7).
    const KERNEL_TABLES_PROTECTED: &str = "
        SELECT n.nspname || '.' || c.relname,
               (c.relkind IN ('r', 'p') AND c.relrowsecurity AND c.relforcerowsecurity
                   AND EXISTS (SELECT 1 FROM pg_policy p WHERE p.polrelid = c.oid)
                   AND NOT EXISTS (
                       SELECT 1 FROM pg_policy p
                        WHERE p.polrelid = c.oid AND p.polpermissive
                          AND (0 = ANY (p.polroles)
                               OR EXISTS (SELECT 1 FROM unnest(p.polroles) AS r(oid)
                                           WHERE r.oid <> 0
                                             AND pg_has_role($1, r.oid, 'MEMBER')))
                          AND ((p.polqual IS NULL AND p.polwithcheck IS NULL)
                               OR (p.polqual IS NOT NULL
                                   AND p.polqual::text !~ '[{](VAR|FUNCEXPR|SUBLINK|PARAM) '
                                   AND pg_get_expr(p.polqual, p.polrelid) <> 'false')
                               OR (p.polwithcheck IS NOT NULL
                                   AND p.polwithcheck::text
                                       !~ '[{](VAR|FUNCEXPR|SUBLINK|PARAM) '
                                   AND pg_get_expr(p.polwithcheck, p.polrelid) <> 'false'))))
               OR NOT (has_table_privilege($1, c.oid,
                           'SELECT, INSERT, UPDATE, DELETE, TRUNCATE, REFERENCES, TRIGGER')
                       OR has_any_column_privilege($1, c.oid,
                           'SELECT, INSERT, UPDATE, REFERENCES'))
          FROM pg_class c
          JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE n.nspname IN ('kernel', 'bff') AND c.relkind IN ('r', 'p', 'v', 'm')
         ORDER BY 1";

    /// A view the runtime role may read, and a policy admitting the runtime
    /// role to every row, leave the kernel open; a policy for another role
    /// or a restrictive one does not (campbooks-cloud#213, item 7).
    #[sqlx::test(migrations = "../../migrations")]
    async fn an_open_view_or_policy_leaves_the_kernel_open(db: sqlx::PgPool) {
        let role = campbooks_kernel::db::DEFAULT_RUNTIME_ROLE;
        let protected = |tables: &[(String, bool)], name: &str| {
            tables
                .iter()
                .find(|(t, _)| t == name)
                .map(|(_, protected)| *protected)
        };
        for statement in [
            "CREATE VIEW kernel.peek AS SELECT principal FROM kernel.idempotency",
            "CREATE MATERIALIZED VIEW kernel.peek_later AS SELECT 1 AS one",
            "GRANT SELECT ON kernel.peek_later TO campbooks_app",
            "CREATE POLICY only_for_the_definer ON kernel.idempotency TO campbooks_definer
                 USING (true)",
            "CREATE POLICY never_more ON kernel.idempotency AS RESTRICTIVE TO campbooks_app
                 USING (true)",
        ] {
            sqlx::query(statement).execute(&db).await.unwrap();
        }
        let tables: Vec<(String, bool)> = sqlx::query_as(KERNEL_TABLES_PROTECTED)
            .bind(role)
            .fetch_all(&db)
            .await
            .unwrap();
        assert_eq!(
            protected(&tables, "kernel.peek"),
            Some(true),
            "no grant yet"
        );
        assert_eq!(protected(&tables, "kernel.peek_later"), Some(false));
        assert_eq!(protected(&tables, "kernel.idempotency"), Some(true));

        for statement in [
            "GRANT SELECT ON kernel.peek TO campbooks_app",
            "CREATE POLICY everyone ON kernel.idempotency FOR SELECT USING (true)",
        ] {
            sqlx::query(statement).execute(&db).await.unwrap();
        }
        let tables: Vec<(String, bool)> = sqlx::query_as(KERNEL_TABLES_PROTECTED)
            .bind(role)
            .fetch_all(&db)
            .await
            .unwrap();
        assert_eq!(protected(&tables, "kernel.peek"), Some(false));
        assert_eq!(protected(&tables, "kernel.idempotency"), Some(false));

        // Any constant expression admits every row, not only `true`, and
        // either side open is open: a policy reading every row whose writes
        // are checked still lets every row be read (campbooks-cloud#260). A
        // lone `false` admits nothing.
        let after = |statements: &'static [&'static str]| {
            let db = db.clone();
            async move {
                for &statement in statements {
                    sqlx::query(statement).execute(&db).await.unwrap();
                }
                let tables: Vec<(String, bool)> = sqlx::query_as(KERNEL_TABLES_PROTECTED)
                    .bind(role)
                    .fetch_all(&db)
                    .await
                    .unwrap();
                protected(&tables, "kernel.idempotency")
            }
        };
        assert_eq!(
            after(&[
                "DROP POLICY everyone ON kernel.idempotency",
                "CREATE POLICY tautology ON kernel.idempotency FOR SELECT TO campbooks_app
                     USING (1 = 1)",
            ])
            .await,
            Some(false)
        );
        assert_eq!(
            after(&[
                "DROP POLICY tautology ON kernel.idempotency",
                "CREATE POLICY read_open ON kernel.idempotency FOR ALL TO campbooks_app
                     USING (true) WITH CHECK (principal = current_user)",
            ])
            .await,
            Some(false),
            "readable by everyone, whatever its writes"
        );
        assert_eq!(
            after(&[
                "DROP POLICY read_open ON kernel.idempotency",
                "CREATE POLICY closed ON kernel.idempotency FOR SELECT TO campbooks_app
                     USING (false)",
            ])
            .await,
            Some(true),
            "`false` admits nothing"
        );
    }

    /// A grant on one column of a BFF table is a privilege the coverage
    /// test sees (campbooks-cloud#214, item 6).
    #[sqlx::test(migrations = "../../migrations")]
    async fn a_column_grant_leaves_a_table_open(db: sqlx::PgPool) {
        let role = campbooks_kernel::db::DEFAULT_RUNTIME_ROLE;
        sqlx::query("GRANT SELECT (subject) ON bff.sessions TO campbooks_app")
            .execute(&db)
            .await
            .unwrap();
        let tables: Vec<(String, bool)> = sqlx::query_as(KERNEL_TABLES_PROTECTED)
            .bind(role)
            .fetch_all(&db)
            .await
            .unwrap();
        assert!(
            tables.contains(&("bff.sessions".to_owned(), false)),
            "{tables:?}"
        );
    }

    /// The role every `SECURITY DEFINER` function belongs to
    /// (campbooks-cloud#214).
    const DEFINER_ROLE: &str = "campbooks_definer";

    /// Every `SECURITY DEFINER` function belongs to the dedicated owner role,
    /// never to a superuser or a role that can log in (such as the migration
    /// login), so a bug in a body runs with that role's few rights
    /// (campbooks-cloud#214). The role cannot log in, holds no privileged
    /// attribute, is a member of no role, and no runtime role is a member of
    /// it; it owns nothing but the functions and holds exactly the
    /// privileges their statements need. A migration adding a definer
    /// function hands it over (`ALTER FUNCTION … OWNER TO
    /// campbooks_definer`), or this fails naming the function.
    #[sqlx::test(migrations = "../../migrations")]
    async fn definer_functions_belong_to_the_nologin_owner_role(db: sqlx::PgPool) {
        let owners: Vec<(String, String, bool, bool)> = sqlx::query_as(
            "SELECT p.oid::regprocedure::text, r.rolname::text, r.rolsuper, r.rolcanlogin
               FROM pg_proc p
               JOIN pg_namespace n ON n.oid = p.pronamespace
               JOIN pg_roles r ON r.oid = p.proowner
              WHERE p.prosecdef AND n.nspname NOT IN ('pg_catalog', 'information_schema')
              ORDER BY 1",
        )
        .fetch_all(&db)
        .await
        .unwrap();
        assert!(owners.len() >= BFF_FUNCTIONS.len() + 2, "{owners:?}");
        for (function, owner, superuser, login) in &owners {
            assert!(
                !superuser && !login,
                "{function} belongs to `{owner}`, a superuser or a role that can log in"
            );
            assert_eq!(owner, DEFINER_ROLE, "{function} belongs to the owner role");
        }

        let attributes: (bool, bool, bool, bool, bool, bool, i64) = sqlx::query_as(
            "SELECT r.rolsuper, r.rolcanlogin, r.rolbypassrls, r.rolcreatedb, r.rolcreaterole,
                    r.rolreplication,
                    (SELECT count(*) FROM pg_auth_members m WHERE m.member = r.oid)
               FROM pg_roles r
              WHERE r.rolname = $1",
        )
        .bind(DEFINER_ROLE)
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(
            attributes,
            (false, false, false, false, false, false, 0),
            "`{DEFINER_ROLE}`: no login, no privileged attribute, a member of no role"
        );
        let runtime_member: bool = sqlx::query_scalar("SELECT pg_has_role($1, $2, 'MEMBER')")
            .bind(campbooks_kernel::db::DEFAULT_RUNTIME_ROLE)
            .bind(DEFINER_ROLE)
            .fetch_one(&db)
            .await
            .unwrap();
        assert!(
            !runtime_member,
            "the runtime role cannot act as `{DEFINER_ROLE}`"
        );

        let owned: Vec<String> = sqlx::query_scalar(
            "SELECT format('relation %s', c.oid::regclass) FROM pg_class c
              WHERE c.relowner = $1::text::regrole
             UNION ALL
             SELECT format('schema %I', n.nspname) FROM pg_namespace n
              WHERE n.nspowner = $1::text::regrole
             UNION ALL
             SELECT format('function %s', p.oid::regprocedure) FROM pg_proc p
              WHERE p.proowner = $1::text::regrole AND NOT p.prosecdef
             ORDER BY 1",
        )
        .bind(DEFINER_ROLE)
        .fetch_all(&db)
        .await
        .unwrap();
        assert!(owned.is_empty(), "`{DEFINER_ROLE}` owns {owned:?}");

        let tables: Vec<(String, String)> = sqlx::query_as(
            "SELECT n.nspname || '.' || c.relname,
                    string_agg(a.privilege_type, ', ' ORDER BY a.privilege_type)
               FROM pg_class c
               JOIN pg_namespace n ON n.oid = c.relnamespace,
                    aclexplode(c.relacl) a
              WHERE a.grantee = $1::text::regrole
              GROUP BY 1
              ORDER BY 1",
        )
        .bind(DEFINER_ROLE)
        .fetch_all(&db)
        .await
        .unwrap();
        let expected = [
            ("bff.login_transactions", "DELETE, INSERT, SELECT"),
            ("bff.retired_tokens", "INSERT, SELECT"),
            ("bff.sessions", "DELETE, INSERT, SELECT, UPDATE"),
            // `conversations.take_rule_resign` (campbooks-cloud#242).
            ("conversations.rule_resigns", "DELETE, SELECT"),
            ("conversations.rules", "SELECT"),
            // `kernel.purge_workspace_idempotency` (campbooks-cloud#65).
            ("kernel.idempotency", "DELETE, SELECT"),
            ("kernel.rate_limits", "DELETE, INSERT, SELECT, UPDATE"),
            // The tracked events outbox's functions (campbooks-cloud#321).
            ("kernel.tracked_events", "DELETE, SELECT, UPDATE"),
        ]
        .map(|(table, privileges)| (table.to_owned(), privileges.to_owned()));
        assert_eq!(
            tables, expected,
            "`{DEFINER_ROLE}` holds what the functions need"
        );
        let schemas: Vec<(String, String)> = sqlx::query_as(
            "SELECT n.nspname::text, a.privilege_type
               FROM pg_namespace n, aclexplode(n.nspacl) a
              WHERE a.grantee = $1::text::regrole
              ORDER BY 1, 2",
        )
        .bind(DEFINER_ROLE)
        .fetch_all(&db)
        .await
        .unwrap();
        assert_eq!(
            schemas,
            [
                ("bff", "USAGE"),
                ("conversations", "USAGE"),
                ("kernel", "USAGE")
            ]
            .map(|(schema, privilege)| (schema.to_owned(), privilege.to_owned())),
            "`{DEFINER_ROLE}` may use the three schemas, and create nothing"
        );
        let elsewhere: i64 = sqlx::query_scalar(
            "SELECT (SELECT count(*) FROM pg_attribute a, aclexplode(a.attacl) x
                      WHERE x.grantee = $1::text::regrole)
                  + (SELECT count(*) FROM pg_proc p, aclexplode(p.proacl) x
                      WHERE x.grantee = $1::text::regrole AND p.proowner <> x.grantee)
                  + (SELECT count(*) FROM pg_database d, aclexplode(d.datacl) x
                      WHERE x.grantee = $1::text::regrole)",
        )
        .bind(DEFINER_ROLE)
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(elsewhere, 0, "no column, function or database grant");

        // The functions still work for the runtime role, as their owner.
        let runtime = testing::app_role_pool(&db).await;
        let hits: Vec<i32> = sqlx::query_scalar(
            "SELECT hits FROM kernel.take_rate_limit(sha256('a bucket'::bytea), 60, 1)
             UNION ALL
             SELECT hits FROM kernel.take_rate_limit(sha256('a bucket'::bytea), 60, 1)",
        )
        .fetch_all(&runtime)
        .await
        .unwrap();
        assert_eq!(hits.iter().max(), Some(&2), "{hits:?}");
        sqlx::query(
            "SELECT bff.start_login(sha256('a state'::bytea), sha256('a browser'::bytea),
                                    '\\x01', '\\x02', '/inbox', now(),
                                    now() + interval '10 minutes', NULL)",
        )
        .execute(&runtime)
        .await
        .unwrap();
        let landing: Option<String> =
            sqlx::query_scalar("SELECT return_to FROM bff.take_login(sha256('a state'::bytea))")
                .fetch_optional(&runtime)
                .await
                .unwrap();
        assert_eq!(landing.as_deref(), Some("/inbox"));
    }

    /// The worker runs as the runtime role in production: it can claim, run
    /// and complete kernel jobs.
    #[sqlx::test(migrations = "../../migrations")]
    async fn the_worker_runs_as_the_runtime_role(db: sqlx::PgPool) {
        use campbooks_kernel::jobs::{self, EnqueueOptions, PruneJobs, Worker};

        let pool = testing::app_role_pool(&db).await;
        let state = with_ports(
            AppState::new(
                pool.clone(),
                testing::config("postgres://test@localhost/test"),
            )
            .with_registry(registry().unwrap()),
        );
        let mut tx = pool.begin().await.unwrap();
        jobs::enqueue(&mut tx, &PruneJobs, EnqueueOptions::default())
            .await
            .unwrap();
        tx.commit().await.unwrap();
        let stats = Worker::new(state).run_until_idle().await;
        assert_eq!(stats.succeeded, 1, "{stats:?}");
    }

    #[test]
    fn registry_builds() {
        let registry = registry().unwrap();
        assert!(registry.kind_names().contains(&"kernel.prune_jobs"));
        assert!(registry.kind_names().contains(&"kernel.prune_idempotency"));
    }

    /// Every schedule's kind runs instance-wide only: the worker refuses a
    /// workspace's job of it (campbooks-cloud#211).
    #[test]
    fn every_schedule_is_instance_only() {
        let registry = registry().unwrap();
        let instance_only = registry.instance_only_kinds();
        let schedules = registry.schedule_names();
        assert!(
            schedules.contains(&"connectors.dispatch_due"),
            "{schedules:?}"
        );
        for schedule in schedules {
            assert!(
                instance_only.contains(&schedule),
                "{schedule} is not instance-only: {instance_only:?}"
            );
        }
    }

    #[test]
    fn events_catalogue_lists_published_events() {
        let catalogue = serde_json::to_value(events_catalogue().unwrap()).unwrap();
        let names: Vec<&str> = catalogue["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|event| event["name"].as_str())
            .collect();
        assert!(names.contains(&"workspaces.workspace_created"), "{names:?}");
        let created = &catalogue["events"][names
            .iter()
            .position(|n| *n == "workspaces.workspace_created")
            .unwrap()];
        assert_eq!(created["module"], "workspaces");
        assert_eq!(created["version"], 1);
        assert_eq!(created["workspaceScoped"], true);
        assert!(created["payload"]["properties"]["owner"].is_object());
    }

    #[tokio::test]
    async fn operations_are_invocable_by_id_and_gated_by_their_metadata() {
        let composed = operations::compose(api(), testing::state(testing::unreachable_pool()));
        let catalogue = composed.state.operations.catalogue().unwrap();
        assert!(
            catalogue
                .get("system.health")
                .is_some_and(|op| !op.mutates())
        );
        let principal = campbooks_kernel::auth::Principal {
            subject: "user-1".to_owned(),
            kind: campbooks_kernel::auth::PrincipalKind::User,
            client_id: None,
            scopes: std::collections::BTreeSet::new(),
            workspace: None,
            email: None,
            email_verified: false,
            name: None,
            expires_at: None,
            issued_at: None,
            auth_time: None,
            sign_in: None,
        };
        // Probes opt neither automation nor the assistant in.
        let refused = operations::invoke(
            &composed.state,
            operations::Invocation::new(
                operations::Caller::Assistant,
                principal,
                "system.version",
                serde_json::Value::Null,
            ),
        )
        .await;
        assert!(matches!(
            refused,
            Err(operations::InvokeError::NotAllowed { .. })
        ));
    }

    #[test]
    fn catalogue_is_at_parity() {
        assert_eq!(MESSAGES.check(), Vec::<String>::new());
    }

    #[test]
    fn module_names_are_unique() {
        let mut names: Vec<_> = modules().iter().map(|m| m.name).collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(names.len(), before);
    }

    #[tokio::test]
    async fn serves_openapi_json() {
        let response = testing::get(&app(), OPENAPI_PATH).await;
        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(response.json["info"]["title"], "Campbooks API");
    }

    #[tokio::test]
    async fn unknown_routes_are_problem_json_404() {
        let response = testing::get(&app(), "/api/v1/nope").await;
        assert_eq!(response.status, StatusCode::NOT_FOUND);
        assert_eq!(
            response.content_type.as_deref(),
            Some("application/problem+json")
        );
    }

    async fn localized(
        uri: &str,
        accept_language: &str,
    ) -> (StatusCode, Option<String>, serde_json::Value) {
        let request = Request::get(uri)
            .header(header::ACCEPT_LANGUAGE, accept_language)
            .body(Body::empty())
            .unwrap();
        let response = app().oneshot(request).await.unwrap();
        let status = response.status();
        let language = response
            .headers()
            .get(header::CONTENT_LANGUAGE)
            .map(|v| v.to_str().unwrap().to_owned());
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, language, serde_json::from_slice(&bytes).unwrap())
    }

    #[tokio::test]
    async fn fallback_404s_speak_the_callers_language() {
        for uri in ["/api/v1/nope", "/nope"] {
            let (status, language, body) = localized(uri, "es-MX,es;q=0.9").await;
            assert_eq!(status, StatusCode::NOT_FOUND);
            assert_eq!(language.as_deref(), Some("es-ES"));
            assert_eq!(body["code"], "not_found");
            assert_eq!(body["title"], "No encontrado");
        }
    }

    #[tokio::test]
    async fn bff_problems_speak_the_callers_language() {
        let (status, language, body) = localized("/bff/login", "pt-BR").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(language.as_deref(), Some("pt-PT"));
        assert_eq!(body["code"], "bff_not_configured");
        assert_eq!(body["title"], "Serviço indisponível");
    }

    #[tokio::test]
    async fn token_errors_speak_the_callers_language() {
        let (status, language, body) = localized("/api/v1/workspaces", "fr-CA").await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(language.as_deref(), Some("fr"));
        assert_eq!(body["code"], "auth_not_configured");
        assert_eq!(body["title"], "Non authentifié");
    }

    #[tokio::test]
    async fn propagates_or_generates_request_ids() {
        let request = Request::get("/api/v1/system/version")
            .header(REQUEST_ID_HEADER, "abc-123")
            .body(Body::empty())
            .unwrap();
        let response = app().oneshot(request).await.unwrap();
        assert_eq!(response.headers()[REQUEST_ID_HEADER], "abc-123");

        let request = Request::get("/api/v1/system/version")
            .body(Body::empty())
            .unwrap();
        let response = app().oneshot(request).await.unwrap();
        assert!(!response.headers()[REQUEST_ID_HEADER].is_empty());
    }

    // --- web client and interim BFF passthrough --------------------------------

    const INDEX: &str = "<!doctype html><title>Campbooks</title>";
    const ASSET: &str = "console.log('campbooks');";

    /// A built web client in a fresh temporary directory, removed on drop.
    struct WebDir(std::path::PathBuf);

    impl WebDir {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let dir =
                std::env::temp_dir().join(format!("campbooks-web-test-{}-{n}", std::process::id()));
            std::fs::create_dir_all(dir.join("assets")).unwrap();
            std::fs::write(dir.join("index.html"), INDEX).unwrap();
            std::fs::write(dir.join("assets/x.js"), ASSET).unwrap();
            Self(dir)
        }
    }

    impl Drop for WebDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn web_app(dir: &WebDir) -> Router {
        let mut config = testing::config("postgres://test@localhost/test");
        config.web_dir = Some(dir.0.clone());
        router(AppState::new(testing::unreachable_pool(), config), None)
    }

    struct Raw {
        status: StatusCode,
        headers: axum::http::HeaderMap,
        body: String,
    }

    async fn fetch(app: &Router, uri: &str, accept: Option<&str>) -> Raw {
        let mut request = Request::get(uri);
        if let Some(accept) = accept {
            request = request.header(header::ACCEPT, accept);
        }
        let response = app
            .clone()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        Raw {
            status,
            headers,
            body: String::from_utf8(bytes.to_vec()).unwrap(),
        }
    }

    const HTML: Option<&str> = Some("text/html,application/xhtml+xml,*/*;q=0.8");

    fn header<'a>(raw: &'a Raw, name: &header::HeaderName) -> Option<&'a str> {
        raw.headers.get(name).and_then(|v| v.to_str().ok())
    }

    fn assert_problem_404(raw: &Raw) {
        assert_eq!(raw.status, StatusCode::NOT_FOUND, "{}", raw.body);
        assert_eq!(
            header(raw, &header::CONTENT_TYPE),
            Some("application/problem+json")
        );
    }

    fn assert_index(raw: &Raw) {
        assert_eq!(raw.status, StatusCode::OK);
        assert_eq!(raw.body, INDEX);
        assert!(
            header(raw, &header::CONTENT_TYPE)
                .unwrap()
                .starts_with("text/html")
        );
        assert_eq!(
            header(raw, &header::CACHE_CONTROL),
            Some(web::CACHE_REVALIDATE)
        );
        assert_eq!(
            header(raw, &header::CONTENT_SECURITY_POLICY),
            Some(web::CONTENT_SECURITY_POLICY)
        );
    }

    #[tokio::test]
    async fn serves_index_at_root() {
        let dir = WebDir::new();
        assert!(web::check_dir(&dir.0).is_ok());
        assert_index(&fetch(&web_app(&dir), "/", None).await);
    }

    #[tokio::test]
    async fn deep_links_fall_back_to_index() {
        let dir = WebDir::new();
        assert_index(&fetch(&web_app(&dir), "/some/route", HTML).await);
    }

    #[tokio::test]
    async fn non_navigation_misses_are_404() {
        let dir = WebDir::new();
        let app = web_app(&dir);
        assert_problem_404(&fetch(&app, "/favicon.ico", Some("image/*")).await);
        assert_problem_404(&fetch(&app, "/some/route", Some("*/*")).await);
        let post = app
            .clone()
            .oneshot(
                Request::post("/some/route")
                    .header(header::ACCEPT, "text/html")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(post.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn hashed_assets_are_immutable() {
        let dir = WebDir::new();
        let raw = fetch(&web_app(&dir), "/assets/x.js", None).await;
        assert_eq!(raw.status, StatusCode::OK);
        assert_eq!(raw.body, ASSET);
        assert_eq!(
            header(&raw, &header::CACHE_CONTROL),
            Some(web::CACHE_IMMUTABLE)
        );
        assert_eq!(header(&raw, &header::CONTENT_SECURITY_POLICY), None);
    }

    #[tokio::test]
    async fn missing_assets_are_404_not_index() {
        let dir = WebDir::new();
        assert_problem_404(&fetch(&web_app(&dir), "/assets/missing.js", HTML).await);
    }

    #[tokio::test]
    async fn api_paths_never_fall_back_to_index() {
        let dir = WebDir::new();
        let app = web_app(&dir);
        for uri in [
            "/api/v1/nope",
            "/api",
            "/bff/nope",
            "/bff/api",
            "/bff/openapi.json",
        ] {
            assert_problem_404(&fetch(&app, uri, HTML).await);
        }
        let spec = fetch(&app, OPENAPI_PATH, HTML).await;
        assert_eq!(spec.status, StatusCode::OK);
    }

    #[tokio::test]
    async fn bff_routes_are_served_next_to_the_web_client() {
        for app in [app(), web_app(&WebDir::new())] {
            // Web sign-in is off in these apps: the BFF says so, as problem+json.
            let raw = fetch(&app, "/bff/api/v1/system/version", HTML).await;
            assert_eq!(raw.status, StatusCode::SERVICE_UNAVAILABLE, "{}", raw.body);
            assert_eq!(
                header(&raw, &header::CONTENT_TYPE),
                Some("application/problem+json")
            );
            let session = fetch(&app, "/bff/session", None).await;
            assert_eq!(session.status, StatusCode::OK);
            assert_eq!(session.body, r#"{"authenticated":false}"#);
        }
    }

    #[tokio::test]
    async fn security_headers_on_every_response() {
        let dir = WebDir::new();
        let web = web_app(&dir);
        for raw in [
            fetch(&app(), "/api/v1/system/version", None).await,
            fetch(&app(), "/api/v1/nope", None).await,
            fetch(&web, "/", HTML).await,
            fetch(&web, "/assets/x.js", None).await,
        ] {
            for (name, value) in &SECURITY_HEADERS {
                assert_eq!(header(&raw, name), Some(*value), "{name}");
            }
        }
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn up_answers_like_the_health_check(db: sqlx::PgPool) {
        let dir = WebDir::new();
        let mut config = testing::config("postgres://test@localhost/test");
        config.web_dir = Some(dir.0.clone());
        let app = router(AppState::new(db, config), None);
        let health = fetch(&app, "/api/v1/system/health", None).await;
        // A browser navigation still gets the health answer, not the web client.
        for accept in [None, HTML] {
            let up = fetch(&app, UP_PATH, accept).await;
            assert_eq!(up.status, StatusCode::OK, "{}", up.body);
            assert_eq!(up.body, health.body);
            assert_eq!(up.body, r#"{"status":"ok","database":"ok"}"#);
            assert_eq!(header(&up, &header::CACHE_CONTROL), Some("no-store"));
            assert_eq!(header(&up, &header::PRAGMA), Some("no-cache"));
        }
    }

    #[tokio::test]
    async fn up_is_a_503_problem_when_the_database_is_down() {
        let dir = WebDir::new();
        for app in [app(), web_app(&dir)] {
            let up = fetch(&app, UP_PATH, HTML).await;
            assert_eq!(up.status, StatusCode::SERVICE_UNAVAILABLE, "{}", up.body);
            assert_eq!(
                header(&up, &header::CONTENT_TYPE),
                Some("application/problem+json")
            );
            assert_eq!(header(&up, &header::CACHE_CONTROL), Some("no-store"));
            let problem: serde_json::Value = serde_json::from_str(&up.body).unwrap();
            assert_eq!(problem["code"], "database_unreachable");
        }
    }

    #[tokio::test]
    async fn up_is_not_in_the_openapi_document() {
        let spec = fetch(&app(), OPENAPI_PATH, None).await;
        let spec: serde_json::Value = serde_json::from_str(&spec.body).unwrap();
        assert!(spec["paths"].get(UP_PATH).is_none());
        assert!(spec["paths"].get("/api/v1/system/health").is_some());
    }

    #[tokio::test]
    async fn api_responses_are_never_stored() {
        let dir = WebDir::new();
        let web = web_app(&dir);
        for (raw, uri) in [
            (fetch(&app(), "/api/v1/system/version", None).await, "api"),
            (fetch(&app(), "/api/v1/nope", None).await, "api 404"),
            (
                fetch(&web, "/bff/api/v1/system/version", None).await,
                "bff api",
            ),
            (fetch(&web, "/bff/session", None).await, "bff session"),
            (
                fetch(&web, "/admin/v1/system/overview", None).await,
                "admin",
            ),
        ] {
            assert_eq!(
                header(&raw, &header::CACHE_CONTROL),
                Some("no-store"),
                "{uri}"
            );
            assert_eq!(header(&raw, &header::PRAGMA), Some("no-cache"), "{uri}");
        }
        // The web client's own files keep their caching.
        for raw in [
            fetch(&web, "/", HTML).await,
            fetch(&web, "/assets/x.js", None).await,
        ] {
            assert_ne!(header(&raw, &header::CACHE_CONTROL), Some("no-store"));
            assert_eq!(header(&raw, &header::PRAGMA), None);
        }
    }

    #[tokio::test]
    async fn without_web_dir_root_is_problem_404() {
        let raw = fetch(&app(), "/", HTML).await;
        assert_problem_404(&raw);
        assert_eq!(header(&raw, &header::CONTENT_SECURITY_POLICY), None);
    }
}
