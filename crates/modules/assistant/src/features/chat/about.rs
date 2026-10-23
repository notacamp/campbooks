//! What a chat may be about: the about registry, by record type. Each entry
//! names the read operations that describe a record of its type (invoked as
//! the member, by operation id only) and how their answers are projected for
//! the model: a conversation, a rule, or one of Scout's noticed suggestions.
//! Calendar events and tasks add an entry each (no migration: the stored
//! type is checked here).

use campbooks_kernel::RecordRef;
use serde_json::{Value, json};

use super::{
    member::{Member, Unread},
    project,
};

/// Most bytes of what a chat is about, as the model reads it.
pub const MAX_ABOUT_BYTES: usize = 12 * 1024;

/// One record type a chat may be about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AboutType {
    /// The record type.
    pub record_type: &'static str,
    /// The operations read, in order; the first decides whether the member
    /// can see the record. Each takes the record's id as `id_param`.
    pub reads: &'static [&'static str],
    /// The path parameter naming the record.
    pub id_param: &'static str,
    /// The message (`chat.*`) an answer starts with when the member can no
    /// longer see the record.
    pub lost_line: &'static str,
}

/// A conversation.
const CONVERSATION: &str = "conversations.conversation";
/// A rule taught for an inbox.
const RULE: &str = "conversations.rule";
/// One of Scout's noticed suggestions (a smart rule it offers).
const RULE_NOTICE: &str = "conversations.rule_notice";

/// Every record type a chat may be about.
pub const REGISTRY: &[AboutType] = &[
    AboutType {
        record_type: CONVERSATION,
        reads: &["conversations.get", "conversations.get_history"],
        id_param: "conversation_id",
        lost_line: "chat.about_lost",
    },
    AboutType {
        record_type: RULE,
        reads: &["conversations.get_rule"],
        id_param: "rule_id",
        lost_line: "chat.about_lost_rule",
    },
    AboutType {
        record_type: RULE_NOTICE,
        reads: &["conversations.get_rule_notice"],
        id_param: "notice_id",
        lost_line: "chat.about_lost_rule_notice",
    },
];

/// The registry's entry for `record_type`.
pub fn entry(record_type: &str) -> Option<&'static AboutType> {
    REGISTRY
        .iter()
        .find(|entry| entry.record_type == record_type)
}

/// An operation's input naming the record by `param`.
fn input(param: &str, id: &str) -> Value {
    let mut fields = serde_json::Map::new();
    fields.insert(param.to_owned(), Value::String(id.to_owned()));
    Value::Object(fields)
}

/// What the model is given about the record.
#[derive(Debug, Clone, PartialEq)]
pub enum Seen {
    /// The record, as projected and bounded.
    Visible(String),
    /// The member can no longer see it.
    Lost,
}

/// Whether the member can see `about` now (its first read answers).
///
/// # Errors
/// [`Unread`] when they cannot, or the read failed.
pub async fn check(member: &Member, about: &RecordRef) -> Result<(), Unread> {
    let entry = entry(&about.record_type).ok_or(Unread::Refused)?;
    let first = entry.reads.first().ok_or(Unread::Broken("empty_entry"))?;
    member
        .read(first, input(entry.id_param, &about.id))
        .await
        .map(drop)
}

/// `about` read again, as the member, for one question.
///
/// # Errors
/// [`Unread::Broken`] when a read failed for another reason than access.
pub async fn read(member: &Member, about: &RecordRef) -> Result<Seen, Unread> {
    let Some(entry) = entry(&about.record_type) else {
        return Ok(Seen::Lost);
    };
    let mut answers: Vec<Value> = Vec::with_capacity(entry.reads.len());
    for operation in entry.reads {
        match member
            .read(operation, input(entry.id_param, &about.id))
            .await
        {
            Ok(body) => answers.push(body),
            Err(Unread::NotVisible | Unread::Refused) => return Ok(Seen::Lost),
            Err(broken) => return Err(broken),
        }
    }
    let first = answers.first().unwrap_or(&Value::Null);
    let projected = match entry.record_type {
        CONVERSATION => project::conversation(first, answers.get(1)),
        RULE => project::rule(first),
        RULE_NOTICE => project::rule_notice(first),
        _ => return Ok(Seen::Lost),
    };
    let mut value = json!({ "type": entry.record_type, "id": about.id });
    if let (Some(out), Value::Object(fields)) = (value.as_object_mut(), projected) {
        out.extend(fields);
    }
    Ok(Seen::Visible(project::bounded(&value, MAX_ABOUT_BYTES).0))
}
