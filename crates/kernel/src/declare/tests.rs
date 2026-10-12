//! Declarations, the catalogue and the generated action endpoints, against a
//! test-only module (`#[sqlx::test]` gives each DB test a fresh database with
//! every migration applied).

use std::time::Duration;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use uuid::Uuid;

use super::{http::IDEMPOTENCY_REPLAYED, *};
use crate::{
    Background, Error, Module, Result,
    auth::{ScopeInfo, ScopeName},
    events::DomainEvent,
    jobs::{self, EnqueueOptions, Registry, Worker},
    testing::{self, TestIdp, TestResponse},
};

// --- the test module -----------------------------------------------------------

struct TestRead;
impl ScopeName for TestRead {
    const NAME: &'static str = "test:read";
    const DESCRIPTION: &'static str = "Read test things.";
}

struct TestWrite;
impl ScopeName for TestWrite {
    const NAME: &'static str = "test:write";
    const DESCRIPTION: &'static str = "Change test things.";
}

const SCOPES: &[ScopeInfo] = &[ScopeInfo::of::<TestRead>(), ScopeInfo::of::<TestWrite>()];

/// A widget.
#[derive(Debug, Serialize, ToSchema)]
struct Widget {
    id: Uuid,
    archived: bool,
}

impl RecordType for Widget {
    const NAME: &'static str = "test.widget";
    const ACCESS: Access = Access::Workspace;
}

/// A part of a widget, visible to whoever sees the widget.
#[derive(Debug, Serialize, ToSchema)]
struct Part {
    id: Uuid,
}

impl RecordType for Part {
    const NAME: &'static str = "test.part";
    const ACCESS: Access = Access::Container("test.widget");
}

/// A widget was archived.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
struct WidgetArchived {
    widget: RecordRef,
}

impl DomainEvent for WidgetArchived {
    const NAME: &'static str = "test.widget_archived";
    const VERSION: u16 = 2;

    fn workspace(&self) -> Option<&str> {
        None
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
struct CountInput {
    prefix: String,
}

#[derive(Debug, Serialize, ToSchema)]
struct Count {
    notes: i64,
}

async fn count(ctx: ActionContext, input: CountInput) -> Result<Count> {
    let notes =
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM notes WHERE text LIKE $1 || '%'")
            .bind(input.prefix)
            .fetch_one(ctx.pool())
            .await?;
    Ok(Count { notes })
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
struct NoteInput {
    text: String,
    #[serde(default)]
    fail: bool,
    #[serde(default)]
    sleep_ms: u64,
}

#[derive(Debug, Serialize, ToSchema)]
struct Noted {
    text: String,
    by: String,
}

/// Writes a note in the action's transaction, then maybe waits or fails.
async fn note(ctx: ActionContext, input: NoteInput) -> Result<Noted> {
    {
        let mut tx = ctx.tx().await?;
        sqlx::query("INSERT INTO notes (text) VALUES ($1)")
            .bind(&input.text)
            .execute(&mut **tx)
            .await?;
    }
    tokio::time::sleep(Duration::from_millis(input.sleep_ms)).await;
    if input.fail {
        return Err(Error::Conflict("told to fail".into()));
    }
    Ok(Noted {
        text: input.text,
        by: ctx.principal().subject.clone(),
    })
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
struct WidgetInput {
    widget: Uuid,
}

#[derive(Debug, Serialize, ToSchema)]
struct Archived {
    widget: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
struct Unarchive {
    widget: Uuid,
}

/// Archives a widget and publishes `test.widget_archived`, atomically;
/// nothing to undo if it was archived already.
async fn archive(ctx: ActionContext, input: WidgetInput) -> Result<Undoable<Archived, Unarchive>> {
    let mut tx = ctx.tx().await?;
    let changed = sqlx::query("UPDATE widgets SET archived = true WHERE id = $1 AND NOT archived")
        .bind(input.widget)
        .execute(&mut **tx)
        .await?
        .rows_affected();
    let output = Archived {
        widget: input.widget,
    };
    if changed == 0 {
        return Ok(Undoable::nothing_to_undo(output));
    }
    let event = WidgetArchived {
        widget: RecordRef::to::<Widget>(input.widget),
    };
    ctx.state().events.publish(&mut tx, &event).await?;
    Ok(Undoable::new(
        output,
        Unarchive {
            widget: input.widget,
        },
    ))
}

async fn unarchive(ctx: ActionContext, undo: Unarchive) -> Result<()> {
    let mut tx = ctx.tx().await?;
    sqlx::query("UPDATE widgets SET archived = false WHERE id = $1")
        .bind(undo.widget)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

fn declarations() -> Declarations {
    Declarations::new()
        .record::<Widget>()
        .record::<Part>()
        .event::<WidgetArchived>()
        .action(
            Action::<CountInput, Count>::new("test.count")
                .describe("Count notes.")
                .scope::<TestRead>()
                .shortcut("g n")
                .handler(count),
        )
        .action(
            Action::<NoteInput, Noted>::new("test.note")
                .describe("Write a note.")
                .scope::<TestWrite>()
                .mutates()
                .handler(note),
        )
        .action(
            Action::<WidgetInput, Archived>::new("test.archive")
                .describe("Archive a widget.")
                .scope::<TestWrite>()
                .undoable(unarchive)
                .shortcut("e")
                .handler(archive),
        )
}

fn module_with(declarations: Declarations) -> Module {
    Module {
        name: "test",
        scopes: SCOPES,
        router: OpenApiRouter::new(),
        background: Background::new(),
        declarations,
    }
}

fn module() -> Module {
    module_with(declarations())
}

// --- the catalogue -------------------------------------------------------------

#[test]
fn catalogue_lists_everything_sorted_with_its_schemas() {
    let catalogue = serde_json::to_value(Catalogue::build(&[module()]).unwrap()).unwrap();
    let names = |kind: &str| -> Vec<String> {
        catalogue[kind]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["name"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(names("records"), ["test.part", "test.widget"]);
    assert_eq!(
        names("actions"),
        ["test.archive", "test.count", "test.note"]
    );
    assert_eq!(names("events"), ["test.widget_archived"]);

    assert_eq!(
        catalogue["actions"][0],
        json!({
            "name": "test.archive",
            "module": "test",
            "description": "Archive a widget.",
            "label": "test:actions.archive",
            "scope": "test:write",
            "mutates": true,
            "undoable": true,
            "undo_window_seconds": 600,
            "shortcut": "e",
            "method": "POST",
            "path": "/api/v1/actions/test.archive",
            "operation_id": "test_archive",
            "input": {"$ref": "#/components/schemas/WidgetInput"},
            "output": {"$ref": "#/components/schemas/Archived"},
        })
    );
    assert_eq!(catalogue["actions"][1]["mutates"], false);
    assert_eq!(catalogue["actions"][2]["undoable"], false);
    assert_eq!(
        catalogue["records"][0]["access"],
        json!({"kind": "container", "container": "test.widget"})
    );
    assert_eq!(catalogue["records"][1]["label"], "test:records.widget");
    assert_eq!(catalogue["events"][0]["version"], 2);
    let schemas = &catalogue["components"]["schemas"];
    for name in [
        "Widget",
        "Part",
        "WidgetArchived",
        "RecordRef",
        "WidgetInput",
        "Archived",
        "CountInput",
        "Count",
        "NoteInput",
        "Noted",
    ] {
        assert!(schemas[name].is_object(), "{name} missing");
    }
}

/// A scope type for actions of a module that does not declare it.
struct ForeignWrite;
impl ScopeName for ForeignWrite {
    const NAME: &'static str = "other:write";
    const DESCRIPTION: &'static str = "Not ours.";
}

/// Same component name as the `Count` above, different shape.
mod clash {
    #[derive(Debug, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
    pub struct Count {
        pub other: bool,
    }
}

async fn clash_count(_: ActionContext, _: CountInput) -> Result<clash::Count> {
    Ok(clash::Count { other: true })
}

#[test]
fn catalogue_refuses_broken_declarations() {
    #[derive(ToSchema)]
    struct Orphan;
    impl RecordType for Orphan {
        const NAME: &'static str = "test.orphan";
        const ACCESS: Access = Access::Container("test.nowhere");
    }
    #[derive(ToSchema)]
    struct Misnamed;
    impl RecordType for Misnamed {
        const NAME: &'static str = "other.misnamed";
        const ACCESS: Access = Access::Member;
    }
    #[derive(Serialize, Deserialize, ToSchema)]
    struct Unversioned;
    impl DomainEvent for Unversioned {
        const NAME: &'static str = "test.unversioned";
        const VERSION: u16 = 0;
        fn workspace(&self) -> Option<&str> {
            None
        }
    }

    let broken = declarations()
        .record::<Orphan>()
        .record::<Misnamed>()
        .record::<Widget>()
        .event::<Unversioned>()
        .action(
            // Undescribed, foreign scope, bad and duplicated shortcut name.
            Action::<CountInput, Count>::new("test.count")
                .scope::<ForeignWrite>()
                .shortcut("Shift+E")
                .handler(count),
        )
        .action(
            Action::<CountInput, Count>::new("count")
                .describe("No module prefix.")
                .scope::<TestRead>()
                .shortcut("e")
                .handler(count),
        )
        .action(
            Action::<CountInput, clash::Count>::new("test.clash")
                .describe("Reuses a schema name.")
                .scope::<TestRead>()
                .handler(clash_count),
        )
        .action(
            Action::<WidgetInput, Archived>::new("test.instant")
                .describe("Undo window too short.")
                .scope::<TestWrite>()
                .undoable(unarchive)
                .undo_window(Duration::from_millis(10))
                .handler(archive),
        );
    let DeclarationError(errors) = Catalogue::build(&[module_with(broken)]).unwrap_err();
    let expect = [
        "access container `test.nowhere` is not a declared record type",
        "record type `other.misnamed`: must be `test.<name>`",
        "record type `test.widget` is declared twice",
        "event `test.unversioned`: VERSION must be at least 1",
        "action `test.count`: needs a description",
        "action `test.count`: requires scope `other:write`, which module `test` does not list",
        "action `test.count`: shortcut `Shift+E`: `Shift+E`",
        "action `test.count` is declared twice",
        "action `count`: must be `test.<name>`",
        "action `count`: shortcut `e` is already bound to `test.archive`",
        "schema `Count` differs from the `Count` of action `test.count`",
        "action `test.instant`: the undo window must be at least one second",
    ];
    for expected in expect {
        assert!(
            errors.iter().any(|e| e.contains(expected)),
            "no error contains {expected:?}:\n{errors:#?}"
        );
    }
    assert_eq!(errors.len(), expect.len(), "{errors:#?}");
}

// --- the OpenAPI operations -----------------------------------------------------

fn spec(declarations: &Declarations) -> Value {
    serde_json::to_value(http::routes(declarations.actions()).into_openapi()).unwrap()
}

#[test]
fn every_action_is_an_operation_with_its_scope_and_schemas() {
    let spec = spec(&declarations());
    let archive = &spec["paths"]["/api/v1/actions/test.archive"]["post"];
    assert_eq!(archive["operationId"], "test_archive");
    assert_eq!(archive["tags"], json!(["test"]));
    assert_eq!(archive["security"], json!([{"bearer": ["test:write"]}]));
    assert_eq!(
        archive["requestBody"]["content"]["application/json"]["schema"],
        json!({"$ref": "#/components/schemas/WidgetInput"})
    );
    let body = &archive["responses"]["200"]["content"]["application/json"]["schema"];
    assert_eq!(body["required"], json!(["result"]));
    assert_eq!(
        body["properties"]["result"],
        json!({"$ref": "#/components/schemas/Archived"})
    );
    assert_eq!(
        body["properties"]["undo"]["$ref"],
        "#/components/schemas/UndoToken"
    );
    assert_eq!(archive["parameters"][0]["name"], "Idempotency-Key");
    assert_eq!(archive["parameters"][0]["in"], "header");

    let count = &spec["paths"]["/api/v1/actions/test.count"]["post"];
    assert_eq!(count["security"], json!([{"bearer": ["test:read"]}]));
    assert!(
        count.get("parameters").is_none(),
        "read-only: no idempotency key"
    );
    assert!(
        count["responses"]["200"]["content"]["application/json"]["schema"]["properties"]
            .get("undo")
            .is_none()
    );

    let undo = &spec["paths"]["/api/v1/actions/undo"]["post"];
    assert_eq!(undo["operationId"], "actions_undo");
    assert_eq!(undo["security"], json!([{"bearer": ["test:write"]}]));
    for schema in [
        "WidgetInput",
        "Archived",
        "UndoToken",
        "UndoRequest",
        "UndoResponse",
        "Problem",
    ] {
        assert!(
            spec["components"]["schemas"][schema].is_object(),
            "{schema}"
        );
    }
}

#[test]
fn no_undo_endpoint_without_undoable_actions() {
    let read_only = Declarations::new().action(
        Action::<CountInput, Count>::new("test.count")
            .describe("Count notes.")
            .scope::<TestRead>()
            .handler(count),
    );
    let spec = spec(&read_only);
    assert!(spec["paths"].get("/api/v1/actions/undo").is_none());
    assert!(spec["paths"]["/api/v1/actions/test.count"].is_object());
}

// --- calling actions ------------------------------------------------------------

struct Fixture {
    idp: TestIdp,
    app: Router,
    db: PgPool,
}

impl Fixture {
    async fn new(db: PgPool) -> Self {
        sqlx::query("CREATE TABLE notes (text text NOT NULL)")
            .execute(&db)
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE widgets (id uuid PRIMARY KEY, archived bool NOT NULL DEFAULT false)",
        )
        .execute(&db)
        .await
        .unwrap();
        let idp = TestIdp::start().await;
        let app = testing::app(module(), idp.state(db.clone()));
        Self { idp, app, db }
    }

    /// A token for `test-user` granting `scope`.
    fn token(&self, scope: &str) -> String {
        self.idp.token(&self.idp.claims(scope))
    }

    /// A token for another person.
    fn token_of(&self, subject: &str, scope: &str) -> String {
        let mut claims = self.idp.claims(scope);
        claims["sub"] = json!(subject);
        self.idp.token(&claims)
    }

    async fn call(&self, name: &str, token: &str, input: &Value) -> TestResponse {
        testing::call_action(&self.app, name, Some(token), input).await
    }

    async fn call_with_key(
        &self,
        name: &str,
        token: &str,
        key: &str,
        input: &Value,
    ) -> TestResponse {
        let request = Request::post(format!("/api/v1/actions/{name}"))
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .header("Idempotency-Key", key)
            .body(Body::from(input.to_string()))
            .unwrap();
        testing::send(&self.app, request).await
    }

    async fn raw(
        &self,
        name: &str,
        token: &str,
        content_type: Option<&str>,
        body: &str,
    ) -> TestResponse {
        let mut request = Request::post(format!("/api/v1/actions/{name}"))
            .header(header::AUTHORIZATION, format!("Bearer {token}"));
        if let Some(content_type) = content_type {
            request = request.header(header::CONTENT_TYPE, content_type);
        }
        testing::send(
            &self.app,
            request.body(Body::from(body.to_owned())).unwrap(),
        )
        .await
    }

    async fn undo(&self, token: &str, undo_token: &Value) -> TestResponse {
        let request = Request::post("/api/v1/actions/undo")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({"token": undo_token}).to_string()))
            .unwrap();
        testing::send(&self.app, request).await
    }

    async fn notes(&self) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM notes")
            .fetch_one(&self.db)
            .await
            .unwrap()
    }

    async fn widget(&self) -> Uuid {
        let id = Uuid::now_v7();
        sqlx::query("INSERT INTO widgets (id) VALUES ($1)")
            .bind(id)
            .execute(&self.db)
            .await
            .unwrap();
        id
    }

    async fn archived(&self, widget: Uuid) -> bool {
        sqlx::query_scalar("SELECT archived FROM widgets WHERE id = $1")
            .bind(widget)
            .fetch_one(&self.db)
            .await
            .unwrap()
    }

    async fn keys(&self) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM kernel.idempotency_keys")
            .fetch_one(&self.db)
            .await
            .unwrap()
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn read_only_action_answers_its_result(db: PgPool) {
    let f = Fixture::new(db).await;
    sqlx::query("INSERT INTO notes (text) VALUES ('ab'), ('ac'), ('b')")
        .execute(&f.db)
        .await
        .unwrap();
    let response = f
        .call("test.count", &f.token("test:read"), &json!({"prefix": "a"}))
        .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.json);
    assert_eq!(response.json, json!({"result": {"notes": 2}}));
}

#[sqlx::test(migrations = "../../migrations")]
async fn scope_is_enforced_like_scoped(db: PgPool) {
    let f = Fixture::new(db).await;
    let anonymous = testing::call_action(&f.app, "test.count", None, &json!({"prefix": ""})).await;
    assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);
    assert_eq!(anonymous.json["detail"], "missing_token");
    assert_eq!(anonymous.headers[header::WWW_AUTHENTICATE], "Bearer");

    let unscoped = f
        .call("test.note", &f.token("test:read"), &json!({"text": "x"}))
        .await;
    assert_eq!(unscoped.status, StatusCode::FORBIDDEN);
    assert_eq!(
        unscoped.headers[header::WWW_AUTHENTICATE],
        r#"Bearer error="insufficient_scope", scope="test:write""#
    );
    assert_eq!(f.notes().await, 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn bad_input_is_problem_json(db: PgPool) {
    let f = Fixture::new(db).await;
    let token = f.token("test:read");

    let missing = f.call("test.count", &token, &json!({})).await;
    assert_eq!(missing.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        missing.content_type.as_deref(),
        Some("application/problem+json")
    );
    assert!(
        missing.json["detail"]
            .as_str()
            .unwrap()
            .contains("missing field `prefix`"),
        "{}",
        missing.json
    );

    let wrong_type = f.call("test.count", &token, &json!({"prefix": 3})).await;
    assert_eq!(wrong_type.status, StatusCode::UNPROCESSABLE_ENTITY);

    let empty = f.raw("test.count", &token, None, "").await;
    assert_eq!(
        empty.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "empty body is `{{}}`"
    );

    let malformed = f
        .raw("test.count", &token, Some("application/json"), "{nope")
        .await;
    assert_eq!(malformed.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        malformed.content_type.as_deref(),
        Some("application/problem+json")
    );

    let form = f
        .raw(
            "test.count",
            &token,
            Some("application/x-www-form-urlencoded"),
            "prefix=a",
        )
        .await;
    assert_eq!(form.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(form.json["detail"], "content_type_must_be_json");

    let bad_key = f
        .call_with_key(
            "test.note",
            &f.token("test:write"),
            "has space",
            &json!({"text": "x"}),
        )
        .await;
    assert_eq!(bad_key.status, StatusCode::BAD_REQUEST);
    assert_eq!(bad_key.json["detail"], "invalid_idempotency_key");
    assert_eq!(f.notes().await, 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn idempotency_key_replays_instead_of_acting_again(db: PgPool) {
    let f = Fixture::new(db).await;
    let token = f.token("test:write");
    let input = json!({"text": "hello"});

    let first = f.call_with_key("test.note", &token, "k-1", &input).await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.json);
    assert_eq!(
        first.json,
        json!({"result": {"text": "hello", "by": "test-user"}})
    );
    assert!(first.headers.get(IDEMPOTENCY_REPLAYED).is_none());

    // Same request (fields in another order, defaults spelled out): replayed.
    let again = f
        .call_with_key(
            "test.note",
            &token,
            "k-1",
            &json!({"sleep_ms": 0, "text": "hello"}),
        )
        .await;
    assert_eq!(again.status, StatusCode::OK);
    assert_eq!(again.json, first.json);
    assert_eq!(again.headers[IDEMPOTENCY_REPLAYED], "true");
    assert_eq!(f.notes().await, 1);

    // Same key, other input or other action: refused.
    let other_input = f
        .call_with_key("test.note", &token, "k-1", &json!({"text": "bye"}))
        .await;
    assert_eq!(other_input.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(other_input.json["detail"], "idempotency_key_reused");
    let other_action = f
        .call_with_key(
            "test.archive",
            &token,
            "k-1",
            &json!({"widget": Uuid::now_v7()}),
        )
        .await;
    assert_eq!(other_action.status, StatusCode::UNPROCESSABLE_ENTITY);

    // Keys belong to their caller: another person's `k-1` is a new request.
    let other_person = f
        .call_with_key(
            "test.note",
            &f.token_of("someone-else", "test:write"),
            "k-1",
            &input,
        )
        .await;
    assert_eq!(other_person.status, StatusCode::OK);
    assert!(other_person.headers.get(IDEMPOTENCY_REPLAYED).is_none());
    assert_eq!(f.notes().await, 2);

    // Without a key, every call acts.
    f.call("test.note", &token, &input).await;
    assert_eq!(f.notes().await, 3);
    assert_eq!(f.keys().await, 2);
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_failed_call_rolls_back_and_keeps_its_key_free(db: PgPool) {
    let f = Fixture::new(db).await;
    let token = f.token("test:write");

    let failed = f
        .call_with_key(
            "test.note",
            &token,
            "k-2",
            &json!({"text": "x", "fail": true}),
        )
        .await;
    assert_eq!(failed.status, StatusCode::CONFLICT);
    assert_eq!(f.notes().await, 0, "the handler's write rolled back");
    assert_eq!(f.keys().await, 0, "no response stored for a failure");

    let retried = f
        .call_with_key("test.note", &token, "k-2", &json!({"text": "x"}))
        .await;
    assert_eq!(retried.status, StatusCode::OK);
    assert_eq!(f.notes().await, 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn concurrent_calls_with_one_key_act_once(db: PgPool) {
    let f = Fixture::new(db).await;
    let token = f.token("test:write");
    let input = json!({"text": "once", "sleep_ms": 300});

    let (a, b) = tokio::join!(
        f.call_with_key("test.note", &token, "k-3", &input),
        f.call_with_key("test.note", &token, "k-3", &input),
    );
    assert_eq!((a.status, b.status), (StatusCode::OK, StatusCode::OK));
    assert_eq!(a.json, b.json);
    let replayed = [&a, &b]
        .iter()
        .filter(|r| r.headers.get(IDEMPOTENCY_REPLAYED).is_some())
        .count();
    assert_eq!(replayed, 1);
    assert_eq!(f.notes().await, 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn undoable_action_hands_out_a_token_that_undoes_once(db: PgPool) {
    let f = Fixture::new(db).await;
    let token = f.token("test:write");
    let widget = f.widget().await;

    let archived = f
        .call("test.archive", &token, &json!({"widget": widget}))
        .await;
    assert_eq!(archived.status, StatusCode::OK, "{}", archived.json);
    assert_eq!(archived.json["result"], json!({"widget": widget}));
    let undo = &archived.json["undo"];
    assert!(undo["token"].is_string(), "{}", archived.json);
    let expires_at: chrono::DateTime<chrono::Utc> =
        serde_json::from_value(undo["expires_at"].clone()).unwrap();
    let window = expires_at - chrono::Utc::now();
    assert!(window > chrono::TimeDelta::minutes(9) && window <= chrono::TimeDelta::minutes(10));
    assert!(f.archived(widget).await);

    // The event was published with the change.
    let (name, version, payload): (String, i32, Value) =
        sqlx::query_as("SELECT name, version, payload FROM kernel.events")
            .fetch_one(&f.db)
            .await
            .unwrap();
    assert_eq!((name.as_str(), version), ("test.widget_archived", 2));
    assert_eq!(
        payload["widget"],
        json!({"type": "test.widget", "id": widget})
    );

    // Only its caller may redeem it, and only with the action's scope.
    let stranger = f
        .undo(&f.token_of("someone-else", "test:write"), &undo["token"])
        .await;
    assert_eq!(stranger.status, StatusCode::NOT_FOUND);
    let unscoped = f.undo(&f.token("test:read"), &undo["token"]).await;
    assert_eq!(unscoped.status, StatusCode::FORBIDDEN);
    let unknown = f.undo(&token, &json!("not-a-token")).await;
    assert_eq!(unknown.status, StatusCode::NOT_FOUND);
    assert!(f.archived(widget).await);

    let undone = f.undo(&token, &undo["token"]).await;
    assert_eq!(undone.status, StatusCode::OK, "{}", undone.json);
    assert_eq!(undone.json, json!({"action": "test.archive"}));
    assert!(!f.archived(widget).await);

    let twice = f.undo(&token, &undo["token"]).await;
    assert_eq!(twice.status, StatusCode::CONFLICT);
    assert_eq!(twice.json["detail"], "undo_already_used");

    // The token itself is never stored.
    let stored: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM kernel.undo_tokens WHERE payload::text LIKE '%' || $1 || '%'",
    )
    .bind(undo["token"].as_str().unwrap())
    .fetch_one(&f.db)
    .await
    .unwrap();
    assert_eq!(stored, 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn no_undo_token_when_nothing_changed_and_none_after_expiry(db: PgPool) {
    let f = Fixture::new(db).await;
    let token = f.token("test:write");
    let widget = f.widget().await;

    let first = f
        .call("test.archive", &token, &json!({"widget": widget}))
        .await;
    let again = f
        .call("test.archive", &token, &json!({"widget": widget}))
        .await;
    assert_eq!(again.status, StatusCode::OK);
    assert!(again.json.get("undo").is_none(), "{}", again.json);

    sqlx::query("UPDATE kernel.undo_tokens SET expires_at = now() - interval '1 second'")
        .execute(&f.db)
        .await
        .unwrap();
    let expired = f.undo(&token, &first.json["undo"]["token"]).await;
    assert_eq!(expired.status, StatusCode::CONFLICT);
    assert_eq!(expired.json["detail"], "undo_expired");
    assert!(f.archived(widget).await);
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_replayed_undoable_call_returns_the_same_token(db: PgPool) {
    let f = Fixture::new(db).await;
    let token = f.token("test:write");
    let widget = f.widget().await;
    let input = json!({"widget": widget});

    let first = f.call_with_key("test.archive", &token, "k-4", &input).await;
    let again = f.call_with_key("test.archive", &token, "k-4", &input).await;
    assert_eq!(again.json, first.json);
    assert!(first.json["undo"]["token"].is_string());
    let tokens: i64 = sqlx::query_scalar("SELECT count(*) FROM kernel.undo_tokens")
        .fetch_one(&f.db)
        .await
        .unwrap();
    assert_eq!(tokens, 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn prune_forgets_expired_keys_and_tokens_only(db: PgPool) {
    let f = Fixture::new(db).await;
    let token = f.token("test:write");
    for key in ["old", "new"] {
        f.call_with_key(
            "test.archive",
            &token,
            key,
            &json!({"widget": f.widget().await}),
        )
        .await;
    }
    sqlx::query(
        "UPDATE kernel.idempotency_keys SET expires_at = now() - interval '1 hour' WHERE key = 'old'",
    )
    .execute(&f.db)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE kernel.undo_tokens SET expires_at = now() - interval '1 hour'
          WHERE created_at = (SELECT min(created_at) FROM kernel.undo_tokens)",
    )
    .execute(&f.db)
    .await
    .unwrap();

    let state = f
        .idp
        .state(f.db.clone())
        .with_registry(Registry::build(&[]).unwrap());
    let mut tx = f.db.begin().await.unwrap();
    jobs::enqueue(&mut tx, &PruneActions, EnqueueOptions::default())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    Worker::new(state).run_until_idle().await;

    let keys: Vec<String> = sqlx::query_scalar("SELECT key FROM kernel.idempotency_keys")
        .fetch_all(&f.db)
        .await
        .unwrap();
    assert_eq!(keys, ["new"]);
    let tokens: i64 = sqlx::query_scalar("SELECT count(*) FROM kernel.undo_tokens")
        .fetch_one(&f.db)
        .await
        .unwrap();
    assert_eq!(tokens, 1);
}
