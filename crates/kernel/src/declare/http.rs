//! The generated HTTP surface of the action registry.
//!
//! Every declared action is one operation, `POST /api/v1/actions/<name>`
//! (e.g. `/api/v1/actions/conversations.archive`): one namespace keyed by the
//! action's name, so any client that read the [catalogue](super::Catalogue)
//! can call any action generically, and each action is still its own OpenAPI
//! operation with its own schemas and scope. Action names always contain a
//! dot, so they never collide with `POST /api/v1/actions/undo`.
//!
//! A call:
//!
//! 1. requires a bearer token granting the action's scope (401/403, as
//!    [`Scoped<S>`](crate::auth::Scoped));
//! 2. reads the JSON body (`Content-Type: application/json`; an empty body
//!    means `{}`) and decodes it as the action's input: malformed JSON → 400,
//!    a body that does not match the input → 422, both problem+json;
//! 3. for a **mutating** action with an `Idempotency-Key` header, claims the
//!    key: a repeat of the same request (same caller, action and input)
//!    replays the stored response (with `Idempotency-Replayed: true`) instead
//!    of running again; the same key with another request → 422
//!    `idempotency_key_reused`. Keys are kept [`IDEMPOTENCY_TTL`]. A failed
//!    request stores nothing, so it can be retried with its key;
//! 4. runs the handler in the call's transaction and answers 200
//!    `{"result": …}`, plus `"undo": {"token", "expires_at"}` for an
//!    undoable action that changed something. The change, its events, the
//!    undo token and the idempotency record commit together.
//!
//! `POST /api/v1/actions/undo` with `{"token": …}` redeems an undo token: once,
//! before it expires, by the caller it was issued to, in the same workspace,
//! whose token still grants the action's scope. A token that is unknown or
//! someone else's → 404; used or expired → 409 `undo_already_used` /
//! `undo_expired`. The undo endpoint exists once some action is undoable.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Duration,
};

use axum::{
    Json,
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;
use utoipa::{
    PartialSchema, ToSchema,
    openapi::{
        HttpMethod, ObjectBuilder, Ref, RefOr, Required, Type,
        content::ContentBuilder,
        path::{OperationBuilder, ParameterBuilder, ParameterIn, PathItem, PathsBuilder},
        request_body::RequestBodyBuilder,
        response::{Response as OpenApiResponse, ResponseBuilder},
        schema::Schema,
        security::SecurityRequirement,
    },
};
use utoipa_axum::router::{OpenApiRouter, UtoipaMethodRouter};

use super::{
    ActionContext, ActionDecl,
    context::SharedTx,
    store::{self, Caller, Claim},
};
use crate::{AppState, Error, Problem, auth, error::PROBLEM_JSON};

/// Where the actions live: `POST /api/v1/actions/<module>.<verb>`.
pub const ACTIONS_PATH: &str = "/api/v1/actions";
/// Where undo tokens are redeemed.
pub const UNDO_PATH: &str = "/api/v1/actions/undo";
/// Header making a mutating call idempotent.
pub const IDEMPOTENCY_KEY: &str = "idempotency-key";
/// Response header marking a replayed response.
pub const IDEMPOTENCY_REPLAYED: &str = "idempotency-replayed";
/// How long an idempotency key is remembered.
pub const IDEMPOTENCY_TTL: Duration = Duration::from_hours(24);
/// Longest idempotency key accepted, in characters.
pub const MAX_IDEMPOTENCY_KEY_LEN: usize = 255;
/// Largest request body an action accepts.
pub const MAX_BODY_BYTES: usize = 1024 * 1024;
/// The OpenAPI tag of the undo endpoint.
const UNDO_TAG: &str = "actions";

/// The routes of `actions` (the first of any duplicated name; the catalogue
/// reports duplicates) and, if any is undoable, the undo endpoint.
pub fn routes<'a>(actions: impl IntoIterator<Item = &'a ActionDecl>) -> OpenApiRouter<AppState> {
    let mut router = OpenApiRouter::new();
    let mut seen = BTreeSet::new();
    let mut undoable = BTreeMap::new();
    for action in actions {
        if !seen.insert(action.name()) {
            continue;
        }
        let action = Arc::new(action.clone());
        if action.undo.is_some() {
            undoable.insert(action.name(), Arc::clone(&action));
        }
        router = router.routes(action_route(action));
    }
    if !undoable.is_empty() {
        router = router.routes(undo_route(Arc::new(undoable)));
    }
    router
}

/// An action's response body.
#[derive(Debug, Serialize)]
struct ActionResponse {
    result: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    undo: Option<UndoToken>,
}

/// How to undo the action just performed.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UndoToken {
    /// Redeem at `POST /api/v1/actions/undo`, once.
    pub token: String,
    /// The token is refused after this instant.
    pub expires_at: DateTime<Utc>,
}

/// Undo an action.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UndoRequest {
    /// The `undo.token` of the action's response.
    pub token: String,
}

/// The action was undone.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UndoResponse {
    /// The action that was undone (`<module>.<verb>`).
    #[schema(example = "conversations.archive")]
    pub action: String,
}

/// A failed call: an [`Error`], or a status the error type does not model.
struct Rejection(Response);

impl From<Error> for Rejection {
    fn from(err: Error) -> Self {
        Self(err.into_response())
    }
}

impl From<sqlx::Error> for Rejection {
    fn from(err: sqlx::Error) -> Self {
        Error::from(err).into()
    }
}

fn reject(status: StatusCode, detail: &str) -> Rejection {
    Rejection(Problem::new(status, Some(detail.to_owned())).into_response())
}

fn action_route(action: Arc<ActionDecl>) -> UtoipaMethodRouter<AppState> {
    let path = action.path();
    let mut schemas = action.input.components.clone();
    schemas.extend(action.output.components.clone());
    if action.undo.is_some() {
        schemas.push((UndoToken::name().into_owned(), UndoToken::schema()));
    }
    schemas.push((Problem::name().into_owned(), Problem::schema()));
    let paths = PathsBuilder::new()
        .path(
            &path,
            PathItem::new(HttpMethod::Post, action_operation(&action)),
        )
        .build();
    let handler = post(
        move |State(state): State<AppState>, request: Request| async move {
            invoke(&state, &action, request)
                .await
                .unwrap_or_else(|Rejection(response)| response)
        },
    );
    (schemas, paths, handler)
}

fn action_operation(action: &ActionDecl) -> utoipa::openapi::path::Operation {
    let mut result = ObjectBuilder::new()
        .schema_type(Type::Object)
        .property("result", action.output.reference())
        .required("result");
    if let Some(undo) = &action.undo {
        let mut token = Ref::from_schema_name(UndoToken::name());
        token.description = format!(
            "Present when the call changed something: redeem the token at \
             `POST {UNDO_PATH}` within {} seconds to undo it.",
            undo.window.as_secs()
        );
        result = result.property("undo", RefOr::Ref(token));
    }
    let summary = first_sentence(action.description());
    let mut description = action.description().to_owned();
    if action.mutates() {
        description
            .push_str("\n\nChanges data: send an `Idempotency-Key` header to make retries safe.");
    }
    let mut operation = OperationBuilder::new()
        .tag(action.module())
        .summary(Some(summary))
        .description((description != summary).then_some(description))
        .operation_id(Some(action.operation_id()))
        .request_body(Some(
            RequestBodyBuilder::new()
                .description(Some("The action's input."))
                .content(
                    "application/json",
                    ContentBuilder::new()
                        .schema(Some(action.input.reference()))
                        .build(),
                )
                .required(Some(Required::True))
                .build(),
        ))
        .response(
            "200",
            ResponseBuilder::new()
                .description("The action ran.")
                .content(
                    "application/json",
                    ContentBuilder::new().schema(Some(result.build())).build(),
                ),
        )
        .response(
            "400",
            problem(if action.mutates() {
                "The body is not JSON, or the `Idempotency-Key` is malformed."
            } else {
                "The body is not JSON."
            }),
        )
        .response("401", problem("No valid access token."))
        .response("403", problem("The token lacks the action's scope."))
        .response("413", problem("The body is over 1 MiB."))
        .response("415", problem("The body is not `application/json`."))
        .response(
            "422",
            problem(if action.mutates() {
                "The body does not match the action's input, or the `Idempotency-Key` was \
                 used for another request."
            } else {
                "The body does not match the action's input."
            }),
        )
        .security(SecurityRequirement::new(
            auth::SECURITY_SCHEME,
            [action.scope()],
        ));
    if action.mutates() {
        operation = operation.parameter(
            ParameterBuilder::new()
                .name("Idempotency-Key")
                .parameter_in(ParameterIn::Header)
                .required(Required::False)
                .description(Some(format!(
                    "A unique value per intended change (e.g. a UUID). A repeat of the same \
                     request with the same key returns the first response instead of acting \
                     again; keys are kept {} hours.",
                    IDEMPOTENCY_TTL.as_secs() / 3600
                )))
                .schema(Some(
                    ObjectBuilder::new()
                        .schema_type(Type::String)
                        .min_length(Some(1))
                        .max_length(Some(MAX_IDEMPOTENCY_KEY_LEN))
                        .build(),
                )),
        );
    }
    operation.build()
}

fn undo_route(
    actions: Arc<BTreeMap<&'static str, Arc<ActionDecl>>>,
) -> UtoipaMethodRouter<AppState> {
    let scopes: BTreeSet<&str> = actions.values().map(|action| action.scope()).collect();
    let undoable: Vec<String> = actions.keys().map(|name| format!("`{name}`")).collect();
    let operation = OperationBuilder::new()
        .tag(UNDO_TAG)
        .summary(Some("Undo an action."))
        .description(Some(format!(
            "Redeem the undo token an undoable action returned: once, before it expires, \
             by the caller it was issued to, with a token that still grants the action's \
             scope. Undoable actions: {}.",
            undoable.join(", ")
        )))
        .operation_id(Some("actions_undo"))
        .request_body(Some(
            RequestBodyBuilder::new()
                .content(
                    "application/json",
                    ContentBuilder::new()
                        .schema(Some(schema_ref::<UndoRequest>()))
                        .build(),
                )
                .required(Some(Required::True))
                .build(),
        ))
        .response(
            "200",
            ResponseBuilder::new()
                .description("The action was undone.")
                .content(
                    "application/json",
                    ContentBuilder::new()
                        .schema(Some(schema_ref::<UndoResponse>()))
                        .build(),
                ),
        )
        .response("400", problem("The body is not JSON."))
        .response("401", problem("No valid access token."))
        .response("403", problem("The token lacks the undone action's scope."))
        .response("404", problem("No such token for this caller."))
        .response(
            "409",
            problem(
                "The token was used already (`undo_already_used`) or expired (`undo_expired`).",
            ),
        )
        .response("422", problem("The body does not match."))
        .securities(Some(scopes.into_iter().map(|scope| {
            SecurityRequirement::new(auth::SECURITY_SCHEME, [scope])
        })))
        .build();
    let schemas = vec![
        (UndoRequest::name().into_owned(), UndoRequest::schema()),
        (UndoResponse::name().into_owned(), UndoResponse::schema()),
        (Problem::name().into_owned(), Problem::schema()),
    ];
    let paths = PathsBuilder::new()
        .path(UNDO_PATH, PathItem::new(HttpMethod::Post, operation))
        .build();
    let handler = post(
        move |State(state): State<AppState>, request: Request| async move {
            undo(&state, &actions, request)
                .await
                .unwrap_or_else(|Rejection(response)| response)
        },
    );
    (schemas, paths, handler)
}

/// The text up to and including its first full stop followed by a space (or
/// all of it): the operation's summary.
fn first_sentence(text: &str) -> &str {
    text.find(". ").map_or(text, |end| &text[..=end])
}

fn schema_ref<T: ToSchema>() -> RefOr<Schema> {
    RefOr::Ref(Ref::from_schema_name(T::name()))
}

fn problem(description: &str) -> OpenApiResponse {
    ResponseBuilder::new()
        .description(description)
        .content(
            PROBLEM_JSON,
            ContentBuilder::new()
                .schema(Some(schema_ref::<Problem>()))
                .build(),
        )
        .build()
}

async fn invoke(
    state: &AppState,
    action: &ActionDecl,
    request: Request,
) -> Result<Response, Rejection> {
    let (parts, body) = request.into_parts();
    let principal = auth::authorize(&parts.headers, &state.auth, action.scope()).await?;
    let input = (action.normalize)(read_json(&parts.headers, body).await?)?;
    let key = if action.mutates() {
        idempotency_key(&parts.headers)?
    } else {
        None
    };
    let principal_key = store::principal_key(&principal);
    let workspace = principal.workspace.clone();
    let caller = Caller {
        principal: &principal_key,
        workspace: workspace.as_deref(),
    };

    let tx: SharedTx = Arc::new(Mutex::new(None));
    if let Some(key) = &key {
        let mut conn = state.db.begin().await?;
        match store::claim_key(
            &mut conn,
            &caller,
            key,
            action.name(),
            &input,
            IDEMPOTENCY_TTL,
        )
        .await?
        {
            Claim::Owned => *tx.lock().await = Some(conn),
            Claim::Replay(body) => {
                let mut response = Json(body).into_response();
                response
                    .headers_mut()
                    .insert(IDEMPOTENCY_REPLAYED, HeaderValue::from_static("true"));
                return Ok(response);
            }
            Claim::Mismatch => {
                return Err(Error::Unprocessable("idempotency_key_reused".into()).into());
            }
        }
    }

    let ctx = ActionContext::new(state.clone(), principal, action.name(), Arc::clone(&tx));
    let outcome = (action.run)(ctx, input).await?;

    let mut slot = tx.lock().await;
    let mut response = ActionResponse {
        result: outcome.output,
        undo: None,
    };
    if let (Some(payload), Some(undo)) = (outcome.undo, &action.undo) {
        let conn = open(&mut slot, state).await?;
        let (token, expires_at) =
            store::issue_undo(conn, &caller, action.name(), &payload, undo.window).await?;
        response.undo = Some(UndoToken { token, expires_at });
    }
    let body = serde_json::to_value(&response).map_err(Error::internal)?;
    if let Some(key) = &key {
        let conn = open(&mut slot, state).await?;
        store::complete_key(conn, &caller, key, &body).await?;
    }
    if let Some(tx) = slot.take() {
        tx.commit().await?;
    }
    Ok(Json(body).into_response())
}

async fn undo(
    state: &AppState,
    actions: &BTreeMap<&'static str, Arc<ActionDecl>>,
    request: Request,
) -> Result<Response, Rejection> {
    let (parts, body) = request.into_parts();
    let principal = auth::authenticate(&parts.headers, &state.auth).await?;
    let request: UndoRequest = serde_json::from_value(read_json(&parts.headers, body).await?)
        .map_err(|e| Error::Unprocessable(e.to_string()))?;

    let mut conn = state.db.begin().await?;
    let record = store::find_undo(&mut conn, &request.token)
        .await?
        .filter(|record| {
            record.principal == store::principal_key(&principal)
                && record.workspace == principal.workspace
        })
        .ok_or(Error::NotFound)?;
    let (action, undo) = actions
        .get(record.action.as_str())
        .and_then(|action| action.undo.as_ref().map(|undo| (action, undo)))
        .ok_or(Error::NotFound)?;
    if !principal.has_scope(action.scope()) {
        return Err(Error::InsufficientScope(action.scope()).into());
    }
    if record.redeemed {
        return Err(Error::Conflict("undo_already_used".into()).into());
    }
    if record.expired {
        return Err(Error::Conflict("undo_expired".into()).into());
    }
    store::redeem_undo(&mut conn, &request.token).await?;

    let tx: SharedTx = Arc::new(Mutex::new(Some(conn)));
    let ctx = ActionContext::new(state.clone(), principal, action.name(), Arc::clone(&tx));
    (undo.run)(ctx, record.payload).await?;
    if let Some(tx) = tx.lock().await.take() {
        tx.commit().await?;
    }
    Ok(Json(UndoResponse {
        action: action.name().to_owned(),
    })
    .into_response())
}

/// The call's transaction, opened if the handler did not.
async fn open<'s>(
    slot: &'s mut Option<sqlx::Transaction<'static, sqlx::Postgres>>,
    state: &AppState,
) -> Result<&'s mut sqlx::PgConnection, Error> {
    if slot.is_none() {
        *slot = Some(state.db.begin().await?);
    }
    match slot {
        Some(tx) => Ok(&mut **tx),
        None => unreachable!("the transaction was opened above"),
    }
}

/// The request body as JSON: `{}` when empty; 415 unless JSON; 413 when over
/// [`MAX_BODY_BYTES`]; 400 when malformed.
async fn read_json(headers: &HeaderMap, body: Body) -> Result<Value, Rejection> {
    let bytes = axum::body::to_bytes(body, MAX_BODY_BYTES)
        .await
        .map_err(|_| reject(StatusCode::PAYLOAD_TOO_LARGE, "body_too_large"))?;
    if bytes.is_empty() {
        return Ok(Value::Object(serde_json::Map::new()));
    }
    if !is_json(headers) {
        return Err(reject(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "content_type_must_be_json",
        ));
    }
    serde_json::from_slice(&bytes)
        .map_err(|e| Error::BadRequest(format!("body is not valid JSON: {e}")).into())
}

/// `Content-Type: application/json` or `application/<something>+json`.
fn is_json(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(|essence| essence.trim().to_ascii_lowercase())
        .is_some_and(|essence| {
            essence == "application/json"
                || (essence.starts_with("application/") && essence.ends_with("+json"))
        })
}

/// The `Idempotency-Key` header: 1–255 visible ASCII characters, optionally
/// as a quoted structured-field string.
fn idempotency_key(headers: &HeaderMap) -> Result<Option<String>, Error> {
    let mut values = headers.get_all(IDEMPOTENCY_KEY).iter();
    let Some(value) = values.next() else {
        return Ok(None);
    };
    let invalid = || Error::BadRequest("invalid_idempotency_key".to_owned());
    if values.next().is_some() {
        return Err(invalid());
    }
    let value = value.to_str().map_err(|_| invalid())?.trim();
    let key = value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .unwrap_or(value);
    let valid = !key.is_empty()
        && key.len() <= MAX_IDEMPOTENCY_KEY_LEN
        && key.bytes().all(|b| b.is_ascii_graphic() && b != b'"');
    if valid {
        Ok(Some(key.to_owned()))
    } else {
        Err(invalid())
    }
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    fn with(name: &'static str, values: &[&'static str]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for value in values {
            headers.append(name, HeaderValue::from_static(value));
        }
        headers
    }

    #[test]
    fn idempotency_keys() {
        assert_eq!(idempotency_key(&HeaderMap::new()).unwrap(), None);
        assert_eq!(
            idempotency_key(&with(IDEMPOTENCY_KEY, &["abc-123"])).unwrap(),
            Some("abc-123".to_owned())
        );
        assert_eq!(
            idempotency_key(&with(IDEMPOTENCY_KEY, &["\"abc\""])).unwrap(),
            Some("abc".to_owned())
        );
        let long: &'static str = "k".repeat(256).leak();
        for bad in [&[""][..], &["\"\""], &["a b"], &["a", "b"], &[long]] {
            assert!(
                idempotency_key(&with(IDEMPOTENCY_KEY, bad)).is_err(),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn summaries_are_first_sentences() {
        assert_eq!(first_sentence("Archive it. It comes back."), "Archive it.");
        assert_eq!(first_sentence("Archive it."), "Archive it.");
        assert_eq!(first_sentence("v1.2 is out"), "v1.2 is out");
    }

    #[test]
    fn json_content_types() {
        let ct = |v| with("content-type", &[v]);
        assert!(is_json(&ct("application/json")));
        assert!(is_json(&ct("Application/JSON; charset=utf-8")));
        assert!(is_json(&ct("application/merge-patch+json")));
        assert!(!is_json(&ct("text/plain")));
        assert!(!is_json(&ct("application/x-www-form-urlencoded")));
        assert!(!is_json(&HeaderMap::new()));
    }
}
