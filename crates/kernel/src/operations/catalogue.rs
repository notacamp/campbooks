//! The operations of a generated OpenAPI document, indexed by id and route.

use std::collections::BTreeMap;

use axum::http::Method;
use serde_json::Value;

use super::{EXTENSION, Operation};

/// HTTP methods an OpenAPI path item holds operations under.
const METHODS: [&str; 8] = [
    "get", "put", "post", "delete", "options", "head", "patch", "trace",
];

/// One operation of the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationInfo {
    /// Its `operationId`, `<module>.<verb>[_<noun>]`.
    pub id: String,
    /// Its HTTP method.
    pub method: Method,
    /// Its path template, e.g. `/api/v1/mail/threads/{id}`.
    pub path: String,
    /// Its `x-campbooks` metadata; `None` when absent or malformed (which
    /// `cargo xtask arch` rejects).
    pub meta: Option<Operation>,
    /// The scopes its `bearer` security requirement lists (empty when public).
    pub scopes: Vec<String>,
    /// Whether it declares a request body.
    pub has_body: bool,
}

impl OperationInfo {
    /// Whether it changes state. Without metadata, every method but
    /// GET/HEAD/OPTIONS/TRACE counts as mutating, so a missing declaration
    /// never switches idempotency off.
    pub fn mutates(&self) -> bool {
        self.meta.as_ref().map_or_else(
            || {
                !matches!(
                    self.method,
                    Method::GET | Method::HEAD | Method::OPTIONS | Method::TRACE
                )
            },
            |meta| meta.mutates,
        )
    }

    /// Whether its successful answer shows a secret this once
    /// (`Operation::secret_once`), so the idempotency layer keeps no copy.
    pub fn secret_once(&self) -> bool {
        self.meta.as_ref().is_some_and(|meta| meta.secret_once)
    }

    /// The path parameters of its template, in order (`{id}` → `id`).
    pub fn path_params(&self) -> impl Iterator<Item = &str> {
        self.path.split('/').filter_map(|segment| {
            segment
                .strip_prefix('{')
                .and_then(|rest| rest.strip_suffix('}'))
        })
    }
}

/// Every operation of an OpenAPI document that has an `operationId`.
#[derive(Debug, Clone, Default)]
pub struct Catalogue {
    by_id: BTreeMap<String, OperationInfo>,
    by_route: BTreeMap<(String, String), String>,
}

impl Catalogue {
    /// The catalogue of a generated document.
    pub fn from_openapi(spec: &utoipa::openapi::OpenApi) -> Self {
        // Serializing the document we generated cannot fail; an empty
        // catalogue would only make every operation unknown.
        Self::from_json(&serde_json::to_value(spec).unwrap_or(Value::Null))
    }

    /// The catalogue of a document in its JSON form.
    pub fn from_json(spec: &Value) -> Self {
        let mut catalogue = Self::default();
        let Some(paths) = spec.get("paths").and_then(Value::as_object) else {
            return catalogue;
        };
        for (path, item) in paths {
            for name in METHODS {
                let Some(operation) = item.get(name) else {
                    continue;
                };
                let Some(id) = operation.get("operationId").and_then(Value::as_str) else {
                    continue;
                };
                let Ok(method) = Method::from_bytes(name.to_uppercase().as_bytes()) else {
                    continue;
                };
                let info = OperationInfo {
                    id: id.to_owned(),
                    method,
                    path: path.clone(),
                    meta: operation
                        .get(EXTENSION)
                        .and_then(|meta| serde_json::from_value(meta.clone()).ok()),
                    scopes: scopes(operation),
                    has_body: operation.get("requestBody").is_some(),
                };
                catalogue
                    .by_route
                    .insert((info.method.to_string(), path.clone()), id.to_owned());
                catalogue.by_id.insert(id.to_owned(), info);
            }
        }
        catalogue
    }

    /// The operation with this `operationId`.
    pub fn get(&self, id: &str) -> Option<&OperationInfo> {
        self.by_id.get(id)
    }

    /// The operation served at `method` + `path` (a template, as axum's
    /// `MatchedPath` reports it). A router nested under a prefix (the web
    /// client's `/bff`) reports the prefixed template; the operation is then
    /// found by the part from `/api/` on. A wildcard segment, which axum
    /// reports as `{*name}`, is `{name}` in the document (the BFF's
    /// `/bff/api/{*path}`).
    pub fn route(&self, method: &Method, path: &str) -> Option<&OperationInfo> {
        let find = |path: &str| {
            self.by_route
                .get(&(method.to_string(), path.to_owned()))
                .and_then(|id| self.by_id.get(id))
        };
        let documented;
        let path = if path.contains("{*") {
            documented = path.replace("{*", "{");
            documented.as_str()
        } else {
            path
        };
        find(path).or_else(|| path.find("/api/").and_then(|at| find(&path[at..])))
    }

    /// Every operation, by id.
    pub fn iter(&self) -> impl Iterator<Item = &OperationInfo> {
        self.by_id.values()
    }

    /// How many operations it holds.
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// Whether it holds none.
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

fn scopes(operation: &Value) -> Vec<String> {
    operation
        .get("security")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
        .flat_map(|requirement| requirement.values())
        .filter_map(Value::as_array)
        .flatten()
        .filter_map(Value::as_str)
        .map(ToOwned::to_owned)
        .collect()
}
