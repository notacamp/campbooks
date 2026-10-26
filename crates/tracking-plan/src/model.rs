//! The tracking plan as data: one row type per sheet, and the closed sets
//! its dropdown columns take.

use serde::{Deserialize, Serialize};

/// Event scopes: whether an event belongs to a workspace.
pub const SCOPES: &[&str] = &["workspace", "instance"];

/// Event statuses.
pub const STATUSES: &[&str] = &["planned", "live", "deprecated"];

/// Property and snapshot field types. Any of them but `record` may be a
/// list (the `list` column). There is no free-text type.
pub const TYPES: &[&str] = &[
    "id",
    "record_ref",
    "record",
    "enum",
    "bool",
    "int",
    "decimal",
    "duration_ms",
    "timestamp",
    "date",
    "bytes",
    "currency",
    "country",
    "locale",
    "code",
];

/// Classifications a field may be written down with.
pub const CLASSIFICATIONS: &[&str] = &["id", "pseudonymous", "operational", "personal", "content"];

/// Classifications the check rejects: a field that would hold one of these
/// never ships, and writing it down keeps the rejection visible in review.
pub const REJECTED_CLASSIFICATIONS: &[&str] = &["personal", "content"];

/// The actor types, in the order the Actors sheet lists them.
pub const ACTOR_TYPES: &[&str] = &[
    "user",
    "client",
    "assistant",
    "automation",
    "system",
    "admin",
    "anonymous",
];

/// Coverage row kinds.
pub const COVERAGE_KINDS: &[&str] = &["operation", "job", "schedule", "handler"];

/// Yes/no columns.
pub const YES_NO: &[&str] = &["yes", "no"];

/// The Actors sheet's fixed rows: `(type, the id is, description)`.
pub const FIXED_ACTORS: &[(&str, &str, &str)] = &[
    (
        "user",
        "the person's Campbooks account id",
        "A person, through one of Campbooks' apps or an app they allowed.",
    ),
    (
        "client",
        "the OAuth client id when Campbooks minted it (campbooks-web, campbooks-native, cb_…, dcr_…, app_…); none for any other client (one known by its metadata document has a URL for an id)",
        "An app acting on its own behalf (client credentials).",
    ),
    (
        "assistant",
        "none; on behalf of names the person",
        "Scout, running an operation for a person.",
    ),
    (
        "automation",
        "none; on behalf of names the person",
        "A rule or workflow, running an operation for a person.",
    ),
    (
        "system",
        "the job, handler or schedule kind, or the command's name",
        "Background work or a command; on behalf of names whoever caused it, when someone did.",
    ),
    (
        "admin",
        "the administrator's Campbooks account id",
        "An instance administrator, through the admin API.",
    ),
    (
        "anonymous",
        "none",
        "Nobody signed in yet (sign-up, sign-in).",
    ),
];

/// One version of one event (Events sheet).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EventRow {
    /// `<module>.<object>_<past_verb>`.
    pub name: String,
    /// From 1; a published version never changes meaning.
    pub version: u16,
    /// The owning module (a crate under `crates/modules`, or `kernel`, `server` or `bff`).
    pub module: String,
    /// What the event is about (`conversation`).
    pub object: String,
    /// What happened to it, in the past tense (`archived`).
    pub action: String,
    /// `workspace` or `instance`.
    pub scope: String,
    /// `planned`, `live` or `deprecated`.
    pub status: String,
    /// Who may cause it.
    pub actor_types: Vec<String>,
    /// What it records.
    pub description: String,
    /// When exactly it is tracked.
    pub fires_when: String,
    /// The plan version that added this version of the event.
    pub added_in: u32,
    /// The OpenSpec change that added it.
    pub openspec_change: String,
}

/// One property of one event version (Properties sheet).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PropertyRow {
    /// The event's name.
    pub event: String,
    /// The event's version.
    pub version: u16,
    /// `snake_case` property name.
    pub property: String,
    /// One of [`TYPES`].
    #[serde(rename = "type")]
    pub kind: String,
    /// A list of values of that type.
    pub list: bool,
    /// The enum (`enum`) or record (`record`) it holds.
    pub of: String,
    /// Always present.
    pub required: bool,
    /// One of [`CLASSIFICATIONS`].
    pub classification: String,
    /// The pattern a `code` matches.
    pub pattern: String,
    /// What it holds.
    pub description: String,
    /// An example value.
    pub example: String,
}

/// One field of a snapshot shape (Records sheet).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RecordRow {
    /// `<module>.<record>`.
    pub record: String,
    /// `snake_case` field name.
    pub field: String,
    /// One of [`TYPES`] but `record`.
    #[serde(rename = "type")]
    pub kind: String,
    /// A list of values of that type.
    pub list: bool,
    /// The enum it holds, for `enum`.
    #[serde(rename = "enum")]
    pub enumeration: String,
    /// Always present.
    pub required: bool,
    /// One of [`CLASSIFICATIONS`].
    pub classification: String,
    /// The pattern a `code` matches.
    pub pattern: String,
    /// What it holds.
    pub description: String,
}

/// One value of one enum (Enums sheet).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EnumRow {
    /// `<module>.<enum>`.
    #[serde(rename = "enum")]
    pub enumeration: String,
    /// `snake_case` value.
    pub value: String,
    /// What it means.
    pub description: String,
}

/// One actor type (Actors sheet; fixed rows).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ActorRow {
    /// One of [`ACTOR_TYPES`].
    #[serde(rename = "type")]
    pub kind: String,
    /// What its id is.
    pub id_is: String,
    /// Who it is.
    pub description: String,
}

/// One operation, job, schedule or handler and what it tracks (Coverage
/// sheet).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CoverageRow {
    /// One of [`COVERAGE_KINDS`].
    pub kind: String,
    /// The `operationId`, or the job, schedule or handler kind.
    pub name: String,
    /// The events it tracks, by name.
    pub events: Vec<String>,
    /// Why it tracks none.
    pub exemption: String,
}

/// One change of the plan (Changelog sheet).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ChangelogRow {
    /// `YYYY-MM-DD`.
    pub date: String,
    /// The plan version it made, from 1.
    pub plan_version: u32,
    /// What changed.
    pub change: String,
    /// The pull request, `#123`.
    pub pr: String,
}

/// The whole plan.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Plan {
    /// Events sheet.
    pub events: Vec<EventRow>,
    /// Properties sheet.
    pub properties: Vec<PropertyRow>,
    /// Records sheet.
    pub records: Vec<RecordRow>,
    /// Enums sheet.
    pub enums: Vec<EnumRow>,
    /// Actors sheet.
    pub actors: Vec<ActorRow>,
    /// Coverage sheet.
    pub coverage: Vec<CoverageRow>,
    /// Changelog sheet.
    pub changelog: Vec<ChangelogRow>,
}

impl Plan {
    /// An empty plan with the fixed Actors rows.
    pub fn empty() -> Self {
        Self {
            actors: FIXED_ACTORS
                .iter()
                .map(|(kind, id_is, description)| ActorRow {
                    kind: (*kind).to_owned(),
                    id_is: (*id_is).to_owned(),
                    description: (*description).to_owned(),
                })
                .collect(),
            ..Self::default()
        }
    }

    /// The plan version: the newest Changelog row's (0 without one).
    pub fn version(&self) -> u32 {
        self.changelog
            .iter()
            .map(|row| row.plan_version)
            .max()
            .unwrap_or(0)
    }

    /// The date of [`version`](Self::version), `YYYY-MM-DD`.
    pub fn date(&self) -> &str {
        self.changelog
            .iter()
            .max_by_key(|row| row.plan_version)
            .map_or(DEFAULT_DATE, |row| row.date.as_str())
    }

    /// Every row in canonical order: what `fmt` writes and generation reads.
    pub fn sort(&mut self) {
        self.events
            .sort_by(|a, b| (&a.name, a.version).cmp(&(&b.name, b.version)));
        self.properties.sort_by(|a, b| {
            (&a.event, a.version, &a.property).cmp(&(&b.event, b.version, &b.property))
        });
        self.records
            .sort_by(|a, b| (&a.record, &a.field).cmp(&(&b.record, &b.field)));
        self.enums
            .sort_by(|a, b| (&a.enumeration, &a.value).cmp(&(&b.enumeration, &b.value)));
        self.actors.sort_by_key(|row| {
            ACTOR_TYPES
                .iter()
                .position(|kind| *kind == row.kind)
                .unwrap_or(usize::MAX)
        });
        self.coverage
            .sort_by(|a, b| (&a.kind, &a.name).cmp(&(&b.kind, &b.name)));
        self.changelog.sort_by_key(|row| row.plan_version);
    }

    /// The properties of `event` version `version`, in canonical order.
    pub fn properties_of<'a>(
        &'a self,
        event: &'a str,
        version: u16,
    ) -> impl Iterator<Item = &'a PropertyRow> + 'a {
        self.properties
            .iter()
            .filter(move |row| row.event == event && row.version == version)
    }

    /// The fields of record `record`, in canonical order.
    pub fn fields_of<'a>(&'a self, record: &'a str) -> impl Iterator<Item = &'a RecordRow> + 'a {
        self.records.iter().filter(move |row| row.record == record)
    }

    /// The values of enum `enumeration`, in canonical order.
    pub fn values_of<'a>(&'a self, enumeration: &'a str) -> impl Iterator<Item = &'a EnumRow> + 'a {
        self.enums
            .iter()
            .filter(move |row| row.enumeration == enumeration)
    }
}

/// The creation date of a plan with no Changelog row.
pub const DEFAULT_DATE: &str = "2026-01-01";

/// The module part of `<module>.<name>`.
pub fn module_of(qualified: &str) -> &str {
    qualified
        .split_once('.')
        .map_or(qualified, |(module, _)| module)
}

/// The name part of `<module>.<name>`.
pub fn local_of(qualified: &str) -> &str {
    qualified
        .split_once('.')
        .map_or(qualified, |(_, name)| name)
}

/// `snake_case` → `PascalCase`.
pub fn pascal(snake: &str) -> String {
    snake
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_ascii_uppercase().to_string() + chars.as_str()
            })
        })
        .collect()
}

/// The generated struct of an event version: `ConversationArchived`, then
/// `ConversationArchivedV2`.
pub fn event_type_name(event: &EventRow) -> String {
    let base = pascal(local_of(&event.name));
    if event.version <= 1 {
        base
    } else {
        format!("{base}V{}", event.version)
    }
}

/// The generated struct of a record: `conversations.conversation` →
/// `ConversationSnapshot`.
pub fn record_type_name(record: &str) -> String {
    format!("{}Snapshot", pascal(local_of(record)))
}

/// The generated enum of an enum: `conversations.stream` → `Stream`.
pub fn enum_type_name(enumeration: &str) -> String {
    pascal(local_of(enumeration))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(pascal("conversation_archived"), "ConversationArchived");
        assert_eq!(
            record_type_name("conversations.conversation"),
            "ConversationSnapshot"
        );
        assert_eq!(
            enum_type_name("conversations.stream_source"),
            "StreamSource"
        );
        let mut event = EventRow {
            name: "conversations.conversation_archived".into(),
            version: 1,
            ..EventRow::default()
        };
        assert_eq!(event_type_name(&event), "ConversationArchived");
        event.version = 2;
        assert_eq!(event_type_name(&event), "ConversationArchivedV2");
    }

    #[test]
    fn version_and_date_come_from_the_newest_changelog_row() {
        let mut plan = Plan::empty();
        assert_eq!((plan.version(), plan.date()), (0, DEFAULT_DATE));
        plan.changelog.push(ChangelogRow {
            date: "2026-10-23".into(),
            plan_version: 2,
            ..ChangelogRow::default()
        });
        plan.changelog.push(ChangelogRow {
            date: "2026-10-01".into(),
            plan_version: 1,
            ..ChangelogRow::default()
        });
        assert_eq!((plan.version(), plan.date()), (2, "2026-10-23"));
    }
}
