//! Record types and references to records of any module.

use serde::{Deserialize, Serialize};
use utoipa::{
    ToSchema,
    openapi::{RefOr, schema::Schema},
};
use uuid::Uuid;

use crate::names;

/// A pointer to one record of any module: its record type
/// (`<module>.<record>`) and its id.
///
/// This is how a task points at the message it came from, and how attention,
/// search, automation and custom fields point at anything without knowing
/// the module that owns it. On the wire:
/// `{"type": "conversations.conversation", "id": "0192…"}`; a malformed
/// type is refused when deserializing.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, ToSchema)]
#[serde(try_from = "RawRecordRef")]
pub struct RecordRef {
    /// The record type, `<module>.<record>`.
    #[serde(rename = "type")]
    #[schema(
        pattern = r"^[a-z][a-z0-9_]*\.[a-z][a-z0-9_]*$",
        example = "conversations.conversation"
    )]
    record_type: String,
    /// The record's id.
    id: Uuid,
}

impl RecordRef {
    /// A reference to the record `id` of type `R`.
    pub fn to<R: RecordType>(id: impl Into<Uuid>) -> Self {
        Self {
            record_type: R::NAME.to_owned(),
            id: id.into(),
        }
    }

    /// A reference from a type name read at run time (an event, a rule).
    pub fn new(record_type: impl Into<String>, id: Uuid) -> Result<Self, InvalidRecordRef> {
        let record_type = record_type.into();
        if names::is_dotted(&record_type) {
            Ok(Self { record_type, id })
        } else {
            Err(InvalidRecordRef(record_type))
        }
    }

    /// The record type, `<module>.<record>`.
    pub fn record_type(&self) -> &str {
        &self.record_type
    }

    /// The module owning the record (the type's prefix).
    pub fn module(&self) -> &str {
        self.record_type
            .split_once('.')
            .map_or(self.record_type.as_str(), |(module, _)| module)
    }

    /// The record's id.
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Whether this points at a record of type `R`.
    pub fn is<R: RecordType>(&self) -> bool {
        self.record_type == R::NAME
    }
}

impl std::fmt::Display for RecordRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.record_type, self.id)
    }
}

/// A record type name that is not `<module>.<record>` (lower-case snake case).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("record type `{0}` must be `<module>.<record>` (lower-case snake case)")]
pub struct InvalidRecordRef(pub String);

#[derive(Deserialize)]
struct RawRecordRef {
    #[serde(rename = "type")]
    record_type: String,
    id: Uuid,
}

impl TryFrom<RawRecordRef> for RecordRef {
    type Error = InvalidRecordRef;

    fn try_from(raw: RawRecordRef) -> Result<Self, Self::Error> {
        Self::new(raw.record_type, raw.id)
    }
}

/// A kind of record a module owns, declared in its
/// [`Declarations`](super::Declarations) with `.record::<R>()`.
///
/// The type itself is the record's **public shape** — what other modules,
/// rules and clients may see of it — and its JSON Schema comes from
/// `ToSchema`. It need not be the module's table row.
///
/// ```
/// use campbooks_kernel::declare::{Access, RecordType};
///
/// /// A conversation: messages exchanged with the same people about one thing.
/// #[derive(serde::Serialize, utoipa::ToSchema)]
/// pub struct Conversation {
///     pub id: uuid::Uuid,
///     pub subject: String,
/// }
///
/// impl RecordType for Conversation {
///     const NAME: &'static str = "conversations.conversation";
///     const ACCESS: Access = Access::Container("conversations.mailbox");
/// }
/// ```
pub trait RecordType: ToSchema + 'static {
    /// Stable wire name, `<module>.<record>`. Stored in every
    /// [`RecordRef`] pointing at one: never rename it.
    const NAME: &'static str;
    /// Who may see a record of this type.
    const ACCESS: Access;
    /// The i18n key of its human name (singular), when not the default
    /// `<module>:records.<record>` (an i18next `namespace:key`).
    const LABEL: Option<&'static str> = None;
}

/// What decides who may see a record: its **access container**. The `Access`
/// port (when it lands) answers the question per container; modules and
/// search only need to know which container a record sits in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "container", rename_all = "snake_case")]
pub enum Access {
    /// Every member of the record's workspace.
    Workspace,
    /// Only the member it belongs to (a draft, a personal note).
    Member,
    /// Whoever may see the record it sits in — a mailbox, a calendar, a
    /// folder — named by that record's type (`<module>.<record>`), which
    /// must itself be declared.
    Container(&'static str),
}

/// A declared record type, as the catalogue lists it.
#[derive(Debug, Clone)]
pub struct RecordTypeDecl {
    pub(crate) name: &'static str,
    pub(crate) label: Option<&'static str>,
    pub(crate) access: Access,
    pub(crate) schema: super::SchemaDecl,
}

impl RecordTypeDecl {
    pub(crate) fn of<R: RecordType>() -> Self {
        Self {
            name: R::NAME,
            label: R::LABEL,
            access: R::ACCESS,
            schema: super::SchemaDecl::of::<R>(),
        }
    }

    /// Wire name, `<module>.<record>`.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Who may see a record of this type.
    pub fn access(&self) -> Access {
        self.access
    }

    /// The i18n key of its human name.
    pub fn label(&self) -> String {
        self.label
            .map_or_else(|| super::label_key(self.name, "records"), str::to_owned)
    }

    /// The JSON Schema of its public shape.
    pub fn schema(&self) -> &RefOr<Schema> {
        &self.schema.schema
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[derive(ToSchema)]
    struct Thing;

    impl RecordType for Thing {
        const NAME: &'static str = "test.thing";
        const ACCESS: Access = Access::Workspace;
    }

    #[test]
    fn serializes_as_type_and_id() {
        let id = Uuid::now_v7();
        let reference = RecordRef::to::<Thing>(id);
        assert_eq!(
            serde_json::to_value(&reference).unwrap(),
            json!({"type": "test.thing", "id": id})
        );
        assert!(reference.is::<Thing>());
        assert_eq!(reference.module(), "test");
        assert_eq!(reference.to_string(), format!("test.thing:{id}"));
    }

    #[test]
    fn deserializing_checks_the_type_name() {
        let id = Uuid::now_v7();
        let parsed: RecordRef =
            serde_json::from_value(json!({"type": "test.thing", "id": id})).unwrap();
        assert_eq!(parsed, RecordRef::to::<Thing>(id));
        for bad in ["thing", "Test.Thing", "test.", ".thing", "test.thing.x"] {
            let err = serde_json::from_value::<RecordRef>(json!({"type": bad, "id": id}))
                .unwrap_err()
                .to_string();
            assert!(err.contains("<module>.<record>"), "{bad}: {err}");
        }
    }

    #[test]
    fn schema_names_the_wire_fields() {
        let schema = serde_json::to_value(<RecordRef as utoipa::PartialSchema>::schema()).unwrap();
        assert!(schema["properties"]["type"].is_object(), "{schema}");
        assert_eq!(schema["properties"]["id"]["format"], "uuid");
    }

    #[test]
    fn access_serializes_with_its_kind() {
        assert_eq!(
            serde_json::to_value(Access::Workspace).unwrap(),
            json!({"kind": "workspace"})
        );
        assert_eq!(
            serde_json::to_value(Access::Container("mail.mailbox")).unwrap(),
            json!({"kind": "container", "container": "mail.mailbox"})
        );
    }

    #[test]
    fn label_defaults_to_the_module_namespace() {
        assert_eq!(RecordTypeDecl::of::<Thing>().label(), "test:records.thing");
    }
}
