//! The catalogue, invoke-by-id and the idempotency layer, through a small
//! test API composed exactly as the server composes the modules.

use axum::{
    Json, Router,
    body::Body,
    extract::{Path, Query},
    http::{Request, StatusCode, header},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt as _;
use utoipa_axum::{router::OpenApiRouter, routes};

use super::*;
use crate::{
    auth::{Principal, PrincipalKind, Scope, ScopeName, Scoped},
    idempotency::{self, PruneIdempotency},
    jobs::{EnqueueOptions, Registry, Worker, enqueue},
    testing::{self, TestIdp},
};

struct TestWrite;
impl ScopeName for TestWrite {
    const NAME: &'static str = "test:write";
    const DESCRIPTION: &'static str = "Change test notes.";
}

struct TestRead;
impl ScopeName for TestRead {
    const NAME: &'static str = "test:read";
    const DESCRIPTION: &'static str = "Read test notes.";
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
struct ArchiveInput {
    reason: String,
}

/// Archive a note. Answers a fresh run id, so a replay is recognisable.
#[utoipa::path(
    post,
    path = "/api/v1/test/notes/{id}/archive",
    operation_id = "test.archive",
    params(("id" = String, Path)),
    request_body = ArchiveInput,
    security(("bearer" = ["test:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("test.unarchive")
        .automation()
        .assistant()
        .records(["test.note"])))),
    responses((status = CREATED, body = Value)),
)]
async fn archive(
    Scoped { principal, .. }: Scoped<TestWrite>,
    Path(id): Path<String>,
    Json(input): Json<ArchiveInput>,
) -> (
    StatusCode,
    [(header::HeaderName, &'static str); 1],
    Json<Value>,
) {
    (
        StatusCode::CREATED,
        [(header::LOCATION, "/api/v1/test/notes/archived")],
        Json(json!({
            "id": id,
            "reason": input.reason,
            "by": principal.subject,
            "run": uuid::Uuid::now_v7(),
        })),
    )
}

/// Unarchive a note (the undo of `test.archive`). Rules may not call it.
#[utoipa::path(
    post,
    path = "/api/v1/test/notes/{id}/unarchive",
    operation_id = "test.unarchive",
    params(("id" = String, Path)),
    security(("bearer" = ["test:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .assistant()
        .records(["test.note"])))),
    responses((status = OK, body = Value)),
)]
async fn unarchive(_: Scoped<TestWrite>, Path(id): Path<String>) -> Json<Value> {
    Json(json!({ "id": id }))
}

#[derive(Debug, Deserialize)]
struct ListQuery {
    label: Option<String>,
    limit: Option<u32>,
}

/// List notes; echoes its query.
#[utoipa::path(
    get,
    path = "/api/v1/test/notes",
    operation_id = "test.list_notes",
    params(("label" = Option<String>, Query), ("limit" = Option<u32>, Query)),
    security(("bearer" = ["test:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant().records(["test.note"])))),
    responses((status = OK, body = Value)),
)]
async fn list_notes(_: Scoped<TestRead>, Query(query): Query<ListQuery>) -> Json<Value> {
    Json(json!({ "label": query.label, "limit": query.limit }))
}

/// Delete a note: high risk.
#[utoipa::path(
    delete,
    path = "/api/v1/test/w/{workspace_id}/notes/{id}",
    operation_id = "test.delete_note",
    params(("workspace_id" = String, Path), ("id" = String, Path)),
    security(("bearer" = ["test:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::High)
        .automation()
        .assistant()
        .records(["test.note"])))),
    responses((status = OK, body = Value)),
)]
async fn delete_note(
    _: Scoped<TestWrite>,
    Path((workspace, id)): Path<(String, String)>,
) -> Json<Value> {
    Json(json!({ "workspace": workspace, "id": id }))
}

/// Reasons of the test operations.
static TEST_MESSAGES: crate::i18n::Catalogue =
    crate::i18n::Catalogue::new("test", [r#"{"errors": {"flaky": "Busy ({{run}})"}}"#; 4]);

/// Always fails with 503.
#[utoipa::path(
    post,
    path = "/api/v1/test/flaky",
    operation_id = "test.flaky",
    security(("bearer" = ["test:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).automation()))),
    responses((status = SERVICE_UNAVAILABLE)),
)]
async fn flaky(_: Scoped<TestWrite>) -> crate::Error {
    crate::Error::Unavailable(
        TEST_MESSAGES
            .reason("flaky")
            .arg("run", uuid::Uuid::now_v7().to_string()),
    )
}

fn api() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(archive))
        .routes(routes!(unarchive))
        .routes(routes!(list_notes))
        .routes(routes!(delete_note))
        .routes(routes!(flaky))
}

/// The test API over `state`: the HTTP router and the state that invokes
/// through it.
fn app(state: AppState) -> (Router, AppState) {
    let Composed { routes, state, .. } = compose(api(), state);
    (routes.with_state(state.clone()), state)
}

fn principal(scopes: &[&str]) -> Principal {
    Principal {
        subject: "user-1".to_owned(),
        kind: PrincipalKind::User,
        client_id: None,
        scopes: scopes.iter().map(|s| Scope::new(*s)).collect(),
        workspace: None,
        email: None,
        email_verified: false,
        name: None,
    }
}

fn status(error: &InvokeError) -> Option<StatusCode> {
    match error {
        InvokeError::Failed { status, .. } => Some(*status),
        _ => None,
    }
}

// --- catalogue ------------------------------------------------------------

#[test]
fn catalogue_reads_ids_metadata_and_routes() {
    let (_, spec) = api().split_for_parts();
    let catalogue = Catalogue::from_openapi(&spec);
    assert_eq!(catalogue.len(), 5);

    let archive = catalogue.get("test.archive").unwrap();
    assert_eq!(archive.method, axum::http::Method::POST);
    assert_eq!(archive.path, "/api/v1/test/notes/{id}/archive");
    assert_eq!(archive.scopes, ["test:write"]);
    assert!(archive.has_body && archive.mutates());
    assert_eq!(archive.path_params().collect::<Vec<_>>(), ["id"]);
    let meta = archive.meta.as_ref().unwrap();
    assert_eq!(meta.undo.as_deref(), Some("test.unarchive"));
    assert_eq!(meta.records, ["test.note"]);
    assert!(meta.allows(Caller::Automation) && meta.allows(Caller::Assistant));

    let list = catalogue.get("test.list_notes").unwrap();
    assert!(!list.mutates() && !list.has_body);
    assert!(!list.meta.as_ref().unwrap().allows(Caller::Automation));

    let method = axum::http::Method::POST;
    assert_eq!(
        catalogue
            .route(&method, "/api/v1/test/notes/{id}/archive")
            .map(|op| op.id.as_str()),
        Some("test.archive")
    );
    // Nested under a prefix (the web client's `/bff`).
    assert_eq!(
        catalogue
            .route(&method, "/bff/api/v1/test/notes/{id}/archive")
            .map(|op| op.id.as_str()),
        Some("test.archive")
    );
    assert!(catalogue.route(&method, "/api/v1/test/notes").is_none());
}

#[test]
fn metadata_serializes_as_the_extension() {
    let meta = Operation::command(Risk::High)
        .undo("mail.restore")
        .assistant()
        .records(["mail.message"]);
    assert_eq!(
        serde_json::to_value(&meta).unwrap(),
        json!({
            "mutates": true,
            "undo": "mail.restore",
            "automation": false,
            "assistant": true,
            "risk": "high",
            "records": ["mail.message"],
        })
    );
    assert_eq!(
        serde_json::to_value(Operation::query()).unwrap(),
        json!({"mutates": false, "automation": false, "assistant": false, "risk": "low", "records": []})
    );
}

#[test]
fn operations_without_metadata_mutate_by_method() {
    let spec = json!({ "paths": { "/x": {
        "post": { "operationId": "x.post" },
        "get": { "operationId": "x.get" },
    }}});
    let catalogue = Catalogue::from_json(&spec);
    assert!(catalogue.get("x.post").unwrap().mutates());
    assert!(!catalogue.get("x.get").unwrap().mutates());
}

// --- invoke ---------------------------------------------------------------

#[tokio::test]
async fn invokes_a_read_with_query_input() {
    let (_, state) = app(testing::state(testing::unreachable_pool()));
    let output = invoke(
        &state,
        Invocation::new(
            Caller::Assistant,
            principal(&["test:read"]),
            "test.list_notes",
            json!({ "label": "a b&c", "limit": 3 }),
        ),
    )
    .await
    .unwrap();
    assert_eq!(output.status, StatusCode::OK);
    assert_eq!(output.body, json!({ "label": "a b&c", "limit": 3 }));
    assert!(!output.replayed);
}

#[tokio::test]
async fn refuses_callers_the_operation_does_not_opt_in() {
    let (_, state) = app(testing::state(testing::unreachable_pool()));
    let error = invoke(
        &state,
        Invocation::new(
            Caller::Automation,
            principal(&["test:read"]),
            "test.list_notes",
            Value::Null,
        ),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(&error, InvokeError::NotAllowed { operation, caller: Caller::Automation } if operation == "test.list_notes"),
        "{error:?}"
    );
}

#[tokio::test]
async fn a_missing_scope_fails_as_over_http() {
    let (_, state) = app(testing::state(testing::unreachable_pool()));
    let error = invoke(
        &state,
        Invocation::new(
            Caller::Assistant,
            principal(&["test:write"]),
            "test.list_notes",
            Value::Null,
        ),
    )
    .await
    .unwrap_err();
    assert_eq!(status(&error), Some(StatusCode::FORBIDDEN), "{error:?}");
    let InvokeError::Failed { problem, .. } = error else {
        unreachable!()
    };
    assert_eq!(problem["code"], "insufficient_scope");
}

#[tokio::test]
async fn refuses_unknown_unconfirmed_and_keyless_calls_before_dispatch() {
    let (_, state) = app(testing::state(testing::unreachable_pool()));
    let p = principal(&["test:write"]);

    let unknown = invoke(
        &state,
        Invocation::new(Caller::Assistant, p.clone(), "test.nope", Value::Null),
    )
    .await;
    assert!(matches!(unknown, Err(InvokeError::UnknownOperation(id)) if id == "test.nope"));

    let delete = Invocation::new(
        Caller::Automation,
        p.clone(),
        "test.delete_note",
        json!({ "id": "n1" }),
    )
    .in_workspace("ws-1")
    .idempotency_key("k1");
    let unconfirmed = invoke(&state, delete.clone()).await;
    assert!(matches!(
        unconfirmed,
        Err(InvokeError::ConfirmationRequired(id)) if id == "test.delete_note"
    ));

    let keyless = invoke(
        &state,
        Invocation::new(
            Caller::Automation,
            p.clone(),
            "test.archive",
            json!({ "id": "n1", "reason": "done" }),
        ),
    )
    .await;
    assert!(matches!(
        keyless,
        Err(InvokeError::IdempotencyKeyRequired(id)) if id == "test.archive"
    ));

    let not_an_object = invoke(
        &state,
        Invocation::new(Caller::Assistant, p.clone(), "test.list_notes", json!([1])),
    )
    .await;
    assert!(matches!(not_an_object, Err(InvokeError::InvalidInput(_))));

    let missing_param = invoke(
        &state,
        Invocation::new(
            Caller::Automation,
            p,
            "test.archive",
            json!({ "reason": "done" }),
        )
        .idempotency_key("k2"),
    )
    .await;
    assert!(
        matches!(&missing_param, Err(InvokeError::InvalidInput(m)) if m.contains("`id`")),
        "{missing_param:?}"
    );
}

#[tokio::test]
async fn without_installed_operations_nothing_runs() {
    let state = testing::state(testing::unreachable_pool());
    let error = invoke(
        &state,
        Invocation::new(
            Caller::Assistant,
            principal(&[]),
            "test.list_notes",
            Value::Null,
        ),
    )
    .await
    .unwrap_err();
    assert!(matches!(error, InvokeError::NotInstalled));
}

#[sqlx::test(migrations = "../../migrations")]
async fn invokes_a_mutation_once_per_key(db: PgPool) {
    let (_, state) = app(testing::state(db));
    let call = || {
        Invocation::new(
            Caller::Automation,
            principal(&["test:write"]),
            "test.archive",
            json!({ "id": "n/1", "reason": "done" }),
        )
        .idempotency_key("rule-7:event-42")
    };
    let first = invoke(&state, call()).await.unwrap();
    assert_eq!(first.status, StatusCode::CREATED);
    assert_eq!(first.body["id"], "n/1");
    assert_eq!(first.body["reason"], "done");
    assert_eq!(first.body["by"], "user-1");
    assert!(!first.replayed);

    let again = invoke(&state, call()).await.unwrap();
    assert!(again.replayed);
    assert_eq!(again.body, first.body);
}

#[sqlx::test(migrations = "../../migrations")]
async fn high_risk_runs_once_confirmed_and_fills_the_workspace(db: PgPool) {
    let (_, state) = app(testing::state(db));
    let output = invoke(
        &state,
        Invocation::new(
            Caller::Assistant,
            principal(&["test:write"]),
            "test.delete_note",
            json!({ "id": "n1" }),
        )
        .in_workspace("ws-1")
        .idempotency_key("k")
        .confirmed(),
    )
    .await
    .unwrap();
    assert_eq!(output.body, json!({ "workspace": "ws-1", "id": "n1" }));
}

// --- idempotency over HTTP --------------------------------------------------

struct Http {
    app: Router,
    token: String,
    idp: TestIdp,
}

async fn http(db: PgPool) -> Http {
    let idp = TestIdp::start().await;
    let (app, _) = app(idp.state(db));
    let token = idp.token(&idp.claims("test:read test:write"));
    Http { app, token, idp }
}

impl Http {
    /// The same app, called with a token for user `subject`.
    fn as_user(&self, subject: &str) -> Self {
        let mut claims = self.idp.claims("test:write");
        claims["sub"] = json!(subject);
        Self {
            app: self.app.clone(),
            token: self.idp.token(&claims),
            idp: self.idp.clone(),
        }
    }

    async fn send(
        &self,
        method: &str,
        uri: &str,
        key: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, axum::http::HeaderMap, Value) {
        let mut request = Request::builder()
            .method(method)
            .uri(uri)
            .header(header::AUTHORIZATION, format!("Bearer {}", self.token));
        if let Some(key) = key {
            request = request.header(idempotency::HEADER, key);
        }
        let body = match body {
            Some(body) => {
                request = request.header(header::CONTENT_TYPE, "application/json");
                Body::from(body.to_string())
            }
            None => Body::empty(),
        };
        let response = self
            .app
            .clone()
            .oneshot(request.body(body).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (
            status,
            headers,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }
}

const ARCHIVE: &str = "/api/v1/test/notes/n1/archive";

#[sqlx::test(migrations = "../../migrations")]
async fn replays_an_identical_request(db: PgPool) {
    let http = http(db).await;
    let body = json!({ "reason": "done" });
    let (status, headers, first) = http
        .send("POST", ARCHIVE, Some("k-1"), Some(body.clone()))
        .await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(!headers.contains_key(idempotency::REPLAYED_HEADER));

    let (status, headers, again) = http.send("POST", ARCHIVE, Some("k-1"), Some(body)).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(again, first, "the stored response, not a second run");
    assert_eq!(headers[idempotency::REPLAYED_HEADER], "true");
    assert_eq!(headers[header::CONTENT_TYPE], "application/json");
    assert_eq!(headers[header::LOCATION], "/api/v1/test/notes/archived");
}

#[sqlx::test(migrations = "../../migrations")]
async fn without_a_key_every_request_runs(db: PgPool) {
    let http = http(db).await;
    let body = json!({ "reason": "done" });
    let (_, _, first) = http.send("POST", ARCHIVE, None, Some(body.clone())).await;
    let (_, _, second) = http.send("POST", ARCHIVE, None, Some(body)).await;
    assert_ne!(first["run"], second["run"]);
}

#[sqlx::test(migrations = "../../migrations")]
async fn reusing_a_key_for_another_request_is_422(db: PgPool) {
    let http = http(db).await;
    let (status, ..) = http
        .send("POST", ARCHIVE, Some("k"), Some(json!({ "reason": "a" })))
        .await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, headers, problem) = http
        .send("POST", ARCHIVE, Some("k"), Some(json!({ "reason": "b" })))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(headers[header::CONTENT_TYPE], crate::error::PROBLEM_JSON);
    assert_eq!(problem["status"], 422);

    // Another operation (or path) with the same key is a different request too.
    let (status, ..) = http
        .send("POST", "/api/v1/test/notes/n1/unarchive", Some("k"), None)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_key_still_in_flight_is_409(db: PgPool) {
    let http = http(db.clone()).await;
    // Another process holds the key for an identical request right now.
    let (_, _, _) = http
        .send(
            "POST",
            ARCHIVE,
            Some("busy"),
            Some(json!({ "reason": "x" })),
        )
        .await;
    sqlx::query(
        "UPDATE kernel.idempotency SET state = 'pending', response_status = NULL,
             response_body = NULL WHERE key = 'busy'",
    )
    .execute(&db)
    .await
    .unwrap();
    let (status, _, problem) = http
        .send(
            "POST",
            ARCHIVE,
            Some("busy"),
            Some(json!({ "reason": "x" })),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(problem["status"], 409);

    // Once abandoned (its process died), the same request takes it over.
    sqlx::query(
        "UPDATE kernel.idempotency SET created_at = now() - interval '1 hour' WHERE key = 'busy'",
    )
    .execute(&db)
    .await
    .unwrap();
    let (status, headers, _) = http
        .send(
            "POST",
            ARCHIVE,
            Some("busy"),
            Some(json!({ "reason": "x" })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(!headers.contains_key(idempotency::REPLAYED_HEADER));
}

#[sqlx::test(migrations = "../../migrations")]
async fn server_errors_release_the_key(db: PgPool) {
    let http = http(db.clone()).await;
    let (status, _, first) = http
        .send("POST", "/api/v1/test/flaky", Some("f"), None)
        .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    let (status, headers, second) = http
        .send("POST", "/api/v1/test/flaky", Some("f"), None)
        .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(!headers.contains_key(idempotency::REPLAYED_HEADER));
    assert_ne!(first["detail"], second["detail"], "ran again");
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM kernel.idempotency")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(rows, 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn reads_ignore_the_header_and_bad_keys_are_400(db: PgPool) {
    let http = http(db.clone()).await;
    let (status, ..) = http
        .send("GET", "/api/v1/test/notes?label=x", Some("read-key"), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, ..) = http
        .send(
            "POST",
            ARCHIVE,
            Some(&"k".repeat(300)),
            Some(json!({ "reason": "x" })),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM kernel.idempotency")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(rows, 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn keys_belong_to_their_caller(db: PgPool) {
    let http = http(db).await;
    let body = json!({ "reason": "done" });
    let (_, _, mine) = http
        .send("POST", ARCHIVE, Some("shared"), Some(body.clone()))
        .await;

    let other = http.as_user("someone-else");
    let (status, headers, theirs) = other
        .send("POST", ARCHIVE, Some("shared"), Some(body))
        .await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(!headers.contains_key(idempotency::REPLAYED_HEADER));
    assert_ne!(mine["run"], theirs["run"]);
    assert_eq!(theirs["by"], "someone-else");
}

#[sqlx::test(migrations = "../../migrations")]
async fn anonymous_requests_pass_through(db: PgPool) {
    let http = http(db.clone()).await;
    let response = http
        .app
        .clone()
        .oneshot(
            Request::post(ARCHIVE)
                .header(idempotency::HEADER, "k")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"reason":"x"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM kernel.idempotency")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(rows, 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn expired_keys_are_pruned_and_reusable(db: PgPool) {
    let state = testing::state(db.clone());
    let (_, invoking) = app(state.clone());
    let call = || {
        Invocation::new(
            Caller::Automation,
            principal(&["test:write"]),
            "test.archive",
            json!({ "id": "n1", "reason": "done" }),
        )
        .idempotency_key("old")
    };
    let first = invoke(&invoking, call()).await.unwrap();
    sqlx::query("UPDATE kernel.idempotency SET expires_at = now() - interval '1 second'")
        .execute(&db)
        .await
        .unwrap();

    // An expired key is free again: the call runs anew.
    let second = invoke(&invoking, call()).await.unwrap();
    assert!(!second.replayed);
    assert_ne!(first.body["run"], second.body["run"]);

    sqlx::query("UPDATE kernel.idempotency SET expires_at = now() - interval '1 second'")
        .execute(&db)
        .await
        .unwrap();
    let state = state.with_registry(Registry::build(&[]).unwrap());
    let mut tx = db.begin().await.unwrap();
    enqueue(&mut tx, &PruneIdempotency, EnqueueOptions::default())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    Worker::new(state).run_until_idle().await;
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM kernel.idempotency")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(rows, 0);
}
