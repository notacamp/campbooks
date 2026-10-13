//! The operation catalogue (ADR 0013): the OpenAPI document generated from
//! the handlers is the one list of what Campbooks can do.
//!
//! Every operation is an ordinary axum handler with a `#[utoipa::path]` that
//! carries
//!
//! - a stable `operation_id`, `<module>.<verb>[_<noun>]`, never renamed;
//! - its `security` (a scope, or explicitly public);
//! - one vendor extension, `x-campbooks`, built from an [`Operation`]:
//!   whether it mutates, the operation that undoes it, whether automation
//!   rules and the assistant may call it, its [`Risk`], and the record types
//!   it touches.
//!
//! ```text
//! #[utoipa::path(
//!     post,
//!     path = "/api/v1/conversations/threads/{id}/archive",
//!     operation_id = "conversations.archive",
//!     security(("bearer" = ["conversations:write"])),
//!     extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
//!         .undo("conversations.unarchive")
//!         .automation()
//!         .assistant()
//!         .records(["conversations.thread"])))),
//!     responses(…),
//! )]
//! pub async fn handler(…) -> Result<…> { … }
//! ```
//!
//! `cargo xtask arch` checks every operation of the generated document: an
//! `operationId` prefixed by its module and unique, a complete `x-campbooks`
//! block, explicit security, `undo` naming an existing mutating operation,
//! and `mutates` matching the HTTP method.
//!
//! The server [`compose`]s the modules' routes: it parses the document into
//! a [`Catalogue`], puts every route behind the idempotency layer and
//! installs the result in [`AppState`]. Automation and the assistant run
//! operations by id through [`invoke`], which dispatches a request through
//! that same router (same extractors, scope and membership checks,
//! idempotency) on behalf of a [`Principal`](crate::auth::Principal).

mod catalogue;
mod invoke;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use axum::Router;
use serde::{Deserialize, Serialize};
use utoipa_axum::router::OpenApiRouter;

use crate::{AppState, idempotency};

pub use catalogue::{Catalogue, OperationInfo};
pub use invoke::{Caller, Invocation, InvokeError, Operations, Output, invoke};

/// Name of the OpenAPI vendor extension that carries an [`Operation`].
pub const EXTENSION: &str = "x-campbooks";

/// How much harm a mistaken call can do. High-risk operations always need
/// the user's confirmation before automation or the assistant runs them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Risk {
    /// Easily reversed or harmless (archive, label, read).
    Low,
    /// Hard or impossible to reverse, or visible to other people (send,
    /// delete, pay).
    High,
}

/// The `x-campbooks` metadata of one operation, written in its
/// `#[utoipa::path]` as
/// `extensions(("x-campbooks" = json!(Operation::query().records([…]))))`.
///
/// Start from [`query`](Self::query) (reads) or [`command`](Self::command)
/// (writes); automation and the assistant are opted in explicitly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operation {
    /// Whether the operation changes state. `true` only for
    /// POST/PUT/PATCH/DELETE, `false` only for GET/HEAD.
    pub mutates: bool,
    /// The `operationId` of the operation that reverses this one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub undo: Option<String>,
    /// Whether automation rules may call it.
    pub automation: bool,
    /// Whether the assistant may call it.
    pub assistant: bool,
    /// How much harm a mistaken call can do.
    pub risk: Risk,
    /// The record types (`<module>.<record>`) it reads or changes.
    pub records: Vec<String>,
}

impl Operation {
    /// A read: does not mutate, low risk, not exposed to automation or the
    /// assistant until opted in.
    pub fn query() -> Self {
        Self {
            mutates: false,
            undo: None,
            automation: false,
            assistant: false,
            risk: Risk::Low,
            records: Vec::new(),
        }
    }

    /// A write of the given risk, not exposed to automation or the
    /// assistant until opted in.
    pub fn command(risk: Risk) -> Self {
        Self {
            mutates: true,
            risk,
            ..Self::query()
        }
    }

    /// Name the operation (`operationId`) that reverses this one.
    #[must_use]
    pub fn undo(mut self, operation_id: &str) -> Self {
        self.undo = Some(operation_id.to_owned());
        self
    }

    /// Let automation rules call it.
    #[must_use]
    pub fn automation(mut self) -> Self {
        self.automation = true;
        self
    }

    /// Let the assistant call it.
    #[must_use]
    pub fn assistant(mut self) -> Self {
        self.assistant = true;
        self
    }

    /// The record types (`<module>.<record>`) it reads or changes.
    #[must_use]
    pub fn records<I, S>(mut self, records: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.records = records.into_iter().map(Into::into).collect();
        self
    }

    /// Whether `caller` may invoke an operation carrying this metadata.
    pub fn allows(&self, caller: Caller) -> bool {
        match caller {
            Caller::Automation => self.automation,
            Caller::Assistant => self.assistant,
        }
    }
}

/// The composed API, ready to serve.
pub struct Composed {
    /// Every route, behind the idempotency layer; attach the state to serve it.
    pub routes: Router<AppState>,
    /// The OpenAPI document: the operation catalogue.
    pub spec: utoipa::openapi::OpenApi,
    /// The state with these operations installed, so
    /// [`invoke`] reaches the very same routes.
    pub state: AppState,
}

impl std::fmt::Debug for Composed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Composed")
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

/// Compose `api` (every module's routes) over `state`: parse its document
/// into the [`Catalogue`], put every route behind the
/// [idempotency](crate::idempotency) layer, with the
/// [`localize`](crate::i18n::localize) layer on both sides of it (inside,
/// so a stored response is already in the caller's language; outside, for
/// the idempotency layer's own problems), and install the result in the
/// state's [`Operations`]. HTTP requests and [`invoke`] then go through the
/// same router.
pub fn compose(api: OpenApiRouter<AppState>, state: AppState) -> Composed {
    let (routes, spec) = api.split_for_parts();
    let catalogue = Arc::new(Catalogue::from_openapi(&spec));
    let localize = || axum::middleware::from_fn_with_state(state.clone(), crate::i18n::localize);
    let routes = routes
        .route_layer(localize())
        .route_layer(axum::middleware::from_fn_with_state(
            idempotency::Idempotency::new(&state, Arc::clone(&catalogue)),
            idempotency::middleware,
        ))
        .route_layer(localize());
    let state = state.with_operations(Operations::new(routes.clone(), catalogue));
    Composed {
        routes,
        spec,
        state,
    }
}
