//! The contract between a domain module and the server.
//!
//! A module crate exposes `pub fn module() -> Module`; `campbooks-server`
//! collects them, merges their routers and registers their background work.
//! Routes are declared with `utoipa-axum` so every endpoint lands in the
//! OpenAPI document by construction.

use utoipa_axum::router::OpenApiRouter;

use crate::{auth::ScopeInfo, jobs::Background, state::AppState};

/// A domain module, as handed to the server.
pub struct Module {
    /// The module's name; equals its crate directory and Postgres schema.
    pub name: &'static str,
    /// The OAuth scopes the module's endpoints require, each named
    /// `<name>:<action>`; usually a `const ALL: &[ScopeInfo]` of
    /// `ScopeInfo::of::<MyScope>()` entries. The server builds the scope
    /// catalogue from these.
    pub scopes: &'static [ScopeInfo],
    /// The module's routes (under `/api/v1/<name>/…`) with their OpenAPI docs.
    pub router: OpenApiRouter<AppState>,
    /// The module's admin operations (under `/admin/v1/<name>/…`, each
    /// taking [`Admin<P>`](crate::admin::Admin)), if it has any. The server
    /// serves them as a separate API with its own document
    /// (`openapi/admin.json`), outside the operation catalogue (ADR 0015).
    pub admin_router: Option<OpenApiRouter<AppState>>,
    /// The module's jobs, event handlers and schedules, each named
    /// `<name>.<…>`. The server builds the
    /// [`Registry`](crate::jobs::Registry) from these.
    pub background: Background,
}

impl std::fmt::Debug for Module {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Module")
            .field("name", &self.name)
            .field("scopes", &self.scopes)
            .field("admin", &self.admin_router.is_some())
            .field("background", &self.background)
            .finish_non_exhaustive()
    }
}
