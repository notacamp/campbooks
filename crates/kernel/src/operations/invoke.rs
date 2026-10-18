//! Running an operation in-process by its `operationId`.

use std::{fmt::Write as _, sync::Arc};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{HeaderValue, Request, StatusCode, header},
};
use serde_json::{Map, Value};
use tower::ServiceExt as _;

use super::{Catalogue, OperationInfo, Risk};
use crate::{
    AppState,
    auth::{AuthenticatedPrincipal, Principal},
    idempotency,
    tenancy::WORKSPACE_PATH_PARAM,
};

/// Largest response body [`invoke`] reads.
const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;

/// The composed API as [`invoke`] reaches it: the module routes (with the
/// idempotency layer) and their catalogue. Installed in
/// [`AppState`] by the server; empty in states that serve no API.
#[derive(Clone, Default)]
pub struct Operations {
    inner: Option<Arc<Inner>>,
}

struct Inner {
    routes: Router<AppState>,
    catalogue: Arc<Catalogue>,
}

impl std::fmt::Debug for Operations {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Operations")
            .field("operations", &self.catalogue().map_or(0, Catalogue::len))
            .finish()
    }
}

impl Operations {
    /// The operations of `catalogue`, served by `routes` (the same router
    /// HTTP requests reach, before its state is attached).
    pub fn new(routes: Router<AppState>, catalogue: Arc<Catalogue>) -> Self {
        Self {
            inner: Some(Arc::new(Inner { routes, catalogue })),
        }
    }

    /// The catalogue, when installed.
    pub fn catalogue(&self) -> Option<&Catalogue> {
        self.inner.as_deref().map(|inner| inner.catalogue.as_ref())
    }
}

/// Who invoked a request in-process, next to its
/// [`AuthenticatedPrincipal`]: the per-caller rate limits count automation
/// and the assistant apart from the person's own apps. Private to the
/// kernel, like the principal it accompanies.
#[derive(Debug, Clone, Copy)]
pub(crate) struct InvokedBy(pub(crate) Caller);

/// Who is invoking: each kind may call only the operations whose
/// `x-campbooks` opts it in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Caller {
    /// An automation rule (`x-campbooks.automation`).
    Automation,
    /// The assistant (`x-campbooks.assistant`).
    Assistant,
}

impl std::fmt::Display for Caller {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Automation => "automation",
            Self::Assistant => "assistant",
        })
    }
}

/// One call of an operation by id.
///
/// `input` is a JSON object (or `null`): the operation's path parameters
/// are taken from it by name (the workspace from
/// [`in_workspace`](Self::in_workspace)); what remains is the JSON request body when the
/// operation declares one, or else its query string (scalar values, arrays
/// repeated).
#[derive(Debug, Clone)]
pub struct Invocation {
    /// Who is calling.
    pub caller: Caller,
    /// On whose behalf: its scopes and memberships are checked exactly as
    /// for an HTTP request carrying its token.
    pub principal: Principal,
    /// The `operationId` to run.
    pub operation_id: String,
    /// Path parameters, query and body, as one object.
    pub input: Value,
    /// The workspace to run in, filling the operation's
    /// [`WORKSPACE_PATH_PARAM`] (`{workspace_id}`); membership is then checked
    /// by its `Workspace<S>` extractor.
    pub workspace: Option<String>,
    /// Required for mutating operations: the same key replays the first
    /// result instead of running twice (e.g. a rule's job id).
    pub idempotency_key: Option<String>,
    /// Whether the user confirmed the call; required for high-risk
    /// operations. The caller sets it, so it is no proof of a click: no
    /// catalogued high-risk operation opts automation or the assistant in
    /// (`cargo xtask arch`), and a handler that must be the person's own
    /// request checks [`Workspace::invoked_by`](crate::tenancy::Workspace).
    pub confirmed: bool,
}

impl Invocation {
    /// A call of `operation_id` by `caller` for `principal`.
    pub fn new(
        caller: Caller,
        principal: Principal,
        operation_id: impl Into<String>,
        input: Value,
    ) -> Self {
        Self {
            caller,
            principal,
            operation_id: operation_id.into(),
            input,
            workspace: None,
            idempotency_key: None,
            confirmed: false,
        }
    }

    /// Run in `workspace`.
    #[must_use]
    pub fn in_workspace(mut self, workspace: impl Into<String>) -> Self {
        self.workspace = Some(workspace.into());
        self
    }

    /// Set the idempotency key.
    #[must_use]
    pub fn idempotency_key(mut self, key: impl Into<String>) -> Self {
        self.idempotency_key = Some(key.into());
        self
    }

    /// Record that the user confirmed the call.
    #[must_use]
    pub fn confirmed(mut self) -> Self {
        self.confirmed = true;
        self
    }
}

/// A successful (2xx) result.
#[derive(Debug, Clone, PartialEq)]
pub struct Output {
    /// The response status.
    pub status: StatusCode,
    /// The response body as JSON (`null` when empty).
    pub body: Value,
    /// Whether it is the stored result of an earlier call with the same
    /// idempotency key.
    pub replayed: bool,
}

/// Why an invocation did not succeed.
#[derive(Debug, thiserror::Error)]
pub enum InvokeError {
    /// The state has no operations installed (no API composed).
    #[error("no operations are installed in this state")]
    NotInstalled,
    /// No operation has that id.
    #[error("no operation is named `{0}`")]
    UnknownOperation(String),
    /// The operation does not opt this caller in.
    #[error("{caller} may not call `{operation}`")]
    NotAllowed {
        /// The operation.
        operation: String,
        /// Who tried.
        caller: Caller,
    },
    /// The operation is high-risk and the call was not confirmed.
    #[error("`{0}` is high-risk and needs the user's confirmation")]
    ConfirmationRequired(String),
    /// The operation mutates and the call carries no idempotency key.
    #[error("`{0}` mutates; invoke it with an idempotency key")]
    IdempotencyKeyRequired(String),
    /// The input does not fit the operation.
    #[error("invalid input: {0}")]
    InvalidInput(String),
    /// The operation answered with an error; `problem` is its body
    /// (problem+json).
    #[error("`{operation}` failed with {status}")]
    Failed {
        /// The operation.
        operation: String,
        /// The response status.
        status: StatusCode,
        /// The response body.
        problem: Value,
    },
}

/// Run an operation by id on behalf of `invocation.principal`, through the
/// same router HTTP requests reach.
///
/// Before dispatching, the operation must exist, opt the caller in
/// (`x-campbooks.automation` / `.assistant`), be confirmed when high-risk,
/// and carry an idempotency key when it mutates. The request then goes
/// through the operation's own extractors, so its scope, the workspace
/// membership and every resource check apply unchanged. The principal is
/// attached in-process; nothing reachable from outside can set it.
pub async fn invoke(state: &AppState, invocation: Invocation) -> Result<Output, InvokeError> {
    let inner = state
        .operations
        .inner
        .as_deref()
        .ok_or(InvokeError::NotInstalled)?;
    let operation = inner
        .catalogue
        .get(&invocation.operation_id)
        .ok_or_else(|| InvokeError::UnknownOperation(invocation.operation_id.clone()))?;
    let meta = operation.meta.as_ref();
    if !meta.is_some_and(|meta| meta.allows(invocation.caller)) {
        return Err(InvokeError::NotAllowed {
            operation: operation.id.clone(),
            caller: invocation.caller,
        });
    }
    if meta.is_some_and(|meta| meta.risk == Risk::High) && !invocation.confirmed {
        return Err(InvokeError::ConfirmationRequired(operation.id.clone()));
    }
    if operation.mutates() && invocation.idempotency_key.is_none() {
        return Err(InvokeError::IdempotencyKeyRequired(operation.id.clone()));
    }

    let request = build_request(operation, &invocation)?;
    let response = match inner
        .routes
        .clone()
        .with_state(state.clone())
        .oneshot(request)
        .await
    {
        Ok(response) => response,
        Err(never) => match never {},
    };
    let status = response.status();
    let replayed = response
        .headers()
        .contains_key(idempotency::REPLAYED_HEADER);
    let bytes = to_bytes(response.into_body(), MAX_RESPONSE_BYTES)
        .await
        .unwrap_or_default();
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    tracing::info!(
        operation = %operation.id,
        caller = %invocation.caller,
        status = status.as_u16(),
        replayed,
        "operation invoked"
    );
    if status.is_success() {
        Ok(Output {
            status,
            body,
            replayed,
        })
    } else {
        Err(InvokeError::Failed {
            operation: operation.id.clone(),
            status,
            problem: body,
        })
    }
}

fn build_request(
    operation: &OperationInfo,
    invocation: &Invocation,
) -> Result<Request<Body>, InvokeError> {
    let mut input = match &invocation.input {
        Value::Object(map) => map.clone(),
        Value::Null => Map::new(),
        _ => {
            return Err(InvokeError::InvalidInput(
                "input must be a JSON object".to_owned(),
            ));
        }
    };

    let mut uri = String::new();
    for (i, segment) in operation.path.split('/').enumerate() {
        if i > 0 {
            uri.push('/');
        }
        let Some(name) = segment
            .strip_prefix('{')
            .and_then(|rest| rest.strip_suffix('}'))
        else {
            uri.push_str(segment);
            continue;
        };
        let value = match (name, &invocation.workspace) {
            (WORKSPACE_PATH_PARAM, Some(workspace)) => {
                input.remove(name);
                workspace.clone()
            }
            _ => input
                .remove(name)
                .as_ref()
                .and_then(scalar)
                .ok_or_else(|| {
                    InvokeError::InvalidInput(format!("path parameter `{name}` is missing"))
                })?,
        };
        uri.push_str(&encode(&value));
    }

    let body = if operation.has_body {
        Body::from(serde_json::to_vec(&Value::Object(input)).unwrap_or_default())
    } else {
        let query = query_string(&input)?;
        if !query.is_empty() {
            uri.push('?');
            uri.push_str(&query);
        }
        Body::empty()
    };

    let mut request = Request::builder()
        .method(operation.method.clone())
        .uri(&uri);
    if operation.has_body {
        request = request.header(header::CONTENT_TYPE, "application/json");
    }
    if let Some(key) = &invocation.idempotency_key {
        let key = HeaderValue::from_str(key).map_err(|_| {
            InvokeError::InvalidInput("idempotency key is not a header value".into())
        })?;
        request = request.header(idempotency::HEADER, key);
    }
    let mut request = request
        .body(body)
        .map_err(|e| InvokeError::InvalidInput(format!("cannot build the request: {e}")))?;
    let extensions = request.extensions_mut();
    extensions.insert(AuthenticatedPrincipal(invocation.principal.clone()));
    extensions.insert(InvokedBy(invocation.caller));
    Ok(request)
}

/// A string, number or boolean as text.
fn scalar(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn query_string(input: &Map<String, Value>) -> Result<String, InvokeError> {
    let mut query = String::new();
    let mut push = |name: &str, value: &Value| -> Result<(), InvokeError> {
        let value = scalar(value).ok_or_else(|| {
            InvokeError::InvalidInput(format!("query parameter `{name}` must be a scalar"))
        })?;
        if !query.is_empty() {
            query.push('&');
        }
        let _ = write!(query, "{}={}", encode(name), encode(&value));
        Ok(())
    };
    for (name, value) in input {
        match value {
            Value::Null => {}
            Value::Array(values) => {
                for value in values {
                    push(name, value)?;
                }
            }
            value => push(name, value)?,
        }
    }
    Ok(query)
}

/// Percent-encode everything but RFC 3986 unreserved characters.
fn encode(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}
