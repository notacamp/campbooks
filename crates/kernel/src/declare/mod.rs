//! Module declarations (ADR 0008): every module declares what it has — its
//! **record types**, **actions** and **events** — and the platform generates
//! the rest from them.
//!
//! ```ignore
//! // crates/modules/conversations/src/lib.rs
//! pub fn module() -> Module {
//!     Module {
//!         name: NAME,
//!         scopes: scopes::ALL,
//!         router: OpenApiRouter::new(),
//!         background: Background::new(),
//!         declarations: Declarations::new()
//!             .record::<Conversation>()
//!             .action(features::archive::action())
//!             .event::<ConversationArchived>(),
//!     }
//! }
//! ```
//!
//! From the declarations the server builds:
//!
//! - **one HTTP endpoint per action**, `POST /api/v1/actions/<module>.<verb>`
//!   ([`http`]): the declared scope enforced exactly like
//!   [`Scoped<S>`](crate::auth::Scoped), the input validated (problem+json
//!   422), `Idempotency-Key` honoured for mutating actions, and an undo token
//!   for undoable ones, redeemed at `POST /api/v1/actions/undo`. Every one is
//!   in the OpenAPI document with its schemas and security;
//! - **the [`Catalogue`]** (`campbooks-server declarations`, committed as
//!   `openapi/declarations.json`): every record type, action (scope, mutates,
//!   undo, shortcut, label key, schemas) and event (version, payload schema).
//!   Clients generate their shortcut registry and command menu from it; MCP
//!   tools, assistant tools and automation steps are generated from it too.
//!
//! Names are `<module>.<name>` of the declaring module (lower-case snake
//! case) and unique; the server refuses to start otherwise, and
//! `cargo xtask arch` checks it, together with a test calling each action.

mod action;
mod catalogue;
mod context;
pub mod http;
mod record;
mod store;

#[cfg(test)]
mod tests;

use std::borrow::Cow;

use utoipa::{
    ToSchema,
    openapi::{Ref, RefOr, schema::Schema},
};

pub use action::{Action, ActionDecl, DEFAULT_UNDO_WINDOW, NoScope, NoUndo, Undoable, WithUndo};
pub use catalogue::{Catalogue, DeclarationError};
pub use context::{ActionContext, TxGuard};
pub use record::{Access, InvalidRecordRef, RecordRef, RecordType, RecordTypeDecl};
pub use store::PruneActions;

use crate::events::DomainEvent;

/// What a module declares: its record types, actions and events. Part of its
/// [`Module`](crate::Module) descriptor.
#[derive(Debug, Clone, Default)]
pub struct Declarations {
    records: Vec<RecordTypeDecl>,
    actions: Vec<ActionDecl>,
    events: Vec<EventDecl>,
}

impl Declarations {
    /// Nothing declared.
    pub fn new() -> Self {
        Self::default()
    }

    /// Declare record type `R`.
    #[must_use]
    pub fn record<R: RecordType>(mut self) -> Self {
        self.records.push(RecordTypeDecl::of::<R>());
        self
    }

    /// Declare an action, built with [`Action`].
    #[must_use]
    pub fn action(mut self, action: ActionDecl) -> Self {
        self.actions.push(action);
        self
    }

    /// Declare that the module publishes event `E`; its payload schema comes
    /// from `ToSchema`.
    #[must_use]
    pub fn event<E: DomainEvent + ToSchema>(mut self) -> Self {
        self.events.push(EventDecl {
            name: E::NAME,
            version: E::VERSION,
            payload: SchemaDecl::of::<E>(),
        });
        self
    }

    /// The declared record types.
    pub fn records(&self) -> &[RecordTypeDecl] {
        &self.records
    }

    /// The declared actions.
    pub fn actions(&self) -> &[ActionDecl] {
        &self.actions
    }

    /// The declared events.
    pub fn events(&self) -> &[EventDecl] {
        &self.events
    }
}

/// A declared event: a [`DomainEvent`] the module publishes.
#[derive(Debug, Clone)]
pub struct EventDecl {
    name: &'static str,
    version: u16,
    payload: SchemaDecl,
}

impl EventDecl {
    /// Wire name, `<module>.<event>`.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Payload schema version.
    pub fn version(&self) -> u16 {
        self.version
    }

    /// The OpenAPI component name of its payload.
    pub fn payload_schema_name(&self) -> &str {
        &self.payload.name
    }
}

/// A type's JSON Schema, as a named component plus the components it refers
/// to.
#[derive(Debug, Clone)]
pub(crate) struct SchemaDecl {
    /// Component name (`ToSchema::name`).
    pub name: Cow<'static, str>,
    /// The type's own schema.
    pub schema: RefOr<Schema>,
    /// Every component it needs, itself included.
    pub components: Vec<(String, RefOr<Schema>)>,
}

impl SchemaDecl {
    pub(crate) fn of<T: ToSchema>() -> Self {
        let name = T::name();
        let schema = T::schema();
        let mut components = Vec::new();
        T::schemas(&mut components);
        components.push((name.clone().into_owned(), schema.clone()));
        Self {
            name,
            schema,
            components,
        }
    }

    /// `{"$ref": "#/components/schemas/<name>"}`.
    pub(crate) fn reference(&self) -> RefOr<Schema> {
        RefOr::Ref(Ref::from_schema_name(self.name.clone()))
    }
}

/// The default i18n key of a declared thing's label: `<module>:<kind>.<rest>`
/// (an i18next `namespace:key`, the namespace being the client module).
fn label_key(name: &str, kind: &str) -> String {
    match name.split_once('.') {
        Some((module, rest)) => format!("{module}:{kind}.{rest}"),
        None => format!("{name}:{kind}"),
    }
}
