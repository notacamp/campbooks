//! `campbooks-server` — composes the modules into one HTTP API.
//!
//! This is the only crate that knows the full list of modules. Adding a module
//! means adding one line to [`modules`]. Their background work (jobs, event
//! handlers, schedules, published events) is collected by [`registry`]; their
//! operations form the catalogue of [`openapi`] (ADR 0013).

#![forbid(unsafe_code)]

pub mod telemetry;
pub mod web;

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
    jobs::{Registry, RegistryError},
    operations,
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

/// Longest a request may take before the server answers 503.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Path the OpenAPI document is served at.
pub const OPENAPI_PATH: &str = "/openapi.json";

/// Header carrying the request id, generated when the client sends none.
pub const REQUEST_ID_HEADER: &str = "x-request-id";

/// Prefix the web client sends API calls under (same origin, ADR 0004).
pub const BFF_PREFIX: &str = "/bff";

/// Security headers set on every response, API and web client alike.
pub const SECURITY_HEADERS: [(HeaderName, &str); 3] = [
    (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
    (header::REFERRER_POLICY, "strict-origin-when-cross-origin"),
    (header::X_FRAME_OPTIONS, "DENY"),
];

/// Name of the OpenAPI security scheme every scoped operation references:
/// `security(("bearer" = ["<scope>"]))`.
pub const SECURITY_SCHEME: &str = "bearer";

/// Every module, in mount order.
pub fn modules() -> Vec<Module> {
    vec![
        campbooks_system::module(),
        campbooks_workspaces::module(),
        // `cargo xtask new-module` inserts above this line.
    ]
}

/// `state` with the ports modules implement for the kernel registered: the
/// `workspaces` module answers who belongs to which workspace.
pub fn with_ports(state: AppState) -> AppState {
    state.with_membership(campbooks_workspaces::membership())
}

/// Every job kind, event handler and schedule of every module, plus the
/// kernel's maintenance. Fails when a registration breaks a naming rule or
/// is duplicated (also checked by `cargo xtask arch`).
pub fn registry() -> Result<Registry, RegistryError> {
    Registry::build(&modules())
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
            (401 without a valid token, 403 without the scope) unless they are public.",
        license(name = "Sustainable Use License", identifier = "LicenseRef-Sustainable-Use"),
    ),
    components(schemas(Problem)),
    tags(
        (name = "system", description = "Instance health and version."),
        (name = "workspaces", description = "Workspaces, members and roles, invitations and the \
            security log. Workspace-owned resources live under `/api/v1/w/{workspace_id}/`; a \
            caller who is not a member (or a client token bound to another workspace) gets 404."),
        // `cargo xtask new-module` inserts above this line.
    ),
)]
struct ApiDoc;

fn api() -> OpenApiRouter<AppState> {
    let mut doc = ApiDoc::openapi();
    env!("CARGO_PKG_VERSION").clone_into(&mut doc.info.version);
    doc.components
        .get_or_insert_with(Default::default)
        .add_security_scheme(SECURITY_SCHEME, security_scheme());
    modules()
        .into_iter()
        .fold(OpenApiRouter::with_openapi(doc), |api, module| {
            api.merge(module.router)
        })
}

/// The OpenAPI document for every module's routes.
pub fn openapi() -> utoipa::openapi::OpenApi {
    api().into_openapi()
}

/// The full HTTP application: module routes (behind the idempotency layer),
/// `/openapi.json`, the interim `/bff/api/*` passthrough, the web client
/// (when `CAMPBOOKS_WEB_DIR` is set) or else a problem+json 404 fallback, and
/// the middleware stack (security headers, request id, tracing and metrics
/// ([`telemetry::observe`]), timeout, compression).
///
/// The same module routes are installed in the state's
/// [`Operations`](operations::Operations), so automation and the assistant invoke operations by id
/// through exactly what HTTP requests reach.
pub fn router(state: AppState) -> Router {
    let operations::Composed {
        routes,
        spec,
        state,
    } = operations::compose(api(), state);
    let spec = Arc::new(spec);
    let request_id = HeaderName::from_static(REQUEST_ID_HEADER);
    let web_client = web::configured_dir(&state.config).map(web::WebClient::new);

    // INTERIM, NO AUTH SEMANTICS. Until the BFF of ADR 0004 exists (session
    // cookie, OIDC code flow, token attachment — tracked in
    // https://github.com/notacamp/campbooks-cloud/issues/16), `/bff/api/*` is
    // the very same module routes with `/bff` stripped. It adds no session and
    // no credentials, so it can never do more than calling `/api/*` directly.
    // Its own fallback keeps `/bff/*` misses problem+json 404s. The Vite dev
    // proxy mirrors this rewrite.
    let bff_passthrough = routes.clone().fallback(|| async { Error::NotFound });

    let routes = routes
        .route(
            OPENAPI_PATH,
            get(move || async move { Json(spec.as_ref().clone()) }),
        )
        .nest(BFF_PREFIX, bff_passthrough);
    let routes = match web_client {
        Some(client) => {
            routes.fallback(move |request: AxumRequest| client.clone().respond(request))
        }
        None => routes.fallback(|| async { Error::NotFound }),
    };

    let [nosniff, referrer, frame] = SECURITY_HEADERS.map(|(name, value)| {
        SetResponseHeaderLayer::overriding(name, HeaderValue::from_static(value))
    });
    let metrics = state.metrics.clone();

    routes.with_state(state).layer(
        ServiceBuilder::new()
            .layer(nosniff)
            .layer(referrer)
            .layer(frame)
            .layer(SetRequestIdLayer::new(request_id.clone(), MakeRequestUuid))
            .layer(middleware::from_fn_with_state(metrics, telemetry::observe))
            .layer(PropagateRequestIdLayer::new(request_id))
            .layer(TimeoutLayer::with_status_code(
                StatusCode::SERVICE_UNAVAILABLE,
                REQUEST_TIMEOUT,
            ))
            .layer(CompressionLayer::new()),
    )
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, http::Request};
    use campbooks_kernel::testing;
    use tower::ServiceExt;

    use super::*;

    fn app() -> Router {
        router(testing::state(testing::unreachable_pool()))
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
                "records": []
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
    #[sqlx::test(migrations = "../../migrations")]
    async fn every_workspace_owned_table_has_row_level_security(db: sqlx::PgPool) {
        let unprotected = sqlx::query_scalar!(
            r#"
            SELECT n.nspname || '.' || c.relname AS "table!"
              FROM pg_class c
              JOIN pg_namespace n ON n.oid = c.relnamespace
             WHERE c.relkind IN ('r', 'p')
               AND n.nspname NOT IN ('pg_catalog', 'information_schema', 'public', 'kernel')
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

    /// Every table of every module (and the kernel) is usable by the runtime
    /// role the server queries as: each migration grants its own.
    #[sqlx::test(migrations = "../../migrations")]
    async fn the_runtime_role_may_use_every_table(db: sqlx::PgPool) {
        let role = campbooks_kernel::db::DEFAULT_RUNTIME_ROLE;
        let missing = sqlx::query_scalar!(
            r#"
            SELECT t.schemaname || '.' || t.tablename AS "table!"
              FROM pg_tables t
             WHERE t.schemaname NOT IN ('pg_catalog', 'information_schema', 'public')
               AND NOT (has_schema_privilege($1, t.schemaname, 'USAGE')
                        AND has_table_privilege($1, format('%I.%I', t.schemaname, t.tablename), 'SELECT')
                        AND has_table_privilege($1, format('%I.%I', t.schemaname, t.tablename), 'INSERT')
                        AND has_table_privilege($1, format('%I.%I', t.schemaname, t.tablename), 'UPDATE')
                        AND has_table_privilege($1, format('%I.%I', t.schemaname, t.tablename), 'DELETE'))
             ORDER BY 1
            "#,
            role
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
        router(AppState::new(testing::unreachable_pool(), config))
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
            "/bff/api/v1/nope",
            "/bff/openapi.json",
        ] {
            assert_problem_404(&fetch(&app, uri, HTML).await);
        }
        let spec = fetch(&app, OPENAPI_PATH, HTML).await;
        assert_eq!(spec.status, StatusCode::OK);
    }

    #[tokio::test]
    async fn bff_passthrough_mirrors_the_api() {
        for app in [app(), web_app(&WebDir::new())] {
            let direct = fetch(&app, "/api/v1/system/version", None).await;
            let via_bff = fetch(&app, "/bff/api/v1/system/version", None).await;
            assert_eq!(direct.status, StatusCode::OK);
            assert_eq!(via_bff.status, direct.status);
            assert_eq!(via_bff.body, direct.body);
            assert_problem_404(&fetch(&app, "/bff/api/v1/nope", None).await);
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
    async fn without_web_dir_root_is_problem_404() {
        let raw = fetch(&app(), "/", HTML).await;
        assert_problem_404(&raw);
        assert_eq!(header(&raw, &header::CONTENT_SECURITY_POLICY), None);
    }
}
