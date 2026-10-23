//! Compact projections of other modules' answers, as the model reads them:
//! only the fields it needs, mail reduced to its own words, every result
//! bounded. Read from the operations' JSON by field name, so a field another
//! module adds is simply not shown.

use campbooks_kernel::text::without_quoted_history;
use serde_json::{Map, Value, json};

use super::words::{clean, cut_bytes};

/// The marker a cut text or result ends with; the instruction explains it.
pub const CUT: &str = "[cut]";
/// Newest messages of a conversation handed on.
const MESSAGES: usize = 4;
/// Most of one message's own words, in bytes.
const MESSAGE_BYTES: usize = 1536;
/// Newest history events handed on with a conversation.
const ABOUT_EVENTS: usize = 40;
/// Most people named as who a conversation is with.
const PEOPLE: usize = 8;
/// Most bytes of a subject, a name or a summary.
const FIELD_BYTES: usize = 300;

/// `value` as JSON text of at most `max` bytes: whole, or cut at a
/// character boundary and ended with [`CUT`]. Whether it was cut.
pub fn bounded(value: &Value, max: usize) -> (String, bool) {
    let text = value.to_string();
    if text.len() <= max {
        return (text, false);
    }
    let room = max.saturating_sub(CUT.len() + 1);
    (format!("{} {CUT}", cut_bytes(&text, room)), true)
}

/// A short field: cleaned, bounded.
fn short(value: &Value) -> Value {
    match value.as_str() {
        Some(text) => Value::String(cut(&clean(text), FIELD_BYTES)),
        None => Value::Null,
    }
}

/// `text` within `max` bytes, ended with [`CUT`] when cut.
fn cut(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    format!(
        "{} {CUT}",
        cut_bytes(text, max.saturating_sub(CUT.len() + 1))
    )
}

/// A person: their name and address.
fn person(address: &Value) -> Value {
    json!({ "name": short(&address["name"]), "address": short(&address["address"]) })
}

/// Only `keys` of `object`, those present.
fn pick(object: &Value, keys: &[&str]) -> Value {
    let mut out = Map::new();
    for key in keys {
        if let Some(value) = object.get(*key)
            && !value.is_null()
        {
            out.insert((*key).to_owned(), value.clone());
        }
    }
    Value::Object(out)
}

/// A conversation's need, as lists and the reader carry it.
fn need(need: &Value) -> Value {
    if need.is_null() {
        return Value::Null;
    }
    let mut out = pick(
        need,
        &[
            "reason",
            "question",
            "trust",
            "basis",
            "due_on",
            "overdue",
            "urgent",
            "lapses_at",
            "nudges",
        ],
    );
    if let (Some(object), false) = (out.as_object_mut(), need["waiting_on"].is_null()) {
        object.insert("waiting_on".into(), person(&need["waiting_on"]));
    }
    out
}

/// Where a conversation is beside its stream.
fn place(summary: &Value) -> &'static str {
    if summary["trashed"] == true {
        "trash"
    } else if summary["spam"] == true {
        "spam"
    } else if summary["archived"] == true {
        "archive"
    } else {
        "inbox"
    }
}

/// A conversation as lists show it (`conversations.list`'s entries).
pub fn summary(summary: &Value) -> Value {
    let mut out = json!({
        "id": summary["id"],
        "subject": short(&summary["subject"]),
        "inbox_id": summary["inbox_id"],
        "stream": summary["stream"],
        "place": place(summary),
        "last_message_at": summary["last_message_at"],
        "messages": summary["message_count"],
        "unread": summary["unread_count"],
        "starred": summary["starred"],
        "latest_from_you": summary["latest_from_you"],
        "need": need(&summary["need"]),
    });
    if !summary["latest_from"].is_null() {
        out["latest_from"] = person(&summary["latest_from"]);
    }
    out
}

/// A conversation as the model reads what a chat is about, or what
/// `read_conversation` found: `conversations.get`'s answer, and the history
/// when given.
pub fn conversation(detail: &Value, history: Option<&Value>) -> Value {
    let head = &detail["conversation"];
    let all: Vec<&Value> = detail["messages"]
        .as_array()
        .map(|messages| messages.iter().collect())
        .unwrap_or_default();
    let mut people: Vec<Value> = Vec::new();
    for message in &all {
        let parties = std::iter::once(&message["from"])
            .chain(message["to"].as_array().into_iter().flatten())
            .chain(message["cc"].as_array().into_iter().flatten());
        for party in parties {
            let known = people
                .iter()
                .any(|p| p["address"] == short(&party["address"]));
            if !known && people.len() < PEOPLE && party["address"].is_string() {
                people.push(person(party));
            }
        }
    }
    let newest: Vec<Value> = all
        .iter()
        .skip(all.len().saturating_sub(MESSAGES))
        .map(|message| {
            let words = message["text"]["content"]
                .as_str()
                .map(|text| cut(&clean(&without_quoted_history(text)), MESSAGE_BYTES));
            json!({
                "id": message["id"],
                "direction": message["direction"],
                "from": person(&message["from"]),
                "sent_at": message["sent_at"],
                "own_words": words,
                "attachments": message["attachments"].as_array().map_or(0, Vec::len),
            })
        })
        .collect();
    let decisions: Vec<Value> = detail["decisions"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|decision| {
            let reasons: Vec<Value> = decision["reasons"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|reason| json!({ "rule": reason["rule"], "evidence": short(&reason["evidence"]) }))
                .collect();
            let mut out = pick(
                decision,
                &["question", "value", "source", "confidence", "acted", "threshold"],
            );
            out["reasons"] = Value::Array(reasons);
            out
        })
        .collect();
    let mut out = json!({
        "id": head["id"],
        "subject": short(&head["subject"]),
        "inbox_id": head["inbox_id"],
        "stream": head["stream"],
        "placed_by": head["stream_source"],
        "place": place(head),
        "starred": head["starred"],
        "messages_in_all": head["message_count"],
        "with": people,
        "newest_messages": newest,
        "scout_read_the_newest_inbound": decisions,
        "need": need(&head["need"]),
        "need_summary": short(&detail["need_summary"]),
    });
    if !head["suggestion"].is_null() {
        out["stream_suggestion"] = pick(&head["suggestion"], &["stream", "source", "confidence"]);
    }
    if !head["rule"].is_null() {
        out["rule"] = pick(&head["rule"], &["id", "conditions", "actions"]);
    }
    if let Some(labels) = head["labels"].as_array() {
        out["labels"] = labels.iter().map(|label| short(&label["name"])).collect();
    }
    if let Some(history) = history {
        out["history"] = events(history, ABOUT_EVENTS);
    }
    out
}

/// Most conditions or actions of a rule handed on.
const RULE_PARTS: usize = 12;

/// A rule's conditions or actions, as the rule holds them, at most
/// [`RULE_PARTS`], each string bounded.
fn parts(parts: &Value) -> Value {
    parts
        .as_array()
        .into_iter()
        .flatten()
        .take(RULE_PARTS)
        .map(|part| match part {
            Value::Object(fields) => Value::Object(
                fields
                    .iter()
                    .map(|(key, value)| {
                        let value = if value.is_string() {
                            short(value)
                        } else {
                            value.clone()
                        };
                        (key.clone(), value)
                    })
                    .collect(),
            ),
            other => short(other),
        })
        .collect()
}

/// A rule as the model reads what a chat is about
/// (`conversations.get_rule`'s answer): what it matches and does, where it
/// came from, whether it acts, and what it has changed.
pub fn rule(rule: &Value) -> Value {
    let mut out = json!({
        "id": rule["id"],
        "inbox_id": rule["inbox_id"],
        "conditions": parts(&rule["conditions"]),
        "actions": parts(&rule["actions"]),
        "origin": rule["origin"],
        "state": rule["state"],
        "created_at": rule["created_at"],
        "conversations_acted_on": rule["applied_count"],
        "last_acted_at": rule["last_applied_at"],
        "changed_back_in_the_last_30_days": rule["recent_exceptions"],
        "shared_with_other_members": rule["shared"],
    });
    if !rule["origin_text"].is_null() {
        out["taught_as"] = short(&rule["origin_text"]);
    }
    if !rule["paused_reason"].is_null() {
        out["paused_reason"] = rule["paused_reason"].clone();
    }
    out
}

/// One of Scout's suggestions as the model reads what a chat is about
/// (`conversations.get_rule_notice`'s answer): what Scout noticed, of whom,
/// the rule it offers and why, in numbers.
pub fn rule_notice(notice: &Value) -> Value {
    json!({
        "id": notice["id"],
        "inbox_id": notice["inbox_id"],
        "kind": notice["kind"],
        "sender": {
            "name": short(&notice["sender_name"]),
            "address": short(&notice["sender"]),
        },
        "rule_offered": {
            "conditions": parts(&notice["conditions"]),
            "actions": parts(&notice["actions"]),
        },
        "evidence": pick(
            &notice["evidence"],
            &["received", "opened", "archived_unread", "last_week", "window_days", "cadence"],
        ),
        "shared_with_other_members": notice["shared"],
    })
}

/// A conversation's history (`conversations.get_history`): its newest
/// `max` events, oldest first.
pub fn events(history: &Value, max: usize) -> Value {
    let all: Vec<&Value> = history["events"]
        .as_array()
        .map(|events| events.iter().collect())
        .unwrap_or_default();
    let older = all.len().saturating_sub(max);
    let kept: Vec<Value> = all
        .into_iter()
        .skip(older)
        .map(|event| {
            let mut event = event.clone();
            if event["from"].is_object() {
                event["from"] = person(&event["from"]);
            }
            event
        })
        .collect();
    json!({
        "events": kept,
        "older_events_left_out": older > 0 || history["truncated"] == true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_conversation_is_its_newest_own_words_with_no_quoted_history() {
        let long = "word ".repeat(1_000);
        let messages: Vec<Value> = (0..6)
            .map(|n| {
                json!({
                    "id": format!("m{n}"),
                    "direction": "inbound",
                    "from": {"name": "Marta Silva", "address": "marta@example.com"},
                    "to": [{"address": "sam@example.com"}],
                    "sent_at": "2026-10-20T09:00:00Z",
                    "text": {"content": format!("Reply {n}.\n\nOn Mon, Sam <sam@example.com> wrote:\n> old {n}\n{long}")},
                    "attachments": [],
                })
            })
            .collect();
        let detail = json!({
            "conversation": {"id": "c-1", "subject": "Rent", "stream": "primary", "need": null},
            "messages": messages,
            "decisions": [{"question": "money", "value": "owed", "source": "model",
                           "confidence": 91, "acted": true, "threshold": 90,
                           "reasons": [{"rule": "model.decide", "evidence": "", "sentence": "Pay by Friday"}]}],
            "need_summary": "Marta asks for the rent.",
        });
        let out = conversation(&detail, None);
        let newest = out["newest_messages"].as_array().unwrap();
        assert_eq!(newest.len(), 4);
        assert_eq!(newest[0]["id"], "m2");
        assert_eq!(newest[3]["own_words"], "Reply 5.");
        assert!(!out.to_string().contains("old 5"));
        assert!(!out.to_string().contains("Pay by Friday"), "no sentences");
        assert_eq!(out["with"].as_array().unwrap().len(), 2);
        assert_eq!(out["scout_read_the_newest_inbound"][0]["confidence"], 91);
    }

    #[test]
    fn a_rule_is_its_parts_origin_state_and_what_it_changed() {
        let long = "x".repeat(2_000);
        let out = rule(&json!({
            "id": "r-1", "inbox_id": "i-1", "teaching_id": "t-1",
            "conditions": [{"field": "sender_address", "value": "news@shop.example"}],
            "actions": [{"action": "archive"}, {"action": "label", "value": long}],
            "origin": "instruction", "origin_text": "Newsletters never need me",
            "state": "active", "created_at": "2026-10-01T09:00:00Z",
            "applied_count": 14, "last_applied_at": "2026-10-20T09:00:00Z",
            "recent_exceptions": 1, "shared": false, "may_change": true,
            "content_token": "secret-token",
        }));
        assert_eq!(out["conversations_acted_on"], 14);
        assert_eq!(out["taught_as"], "Newsletters never need me");
        assert_eq!(out["conditions"][0]["value"], "news@shop.example");
        assert!(out["actions"][1]["value"].as_str().unwrap().ends_with(CUT));
        assert!(!out.to_string().contains("secret-token"), "no tokens");
        assert!(out.get("paused_reason").is_none());
    }

    #[test]
    fn a_suggestion_is_its_sender_rule_and_numbers() {
        let out = rule_notice(&json!({
            "id": "n-1", "inbox_id": "i-1", "kind": "keep_off_home",
            "sender": "builds@ci.example.com", "sender_name": "CI",
            "conditions": [{"field": "sender_address", "value": "builds@ci.example.com"}],
            "actions": [{"action": "surface", "value": "never"}],
            "evidence": {"received": 12, "opened": 0, "archived_unread": 0,
                         "last_week": 7, "window_days": 60, "cadence": "daily"},
            "shared": true,
        }));
        assert_eq!(out["sender"]["address"], "builds@ci.example.com");
        assert_eq!(out["sender"]["name"], "CI");
        assert_eq!(out["rule_offered"]["actions"][0]["value"], "never");
        assert_eq!(out["evidence"]["cadence"], "daily");
        assert_eq!(out["shared_with_other_members"], true);
    }

    #[test]
    fn long_text_is_cut_with_the_marker() {
        let (text, cut) = bounded(&json!({"text": "é".repeat(5_000)}), 1_000);
        assert!(cut && text.len() <= 1_000 && text.ends_with(CUT), "{text}");
        let (text, cut) = bounded(&json!({"text": "short"}), 1_000);
        assert!(!cut && !text.contains(CUT));
    }
}
