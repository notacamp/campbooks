//! Cross-module record references (ADR 0008, ADR 0013).
//!
//! Modules never join each other's tables; when one has to point at a record
//! another owns (an activity entry, a rule's target, a search hit, a link
//! between two records), it stores a [`RecordRef`]: the record's type and id,
//! nothing else.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// A pointer to any record of any module.
///
/// `type` is `<module>.<record>` (for example `conversations.thread`), the
/// same names operations list in `x-campbooks.records`; `id` is the record's
/// id as its module writes it (usually a UUID).
///
/// ```
/// use campbooks_kernel::records::RecordRef;
///
/// let thread = RecordRef::new(
///     "conversations.thread",
///     "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c11",
/// );
/// assert_eq!(thread.module(), Some("conversations"));
/// assert_eq!(
///     serde_json::to_value(&thread).unwrap(),
///     serde_json::json!({"type": "conversations.thread", "id": "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c11"})
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, ToSchema)]
pub struct RecordRef {
    /// The record type, `<module>.<record>`.
    #[serde(rename = "type")]
    #[schema(example = "conversations.thread")]
    pub record_type: String,
    /// The record's id within its module.
    #[schema(example = "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c11")]
    pub id: String,
}

impl RecordRef {
    /// A reference to record `id` (any id type, or a string) of type
    /// `record_type` (`<module>.<record>`).
    pub fn new(record_type: impl Into<String>, id: &(impl ToString + ?Sized)) -> Self {
        Self {
            record_type: record_type.into(),
            id: id.to_string(),
        }
    }

    /// The module that owns the record: the part of `type` before the dot,
    /// or `None` when `type` is not `<module>.<record>`.
    pub fn module(&self) -> Option<&str> {
        is_record_type(&self.record_type)
            .then(|| self.record_type.split_once('.').map(|(module, _)| module))
            .flatten()
    }
}

/// Whether `name` is a well-formed record type, `<module>.<record>`, both
/// parts lower-case snake case.
pub fn is_record_type(name: &str) -> bool {
    name.split_once('.')
        .is_some_and(|(module, record)| is_snake(module) && is_snake(record))
}

fn is_snake(s: &str) -> bool {
    s.starts_with(|c: char| c.is_ascii_lowercase())
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_with_a_type_field() {
        let reference = RecordRef::new("mail.message", &42);
        let json = serde_json::to_value(&reference).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"type": "mail.message", "id": "42"})
        );
        assert_eq!(
            serde_json::from_value::<RecordRef>(json).unwrap(),
            reference
        );
    }

    #[test]
    fn module_needs_a_well_formed_type() {
        assert_eq!(RecordRef::new("mail.message", &1).module(), Some("mail"));
        assert_eq!(RecordRef::new("message", &1).module(), None);
        assert_eq!(RecordRef::new("Mail.Message", &1).module(), None);
        assert!(is_record_type("mail_sync.cursor_2"));
        assert!(!is_record_type("mail.message.part"));
        assert!(!is_record_type(".message"));
    }
}
