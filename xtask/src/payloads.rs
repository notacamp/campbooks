//! Personal data stays out of what the queue and the event log keep
//! (campbooks-cloud#65): event payloads, job payloads and the reasons jobs
//! fail (`kernel.jobs.last_error`). The event log is kept for 90 days and
//! finished jobs for up to 30, longer than a member's or a workspace's data
//! once they ask for it to go; personal data (addresses, names, subjects,
//! bodies) travels as blob references instead, which the purges delete.
//!
//! - **Event payloads**: every field of every event's payload that can hold
//!   free text (a string that is not an id, a date or one of a closed set)
//!   is listed by its full path in [`EVENT_FIELDS_ALLOWED`] with the reason
//!   it holds no personal data. Blob references, ids, enums, dates,
//!   numbers and flags need no entry.
//! - **Job payloads**: every field of every job kind's payload is listed in
//!   [`JOB_PAYLOAD_FIELDS`] with the reason it is no personal data: a field
//!   by field audit, so a new field is reviewed before it is stored. A job
//!   whose name or payload cannot be read is reported, never skipped.
//! - **Failure reasons**: every value a job's error message is built from
//!   (other than its own wording) is listed per file, with how many times it
//!   is used there, in [`JOB_ERROR_VALUES`] with the reason it is no personal
//!   data; a new use fails until it is reviewed. A `JobError` built any other
//!   way (a variant written out, `JobError::retry` passed as a function) is
//!   reported. `redact::job_error` masks quotes, addresses, links and tokens
//!   as well, but only as a backstop.
//!
//! - **Job origins**: what a job remembers of who caused it
//!   (`kernel.jobs.origin`, ADR 0020): every field of the origin and of the
//!   actor it holds is listed in [`JOB_ORIGIN_FIELDS`] with the reason it
//!   is no personal data.
//!
//! Each list's entries must still match something: a stale entry is
//! reported, so the lists stay an accurate audit.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use regex::Regex;

use crate::access::{is_test_file, without_comments, without_tests};

/// Event payload fields that can hold free text, by their full path
/// (`items[].change.provider_key`; `[]` for a list's items, `{}` for a map's
/// values), each with the reason it holds no personal data:
/// `(event, path, reason)`. An event of `*` covers that path in every event:
/// only for `workspace`, the routing field `check_workspace` checks on every
/// publish; any other free-text field is reviewed event by event.
pub const EVENT_FIELDS_ALLOWED: &[(&str, &str, &str)] = &[
    (
        "*",
        "workspace",
        "a workspace's event routing field: the workspace's id as text (`DomainEvent::workspace`)",
    ),
    (
        "attention.item_changed",
        "container.type",
        "a record type, from the code (`<module>.<record>`)",
    ),
    (
        "attention.item_changed",
        "container.id",
        "a record's id (a UUID as text)",
    ),
    (
        "tasks.task_changed",
        "source.type",
        "a record type, from the code (`<module>.<record>`)",
    ),
    (
        "tasks.task_changed",
        "source.id",
        "a record's id (a UUID as text)",
    ),
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
        "connectors.messages_changed",
        "cursor",
        "the stream's position at the provider (a history id or a time), opaque",
    ),
    (
        "connectors.send_completed",
        "provider_message_id",
        PROVIDER_MESSAGE_ID,
    ),
    (
        "conversations.changes_requested",
        "items[].provider_message_id",
        PROVIDER_MESSAGE_ID,
    ),
    (
        "conversations.changes_requested",
        "items[].change.provider_key",
        "the provider's id of a label or folder (Gmail's `Label_12`, Zoho's number), never its \
         name",
    ),
    (
        "conversations.conversation_changed",
        "provider_message_ids[]",
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
        "identity.client_registered",
        "kind",
        "the client's kind, from the code (`confidential`, `public`)",
    ),
    (
        "identity.way_in_added",
        "via",
        "how the way in was added, from the code (`sign_up`, `link`, `connect`, `passkey`)",
    ),
    (
        "workspaces.member_left",
        "subject",
        "the former member's identity subject: an opaque account id, so the issuer removes \
         the apps they registered; never an address",
    ),
    (
        "workspaces.workspace_updated",
        "fields[]",
        "the names of the settings that changed (`name`, `accent`), from the code; never their \
         values",
    ),
    (
        "connectors.connection_needs_reauth",
        "reason",
        "why the connection needs signing in again, a code from the code (e.g. `grant_revoked`, \
         `credentials_unreadable`)",
    ),
    (
        "connectors.send_completed",
        "reason",
        "why a send failed, a code from the code (e.g. `not_connected`, `needs_reauth`, \
         `rejected`, `too_large`)",
    ),
    (
        "conversations.send_failed",
        "reason",
        "why a send failed, a code from the code (e.g. `not_connected`, `rejected`, \
         `unknown_outcome`)",
    ),
    (
        "identity.consent_revoked",
        "reason",
        "why the consent ended, a code from the code (`revoked`, `member_left`, `workspace_deleting`, `client_removed`)",
    ),
    ("schedule.event_created", "starts", EVENT_TIME),
    ("schedule.event_created", "ends", EVENT_TIME),
    ("schedule.event_created", "source.type", RECORD_TYPE),
    ("schedule.event_created", "source.id", RECORD_ID),
    ("schedule.event_changed", "starts", EVENT_TIME),
    ("schedule.event_changed", "ends", EVENT_TIME),
    ("schedule.event_changed", "source.type", RECORD_TYPE),
    ("schedule.event_changed", "source.id", RECORD_ID),
    ("schedule.event_cancelled", "starts", EVENT_TIME),
    ("schedule.event_cancelled", "ends", EVENT_TIME),
    ("schedule.event_cancelled", "source.type", RECORD_TYPE),
    ("schedule.event_cancelled", "source.id", RECORD_ID),
    ("conversations.message_read", "items[].key", READ_KEY),
    ("conversations.message_read", "items[].starts", EVENT_TIME),
    ("conversations.message_read", "items[].ends", EVENT_TIME),
    ("conversations.message_read", "items[].time_zone", ZONE_NAME),
    ("conversations.message_read", "items[].updates", READ_KEY),
    ("conversations.message_read", "items[].cancels", READ_KEY),
    ("conversations.message_read", "asks[].key", READ_KEY),
    ("conversations.message_read", "asks[].due", EVENT_TIME),
    (
        "conversations.message_read",
        "asks[].same_as",
        TASK_REFERENCE,
    ),
    (
        "connectors.calendar_change_completed",
        "provider_event_id",
        "the provider's id of a calendar event (Google's base32hex id): opaque, never its content",
    ),
    (
        "connectors.calendar_change_completed",
        "etag",
        "the provider's version marker of a calendar event: opaque, never its content",
    ),
    ("identity.client_registered", "client", CLIENT_ID),
    ("identity.client_removed", "client", CLIENT_ID),
    ("identity.consent_granted", "client", CLIENT_ID),
    ("identity.consent_revoked", "client", CLIENT_ID),
    ("identity.refresh_reuse_detected", "client", CLIENT_ID),
    ("identity.account_created", "provider", PROVIDER_NAME),
    ("identity.signed_in", "provider", PROVIDER_NAME),
    ("identity.way_in_added", "provider", PROVIDER_NAME),
    ("identity.way_in_removed", "provider", PROVIDER_NAME),
    ("identity.signed_in", "method", METHOD_NAME),
    ("identity.way_in_added", "method", METHOD_NAME),
    ("identity.way_in_removed", "method", METHOD_NAME),
];

const EVENT_TIME: &str = "when an event starts or ends: an RFC 3339 time with an offset or a \
     `YYYY-MM-DD` date, parsed before it is kept, never a word of the event";

const READ_KEY: &str = "the key of an item or an ask of a read: `<message id>:<layer>:item|ask:<n>`, \
     built by the code from ids and a position";

const ZONE_NAME: &str =
    "an IANA time zone name, checked against the zone database before it is kept";

const TASK_REFERENCE: &str = "the reference of an open task, as the module that owns tasks gave it \
     (an opaque id); never its title";

const RECORD_TYPE: &str = "a record type, from the code (`<module>.<record>`)";

const RECORD_ID: &str = "a record's id (a UUID as text)";

const CLIENT_ID: &str = "an app's OAuth client id: an app identifier (a first-party id, a \
     generated id, or the app's own HTTPS metadata-document URL), not personal data";
const PROVIDER_NAME: &str =
    "a mail or sign-in provider's name, from the code (e.g. `google`, `zoho`, `passkey`)";
const METHOD_NAME: &str = "how someone signs in, from the code (`provider`, `passkey`)";

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
        "pages.compact",
        "page",
        "the page's id (a UUID), never its title or text",
    ),
    (
        "pages.push_to_notion",
        "page",
        "the page's id (a UUID), never its title or text",
    ),
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
    (
        "connectors.send_address_check",
        "connection",
        "the connection's id (a UUID), never its address",
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
    (
        "conversations.apply_rule",
        "newest",
        "the conversation a Worth a look run raises, and when its mail was sent (an id and a timestamp)",
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
    ("conversations.read_message", "message", MESSAGE_ID),
    ("conversations.read_structured", "message", MESSAGE_ID),
    (
        "schedule.release_delete",
        "event",
        "the event's id (a UUID), never its content",
    ),
    (
        "schedule.release_response",
        "event",
        "the event's id (a UUID), never its content or the answer",
    ),
];

/// Where the structs a job's origin is made of are defined.
pub const JOB_ORIGIN_SOURCE: &str = "crates/kernel/src/tracking/context.rs";

/// Every field of a job's origin (`kernel.jobs.origin`, ADR 0020) and of
/// the actor it holds, each with the reason it is no personal data:
/// `(struct, field, reason)`.
pub const JOB_ORIGIN_FIELDS: &[(&str, &str, &str)] = &[
    (
        "JobOrigin",
        "actor",
        "who the job works for: an `Actor` (its fields below), never a name or an address",
    ),
    (
        "JobOrigin",
        "correlation",
        "the causing request's trace id (32 hex digits), opaque",
    ),
    (
        "Actor",
        "kind",
        "the actor's type, from the code (`user`, `client`, `assistant`, `automation`, \
         `system`, `admin`, `anonymous`)",
    ),
    (
        "Actor",
        "id",
        "a Campbooks account id, an OAuth client id or a job kind: ids, never a name or an \
         address",
    ),
];

/// Every field of the origin's structs is listed in `audit` with a reason,
/// and every entry still names a field.
pub fn check_job_origin(
    sources: &[(String, String)],
    path: &str,
    audit: &[(&str, &str, &str)],
) -> Vec<String> {
    let code: Vec<(String, String)> = sources
        .iter()
        .map(|(p, text)| (p.clone(), without_comments(without_tests(text))))
        .collect();
    let mut violations = duplicates(
        "JOB_ORIGIN_FIELDS",
        audit
            .iter()
            .map(|(s, field, _)| format!("`{field}` of `{s}`")),
    );
    let structs: BTreeSet<&str> = audit.iter().map(|(s, ..)| *s).collect();
    for name in structs {
        let fields = match struct_fields(&code, path, name) {
            Ok(fields) => fields,
            Err(why) => {
                violations.push(format!("{path}: the job origin's `{name}`: {why}"));
                continue;
            }
        };
        for field in &fields {
            if !audit.iter().any(|(s, f, _)| *s == name && f == field) {
                violations.push(format!(
                    "{path}: a job's origin stores `{field}` of `{name}`, which no audit \
                     lists: add it to `JOB_ORIGIN_FIELDS` with the reason it is no personal \
                     data (ids and enums only)"
                ));
            }
        }
        for (_, field, reason) in audit.iter().filter(|(s, ..)| *s == name) {
            if reason.trim().is_empty() {
                violations.push(format!(
                    "`JOB_ORIGIN_FIELDS` lists `{field}` of `{name}` without a reason"
                ));
            }
            if !fields.iter().any(|f| f == field) {
                violations.push(format!(
                    "`JOB_ORIGIN_FIELDS` lists `{field}` of `{name}`, which it no longer has: \
                     remove the entry"
                ));
            }
        }
    }
    violations
}

const WORKSPACE_ID: &str = "the workspace's id (a UUID)";
const INBOX_ID: &str = "the inbox's id (a UUID)";
const DRAFT_ID: &str = "the draft's id (a UUID)";
const MESSAGE_ID: &str = "the message's id (a UUID), never its content";
const CONVERSATION_CURSOR: &str = "the id of the last conversation done (a cursor), opaque";

/// Every value a job's error message is built from, other than its own
/// wording, each with how many times it is used in that file and the reason
/// it is no personal data: `(path, expression, uses, reason)`.
pub const JOB_ERROR_VALUES: &[(&str, &str, usize, &str)] = &[
    (
        "crates/kernel/src/blobs/mod.rs",
        "err.to_string()",
        1,
        BLOB_ERROR,
    ),
    (
        "crates/kernel/src/blobs/mod.rs",
        "other.to_string()",
        1,
        BLOB_ERROR,
    ),
    (
        "crates/kernel/src/jobs/mod.rs",
        "crate::redact::sqlx(&err)",
        1,
        REDACTED_SQL,
    ),
    (
        "crates/kernel/src/jobs/mod.rs",
        "err.to_string()",
        1,
        KERNEL_ERROR,
    ),
    (
        "crates/kernel/src/jobs/mod.rs",
        "other.to_string()",
        1,
        KERNEL_ERROR,
    ),
    (
        "crates/kernel/src/jobs/registry.rs",
        "E::NAME",
        3,
        CODE_NAME,
    ),
    (
        "crates/kernel/src/jobs/registry.rs",
        "H::NAME",
        1,
        CODE_NAME,
    ),
    (
        "crates/kernel/src/jobs/registry.rs",
        "J::NAME",
        1,
        CODE_NAME,
    ),
    (
        "crates/kernel/src/jobs/registry.rs",
        "E::VERSION",
        1,
        VERSION,
    ),
    (
        "crates/kernel/src/jobs/registry.rs",
        "row.version",
        1,
        VERSION,
    ),
    (
        "crates/kernel/src/jobs/registry.rs",
        "row.name",
        1,
        "the logged event's name, a name from the code",
    ),
    (
        "crates/kernel/src/jobs/registry.rs",
        "id",
        5,
        "the event's id (a UUIDv7)",
    ),
    (
        "crates/kernel/src/jobs/registry.rs",
        "crate::redact::json(&e)",
        3,
        REDACTED_JSON,
    ),
    (
        "crates/kernel/src/jobs/worker.rs",
        "crate::redact::sqlx(&e)",
        1,
        REDACTED_SQL,
    ),
    (
        "crates/kernel/src/jobs/worker.rs",
        "info.kind",
        2,
        "the queued job's kind, a name from the code",
    ),
    (
        "crates/modules/connectors/src/features/member_left/mod.rs",
        "e.to_string()",
        1,
        KERNEL_ERROR,
    ),
    (
        "crates/modules/connectors/src/features/request_changes/mod.rs",
        "campbooks_kernel::redact::json(&e)",
        1,
        REDACTED_JSON,
    ),
    (
        "crates/modules/connectors/src/features/send_message/mod.rs",
        "OUTGOING_CONTENT_TYPE",
        1,
        "a content type constant",
    ),
    (
        "crates/modules/connectors/src/features/send_message/mod.rs",
        "document.version",
        1,
        VERSION,
    ),
    (
        "crates/modules/connectors/src/features/send_message/mod.rs",
        "e.line()",
        1,
        POSITION,
    ),
    (
        "crates/modules/connectors/src/features/send_message/mod.rs",
        "e.column()",
        1,
        POSITION,
    ),
    (
        "crates/modules/connectors/src/features/send_message/mod.rs",
        "e.to_string()",
        1,
        READ_ERROR,
    ),
    (
        "crates/modules/connectors/src/features/send_message/mod.rs",
        "err",
        2,
        PROVIDER_ERROR,
    ),
    (
        "crates/modules/connectors/src/features/send_message/mod.rs",
        "redact::json(&e)",
        3,
        REDACTED_JSON,
    ),
    (
        "crates/modules/connectors/src/features/send_message/mod.rs",
        "send",
        1,
        "the send's id (a UUID)",
    ),
    (
        "crates/modules/connectors/src/features/send_message/via_zoho.rs",
        "err",
        1,
        PROVIDER_ERROR,
    ),
    (
        "crates/modules/connectors/src/features/send_message/via_zoho.rs",
        "why",
        1,
        "why Zoho could not say whether an attempt was sent: a provider failure (as for \
         `ProviderError`) or the code's own wording",
    ),
    (
        "crates/modules/connectors/src/features/sync_stream/mod.rs",
        "other.to_string()",
        4,
        READ_ERROR,
    ),
    (
        "crates/modules/connectors/src/features/sync_stream/mod.rs",
        "redact::json(&e)",
        9,
        REDACTED_JSON,
    ),
    (
        "crates/modules/connectors/src/features/sync_stream/mod.rs",
        "target.stream",
        4,
        "the stream's id (a UUID)",
    ),
    (
        "crates/modules/connectors/src/features/workspace_deleting/mod.rs",
        "e.to_string()",
        1,
        KERNEL_ERROR,
    ),
    (
        "crates/modules/conversations/src/domain/redact.rs",
        "status",
        1,
        "an HTTP status code",
    ),
    (
        "crates/modules/conversations/src/features/apply_changes/mod.rs",
        "BATCH_VERSION",
        1,
        VERSION,
    ),
    (
        "crates/modules/conversations/src/features/apply_changes/mod.rs",
        "batch.version",
        1,
        VERSION,
    ),
    (
        "crates/modules/conversations/src/features/apply_changes/mod.rs",
        "blob.hash",
        1,
        "a blob's content hash (SHA-256): it identifies bytes, it cannot be read back into them",
    ),
    (
        "crates/modules/conversations/src/features/apply_changes/mod.rs",
        "redact::json(&e)",
        2,
        REDACTED_JSON,
    ),
    (
        "crates/modules/conversations/src/features/create_inbox/mod.rs",
        "redact::json(&e)",
        1,
        REDACTED_JSON,
    ),
    (
        "crates/modules/conversations/src/features/decide_message/mod.rs",
        "redact::json(&e)",
        1,
        REDACTED_JSON,
    ),
    (
        "crates/modules/conversations/src/features/release_send/mod.rs",
        "redact::json(&e)",
        2,
        REDACTED_JSON,
    ),
    (
        "crates/modules/conversations/src/features/read_message/mod.rs",
        "redact::json(&e)",
        1,
        REDACTED_JSON,
    ),
    (
        "crates/modules/conversations/src/features/read_structured/mod.rs",
        "redact::json(&e)",
        1,
        REDACTED_JSON,
    ),
    (
        "crates/modules/identity/src/features/forget_connected_way_in/mod.rs",
        "e.to_string()",
        1,
        KERNEL_ERROR,
    ),
    (
        "crates/modules/identity/src/features/member_left/mod.rs",
        "e.to_string()",
        3,
        KERNEL_ERROR,
    ),
    (
        "crates/modules/identity/src/features/name_connected_mailbox/mod.rs",
        "e.to_string()",
        1,
        KERNEL_ERROR,
    ),
    (
        "crates/modules/identity/src/features/notify_way_in_added/mod.rs",
        "err.to_string()",
        1,
        "a mail transport failure (`MailError::Unavailable` or `NotConfigured`): fixed wording, \
         no recipient or content",
    ),
    (
        "crates/modules/identity/src/features/workspace_deleting/mod.rs",
        "e.to_string()",
        1,
        KERNEL_ERROR,
    ),
    ("crates/modules/identity/src/prune.rs", "e", 2, KERNEL_ERROR),
    (
        "crates/modules/identity/src/prune.rs",
        "redact::sqlx(&e)",
        3,
        REDACTED_SQL,
    ),
    (
        "crates/modules/workspaces/src/features/delete_workspace/mod.rs",
        "e",
        1,
        BLOB_ERROR,
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
const BLOB_ERROR: &str = "a blob store error (`BlobError`): its kind, a limit, a content hash, \
     a database error by kind (`redact::sqlx`), or the object store's failure (its keys are \
     workspace ids and hashes, never names)";
const READ_ERROR: &str = "a mail read failure (`ReadError`): a provider failure (as for \
     `ProviderError`), a database error by kind, a blob store error, or what stored data did not \
     decode, never the data";

/// [`EVENT_FIELDS_ALLOWED`] against the events catalogue (`events`, what
/// `openapi/events.json` holds): every field of every event's payload, at
/// any depth, that can hold free text ([`free_text_fields`]) is listed for
/// that event (or `*`) with its reason. An entry no event matches any more
/// is stale.
pub fn check_event_payloads(
    events: &serde_json::Value,
    allowed: &[(&str, &str, &str)],
) -> Vec<String> {
    let mut violations = duplicates(
        "EVENT_FIELDS_ALLOWED",
        allowed
            .iter()
            .map(|(event, path, _)| format!("`{path}` of `{event}`")),
    );
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
        let mut fields = BTreeSet::new();
        free_text_fields(payload, schemas, "", &mut BTreeSet::new(), &mut fields);
        for open in fields.iter().filter(|path| path.ends_with(OPEN_MAP)) {
            let path = open.trim_end_matches(OPEN_MAP);
            violations.push(format!(
                "event `{name}` carries an open map at `{path}` (`additionalProperties: true`): \
                 its values cannot be checked; give them a schema"
            ));
        }
        fields.retain(|path| !path.ends_with(OPEN_MAP));
        for path in &fields {
            let entry = allowed
                .iter()
                .find(|(e, p, _)| *p == path && *e == name)
                .or_else(|| allowed.iter().find(|(e, p, _)| *p == path && *e == "*"));
            if let Some((e, p, _)) = entry {
                used.insert(((*e).to_owned(), (*p).to_owned()));
            } else {
                violations.push(format!(
                    "event `{name}` carries `{path}` as free text: send personal data as a blob \
                     reference, use an id or a closed set of values, or list it in \
                     `EVENT_FIELDS_ALLOWED` with the reason it is no personal data"
                ));
            }
        }
    }
    for (event, path, reason) in allowed {
        if reason.trim().is_empty() {
            violations.push(format!(
                "`EVENT_FIELDS_ALLOWED` lists `{path}` of `{event}` without a reason"
            ));
        }
        if *event == "*" && *path != WILDCARD_PATH {
            violations.push(format!(
                "`EVENT_FIELDS_ALLOWED` lists `{path}` for every event (`*`): only `{WILDCARD_PATH}` \
                 may be, list it event by event"
            ));
        }
        if !used.contains(&((*event).to_owned(), (*path).to_owned())) {
            violations.push(format!(
                "`EVENT_FIELDS_ALLOWED` lists `{path}` of `{event}`, which no event carries as \
                 free text any more: remove the entry"
            ));
        }
    }
    violations
}

/// The paths of `schema`'s fields that can hold free text, at any depth
/// (through `$ref`, `allOf`, `oneOf`, `anyOf`, a list's `items` as `[]` and
/// a map's `additionalProperties` as `{}`), below `path`. A blob reference
/// ([`BLOB_REFERENCES`]) is opaque as a whole and not looked into.
fn free_text_fields(
    schema: &serde_json::Value,
    schemas: Option<&serde_json::Value>,
    path: &str,
    seen: &mut BTreeSet<(String, String)>,
    out: &mut BTreeSet<String>,
) {
    let Some(object) = schema.as_object() else {
        return;
    };
    if let Some(name) = reference(schema) {
        if BLOB_REFERENCES.contains(&name) || !seen.insert((name.to_owned(), path.to_owned())) {
            return;
        }
        if let Some(target) = schemas.and_then(|s| s.get(name)) {
            free_text_fields(target, schemas, path, seen, out);
        }
        return;
    }
    let mut container = false;
    for key in ["allOf", "oneOf", "anyOf"] {
        for part in object
            .get(key)
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
        {
            container = true;
            if part.get("type").and_then(serde_json::Value::as_str) == Some("null")
                && reference(part).is_none()
            {
                continue;
            }
            free_text_fields(part, schemas, path, seen, out);
        }
    }
    if let Some(props) = object
        .get("properties")
        .and_then(serde_json::Value::as_object)
    {
        container = true;
        for (name, property) in props {
            let inner = if path.is_empty() {
                name.clone()
            } else {
                format!("{path}.{name}")
            };
            free_text_fields(property, schemas, &inner, seen, out);
        }
    }
    if types(schema).contains(&"array") {
        container = true;
        if let Some(items) = object.get("items") {
            free_text_fields(items, schemas, &format!("{path}[]"), seen, out);
        }
    }
    match object.get("additionalProperties") {
        Some(values @ serde_json::Value::Object(_)) => {
            container = true;
            free_text_fields(values, schemas, &format!("{path}{{}}"), seen, out);
        }
        Some(serde_json::Value::Bool(true)) => {
            container = true;
            out.insert(format!("{path}{OPEN_MAP}"));
        }
        _ => {}
    }
    if !container && !opaque_leaf(schema) {
        out.insert(path.to_owned());
    }
}

/// The one path an `*` entry may cover: a workspace's event routing field,
/// which `check_workspace` checks on every publish.
const WILDCARD_PATH: &str = "workspace";

/// Marks the path of an object whose values are open
/// (`additionalProperties: true`): reported, since nothing can be checked.
const OPEN_MAP: &str = "{open}";

/// Every entry of `list` that appears more than once (each `entries` item
/// names one), as violations.
fn duplicates(list: &str, entries: impl Iterator<Item = String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    entries
        .filter(|entry| !seen.insert(entry.clone()))
        .map(|entry| format!("`{list}` lists {entry} twice: keep one entry"))
        .collect()
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

/// A schema's `type`, one or several.
fn types(schema: &serde_json::Value) -> Vec<&str> {
    match schema.get("type") {
        Some(serde_json::Value::String(t)) => vec![t.as_str()],
        Some(serde_json::Value::Array(ts)) => ts.iter().filter_map(|t| t.as_str()).collect(),
        _ => Vec::new(),
    }
}

/// Whether a leaf value cannot hold free text: one of a closed set
/// (`enum`), an id (`uuid`), a date or a time, a number, a flag, or `null`.
fn opaque_leaf(schema: &serde_json::Value) -> bool {
    if schema.get("enum").is_some() {
        return true;
    }
    let format = schema.get("format").and_then(serde_json::Value::as_str);
    let types = types(schema);
    !types.is_empty()
        && types.iter().all(|t| match *t {
            "null" | "integer" | "number" | "boolean" => true,
            "string" => matches!(format, Some("uuid" | "date" | "date-time")),
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
    /// The payload's fields (none for a unit payload).
    pub fields: Vec<String>,
}

/// What [`job_payloads`] read: the payloads, and every job it could not
/// read (each a violation, never skipped).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JobScan {
    /// The job kinds and their payloads' fields.
    pub payloads: Vec<JobPayload>,
    /// What could not be read, and where.
    pub problems: Vec<String>,
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
/// fields. A kind whose `NAME` or `Payload` cannot be read, a `NAME`
/// constant defined with different values, and a payload struct that is
/// missing or defined more than once outside the job's file are problems.
pub fn job_payloads(sources: &[(String, String)]) -> JobScan {
    let code: Vec<(String, String)> = sources
        .iter()
        .filter(|(path, _)| !is_test_file(path))
        .map(|(path, text)| (path.clone(), without_comments(without_tests(text))))
        .collect();
    let mut constants: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (_, text) in &code {
        for found in STR_CONST.captures_iter(text) {
            constants
                .entry(found[1].to_owned())
                .or_default()
                .insert(found[2].to_owned());
        }
    }
    let mut scan = JobScan::default();
    for (path, text) in &code {
        for found in JOB_IMPL.captures_iter(text) {
            let implementor = &found[2];
            let Some(body) = found
                .get(0)
                .and_then(|whole| block_after(text, whole.end()))
            else {
                scan.problems.push(format!(
                    "{path}: the `{}` impl for `{implementor}` has no body that can be read",
                    &found[1]
                ));
                continue;
            };
            let kind = match kind_name(body, &constants) {
                Ok(kind) => kind,
                Err(why) => {
                    scan.problems
                        .push(format!("{path}: job `{implementor}`: {why}"));
                    continue;
                }
            };
            let payload = if &found[1] == "Job" {
                implementor.to_owned()
            } else if let Some(t) = PAYLOAD_TYPE.captures(body) {
                t[1].rsplit("::").next().unwrap_or_default().to_owned()
            } else {
                scan.problems.push(format!(
                    "{path}: job `{kind}` declares no `type Payload` that can be read"
                ));
                continue;
            };
            match struct_fields(&code, path, &payload) {
                Ok(fields) => scan.payloads.push(JobPayload {
                    kind,
                    path: path.clone(),
                    fields,
                }),
                Err(why) => scan.problems.push(format!("{path}: job `{kind}`: {why}")),
            }
        }
    }
    scan.payloads.sort_by(|a, b| a.kind.cmp(&b.kind));
    scan
}

/// The kind a job's impl body names: a literal `NAME`, or a string constant
/// defined once (with one value) in the sources.
fn kind_name(body: &str, constants: &BTreeMap<String, BTreeSet<String>>) -> Result<String, String> {
    let Some(name) = KIND_NAME.captures(body) else {
        return Err("no `const NAME: &str` that can be read".to_owned());
    };
    if let Some(literal) = name.get(1) {
        return Ok(literal.as_str().to_owned());
    }
    let constant = name.get(2).map(|m| m.as_str()).unwrap_or_default();
    let last = constant.rsplit("::").next().unwrap_or_default();
    match constants
        .get(last)
        .map(|values| values.iter().collect::<Vec<_>>())
    {
        Some(values) if values.len() == 1 => Ok(values[0].clone()),
        Some(values) => Err(format!(
            "its NAME `{constant}` is a constant defined with several values ({}): give the \
             constants distinct names",
            values
                .iter()
                .map(|v| format!("`{v}`"))
                .collect::<Vec<_>>()
                .join(", ")
        )),
        None => Err(format!(
            "its NAME `{constant}` names no string constant the sources define"
        )),
    }
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

/// The fields of struct `name` (none for a unit struct, `0`, `1`, … for a
/// tuple struct): its definition in `path`, else its one definition in the
/// other sources. Missing, or defined in several other files, is an error.
fn struct_fields(code: &[(String, String)], path: &str, name: &str) -> Result<Vec<String>, String> {
    let header = Regex::new(&format!(
        r"\bstruct\s+{}\b\s*(?:<[^>{{;]*>)?",
        regex::escape(name)
    ))
    .map_err(|e| format!("struct `{name}`: {e}"))?;
    let local: Vec<&str> = code
        .iter()
        .filter(|(p, _)| p == path)
        .filter_map(|(_, text)| header.find(text).map(|found| &text[found.end()..]))
        .collect();
    let elsewhere: Vec<(&str, &str)> = code
        .iter()
        .filter(|(p, _)| p != path)
        .filter_map(|(p, text)| {
            header
                .find(text)
                .map(|found| (p.as_str(), &text[found.end()..]))
        })
        .collect();
    let rest = match (local.first(), elsewhere.as_slice()) {
        (Some(rest), _) | (None, [(_, rest)]) => *rest,
        (None, []) => return Err(format!("its payload struct `{name}` is not in the sources")),
        (None, several) => {
            return Err(format!(
                "its payload struct `{name}` is defined in several files ({}): keep it in the \
                 job's file or give it a distinct name",
                several
                    .iter()
                    .map(|(p, _)| *p)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    };
    let rest = rest.trim_start();
    if rest.starts_with(';') {
        return Ok(Vec::new());
    }
    if let Some(tuple) = rest.strip_prefix('(') {
        let inner = closing(tuple, '(', ')')
            .map(|end| &tuple[..end])
            .ok_or_else(|| format!("its payload struct `{name}` does not close"))?;
        let count = split_top_level(inner, true).len();
        return Ok((0..count).map(|i| i.to_string()).collect());
    }
    let body = block_after(rest, 0)
        .ok_or_else(|| format!("its payload struct `{name}` does not close"))?;
    Ok(split_top_level(body, true)
        .iter()
        .filter_map(|field| {
            let field = strip_attributes(field);
            let (name, _) = field.split_once(':')?;
            let name = name.trim();
            let name = name.rsplit(char::is_whitespace).next().unwrap_or(name);
            (!name.is_empty()).then(|| name.to_owned())
        })
        .collect())
}

/// Where the bracket opened just before `text` closes, as an index into
/// `text`, skipping brackets inside string literals (plain, byte and raw)
/// and character literals (a lifetime is not one).
fn closing(text: &str, open: char, close: char) -> Option<usize> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut depth = 1usize;
    let mut i = 0;
    while let Some(&(at, c)) = chars.get(i) {
        if let Some(end) = literal_end(&chars, i) {
            i = end;
            continue;
        }
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                return Some(at);
            }
        }
        i += 1;
    }
    None
}

/// When a string or character literal starts at `chars[i]`, the index just
/// past it (or past the end, for one that never closes).
fn literal_end(chars: &[(usize, char)], i: usize) -> Option<usize> {
    let at = |k: usize| chars.get(k).map(|&(_, c)| c);
    let follows_word = i
        .checked_sub(1)
        .and_then(&at)
        .is_some_and(|p| p.is_alphanumeric() || p == '_');
    match at(i)? {
        '"' => {
            let mut k = i + 1;
            while let Some(c) = at(k) {
                match c {
                    '\\' => k += 2,
                    '"' => return Some(k + 1),
                    _ => k += 1,
                }
            }
            Some(k)
        }
        'r' if !follows_word => {
            let mut k = i + 1;
            while at(k) == Some('#') {
                k += 1;
            }
            if at(k) != Some('"') {
                return None;
            }
            let hashes = k - i - 1;
            k += 1;
            while let Some(c) = at(k) {
                if c == '"' && (1..=hashes).all(|h| at(k + h) == Some('#')) {
                    return Some(k + 1 + hashes);
                }
                k += 1;
            }
            Some(k)
        }
        '\'' => match (at(i + 1), at(i + 2)) {
            (Some('\\'), Some(_)) => {
                // An escape: `'\n'`, `'\''`, `'\u{…}'`.
                let mut k = i + 3;
                while let Some(c) = at(k) {
                    k += 1;
                    if c == '\'' {
                        return Some(k);
                    }
                    if k > i + 12 {
                        break;
                    }
                }
                None
            }
            (Some(_), Some('\'')) => Some(i + 3),
            _ => None,
        },
        _ => None,
    }
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

/// `text` split at its top-level commas (outside brackets, strings and
/// character literals; with
/// `angles`, outside `<…>` too, for types), each part trimmed, empty parts
/// dropped.
fn split_top_level(text: &str, angles: bool) -> Vec<String> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    let mut i = 0;
    while let Some(&(at, c)) = chars.get(i) {
        if let Some(end) = literal_end(&chars, i) {
            i = end;
            continue;
        }
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            '<' if angles => depth += 1,
            '>' if angles => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&text[start..at]);
                start = at + c.len_utf8();
            }
            _ => {}
        }
        i += 1;
    }
    parts.push(&text[start..]);
    parts
        .into_iter()
        .map(|p| p.trim().to_owned())
        .filter(|p| !p.is_empty())
        .collect()
}

/// [`JOB_PAYLOAD_FIELDS`] against the job kinds the sources implement:
/// every job [`job_payloads`] could not read is reported, every field of
/// every payload is listed with its reason, and an entry naming a field no
/// payload has any more is stale.
pub fn check_job_payloads(scan: &JobScan, audit: &[(&str, &str, &str)]) -> Vec<String> {
    let mut violations = scan.problems.clone();
    violations.extend(duplicates(
        "JOB_PAYLOAD_FIELDS",
        audit
            .iter()
            .map(|(kind, field, _)| format!("`{field}` of `{kind}`")),
    ));
    let mut used = BTreeSet::new();
    for payload in &scan.payloads {
        for field in &payload.fields {
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

static ERROR_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\bJobError::(fatal|retry|retry_after|Fatal|Retry)\b")
        .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
});
static FROM_IMPL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\bimpl\s+From<[^{]*>\s+for\s+(?:[\w:]+::)?JobError\s*\{")
        .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
});
static SELF_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\bSelf::(fatal|retry|retry_after|Fatal|Retry)\b")
        .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
});

/// What [`job_error_values`] read: every value a job error message is built
/// from, and every `JobError` built in a way it cannot read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JobErrorScan {
    /// `(path, expression)`, once per use.
    pub values: Vec<(String, String)>,
    /// `JobError::retry` passed as a function, and the like.
    pub problems: Vec<String>,
}

/// Where a job's error message is built from values, outside tests and
/// comments: every value a `JobError::fatal`, `retry` or `retry_after`
/// message interpolates (or is, when it is not a string literal), and the
/// message of a `JobError::Fatal(…)` or `JobError::Retry { message, … }`
/// written out (a pattern, in a `match` arm, `if let` or inside `matches!`,
/// is not one); the same for `Self::…` in any `impl From<…> for JobError`.
/// A constructor passed as a function (`.map_err(JobError::retry)`,
/// `JobError::Fatal`) hides what the message is built from: a problem.
pub fn job_error_values(sources: &[(String, String)]) -> JobErrorScan {
    let mut scan = JobErrorScan::default();
    for (path, text) in sources {
        if is_test_file(path) || path.starts_with("xtask/") {
            continue;
        }
        let code = without_comments(without_tests(text));
        for found in ERROR_NAME.captures_iter(&code) {
            constructor_values(path, &code, &found, &mut scan);
        }
        for block in FROM_IMPL.find_iter(&code) {
            let Some(body) = block_after(&code, block.end() - 1) else {
                continue;
            };
            for found in SELF_NAME.captures_iter(body) {
                constructor_values(path, body, &found, &mut scan);
            }
        }
    }
    scan
}

/// The values the `JobError` constructor `found` in `code` builds its
/// message from, into `scan` (or a problem when they cannot be read).
fn constructor_values(
    path: &str,
    code: &str,
    found: &regex::Captures<'_>,
    scan: &mut JobErrorScan,
) {
    let Some(whole) = found.get(0) else {
        return;
    };
    let after = code[whole.end()..].trim_start();
    let next = after.chars().next();
    let message = match (&found[1], next) {
        ("fatal" | "retry" | "retry_after", Some('(')) => {
            let rest = &after[1..];
            closing(rest, '(', ')')
                .and_then(|end| split_top_level(&rest[..end], false).into_iter().next())
        }
        ("Fatal", Some('(')) => {
            let rest = &after[1..];
            closing(rest, '(', ')').and_then(|end| {
                written_out(code, whole.start(), &rest[end + 1..]).then(|| rest[..end].to_owned())
            })
        }
        ("Retry", Some('{')) => {
            let rest = &after[1..];
            closing(rest, '{', '}').and_then(|end| {
                if !written_out(code, whole.start(), &rest[end + 1..]) {
                    return None;
                }
                split_top_level(&rest[..end], false)
                    .into_iter()
                    .find_map(|field| match field.split_once(':') {
                        Some((name, value)) => {
                            (name.trim() == "message").then(|| value.trim().to_owned())
                        }
                        // Shorthand: the value is a variable named `message`.
                        None => (field.trim() == "message").then(|| "message".to_owned()),
                    })
            })
        }
        ("fatal" | "retry" | "retry_after" | "Fatal", _) => {
            scan.problems.push(format!(
                "{path}: `{}` is passed as a function, so what its message is built from \
                 cannot be read: call it with the message (`|e| JobError::{}(…)`)",
                whole.as_str(),
                &found[1]
            ));
            None
        }
        _ => None,
    };
    for value in message.iter().flat_map(|m| message_values(m)) {
        scan.values.push((path.to_owned(), value));
    }
}

/// Whether a `JobError` variant at `start` of `code`, followed by `rest`, is
/// built (an expression) rather than matched (a pattern: followed by `=>`,
/// `|` or `=`, or inside the parentheses of a `matches!` that closes).
fn written_out(code: &str, start: usize, rest: &str) -> bool {
    let next = rest.trim_start();
    if next.starts_with("=>")
        || next.starts_with('|')
        || (next.starts_with('=') && !next.starts_with("=="))
    {
        return false;
    }
    !code[..start]
        .match_indices("matches!(")
        .any(|(at, opener)| {
            let inside = at + opener.len();
            // A `matches!(` that never closes (a parse this check cannot
            // follow) holds nothing.
            closing(&code[inside..], '(', ')').is_some_and(|end| inside + end > start)
        })
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

/// [`JOB_ERROR_VALUES`] against the sources' job errors: every problem
/// [`job_error_values`] found is reported, every value a message is built
/// from is listed for its file with how many times it is used there and its
/// reason (a new use fails until it is reviewed), and an entry no message
/// uses any more is stale.
pub fn check_job_errors(scan: &JobErrorScan, audit: &[(&str, &str, usize, &str)]) -> Vec<String> {
    let mut violations = scan.problems.clone();
    violations.extend(duplicates(
        "JOB_ERROR_VALUES",
        audit
            .iter()
            .map(|(path, value, _, _)| format!("`{value}` in {path}")),
    ));
    let mut counts: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    for (path, value) in &scan.values {
        *counts.entry((path.as_str(), value.as_str())).or_default() += 1;
    }
    for ((path, value), count) in &counts {
        match audit.iter().find(|(p, v, _, _)| p == path && v == value) {
            None => violations.push(format!(
                "{path}: a job's error message is built from `{value}` ({count} use(s)), which \
                 no audit lists: say what failed with ids and status codes, or add it to \
                 `JOB_ERROR_VALUES` with its count and the reason it is no personal data (it is \
                 stored in `kernel.jobs.last_error`)"
            )),
            Some((_, _, listed, _)) if listed != count => violations.push(format!(
                "{path}: a job's error message is built from `{value}` {count} time(s), and \
                 `JOB_ERROR_VALUES` lists {listed}: review the change and update the count"
            )),
            Some(_) => {}
        }
    }
    for (path, value, _, reason) in audit {
        if reason.trim().is_empty() {
            violations.push(format!(
                "`JOB_ERROR_VALUES` lists `{value}` in {path} without a reason"
            ));
        }
        if !counts.contains_key(&(*path, *value)) {
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

    fn catalogue(payload: &serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "events": [{ "name": "test.happened", "payload": payload }],
            "components": { "schemas": {
                "BlobPointer": { "type": "object", "properties": {
                    "content_type": { "type": "string" } } },
                "Role": { "type": "string", "enum": ["owner", "member"] },
                "UserId": { "type": "string", "format": "uuid" },
                "Change": { "type": "object", "properties": {
                    "key": { "type": ["string", "null"] },
                    "label": { "$ref": "#/components/schemas/UserId" } } }
            } }
        })
    }

    #[test]
    fn every_field_of_a_jobs_origin_is_audited() {
        let path = "crates/kernel/src/tracking/context.rs";
        let code = |extra: &str| {
            vec![(
                path.to_owned(),
                format!(
                    "pub struct JobOrigin {{ pub actor: Option<Actor>, pub correlation: String,{extra} }}\n\
                     pub struct Actor {{ #[serde(rename = \"type\")] pub kind: ActorType, pub id: Option<String> }}"
                ),
            )]
        };
        assert_eq!(
            check_job_origin(&code(""), path, JOB_ORIGIN_FIELDS),
            Vec::<String>::new()
        );
        let added = check_job_origin(&code(" pub email: String,"), path, JOB_ORIGIN_FIELDS);
        assert_eq!(added.len(), 1, "{added:?}");
        assert!(added[0].contains("`email` of `JobOrigin`"));
        let stale = [("JobOrigin", "trace", "a trace")];
        let mut audit = JOB_ORIGIN_FIELDS.to_vec();
        audit.extend(stale);
        let violations = check_job_origin(&code(""), path, &audit);
        assert!(
            violations.iter().any(|v| v.contains("no longer has")),
            "{violations:?}"
        );
    }

    #[test]
    fn the_real_origin_is_audited() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let text = std::fs::read_to_string(root.join(JOB_ORIGIN_SOURCE)).unwrap();
        let sources = vec![(JOB_ORIGIN_SOURCE.to_owned(), text)];
        assert_eq!(
            check_job_origin(&sources, JOB_ORIGIN_SOURCE, JOB_ORIGIN_FIELDS),
            Vec::<String>::new()
        );
    }

    #[test]
    fn every_free_text_event_field_is_listed_by_its_path() {
        let opaque = serde_json::json!({ "type": "object", "properties": {
            "participants": { "$ref": "#/components/schemas/BlobPointer" },
            "text": { "oneOf": [
                { "$ref": "#/components/schemas/BlobPointer" }, { "type": "null" } ] },
            "from": { "$ref": "#/components/schemas/Role" },
            "author": { "oneOf": [{ "$ref": "#/components/schemas/UserId" }, { "type": "null" }] },
            "ids": { "type": "array", "items": { "type": "string", "format": "uuid" } },
            "sent_at": { "type": "string", "format": "date-time" },
            "count": { "type": "integer" }
        } });
        assert_eq!(
            check_event_payloads(&catalogue(&opaque), &[]),
            Vec::<String>::new()
        );

        let free = serde_json::json!({ "type": "object", "properties": {
            "workspace": { "type": "string" },
            "subject": { "type": "string" },
            "items": { "type": "array", "items": { "type": "object", "properties": {
                "change": { "$ref": "#/components/schemas/Change" },
                "tags": { "type": "array", "items": { "type": "string" } } } } }
        } });
        let violations = check_event_payloads(&catalogue(&free), &[]);
        let named: Vec<&str> = [
            "items[].change.key",
            "items[].tags[]",
            "subject",
            "workspace",
        ]
        .into_iter()
        .filter(|path| violations.iter().any(|v| v.contains(&format!("`{path}`"))))
        .collect();
        assert_eq!(violations.len(), 4, "{violations:?}");
        assert_eq!(named.len(), 4, "{violations:?}");

        let allowed = [
            ("*", "workspace", "the routing field"),
            ("test.happened", "subject", "an opaque account id"),
            ("test.happened", "items[].change.key", "a provider key"),
            ("test.happened", "items[].tags[]", "code names"),
            (
                "test.happened",
                "change.key",
                "the flattened name is not the path",
            ),
            ("*", "gone", "no longer carried"),
        ];
        let violations = check_event_payloads(&catalogue(&free), &allowed);
        assert_eq!(violations.len(), 3, "{violations:?}");
        assert_eq!(
            violations
                .iter()
                .filter(|v| v.contains("no event carries"))
                .count(),
            2,
            "{violations:?}"
        );
        assert!(
            violations
                .iter()
                .any(|v| v.contains("only `workspace` may be")),
            "{violations:?}"
        );
    }

    #[test]
    fn a_duplicate_entry_and_an_open_map_are_reported() {
        let open = serde_json::json!({ "type": "object", "properties": {
            "extra": { "type": "object", "additionalProperties": true }
        } });
        let violations = check_event_payloads(&catalogue(&open), &[]);
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].contains("open map at `extra`"),
            "{violations:?}"
        );

        let free = serde_json::json!({ "type": "object", "properties": {
            "subject": { "type": "string" } } });
        let twice = [
            ("test.happened", "subject", "an id"),
            ("test.happened", "subject", "an id again"),
        ];
        let violations = check_event_payloads(&catalogue(&free), &twice);
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations[0].contains("twice"), "{violations:?}");

        let scan = JobErrorScan {
            values: vec![("crates/x/src/a.rs".to_owned(), "id".to_owned())],
            problems: Vec::new(),
        };
        let twice = [
            ("crates/x/src/a.rs", "id", 1, "an id"),
            ("crates/x/src/a.rs", "id", 1, "an id again"),
        ];
        let violations = check_job_errors(&scan, &twice);
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations[0].contains("twice"), "{violations:?}");
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
        let scan = job_payloads(&sources(&[("crates/x/src/jobs.rs", JOBS)]));
        assert_eq!(scan.problems, Vec::<String>::new());
        assert_eq!(
            scan.payloads,
            [
                JobPayload {
                    kind: "test.page".to_owned(),
                    path: "crates/x/src/jobs.rs".to_owned(),
                    fields: vec!["inbox".to_owned(), "after".to_owned()],
                },
                JobPayload {
                    kind: "test.sweep".to_owned(),
                    path: "crates/x/src/jobs.rs".to_owned(),
                    fields: Vec::new(),
                },
            ]
        );
    }

    const UNREADABLE: &str = r#"
const SHARED: &str = "test.one";

impl Job for NoName {
    async fn run(self) {}
}

impl JobHandler for NoPayload {
    const NAME: &'static str = "test.no_payload";
}

impl JobHandler for Collides {
    type Payload = Twice;
    const NAME: &'static str = SHARED;
}

impl Job for Missing {
    const NAME: &'static str = "test.missing";
}
"#;

    #[test]
    fn a_job_that_cannot_be_read_is_reported() {
        let scan = job_payloads(&sources(&[
            ("crates/x/src/odd.rs", UNREADABLE),
            (
                "crates/y/src/a.rs",
                "const SHARED: &str = \"test.two\";\npub struct Twice;",
            ),
            ("crates/z/src/b.rs", "pub struct Twice { pub id: Id }"),
        ]));
        assert!(scan.payloads.is_empty(), "{:?}", scan.payloads);
        let expected = [
            "job `NoName`: no `const NAME",
            "job `test.no_payload` declares no `type Payload`",
            "constant defined with several values",
            "job `test.missing`: its payload struct `Missing` is not in the sources",
        ];
        assert_eq!(scan.problems.len(), expected.len(), "{:?}", scan.problems);
        for part in expected {
            assert!(
                scan.problems.iter().any(|p| p.contains(part)),
                "{part}: {:?}",
                scan.problems
            );
        }

        let ambiguous = job_payloads(&sources(&[
            (
                "crates/x/src/job.rs",
                "impl JobHandler for J { type Payload = Twice; const NAME: &'static str = \"test.j\"; }",
            ),
            ("crates/y/src/a.rs", "pub struct Twice;"),
            ("crates/z/src/b.rs", "pub struct Twice { pub id: Id }"),
        ]));
        assert_eq!(ambiguous.problems.len(), 1, "{:?}", ambiguous.problems);
        assert!(ambiguous.problems[0].contains("defined in several files"));
    }

    #[test]
    fn every_job_payload_field_is_audited() {
        let scan = job_payloads(&sources(&[("crates/x/src/jobs.rs", JOBS)]));
        let violations = check_job_payloads(&scan, &[("test.page", "inbox", "an id")]);
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations[0].contains("stores `after`"), "{violations:?}");

        let audit = [
            ("test.page", "inbox", "an id"),
            ("test.page", "after", "a cursor"),
            ("test.page", "before", "gone"),
        ];
        let violations = check_job_payloads(&scan, &audit);
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations[0].contains("no longer has"), "{violations:?}");
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
    let d = JobError::retry(format!("again {stream}"));
    let e = Self::retry(other.to_string());
    let f = JobError::Fatal(built.to_string());
    let g = JobError::Retry { message: format!("{kind}"), after: None };
    let h = JobError::Retry { message, after };
    let i = if matches!(err, Some(_)) { JobError::Fatal(m) } else { JobError::fatal("x") };
    match err {
        JobError::Fatal(_) => {}
        JobError::Retry { after, .. } => {}
    }
    if matches!(err, JobError::Fatal(_)) {}
    // JobError::fatal(format!("{commented}"))
}

impl From<Problem> for crate::JobError {
    fn from(err: Problem) -> Self {
        Self::fatal(err.code())
    }
}

impl From<Other> for JobError {
    fn from(err: Other) -> Self {
        match err {
            Other::Gone(why) => Self::Fatal(why),
            Other::Busy => Self::Retry { message: err.kind(), after: None },
            Other::Mapped(e) => e.map(Self::Fatal),
        }
    }
}

#[cfg(test)]
mod tests {
    fn t() { JobError::fatal(format!("{in_tests}")); }
}
"#;

    #[test]
    fn job_error_values_are_audited_per_file_and_use() {
        let scan = job_error_values(&sources(&[
            ("crates/x/src/run.rs", ERRORS),
            ("crates/x/src/run_tests.rs", ERRORS),
            (
                "crates/x/src/free.rs",
                "fn f() { x.map_err(JobError::retry)?; y.map_err(JobError::Fatal)?; }",
            ),
        ]));
        let mut values = scan.values.clone();
        values.sort();
        let at = |v: &str| ("crates/x/src/run.rs".to_owned(), v.to_owned());
        let mut expected = vec![
            at("stream"),
            at("redact::sqlx(&e)"),
            at("why"),
            at("stream"),
            at("built.to_string()"),
            at("kind"),
            at("err.code()"),
            at("message"),
            at("m"),
            at("why"),
            at("err.kind()"),
        ];
        expected.sort();
        assert_eq!(
            values, expected,
            "`Self::` counts only in a `From<…> for JobError`; a shorthand `message` is the \
             value `message`; a constructor after a closed `matches!` is built; patterns, \
             tests and comments are skipped"
        );
        assert_eq!(scan.problems.len(), 3, "{:?}", scan.problems);
        for name in ["JobError::retry", "JobError::Fatal", "Self::Fatal"] {
            assert!(
                scan.problems
                    .iter()
                    .any(|p| p.contains(&format!("`{name}` is passed as a function"))),
                "{name}: {:?}",
                scan.problems
            );
        }

        let audit = [
            ("crates/x/src/run.rs", "stream", 1, "an id"),
            ("crates/x/src/run.rs", "redact::sqlx(&e)", 1, "by kind"),
            ("crates/x/src/run.rs", "built.to_string()", 1, "a code"),
            ("crates/x/src/run.rs", "kind", 1, "a name"),
            ("crates/x/src/run.rs", "err.code()", 1, "a code"),
            ("crates/x/src/run.rs", "message", 1, "a code"),
            ("crates/x/src/run.rs", "m", 1, "a code"),
            ("crates/x/src/run.rs", "err.kind()", 1, "a code"),
            ("crates/x/src/other.rs", "why", 1, "elsewhere"),
        ];
        let violations = check_job_errors(&scan, &audit);
        assert_eq!(violations.len(), 6, "{violations:?}");
        assert!(
            violations[0].contains("passed as a function"),
            "{violations:?}"
        );
        assert!(
            violations
                .iter()
                .any(|v| v.contains("`stream` 2 time(s)") && v.contains("lists 1")),
            "{violations:?}"
        );
        assert!(
            violations.iter().any(|v| v.contains("built from `why`")),
            "{violations:?}"
        );
        assert!(
            violations.iter().any(|v| v.contains("no job error there")),
            "{violations:?}"
        );
    }

    #[test]
    fn a_count_that_went_down_fails_too() {
        let scan = JobErrorScan {
            values: vec![("crates/x/src/a.rs".to_owned(), "id".to_owned())],
            problems: Vec::new(),
        };
        let violations = check_job_errors(&scan, &[("crates/x/src/a.rs", "id", 2, "an id")]);
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].contains("`id` 1 time(s)") && violations[0].contains("lists 2"),
            "{violations:?}"
        );
    }

    #[test]
    fn brackets_inside_literals_do_not_count() {
        assert_eq!(closing("c, '(') rest", '(', ')'), Some(6));
        assert_eq!(closing(r#"s, "(") rest"#, '(', ')'), Some(6));
        assert_eq!(closing(r##"s, r#"("#) x"##, '(', ')'), Some(9));
        assert_eq!(closing(r"c, '\'') x", '(', ')'), Some(7));
        assert_eq!(
            closing("&'a str) x", '(', ')'),
            Some(7),
            "a lifetime is no literal"
        );
        assert_eq!(
            split_top_level("',', \"a, b\", c", false),
            ["','", "\"a, b\"", "c"]
        );
    }

    #[test]
    fn a_matches_that_never_closes_holds_nothing() {
        let code = "matches!(x JobError::Fatal(m)";
        assert!(written_out(
            code,
            code.find("JobError").unwrap_or_default(),
            ";"
        ));

        let scan = job_error_values(&sources(&[(
            "crates/x/src/m.rs",
            r#"
fn f() {
    if matches!(c, '(') { JobError::Fatal(m1) } else { JobError::fatal("x") };
    if matches!(s, "(") { JobError::Fatal(m2) } else { JobError::fatal("y") };
    if matches!(e, JobError::Fatal(_)) {}
}
"#,
        )]));
        let values: Vec<&str> = scan.values.iter().map(|(_, v)| v.as_str()).collect();
        assert_eq!(values, ["m1", "m2"], "{:?}", scan.problems);
    }
}
