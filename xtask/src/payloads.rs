//! Personal data stays out of what the queue and the event log keep
//! (campbooks-cloud#65): event payloads, job payloads and the reasons jobs
//! fail (`kernel.jobs.last_error`). The event log is kept for 90 days and
//! finished jobs for up to 30, longer than a member's or a workspace's data
//! once they ask for it to go; personal data (addresses, names, subjects,
//! bodies) travels as blob references instead, which the purges delete.
//!
//! - **Event payloads**: a field named like personal data ([`personal`])
//!   must hold an opaque value (a blob reference, an id, a closed set of
//!   values, a number or a flag) or be listed in [`EVENT_FIELDS_ALLOWED`]
//!   with the reason it is none.
//! - **Job payloads**: every field of every job kind's payload is listed in
//!   [`JOB_PAYLOAD_FIELDS`] with the reason it is no personal data: a field
//!   by field audit, so a new field is reviewed before it is stored.
//! - **Failure reasons**: every value a job's error message is built from
//!   (other than its own wording) is listed in [`JOB_ERROR_VALUES`] with the
//!   reason it is no personal data. `redact::job_error` masks quotes,
//!   addresses, links and tokens as well, but only as a backstop.
//!
//! Each list's entries must still match something: a stale entry is
//! reported, so the lists stay an accurate audit.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use regex::Regex;

use crate::access::{is_test_file, without_comments, without_tests};
use crate::arch::name_words;

/// Words that name personal data, matched against a field name's words
/// (singular or plural): addresses and who sent or received, names, and
/// what people wrote.
const PERSONAL_WORDS: [&str; 24] = [
    "email",
    "mail",
    "address",
    "phone",
    "from",
    "to",
    "cc",
    "bcc",
    "sender",
    "recipient",
    "participant",
    "name",
    "display",
    "subject",
    "title",
    "body",
    "text",
    "content",
    "snippet",
    "summary",
    "note",
    "comment",
    "message",
    "signature",
];

/// Whether a field name reads like personal data, by its words.
pub fn personal(name: &str) -> bool {
    name_words(name).iter().any(|word| {
        let singular = word.strip_suffix('s').unwrap_or(word);
        PERSONAL_WORDS.contains(&word.as_str()) || PERSONAL_WORDS.contains(&singular)
    })
}

/// Event payload fields named like personal data that hold none, each with
/// the reason: `(event, field, reason)`.
pub const EVENT_FIELDS_ALLOWED: &[(&str, &str, &str)] = &[
    (
        "connectors.connection_added",
        "account_subject",
        PROVIDER_SUBJECT,
    ),
    (
        "connectors.connection_removed",
        "account_subject",
        PROVIDER_SUBJECT,
    ),
    (
        "connectors.connection_added",
        "grantor_subject",
        "the grantor's identity subject: an opaque account id, read only with the identity \
         module's tables; never an address",
    ),
    (
        "connectors.send_completed",
        "provider_message_id",
        PROVIDER_MESSAGE_ID,
    ),
    (
        "conversations.changes_requested",
        "provider_message_id",
        PROVIDER_MESSAGE_ID,
    ),
    (
        "conversations.conversation_changed",
        "provider_message_ids",
        PROVIDER_MESSAGE_ID,
    ),
    (
        "conversations.message_received",
        "provider_message_id",
        PROVIDER_MESSAGE_ID,
    ),
    (
        "conversations.message_sent",
        "provider_message_id",
        PROVIDER_MESSAGE_ID,
    ),
    (
        "workspaces.member_left",
        "subject",
        "the former member's identity subject: an opaque account id, so the issuer removes \
         the apps they registered; never an address",
    ),
];

const PROVIDER_MESSAGE_ID: &str = "the provider's id of a message (Gmail's hex id, Zoho's \
     number): opaque, never its content or its sender";

const PROVIDER_SUBJECT: &str = "the provider's stable id of the account (the OpenID `sub`): \
     opaque, never an address";

/// Every field of every job kind's payload, each with the reason it is no
/// personal data: `(kind, field, reason)`. A unit payload has none.
pub const JOB_PAYLOAD_FIELDS: &[(&str, &str, &str)] = &[
    ("kernel.sequence_changes", "workspace", WORKSPACE_ID),
    ("workspaces.purge_workspace", "workspace", WORKSPACE_ID),
    (
        "search.backfill",
        "group_type",
        "a record type's name (`<module>.<record>`), from the code",
    ),
    (
        "search.backfill",
        "after",
        "the id of the last record indexed (a cursor), opaque",
    ),
    ("connectors.send_message", "send", "the send's id (a UUID)"),
    (
        "connectors.send_message",
        "give_up",
        "a flag: stop retrying",
    ),
    (
        "connectors.sync_stream",
        "stream",
        "the stream's id (a UUID)",
    ),
    ("conversations.apply_changes", "inbox", INBOX_ID),
    (
        "conversations.apply_changes",
        "batch",
        "a blob reference to the batch of provider changes: the content stays in the blob \
         store, which the purges delete",
    ),
    (
        "conversations.apply_rule",
        "run",
        "the rule run's id (a UUID)",
    ),
    ("conversations.apply_rule", "after", CONVERSATION_CURSOR),
    (
        "conversations.apply_rule",
        "counting",
        "a flag: count matches without acting",
    ),
    ("conversations.backfill_needs", "inbox", INBOX_ID),
    ("conversations.backfill_needs", "after", CONVERSATION_CURSOR),
    ("conversations.check_send", "draft", DRAFT_ID),
    (
        "conversations.check_send",
        "send_id",
        "the send's id (a UUID)",
    ),
    ("conversations.decide_backlog", "message", MESSAGE_ID),
    ("conversations.fill_templates", "inbox", INBOX_ID),
    ("conversations.fill_templates", "page", "a page number"),
    (
        "conversations.refresh_need",
        "conversation",
        "the conversation's id (a UUID)",
    ),
    ("conversations.release_send", "draft", DRAFT_ID),
    (
        "conversations.release_send",
        "generation",
        "a counter of the draft's sends",
    ),
    ("conversations.notice_rules", "inbox", INBOX_ID),
    ("conversations.summarize_need", "message", MESSAGE_ID),
];

const WORKSPACE_ID: &str = "the workspace's id (a UUID)";
const INBOX_ID: &str = "the inbox's id (a UUID)";
const DRAFT_ID: &str = "the draft's id (a UUID)";
const MESSAGE_ID: &str = "the message's id (a UUID), never its content";
const CONVERSATION_CURSOR: &str = "the id of the last conversation done (a cursor), opaque";

/// Every value a job's error message is built from, other than its own
/// wording, each with the reason it is no personal data:
/// `(path, expression, reason)`.
pub const JOB_ERROR_VALUES: &[(&str, &str, &str)] = &[
    (
        "crates/kernel/src/jobs/mod.rs",
        "crate::redact::sqlx(&err)",
        REDACTED_SQL,
    ),
    (
        "crates/kernel/src/jobs/mod.rs",
        "err.to_string()",
        KERNEL_ERROR,
    ),
    (
        "crates/kernel/src/jobs/mod.rs",
        "other.to_string()",
        KERNEL_ERROR,
    ),
    ("crates/kernel/src/jobs/registry.rs", "E::NAME", CODE_NAME),
    ("crates/kernel/src/jobs/registry.rs", "H::NAME", CODE_NAME),
    ("crates/kernel/src/jobs/registry.rs", "J::NAME", CODE_NAME),
    ("crates/kernel/src/jobs/registry.rs", "E::VERSION", VERSION),
    ("crates/kernel/src/jobs/registry.rs", "row.version", VERSION),
    (
        "crates/kernel/src/jobs/registry.rs",
        "row.name",
        "the logged event's name, a name from the code",
    ),
    (
        "crates/kernel/src/jobs/registry.rs",
        "id",
        "the event's id (a UUIDv7)",
    ),
    (
        "crates/kernel/src/jobs/registry.rs",
        "crate::redact::json(&e)",
        REDACTED_JSON,
    ),
    (
        "crates/kernel/src/jobs/worker.rs",
        "crate::redact::sqlx(&e)",
        REDACTED_SQL,
    ),
    (
        "crates/kernel/src/jobs/worker.rs",
        "info.kind",
        "the queued job's kind, a name from the code",
    ),
    (
        "crates/modules/connectors/src/features/member_left/mod.rs",
        "e.to_string()",
        KERNEL_ERROR,
    ),
    (
        "crates/modules/connectors/src/features/request_changes/mod.rs",
        "campbooks_kernel::redact::json(&e)",
        REDACTED_JSON,
    ),
    (
        "crates/modules/connectors/src/features/send_message/mod.rs",
        "OUTGOING_CONTENT_TYPE",
        "a content type constant",
    ),
    (
        "crates/modules/connectors/src/features/send_message/mod.rs",
        "document.version",
        VERSION,
    ),
    (
        "crates/modules/connectors/src/features/send_message/mod.rs",
        "e.line()",
        POSITION,
    ),
    (
        "crates/modules/connectors/src/features/send_message/mod.rs",
        "e.column()",
        POSITION,
    ),
    (
        "crates/modules/connectors/src/features/send_message/mod.rs",
        "e.to_string()",
        READ_ERROR,
    ),
    (
        "crates/modules/connectors/src/features/send_message/mod.rs",
        "err",
        PROVIDER_ERROR,
    ),
    (
        "crates/modules/connectors/src/features/send_message/mod.rs",
        "redact::json(&e)",
        REDACTED_JSON,
    ),
    (
        "crates/modules/connectors/src/features/send_message/mod.rs",
        "send",
        "the send's id (a UUID)",
    ),
    (
        "crates/modules/connectors/src/features/send_message/via_zoho.rs",
        "err",
        PROVIDER_ERROR,
    ),
    (
        "crates/modules/connectors/src/features/send_message/via_zoho.rs",
        "why",
        "why Zoho could not say whether an attempt was sent: a provider failure (as for \
         `ProviderError`) or the code's own wording",
    ),
    (
        "crates/modules/connectors/src/features/sync_stream/mod.rs",
        "other.to_string()",
        READ_ERROR,
    ),
    (
        "crates/modules/connectors/src/features/sync_stream/mod.rs",
        "redact::json(&e)",
        REDACTED_JSON,
    ),
    (
        "crates/modules/connectors/src/features/sync_stream/mod.rs",
        "target.stream",
        "the stream's id (a UUID)",
    ),
    (
        "crates/modules/connectors/src/features/workspace_deleting/mod.rs",
        "e.to_string()",
        KERNEL_ERROR,
    ),
    (
        "crates/modules/conversations/src/domain/redact.rs",
        "status",
        "an HTTP status code",
    ),
    (
        "crates/modules/conversations/src/features/apply_changes/mod.rs",
        "BATCH_VERSION",
        VERSION,
    ),
    (
        "crates/modules/conversations/src/features/apply_changes/mod.rs",
        "batch.version",
        VERSION,
    ),
    (
        "crates/modules/conversations/src/features/apply_changes/mod.rs",
        "blob.hash",
        "a blob's content hash (SHA-256): it identifies bytes, it cannot be read back into them",
    ),
    (
        "crates/modules/conversations/src/features/apply_changes/mod.rs",
        "redact::json(&e)",
        REDACTED_JSON,
    ),
    (
        "crates/modules/conversations/src/features/create_inbox/mod.rs",
        "redact::json(&e)",
        REDACTED_JSON,
    ),
    (
        "crates/modules/conversations/src/features/decide_message/mod.rs",
        "redact::json(&e)",
        REDACTED_JSON,
    ),
    (
        "crates/modules/conversations/src/features/release_send/mod.rs",
        "redact::json(&e)",
        REDACTED_JSON,
    ),
    (
        "crates/modules/conversations/src/features/summarize_need/mod.rs",
        "redact::json(&e)",
        REDACTED_JSON,
    ),
    (
        "crates/modules/identity/src/features/forget_connected_way_in/mod.rs",
        "e.to_string()",
        KERNEL_ERROR,
    ),
    (
        "crates/modules/identity/src/features/member_left/mod.rs",
        "e.to_string()",
        KERNEL_ERROR,
    ),
    (
        "crates/modules/identity/src/features/name_connected_mailbox/mod.rs",
        "e.to_string()",
        KERNEL_ERROR,
    ),
    (
        "crates/modules/identity/src/features/notify_way_in_added/mod.rs",
        "err.to_string()",
        "a mail transport failure (`MailError::Unavailable` or `NotConfigured`): fixed wording, \
         no recipient or content",
    ),
    (
        "crates/modules/identity/src/features/workspace_deleting/mod.rs",
        "e.to_string()",
        KERNEL_ERROR,
    ),
    ("crates/modules/identity/src/prune.rs", "e", KERNEL_ERROR),
    (
        "crates/modules/identity/src/prune.rs",
        "redact::sqlx(&e)",
        REDACTED_SQL,
    ),
    (
        "crates/modules/workspaces/src/features/delete_workspace/mod.rs",
        "e",
        "a blob store error (`BlobError`): its kind, a limit, a content hash, or the storage \
         backend's failure (its object keys are workspace ids and hashes, never names)",
    ),
];

const REDACTED_SQL: &str =
    "a database error by kind (`redact::sqlx`: SQLSTATE, constraint, table), never its message";
const REDACTED_JSON: &str =
    "a JSON error by kind and position (`redact::json`), never the text it failed on";
const KERNEL_ERROR: &str = "a kernel error (`campbooks_kernel::Error`): its kind and reason code \
     (`not found`, `bad request: <code>`), never internal detail";
const CODE_NAME: &str = "a job, handler or event name from the code";
const VERSION: &str = "a payload schema version number";
const POSITION: &str = "a line or column number";
const PROVIDER_ERROR: &str = "a provider failure (`ProviderError`): its kind and, for an \
     unavailable or unexpected answer, a description the clients build from status codes, error \
     codes, URLs without their path and decode errors; `redact::job_error` masks quotes, \
     addresses and tokens on top";
const READ_ERROR: &str = "a mail read failure (`ReadError`): a provider failure (as for \
     `ProviderError`), a database error by kind, a blob store error, or what stored data did not \
     decode, never the data";

/// [`EVENT_FIELDS_ALLOWED`] against the events catalogue (`events`, what
/// `openapi/events.json` holds): every field of every event's payload, at
/// any depth, named like personal data ([`personal`]) holds an opaque value
/// ([`opaque`]) or is listed with its reason. An entry naming a field no
/// event carries any more is stale.
pub fn check_event_payloads(
    events: &serde_json::Value,
    allowed: &[(&str, &str, &str)],
) -> Vec<String> {
    let mut violations = Vec::new();
    let mut used = BTreeSet::new();
    let schemas = events.pointer("/components/schemas");
    let list = events
        .get("events")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    for event in &list {
        let name = event
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let Some(payload) = event.get("payload") else {
            continue;
        };
        let mut fields = BTreeMap::new();
        properties(payload, schemas, &mut BTreeSet::new(), &mut fields);
        for (field, schemas_of) in &fields {
            if !personal(field) {
                continue;
            }
            if allowed.iter().any(|(e, f, _)| *e == name && f == field) {
                used.insert((name.to_owned(), field.clone()));
                continue;
            }
            if schemas_of.iter().all(|schema| opaque(schema, schemas)) {
                continue;
            }
            violations.push(format!(
                "event `{name}` carries `{field}`, named like personal data, as a value that \
                 can hold it: send it as a blob reference (personal data stays out of the \
                 event log, AGENTS.md), or list it in `EVENT_FIELDS_ALLOWED` with the reason \
                 it is none"
            ));
        }
    }
    for (event, field, reason) in allowed {
        if reason.trim().is_empty() {
            violations.push(format!(
                "`EVENT_FIELDS_ALLOWED` lists `{field}` of `{event}` without a reason"
            ));
        }
        if !used.contains(&((*event).to_owned(), (*field).to_owned())) {
            violations.push(format!(
                "`EVENT_FIELDS_ALLOWED` lists `{field}` of `{event}`, which that event no \
                 longer carries: remove the entry"
            ));
        }
    }
    violations
}

/// Every property of `schema`, at any depth (through `$ref`, `allOf`,
/// `oneOf`, `anyOf`, `items` and `additionalProperties`), with the schemas
/// it is declared with.
fn properties(
    schema: &serde_json::Value,
    schemas: Option<&serde_json::Value>,
    seen: &mut BTreeSet<String>,
    out: &mut BTreeMap<String, Vec<serde_json::Value>>,
) {
    let Some(object) = schema.as_object() else {
        return;
    };
    if let Some(name) = reference(schema) {
        // A blob reference is opaque as a whole: its own fields (a hash, a
        // size, a media type) describe stored bytes, not a person.
        if BLOB_REFERENCES.contains(&name) {
            return;
        }
        if seen.insert(name.to_owned())
            && let Some(target) = schemas.and_then(|s| s.get(name))
        {
            properties(target, schemas, seen, out);
        }
        return;
    }
    if let Some(props) = object
        .get("properties")
        .and_then(serde_json::Value::as_object)
    {
        for (name, property) in props {
            out.entry(name.clone()).or_default().push(property.clone());
            properties(property, schemas, seen, out);
        }
    }
    for key in ["allOf", "oneOf", "anyOf"] {
        for part in object
            .get(key)
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
        {
            properties(part, schemas, seen, out);
        }
    }
    for key in ["items", "additionalProperties"] {
        if let Some(inner) = object.get(key) {
            properties(inner, schemas, seen, out);
        }
    }
}

/// The schemas of a stored blob's reference, under the names the events
/// catalogue gives them: the content stays in the blob store, which the
/// purges delete.
const BLOB_REFERENCES: [&str; 2] = ["BlobPointer", "BlobRef"];

/// The schema name a `$ref` points at.
fn reference(schema: &serde_json::Value) -> Option<&str> {
    schema
        .get("$ref")
        .and_then(serde_json::Value::as_str)
        .and_then(|r| r.rsplit('/').next())
}

/// Whether a value of `schema` cannot hold personal data inline: a blob
/// reference ([`BLOB_REFERENCES`]), a UUID,
/// a closed set of strings (`enum`), a number or a flag, a list of such, or
/// a choice between such and `null`.
fn opaque(schema: &serde_json::Value, schemas: Option<&serde_json::Value>) -> bool {
    if let Some(name) = reference(schema) {
        if BLOB_REFERENCES.contains(&name) {
            return true;
        }
        return schemas
            .and_then(|s| s.get(name))
            .is_some_and(|target| reference(target).is_none() && opaque(target, schemas));
    }
    for key in ["oneOf", "anyOf"] {
        if let Some(parts) = schema.get(key).and_then(serde_json::Value::as_array) {
            return parts.iter().all(|part| {
                part.get("type").and_then(serde_json::Value::as_str) == Some("null")
                    || opaque(part, schemas)
            });
        }
    }
    if schema.get("enum").is_some() {
        return true;
    }
    let types: Vec<&str> = match schema.get("type") {
        Some(serde_json::Value::String(t)) => vec![t.as_str()],
        Some(serde_json::Value::Array(ts)) => ts.iter().filter_map(|t| t.as_str()).collect(),
        _ => return false,
    };
    types.iter().all(|t| match *t {
        "null" | "integer" | "number" | "boolean" => true,
        "string" => schema.get("format").and_then(serde_json::Value::as_str) == Some("uuid"),
        "array" => schema
            .get("items")
            .is_some_and(|items| opaque(items, schemas)),
        _ => false,
    })
}

/// A job kind's payload as the sources declare it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobPayload {
    /// The kind (`Job::NAME` / `JobHandler::NAME`).
    pub kind: String,
    /// Where the job is implemented.
    pub path: String,
    /// The payload's fields (none for a unit payload); `None` when its
    /// struct was not found.
    pub fields: Option<Vec<String>>,
}

static JOB_IMPL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\bimpl\s*(?:<[^>]*>)?\s*(?:[\w:]+::)?(Job|JobHandler)\s+for\s+(\w+)")
        .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
});
static KIND_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"const\s+NAME\s*:\s*&\s*(?:'static\s+)?str\s*=\s*(?:"([^"]+)"|([\w:]+))\s*;"#)
        .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
});
static PAYLOAD_TYPE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"type\s+Payload\s*=\s*([\w:]+)")
        .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
});
static STR_CONST: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"const\s+(\w+)\s*:\s*&\s*(?:'static\s+)?str\s*=\s*"([^"]+)"\s*;"#)
        .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
});

/// Every job kind the sources implement (`impl Job for …`, `impl
/// JobHandler for …`), outside tests and comments, with its payload's
/// fields.
pub fn job_payloads(sources: &[(String, String)]) -> Vec<JobPayload> {
    let code: Vec<(String, String)> = sources
        .iter()
        .filter(|(path, _)| !is_test_file(path))
        .map(|(path, text)| (path.clone(), without_comments(without_tests(text))))
        .collect();
    let constants: BTreeMap<String, String> = code
        .iter()
        .flat_map(|(_, text)| STR_CONST.captures_iter(text))
        .map(|c| (c[1].to_owned(), c[2].to_owned()))
        .collect();
    let mut payloads = Vec::new();
    for (path, text) in &code {
        for found in JOB_IMPL.captures_iter(text) {
            let Some(whole) = found.get(0) else {
                continue;
            };
            let Some(body) = block_after(text, whole.end()) else {
                continue;
            };
            let Some(name) = KIND_NAME.captures(body) else {
                continue;
            };
            let kind = match (name.get(1), name.get(2)) {
                (Some(literal), _) => literal.as_str().to_owned(),
                (None, Some(constant)) => {
                    let last = constant.as_str().rsplit("::").next().unwrap_or_default();
                    constants
                        .get(last)
                        .cloned()
                        .unwrap_or_else(|| constant.as_str().to_owned())
                }
                (None, None) => continue,
            };
            let payload = if &found[1] == "Job" {
                found[2].to_owned()
            } else {
                match PAYLOAD_TYPE.captures(body) {
                    Some(t) => t[1].rsplit("::").next().unwrap_or_default().to_owned(),
                    None => continue,
                }
            };
            payloads.push(JobPayload {
                kind,
                path: path.clone(),
                fields: struct_fields(&code, path, &payload),
            });
        }
    }
    payloads.sort_by(|a, b| a.kind.cmp(&b.kind));
    payloads
}

/// The text of the `{ … }` block that starts after `from`.
fn block_after(text: &str, from: usize) -> Option<&str> {
    let open = from + text[from..].find('{')?;
    let mut depth = 0usize;
    for (i, c) in text[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&text[open + 1..open + i]);
                }
            }
            _ => {}
        }
    }
    None
}

/// The fields of struct `name`, looked for in `path` first, then anywhere:
/// none for a unit struct, `0`, `1`, … for a tuple struct.
fn struct_fields(code: &[(String, String)], path: &str, name: &str) -> Option<Vec<String>> {
    let header = Regex::new(&format!(
        r"\bstruct\s+{}\b\s*(?:<[^>{{;]*>)?",
        regex::escape(name)
    ))
    .ok()?;
    let ordered = code
        .iter()
        .filter(|(p, _)| p == path)
        .chain(code.iter().filter(|(p, _)| p != path));
    for (_, text) in ordered {
        let Some(found) = header.find(text) else {
            continue;
        };
        let rest = text[found.end()..].trim_start();
        if rest.starts_with(';') {
            return Some(Vec::new());
        }
        if let Some(tuple) = rest.strip_prefix('(') {
            let inner = &tuple[..closing(tuple, '(', ')')?];
            let count = split_top_level(inner, true).len();
            return Some((0..count).map(|i| i.to_string()).collect());
        }
        let body = block_after(rest, 0)?;
        return Some(
            split_top_level(body, true)
                .iter()
                .filter_map(|field| {
                    let field = strip_attributes(field);
                    let (name, _) = field.split_once(':')?;
                    let name = name.trim();
                    let name = name.rsplit(char::is_whitespace).next().unwrap_or(name);
                    (!name.is_empty()).then(|| name.to_owned())
                })
                .collect(),
        );
    }
    None
}

/// Where the bracket opened just before `text` closes, as an index into
/// `text`.
fn closing(text: &str, open: char, close: char) -> Option<usize> {
    let mut depth = 1usize;
    for (i, c) in text.char_indices() {
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
    }
    None
}

/// `text` without its leading `#[…]` attributes.
fn strip_attributes(text: &str) -> &str {
    let mut rest = text.trim_start();
    while let Some(attribute) = rest.strip_prefix("#[") {
        let Some(end) = closing(attribute, '[', ']') else {
            return rest;
        };
        rest = attribute[end + 1..].trim_start();
    }
    rest
}

/// `text` split at its top-level commas (outside brackets and strings; with
/// `angles`, outside `<…>` too, for types), each part trimmed, empty parts
/// dropped.
fn split_top_level(text: &str, angles: bool) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut current = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                current.push(c);
                while let Some(s) = chars.next() {
                    current.push(s);
                    if s == '\\' {
                        if let Some(escaped) = chars.next() {
                            current.push(escaped);
                        }
                    } else if s == '"' {
                        break;
                    }
                }
            }
            '(' | '[' | '{' => {
                depth += 1;
                current.push(c);
            }
            ')' | ']' | '}' => {
                depth -= 1;
                current.push(c);
            }
            '<' if angles => {
                depth += 1;
                current.push(c);
            }
            '>' if angles => {
                depth -= 1;
                current.push(c);
            }
            ',' if depth == 0 => parts.push(std::mem::take(&mut current)),
            _ => current.push(c),
        }
    }
    parts.push(current);
    parts
        .into_iter()
        .map(|p| p.trim().to_owned())
        .filter(|p| !p.is_empty())
        .collect()
}

/// [`JOB_PAYLOAD_FIELDS`] against the job kinds the sources implement:
/// every field of every payload is listed with its reason, a payload whose
/// struct cannot be read is reported, and an entry naming a field no
/// payload has any more is stale.
pub fn check_job_payloads(payloads: &[JobPayload], audit: &[(&str, &str, &str)]) -> Vec<String> {
    let mut violations = Vec::new();
    let mut used = BTreeSet::new();
    for payload in payloads {
        let Some(fields) = &payload.fields else {
            violations.push(format!(
                "{}: the payload of job `{}` could not be read: declare it as a struct in the \
                 crate's sources",
                payload.path, payload.kind
            ));
            continue;
        };
        for field in fields {
            if audit
                .iter()
                .any(|(k, f, _)| *k == payload.kind && f == field)
            {
                used.insert((payload.kind.clone(), field.clone()));
            } else {
                violations.push(format!(
                    "{}: job `{}` stores `{field}` in its payload, which no audit lists: add it \
                     to `JOB_PAYLOAD_FIELDS` with the reason it is no personal data (an id, a \
                     blob reference), or send the data as a blob",
                    payload.path, payload.kind
                ));
            }
        }
    }
    for (kind, field, reason) in audit {
        if reason.trim().is_empty() {
            violations.push(format!(
                "`JOB_PAYLOAD_FIELDS` lists `{field}` of `{kind}` without a reason"
            ));
        }
        if !used.contains(&((*kind).to_owned(), (*field).to_owned())) {
            violations.push(format!(
                "`JOB_PAYLOAD_FIELDS` lists `{field}` of `{kind}`, which that job's payload no \
                 longer has: remove the entry"
            ));
        }
    }
    violations
}

static ERROR_CALL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:JobError|Self)::(?:fatal|retry|retry_after)\s*\(")
        .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
});

/// Where a job's error message is built from values: `(path, expression)`
/// for every value a `JobError::fatal`, `retry` or `retry_after` message
/// interpolates (or is, when it is not a string literal), outside tests and
/// comments. `Self::…` counts in `JobError`'s own file only.
pub fn job_error_values(sources: &[(String, String)]) -> Vec<(String, String)> {
    let mut values = Vec::new();
    for (path, text) in sources {
        if is_test_file(path) || path.starts_with("xtask/") {
            continue;
        }
        let code = without_comments(without_tests(text));
        let own = path.ends_with("crates/kernel/src/jobs/mod.rs");
        for call in ERROR_CALL.find_iter(&code) {
            if call.as_str().starts_with("Self") && !own {
                continue;
            }
            let rest = &code[call.end()..];
            let Some(end) = closing(rest, '(', ')') else {
                continue;
            };
            let arguments = split_top_level(&rest[..end], false);
            let Some(message) = arguments.first() else {
                continue;
            };
            for value in message_values(message) {
                values.push((path.clone(), value));
            }
        }
    }
    values
}

/// The values `message` (a message argument) is built from: none for a
/// string literal; the interpolated values of a `format!`; else the
/// expression itself. Whitespace is collapsed.
fn message_values(message: &str) -> Vec<String> {
    let message = message.trim();
    if is_literal(message) {
        return Vec::new();
    }
    let Some(inner) = message
        .strip_prefix("format!(")
        .and_then(|rest| rest.strip_suffix(')'))
    else {
        return vec![collapse(message)];
    };
    let arguments = split_top_level(inner, false);
    let Some((template, rest)) = arguments.split_first() else {
        return Vec::new();
    };
    if !is_literal(template) {
        return vec![collapse(message)];
    }
    let mut positional = Vec::new();
    let mut named = BTreeMap::new();
    for argument in rest {
        match argument.split_once('=') {
            Some((name, value))
                if !name.trim().is_empty()
                    && name.trim().chars().all(|c| c.is_alphanumeric() || c == '_')
                    && !value.starts_with('=') =>
            {
                named.insert(name.trim().to_owned(), collapse(value));
            }
            _ => positional.push(collapse(argument)),
        }
    }
    let mut values = Vec::new();
    let mut next = 0usize;
    for placeholder in placeholders(template) {
        let value = if placeholder.is_empty() {
            let value = positional.get(next).cloned();
            next += 1;
            value
        } else if let Ok(index) = placeholder.parse::<usize>() {
            positional.get(index).cloned()
        } else {
            Some(named.get(&placeholder).cloned().unwrap_or(placeholder))
        };
        if let Some(value) = value {
            values.push(value);
        }
    }
    values
}

/// Whether `text` is one string literal (plain or raw).
fn is_literal(text: &str) -> bool {
    let text = text.trim();
    (text.starts_with('"') && text.ends_with('"') && text.len() >= 2)
        || ((text.starts_with("r\"") || text.starts_with("r#")) && text.ends_with(['"', '#']))
}

/// The argument names of a format string's placeholders, in order: empty
/// for `{}` (the next positional), a number or a name otherwise.
fn placeholders(template: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
            }
            '{' => {
                let mut inside = String::new();
                for d in chars.by_ref() {
                    if d == '}' {
                        break;
                    }
                    inside.push(d);
                }
                let name = inside.split(':').next().unwrap_or_default().trim();
                names.push(name.to_owned());
            }
            _ => {}
        }
    }
    names
}

/// `text` with every run of whitespace as one space.
fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// [`JOB_ERROR_VALUES`] against the sources' job errors: every value a
/// message is built from is listed for its file with its reason, and an
/// entry no message uses any more is stale.
pub fn check_job_errors(values: &[(String, String)], audit: &[(&str, &str, &str)]) -> Vec<String> {
    let mut violations = Vec::new();
    let mut used = BTreeSet::new();
    let mut reported = BTreeSet::new();
    for (path, value) in values {
        if audit.iter().any(|(p, v, _)| p == path && v == value) {
            used.insert((path.clone(), value.clone()));
        } else if reported.insert((path.clone(), value.clone())) {
            violations.push(format!(
                "{path}: a job's error message is built from `{value}`, which no audit lists: \
                 say what failed with ids and status codes, or add it to `JOB_ERROR_VALUES` \
                 with the reason it is no personal data (it is stored in \
                 `kernel.jobs.last_error`)"
            ));
        }
    }
    for (path, value, reason) in audit {
        if reason.trim().is_empty() {
            violations.push(format!(
                "`JOB_ERROR_VALUES` lists `{value}` in {path} without a reason"
            ));
        }
        if !used.contains(&((*path).to_owned(), (*value).to_owned())) {
            violations.push(format!(
                "`JOB_ERROR_VALUES` lists `{value}` in {path}, which no job error there is built \
                 from any more: remove the entry"
            ));
        }
    }
    violations
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn personal_names_are_read_by_their_words() {
        for name in [
            "from",
            "toAddress",
            "display_name",
            "subjects",
            "email",
            "body_text",
            "provider_message_id",
        ] {
            assert!(personal(name), "{name}");
        }
        for name in [
            "inbox_id",
            "tomorrow",
            "token",
            "mailbox",
            "count",
            "workspace",
        ] {
            assert!(!personal(name), "{name}");
        }
    }

    fn catalogue(payload: &serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "events": [{ "name": "test.happened", "payload": payload }],
            "components": { "schemas": {
                "BlobPointer": { "type": "object", "properties": {
                    "content_type": { "type": "string" } } },
                "Role": { "type": "string", "enum": ["owner", "member"] },
                "UserId": { "type": "string", "format": "uuid" },
                "Facts": { "type": "object", "properties": {
                    "sender": { "type": "string" } } }
            } }
        })
    }

    #[test]
    fn event_fields_named_like_personal_data_hold_opaque_values() {
        let opaque = serde_json::json!({ "type": "object", "properties": {
            "participants": { "$ref": "#/components/schemas/BlobPointer" },
            "text": { "oneOf": [
                { "$ref": "#/components/schemas/BlobPointer" }, { "type": "null" } ] },
            "from": { "$ref": "#/components/schemas/Role" },
            "message": { "$ref": "#/components/schemas/UserId" },
            "messages": { "type": "array", "items": { "type": "string", "format": "uuid" } },
            "body_size": { "type": "integer" }
        } });
        assert_eq!(
            check_event_payloads(&catalogue(&opaque), &[]),
            Vec::<String>::new()
        );

        let leaky = serde_json::json!({ "type": "object", "properties": {
            "subject": { "type": "string" },
            "facts": { "$ref": "#/components/schemas/Facts" }
        } });
        let violations = check_event_payloads(&catalogue(&leaky), &[]);
        assert_eq!(violations.len(), 2, "{violations:?}");
        assert!(violations[0].contains("`test.happened` carries `sender`"));
        assert!(violations[1].contains("carries `subject`"));

        let allowed = [
            ("test.happened", "subject", "an opaque account id"),
            ("test.happened", "sender", "an opaque account id"),
            ("test.gone", "subject", "no longer carried"),
        ];
        let violations = check_event_payloads(&catalogue(&leaky), &allowed);
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].contains("no longer carries"),
            "{violations:?}"
        );
    }

    fn sources(files: &[(&str, &str)]) -> Vec<(String, String)> {
        files
            .iter()
            .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
            .collect()
    }

    const JOBS: &str = r#"
pub const PAGE_JOB: &str = "test.page";

/// A unit job.
#[derive(Serialize, Deserialize)]
pub struct Sweep;

impl Job for Sweep {
    const NAME: &'static str = "test.sweep";
    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> { Ok(()) }
}

pub struct Pager;

/// The page to read.
pub struct PagePayload {
    /// The inbox.
    pub inbox: InboxId,
    #[serde(default)]
    pub(crate) after: Option<HashMap<String, Vec<u8>>>,
}

impl JobHandler for Pager {
    type Payload = crate::jobs::PagePayload;
    const NAME: &'static str = PAGE_JOB;
    const QUEUE: &'static str = "default";
}

// impl Job for Commented {}

#[cfg(test)]
mod tests {
    impl Job for TestOnly { const NAME: &'static str = "test.only"; }
}
"#;

    #[test]
    fn job_payloads_are_read_from_the_sources() {
        let found = job_payloads(&sources(&[("crates/x/src/jobs.rs", JOBS)]));
        assert_eq!(
            found,
            [
                JobPayload {
                    kind: "test.page".to_owned(),
                    path: "crates/x/src/jobs.rs".to_owned(),
                    fields: Some(vec!["inbox".to_owned(), "after".to_owned()]),
                },
                JobPayload {
                    kind: "test.sweep".to_owned(),
                    path: "crates/x/src/jobs.rs".to_owned(),
                    fields: Some(Vec::new()),
                },
            ]
        );
    }

    #[test]
    fn every_job_payload_field_is_audited() {
        let found = job_payloads(&sources(&[("crates/x/src/jobs.rs", JOBS)]));
        let violations = check_job_payloads(&found, &[("test.page", "inbox", "an id")]);
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations[0].contains("stores `after`"), "{violations:?}");

        let audit = [
            ("test.page", "inbox", "an id"),
            ("test.page", "after", "a cursor"),
            ("test.page", "before", "gone"),
        ];
        let violations = check_job_payloads(&found, &audit);
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations[0].contains("no longer has"), "{violations:?}");

        let lost = [JobPayload {
            kind: "test.lost".to_owned(),
            path: "crates/x/src/lost.rs".to_owned(),
            fields: None,
        }];
        assert!(check_job_payloads(&lost, &[])[0].contains("could not be read"));
    }

    #[test]
    fn a_messages_values_are_what_it_interpolates() {
        assert!(message_values(r#""a literal""#).is_empty());
        assert!(message_values(r##"r#"raw"#"##).is_empty());
        assert_eq!(message_values("e.to_string()"), ["e.to_string()"]);
        assert_eq!(
            message_values(r#"format!("stream {} failed: {e}", target.stream)"#),
            ["target.stream", "e"]
        );
        assert_eq!(
            message_values(r#"format!("{1} then {0} {{literal}} {name:?}", a, b, name = c.d())"#),
            ["b", "a", "c.d()"]
        );
        assert_eq!(
            message_values(r#"format!("at {}", redact::json(&e, "x, y"))"#),
            [r#"redact::json(&e, "x, y")"#]
        );
    }

    const ERRORS: &str = r#"
fn run() -> Result<(), JobError> {
    let a = JobError::fatal("no values");
    let b = JobError::retry(format!("stream {stream} failed: {}", redact::sqlx(&e)));
    let c = JobError::retry_after(
        why,
        WAIT,
    );
    let d = Self::retry(other.to_string());
    // JobError::fatal(format!("{commented}"))
}

#[cfg(test)]
mod tests {
    fn t() { JobError::fatal(format!("{in_tests}")); }
}
"#;

    #[test]
    fn job_error_values_are_audited_per_file() {
        let found = job_error_values(&sources(&[
            ("crates/x/src/run.rs", ERRORS),
            ("crates/x/src/run_tests.rs", ERRORS),
        ]));
        assert_eq!(
            found,
            [
                ("crates/x/src/run.rs".to_owned(), "stream".to_owned()),
                (
                    "crates/x/src/run.rs".to_owned(),
                    "redact::sqlx(&e)".to_owned()
                ),
                ("crates/x/src/run.rs".to_owned(), "why".to_owned()),
            ],
            "`Self::` counts in `JobError`'s own file only; tests and comments are skipped"
        );
        let audit = [
            ("crates/x/src/run.rs", "stream", "an id"),
            ("crates/x/src/run.rs", "redact::sqlx(&e)", "by kind"),
            ("crates/x/src/other.rs", "why", "elsewhere"),
        ];
        let violations = check_job_errors(&found, &audit);
        assert_eq!(violations.len(), 2, "{violations:?}");
        assert!(violations[0].contains("built from `why`"), "{violations:?}");
        assert!(
            violations[1].contains("no job error there"),
            "{violations:?}"
        );
    }
}
