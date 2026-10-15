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
pub mod telemetry;
pub mod web;

#[cfg(test)]
mod access_matrix_tests;
#[cfg(test)]
mod admin_tests;
#[cfg(test)]
mod identity_tests;
#[cfg(test)]
mod search_tests;

use std::{fmt::Write as _, sync::Arc, time::Duration};

use axum::{
    Json, Router,
    extract::Request as AxumRequest,
    http::{HeaderName, HeaderValue, StatusCode, header},
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
    timeout::TimeoutLayer,
};
use utoipa::{
    OpenApi as _,
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
};
use utoipa_axum::router::OpenApiRouter;

/// The server's own messages (`i18n/<locale>.json`): the reasons of the web
/// BFF's errors.
pub static MESSAGES: i18n::Catalogue = campbooks_kernel::catalogue!("server");

/// Longest a request may take before the server answers 503.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Path the OpenAPI document is served at.
pub const OPENAPI_PATH: &str = "/openapi.json";

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

/// `Cache-Control: no-store` and `Pragma: no-cache` on every response under
/// [`NO_STORE_PREFIXES`], whatever the handler set (a handler's own
/// `no-store` variant, such as `private, no-store`, is kept). One layer, so
/// no endpoint can forget it.
async fn no_store(request: AxumRequest, next: middleware::Next) -> axum::response::Response {
    let path = request.uri().path();
    let private = NO_STORE_PREFIXES
        .iter()
        .any(|prefix| path.starts_with(prefix) || path == prefix.trim_end_matches('/'));
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
}

impl Composition {
    /// Read every module's dependencies from the environment. A broken
    /// setting is an error naming it.
    pub fn from_env(config: &campbooks_kernel::Config) -> Result<Self, String> {
        let ai = campbooks_ai::AiConfig::from_env().map_err(|e| e.to_string())?;
        let gateway = if ai.roles.is_empty() {
            None
        } else {
            Some(campbooks_ai::Gateway::new(ai).map_err(|e| e.to_string())?)
        };
        let connectors = campbooks_connectors::Deps::from_env(config)?;
        let identity = campbooks_identity::Deps::from_env(config, &scope_catalogue())?;
        let system = campbooks_system::Deps {
            ai: gateway.clone(),
            facts: facts(config, &identity, &connectors),
            started_at: chrono::Utc::now(),
        };
        Ok(Self {
            connectors,
            identity,
            conversations: campbooks_conversations::ConversationsDeps::new(
                gateway,
                campbooks_conversations::ConversationsConfig::from_env()?,
            ),
            system,
        })
    }
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
        // `cargo xtask new-module` inserts above this line.
    ]
}

/// `state` with the ports modules implement for the kernel registered: the
/// `workspaces` module answers who belongs to which workspace and which
/// language each user chose; the `conversations` module, who may see the
/// change feed's records inside an inbox and search's results, and how
/// search reads its conversations (ADR 0016); the `admin` module, who is an
/// administrator, and keeps the trail of what they do.
pub fn with_ports(state: AppState) -> AppState {
    state
        .with_resources(resource_catalogue())
        .with_admin(campbooks_admin::profiles(), campbooks_admin::audit())
        .with_membership(campbooks_workspaces::membership())
        .with_locales(campbooks_workspaces::locale_preference())
        .with_invitations(campbooks_workspaces::invitations())
        .with_visibility(campbooks_conversations::visibility())
        .with_kept_mail(campbooks_conversations::kept_mail())
        .with_search_sources(SearchSources::new(vec![
            campbooks_conversations::search_source(),
        ]))
}

/// [`with_ports`], plus the ports of the modules composed with
/// `composition`: the connectors module takes mailbox grants obtained at
/// sign-in, and when the issuer runs in this process the API validates
/// tokens with its keys, read in-process (ADR 0014).
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
            let verifier = campbooks_kernel::auth::TokenVerifier::in_process(
                &auth,
                &issuer.url,
                Arc::new(keys),
            );
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
            audit trail. These operations are not part of the operation catalogue \
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
/// idempotency layer never see it; every route behind the audit middleware,
/// problems in the caller's language.
fn admin_routes(state: &AppState, composition: &Composition) -> Router<AppState> {
    let (routes, spec) = admin_api_with(composition).split_for_parts();
    routes
        .route_layer(middleware::from_fn_with_state(
            campbooks_kernel::admin::AuditLayer::new(state, &spec),
            campbooks_kernel::admin::audit,
        ))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            i18n::localize,
        ))
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

    let routes = routes
        .route(
            OPENAPI_PATH,
            get(move || async move { Json(spec.as_ref().clone()) }),
        )
        .with_state(state)
        .merge(bff.router())
        .merge(identity);
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
            .layer(TimeoutLayer::with_status_code(
                StatusCode::SERVICE_UNAVAILABLE,
                REQUEST_TIMEOUT,
            ))
            .layer(CompressionLayer::new())
            .layer(localize),
    )
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, http::Request};
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

    /// A transaction that sets no workspace (the scheduler's dispatcher) may
    /// only read and lease the scheduling index, never insert or delete
    /// rows of any workspace's: its maintenance policies are `FOR SELECT`
    /// and `FOR UPDATE`, and no other policy admits a transaction without a
    /// workspace on a module's table.
    #[sqlx::test(migrations = "../../migrations")]
    async fn maintenance_policies_only_read_and_lease(db: sqlx::PgPool) {
        let rows: Vec<(String, String, String, String)> = sqlx::query_as(
            "SELECT tablename::text, policyname::text, cmd::text, coalesce(qual, '')
               FROM pg_policies
              WHERE schemaname NOT IN ('kernel', 'admin', 'pg_catalog')
              ORDER BY 1, 2",
        )
        .fetch_all(&db)
        .await
        .unwrap();
        // Policies admitting a transaction that set no workspace.
        let unset = "'campbooks.workspace_id'::text, true), ''::text) IS NULL";
        let policies: Vec<(String, String, String)> = rows
            .into_iter()
            .filter(|(_, _, _, qual)| qual.contains(unset))
            .map(|(table, name, cmd, _)| (table, name, cmd))
            .collect();
        assert_eq!(
            policies,
            [
                // The needs backfill's dispatcher (conversations): reads ids
                // and states, and may only start a row (`WITH CHECK`).
                (
                    "needs_backfills".to_owned(),
                    "maintenance_reads_backfills".to_owned(),
                    "SELECT".to_owned()
                ),
                (
                    "needs_backfills".to_owned(),
                    "maintenance_starts_backfills".to_owned(),
                    "UPDATE".to_owned()
                ),
                (
                    "sync_due".to_owned(),
                    "maintenance_leases_schedule".to_owned(),
                    "UPDATE".to_owned()
                ),
                (
                    "sync_due".to_owned(),
                    "maintenance_reads_schedule".to_owned(),
                    "SELECT".to_owned()
                ),
            ]
        );
    }

    /// Every table of every module (and the kernel) is usable by the runtime
    /// role the server queries as: each migration grants its own. The
    /// `admin` module's tables are the deliberate exception (ADR 0015): the
    /// runtime role may only read profiles and append to the trail, which
    /// the `admin` crate's own tests pin.
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
        assert!(
            missing.is_empty(),
            "`{role}` lacks privileges on {missing:?}"
        );
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
            auth_time: None,
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
