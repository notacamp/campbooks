//! What a chat may be about: the about registry, by record type. Each entry
//! names the read operations that describe a record of its type (invoked as
//! the member, by operation id only) and how their answers are projected for
//! the model: a conversation, a rule, one of Scout's noticed suggestions, or
//! a task. Calendar events add an entry (no migration: the stored
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
/// A task.
const TASK: &str = "tasks.task";
/// An event of the member's calendar.
const SCHEDULE_EVENT: &str = "schedule.event";
/// A page (the pages module's), while mail and Scout work with pages.
const PAGE: &str = "pages.page";

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
    AboutType {
        record_type: TASK,
        reads: &["tasks.get"],
        id_param: "task_id",
        lost_line: "chat.about_lost_task",
    },
    // An event of the member's calendar, and what Scout read in the email it
    // came from (the second read is made only when the event has a source
    // message, and only if the member can still read it).
    AboutType {
        record_type: SCHEDULE_EVENT,
        reads: &["schedule.get_event", "conversations.get_message_read"],
        id_param: "event_id",
        lost_line: "chat.about_lost_event",
    },
    // A page of the member's, read as text: a mention in it stays a
    // reference (campbooks-cloud#364). Only while mail and Scout work with
    // pages.
    AboutType {
        record_type: PAGE,
        reads: &["pages.read_text"],
        id_param: "page_id",
        lost_line: "chat.about_lost_page",
    },
];

/// The registry's entry for `record_type`; a page only while `pages`.
pub fn entry(record_type: &str, pages: bool) -> Option<&'static AboutType> {
    REGISTRY
        .iter()
        .filter(|entry| pages || entry.record_type != PAGE)
        .find(|entry| entry.record_type == record_type)
}

/// An operation's input naming the record by `param`.
fn input(param: &str, id: &str) -> Value {
    let mut fields = serde_json::Map::new();
    fields.insert(param.to_owned(), Value::String(id.to_owned()));
    Value::Object(fields)
}

/// The input of read number `step` of `entry`: the first names the record;
/// a follow-up is made from what the earlier reads answered, or not at all.
fn input_for(
    entry: &AboutType,
    step: usize,
    about: &RecordRef,
    earlier: &[Value],
) -> Option<Value> {
    if step == 0 {
        return Some(input(entry.id_param, &about.id));
    }
    match entry.record_type {
        // What Scout read in the event's source message.
        SCHEDULE_EVENT => {
            let event = earlier.first()?;
            let source = &event["source"];
            let message = source["id"]
                .as_str()
                .filter(|_| source["type"] == "conversations.message")?;
            let conversation = event["source_conversation"].as_str()?;
            Some(json!({ "conversation_id": conversation, "message_id": message }))
        }
        _ => Some(input(entry.id_param, &about.id)),
    }
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
    let entry = entry(&about.record_type, member.pages).ok_or(Unread::Refused)?;
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
    let Some(entry) = entry(&about.record_type, member.pages) else {
        return Ok(Seen::Lost);
    };
    let mut answers: Vec<Value> = Vec::with_capacity(entry.reads.len());
    for (step, operation) in entry.reads.iter().enumerate() {
        let Some(request) = input_for(entry, step, about, &answers) else {
            answers.push(Value::Null);
            continue;
        };
        match member.read(operation, request).await {
            Ok(body) => answers.push(body),
            // The email behind an event may be one the member cannot read:
            // the event is still theirs to ask about.
            Err(Unread::NotVisible | Unread::Refused)
                if step > 0 && entry.record_type == SCHEDULE_EVENT =>
            {
                answers.push(Value::Null);
            }
            Err(Unread::NotVisible | Unread::Refused) => return Ok(Seen::Lost),
            Err(broken) => return Err(broken),
        }
    }
    let first = answers.first().unwrap_or(&Value::Null);
    let projected = match entry.record_type {
        CONVERSATION => project::conversation(first, answers.get(1)),
        RULE => project::rule(first),
        RULE_NOTICE => project::rule_notice(first),
        TASK => project::task_detail(first),
        SCHEDULE_EVENT => project::event(first, answers.get(1)),
        PAGE => project::page(first, MAX_ABOUT_BYTES - 2 * 1024),
        _ => return Ok(Seen::Lost),
    };
    let mut value = json!({ "type": entry.record_type, "id": about.id });
    if let (Some(out), Value::Object(fields)) = (value.as_object_mut(), projected) {
        out.extend(fields);
    }
    Ok(Seen::Visible(project::bounded(&value, MAX_ABOUT_BYTES).0))
}
