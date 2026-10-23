//! Scout's read-only tools: each one or two catalogue operations the
//! assistant may call, run as the member, with a narrow schema and a
//! compact, bounded result. A call naming any other tool, or with arguments
//! outside its schema, is never invoked: the model is told so. The
//! proposing tools ([`super::propose`]) are offered with them and only
//! checked and stored, never run.

use campbooks_ai::{ToolCall, ToolDefinition};
use campbooks_kernel::RecordRef;
use serde_json::{Map, Value, json};

use super::{
    member::{Member, Unread},
    project,
    propose::{self, Proposer, Proposing},
    types::{ChatStep, ChatTool},
};

/// Most bytes of one tool's result.
pub const MAX_RESULT_BYTES: usize = 8 * 1024;
/// Most bytes of all the tools' results of one question.
pub const MAX_QUESTION_BYTES: usize = 24 * 1024;
/// Most records a step lists.
const STEP_RECORDS: usize = 10;
/// Most conversations or people a search answers.
const FOUND: usize = 10;
/// Most needs `what_needs_me` answers.
const NEEDS: usize = 20;
/// Newest history events `conversation_history` answers.
const HISTORY_EVENTS: usize = 60;
/// Record type of a conversation (the conversations module's).
const CONVERSATION: &str = "conversations.conversation";
/// Record type of an inbox (the conversations module's).
const INBOX: &str = "conversations.inbox";
/// Record type of a task (the tasks module's).
const TASK: &str = "tasks.task";
/// Most tasks `my_tasks` answers.
const TASKS: usize = 30;

/// The catalogue operations each tool may invoke: the test checks that each
/// is in the catalogue, may be called by the assistant and changes nothing.
#[cfg(test)]
pub const OPERATIONS: &[(ChatTool, &[&str])] = &[
    (
        ChatTool::FindConversations,
        &["search.query", "conversations.list"],
    ),
    (ChatTool::ReadConversation, &["conversations.get"]),
    (
        ChatTool::ConversationHistory,
        &["conversations.get_history"],
    ),
    (ChatTool::FindPeople, &["conversations.list_senders"]),
    (
        ChatTool::WhatNeedsMe,
        &["attention.list", "conversations.list"],
    ),
    (
        ChatTool::MyInboxes,
        &["conversations.list_inboxes", "conversations.list_streams"],
    ),
    (ChatTool::MyTasks, &["tasks.list"]),
    (ChatTool::ReadTask, &["tasks.get"]),
];

impl ChatTool {
    /// Every tool, in the order they are offered.
    pub const ALL: [Self; 8] = [
        Self::FindConversations,
        Self::ReadConversation,
        Self::ConversationHistory,
        Self::FindPeople,
        Self::WhatNeedsMe,
        Self::MyInboxes,
        Self::MyTasks,
        Self::ReadTask,
    ];

    /// The name the model calls it by.
    pub const fn name(self) -> &'static str {
        match self {
            Self::FindConversations => "find_conversations",
            Self::ReadConversation => "read_conversation",
            Self::ConversationHistory => "conversation_history",
            Self::FindPeople => "find_people",
            Self::WhatNeedsMe => "what_needs_me",
            Self::MyInboxes => "my_inboxes",
            Self::MyTasks => "my_tasks",
            Self::ReadTask => "read_task",
        }
    }

    /// The tool named `name`, if there is one.
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tool| tool.name() == name)
    }
}

/// A conversation id argument.
fn conversation_id() -> Value {
    json!({
        "type": "string",
        "description": "The conversation's id, as `about` or another tool gave it.",
        "minLength": 1,
        "maxLength": 64,
    })
}

/// Every tool's definition, for the model: the read-only tools, then the
/// proposing ones.
pub fn definitions() -> Vec<ToolDefinition> {
    let mut all = read_definitions();
    all.extend(propose::definitions());
    all
}

/// The read-only tools' definitions.
fn read_definitions() -> Vec<ToolDefinition> {
    let object = |properties: Value, required: &[&str]| {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": properties,
            "required": required,
        })
    };
    ChatTool::ALL
        .into_iter()
        .map(|tool| {
            let (description, parameters) = match tool {
                ChatTool::FindConversations => (
                    "Find the member's conversations: by words (a name, an address, a subject; \
                     search operators such as from: work), or the newest of a stream or place. \
                     Up to 10, newest first.",
                    object(
                        json!({
                            "query": {"type": "string", "minLength": 2, "maxLength": 200,
                                      "description": "Words to search for."},
                            "stream": {"type": "string",
                                       "enum": ["primary", "updates", "receipts", "newsletters"]},
                            "place": {"type": "string",
                                      "enum": ["inbox", "sent", "starred", "archive", "spam", "trash"]},
                        }),
                        &[],
                    ),
                ),
                ChatTool::ReadConversation => (
                    "Read one conversation as the member can: its newest messages' own words, \
                     Scout's answers about it and its need.",
                    object(json!({ "conversation_id": conversation_id() }), &["conversation_id"]),
                ),
                ChatTool::ConversationHistory => (
                    "What happened to one conversation and who did it: messages, Scout's \
                     answers, corrections, rules, its need. Oldest first.",
                    object(json!({ "conversation_id": conversation_id() }), &["conversation_id"]),
                ),
                ChatTool::FindPeople => (
                    "Find people and services who write to the member, by name, address or \
                     domain, replied to most first. Up to 10.",
                    object(
                        json!({
                            "query": {"type": "string", "minLength": 2, "maxLength": 100},
                        }),
                        &[],
                    ),
                ),
                ChatTool::WhatNeedsMe => (
                    "What needs the member now across all their mail (their list on \
                     Home), or what they are waiting on others for. Up to 20. Not for \
                     what the chat's own record asks of them: that is under `about`.",
                    object(
                        json!({
                            "waiting": {"type": "boolean",
                                        "description": "True for what the member waits on others for."},
                        }),
                        &[],
                    ),
                ),
                ChatTool::MyInboxes => (
                    "The member's inboxes (addresses, roles) and how many conversations each \
                     stream holds.",
                    object(json!({}), &[]),
                ),
                ChatTool::MyTasks => (
                    "The member's open tasks, overdue first, then by due date, undated last. \
                     Narrow them by due day (`YYYY-MM-DD`, in the member's days), only the \
                     overdue or only the undated, or the ones the member handed to a teammate. \
                     Up to 30.",
                    object(
                        json!({
                            "due_from": {"type": "string", "minLength": 10, "maxLength": 10,
                                         "description": "Due on or after this day, YYYY-MM-DD."},
                            "due_to": {"type": "string", "minLength": 10, "maxLength": 10,
                                       "description": "Due on or before this day, YYYY-MM-DD."},
                            "overdue": {"type": "boolean"},
                            "undated": {"type": "boolean"},
                            "handed_off": {"type": "boolean",
                                           "description": "True for the open tasks the member handed to someone else."},
                        }),
                        &[],
                    ),
                ),
                ChatTool::ReadTask => (
                    "Read one task of the member's: its title, dates, plan, where it came from \
                     and what changed in it.",
                    object(
                        json!({"task_id": {"type": "string", "minLength": 1, "maxLength": 64,
                                           "description": "The task's id, as `about` or another tool gave it."}}),
                        &["task_id"],
                    ),
                ),
            };
            ToolDefinition::new(tool.name(), description, parameters)
        })
        .collect()
}

/// A tool call's outcome: what the model reads, and the step, when the tool
/// ran.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    /// The result, as the model reads it.
    pub result: String,
    /// The step, when the tool ran (never for an unknown tool or bad
    /// arguments).
    pub step: Option<ChatStep>,
    /// For the log: a fixed word.
    pub kind: &'static str,
}

/// A question's tools: the member they act for, how much room is left for
/// their results, and the answer's proposals.
#[derive(Debug)]
pub struct Toolbox {
    member: Member,
    /// The answer's proposals, checked and kept.
    pub proposer: Proposer,
    /// The caller's offset from UTC in minutes (what is overdue).
    utc_offset: i32,
    left: usize,
    /// Bytes of results handed on.
    pub used: usize,
}

impl Toolbox {
    /// Tools acting for `member`.
    pub fn new(member: Member, utc_offset: i32) -> Self {
        Self {
            proposer: Proposer::new(member.clone(), utc_offset),
            member,
            utc_offset,
            left: MAX_QUESTION_BYTES,
            used: 0,
        }
    }

    /// The tool `call` runs, when it names one and its arguments fit.
    pub fn tool_for(call: &ToolCall) -> Option<ChatTool> {
        let tool = ChatTool::named(&call.name)?;
        arguments(tool, &call.arguments).map(|_| tool)
    }

    /// Run `call`, if it names a tool and its arguments fit its schema; a
    /// proposing call is only checked and kept.
    pub async fn run(&mut self, call: &ToolCall) -> Outcome {
        if let Some(proposing) = Proposing::named(&call.name) {
            let proposed = self.proposer.propose(proposing, call).await;
            return Outcome {
                result: proposed.result,
                step: None,
                kind: proposed.kind,
            };
        }
        let Some(tool) = ChatTool::named(&call.name) else {
            return refusal(
                "no_such_tool",
                "There is no tool by that name. Use only the tools you were given.",
            );
        };
        let Some(arguments) = arguments(tool, &call.arguments) else {
            return refusal(
                "invalid_arguments",
                "The arguments do not fit this tool's schema. Nothing was looked up.",
            );
        };
        if self.left == 0 {
            return refusal(
                "no_room",
                "[cut] There is no more room for results in this question: answer with \
                 what you have.",
            );
        }
        match self.invoke(tool, &arguments).await {
            Ok((value, records, count)) => {
                let (text, _) = project::bounded(&value, MAX_RESULT_BYTES.min(self.left));
                self.left = self.left.saturating_sub(text.len());
                self.used += text.len();
                Outcome {
                    result: text,
                    step: Some(ChatStep {
                        tool,
                        records: records.into_iter().take(STEP_RECORDS).collect(),
                        count,
                    }),
                    kind: "ran",
                }
            }
            Err(Unread::NotVisible) => Outcome {
                result: json!({"error": "not_visible",
                    "message": "Scout cannot see this: it does not exist, or the member cannot read it."})
                .to_string(),
                step: Some(ChatStep {
                    tool,
                    records: Vec::new(),
                    count: 0,
                }),
                kind: "not_visible",
            },
            Err(other) => Outcome {
                result: json!({"error": "failed", "message": "This look-up failed. Say so if it matters."})
                    .to_string(),
                step: None,
                kind: other.kind(),
            },
        }
    }

    /// The tool's operations, as the member: its projected result, the
    /// records it found and how many.
    async fn invoke(
        &self,
        tool: ChatTool,
        arguments: &Map<String, Value>,
    ) -> Result<(Value, Vec<RecordRef>, u32), Unread> {
        let text = |key: &str| arguments.get(key).and_then(Value::as_str);
        match tool {
            ChatTool::FindConversations => {
                self.find_conversations(text("query"), text("stream"), text("place"))
                    .await
            }
            ChatTool::ReadConversation | ChatTool::ConversationHistory => {
                let id = text("conversation_id").unwrap_or_default();
                let input = json!({ "conversation_id": id });
                let value = if tool == ChatTool::ReadConversation {
                    project::conversation(
                        &self.member.read("conversations.get", input).await?,
                        None,
                    )
                } else {
                    project::events(
                        &self.member.read("conversations.get_history", input).await?,
                        HISTORY_EVENTS,
                    )
                };
                Ok((value, vec![RecordRef::new(CONVERSATION, id)], 1))
            }
            ChatTool::FindPeople => self.find_people(text("query")).await,
            ChatTool::WhatNeedsMe => {
                let waiting = arguments.get("waiting").and_then(Value::as_bool) == Some(true);
                self.what_needs_me(waiting).await
            }
            ChatTool::MyInboxes => self.my_inboxes().await,
            ChatTool::MyTasks => self.my_tasks(arguments).await,
            ChatTool::ReadTask => {
                let id = text("task_id").unwrap_or_default();
                let detail = self
                    .member
                    .read("tasks.get", json!({ "task_id": id }))
                    .await?;
                Ok((
                    project::task_detail(&detail),
                    vec![RecordRef::new(TASK, id)],
                    1,
                ))
            }
        }
    }

    /// The member's open tasks, narrowed by the model's arguments.
    async fn my_tasks(
        &self,
        arguments: &Map<String, Value>,
    ) -> Result<(Value, Vec<RecordRef>, u32), Unread> {
        let mut input = json!({ "utc_offset": self.utc_offset });
        for key in ["due_from", "due_to", "overdue", "undated"] {
            if let Some(value) = arguments.get(key) {
                input[key] = value.clone();
            }
        }
        if arguments.get("handed_off").and_then(Value::as_bool) == Some(true) {
            input["whose"] = "handed_off".into();
        }
        let list = self.member.read("tasks.list", input).await?;
        let found: Vec<&Value> = list["tasks"]
            .as_array()
            .into_iter()
            .flatten()
            .take(TASKS)
            .collect();
        let records = found
            .iter()
            .filter_map(|task| task["id"].as_str())
            .map(|id| RecordRef::new(TASK, id))
            .collect();
        let tasks: Vec<Value> = found.iter().map(|task| project::task(task)).collect();
        let count = count(tasks.len());
        Ok((json!({ "tasks": tasks }), records, count))
    }

    /// Conversations by words (search, then their own facts), or the newest
    /// of a stream or place.
    async fn find_conversations(
        &self,
        query: Option<&str>,
        stream: Option<&str>,
        place: Option<&str>,
    ) -> Result<(Value, Vec<RecordRef>, u32), Unread> {
        let found = if let Some(query) = query {
            let hits = self
                .member
                .read(
                    "search.query",
                    json!({ "q": query, "limit": FOUND, "sort": "relevance" }),
                )
                .await?;
            let ids: Vec<String> = hits["hits"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|hit| hit["record"]["type"] == CONVERSATION)
                .filter_map(|hit| hit["record"]["id"].as_str().map(ToOwned::to_owned))
                .take(FOUND)
                .collect();
            self.conversations(&ids).await?
        } else {
            let mut input = json!({ "limit": FOUND });
            if let Some(stream) = stream {
                input["stream"] = stream.into();
            }
            if let Some(place) = place {
                input["view"] = place.into();
            }
            self.member
                .read("conversations.list", input)
                .await?
                .get("conversations")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        };
        Ok(listed(&found))
    }

    /// People who write to the member.
    async fn find_people(
        &self,
        query: Option<&str>,
    ) -> Result<(Value, Vec<RecordRef>, u32), Unread> {
        let mut input = json!({ "limit": FOUND });
        if let Some(query) = query {
            input["q"] = query.into();
        }
        let senders = self
            .member
            .read("conversations.list_senders", input)
            .await?;
        let people: Vec<Value> = senders["senders"]
            .as_array()
            .into_iter()
            .flatten()
            .take(FOUND)
            .map(|sender| {
                json!({
                    "name": sender["name"],
                    "address": sender["address"],
                    "received": sender["received"],
                    "replied": sender["replied"],
                })
            })
            .collect();
        let count = count(people.len());
        Ok((json!({ "people": people }), Vec::new(), count))
    }

    /// What needs the member, or what they wait on others for, with the
    /// conversations it is about.
    async fn what_needs_me(&self, waiting: bool) -> Result<(Value, Vec<RecordRef>, u32), Unread> {
        let list = self
            .member
            .read(
                "attention.list",
                json!({
                    "state": if waiting { "waiting" } else { "open" },
                    "utc_offset": self.utc_offset,
                }),
            )
            .await?;
        let items: Vec<&Value> = list["items"]
            .as_array()
            .into_iter()
            .flatten()
            .take(NEEDS)
            .collect();
        let ids: Vec<String> = items
            .iter()
            .filter(|item| item["record"]["type"] == CONVERSATION)
            .filter_map(|item| item["record"]["id"].as_str().map(ToOwned::to_owned))
            .collect();
        let conversations = self.conversations(&ids).await?;
        let needs: Vec<Value> = items
            .iter()
            .map(|item| {
                let about = conversations
                    .iter()
                    .find(|c| c["id"] == item["record"]["id"])
                    .map(project::summary);
                json!({
                    "reason": item["reason"],
                    "due_on": item["due_on"],
                    "overdue": item["overdue"],
                    "urgent": item["urgent"],
                    "since": item["since"],
                    "conversation": about,
                })
            })
            .collect();
        let records = ids
            .iter()
            .map(|id| RecordRef::new(CONVERSATION, id))
            .collect();
        Ok((
            json!({ "needs": needs, "counts": list["counts"] }),
            records,
            count(items.len()),
        ))
    }

    /// The member's inboxes and their streams.
    async fn my_inboxes(&self) -> Result<(Value, Vec<RecordRef>, u32), Unread> {
        let inboxes = self
            .member
            .read("conversations.list_inboxes", Value::Null)
            .await?;
        let streams = self
            .member
            .read("conversations.list_streams", Value::Null)
            .await?;
        let listed: Vec<Value> = inboxes["inboxes"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|inbox| {
                json!({
                    "id": inbox["id"],
                    "address": inbox["address"],
                    "name": inbox["display_name"],
                    "your_role": inbox["role"],
                    "shared": inbox["shared"],
                    "read_only": inbox["read_only"],
                    "unread": inbox["unread_count"],
                })
            })
            .collect();
        let records: Vec<RecordRef> = listed
            .iter()
            .filter_map(|inbox| inbox["id"].as_str())
            .map(|id| RecordRef::new(INBOX, id))
            .collect();
        let count = count(listed.len());
        Ok((
            json!({ "inboxes": listed, "streams": streams["streams"] }),
            records,
            count,
        ))
    }

    /// `conversations.list`'s entries for `ids`, in their order.
    async fn conversations(&self, ids: &[String]) -> Result<Vec<Value>, Unread> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let listed = self
            .member
            .read(
                "conversations.list",
                json!({ "ids": ids.join(","), "limit": ids.len() }),
            )
            .await?;
        let all = listed["conversations"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        Ok(ids
            .iter()
            .filter_map(|id| all.iter().find(|c| c["id"] == id.as_str()).cloned())
            .collect())
    }
}

/// Conversations as a found list: their projection, references, count.
fn listed(found: &[Value]) -> (Value, Vec<RecordRef>, u32) {
    let records = found
        .iter()
        .filter_map(|c| c["id"].as_str())
        .map(|id| RecordRef::new(CONVERSATION, id))
        .collect();
    let conversations: Vec<Value> = found.iter().map(project::summary).collect();
    (
        json!({ "conversations": conversations }),
        records,
        count(found.len()),
    )
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// A result saying the call was not run.
fn refusal(code: &'static str, message: &str) -> Outcome {
    Outcome {
        result: json!({ "error": code, "message": message }).to_string(),
        step: None,
        kind: code,
    }
}

/// `raw`'s arguments when they fit `tool`'s schema: a JSON object (or
/// nothing) with only its properties, each of the right type and length.
fn arguments(tool: ChatTool, raw: &str) -> Option<Map<String, Value>> {
    let raw = raw.trim();
    let object = if raw.is_empty() {
        Map::new()
    } else {
        match serde_json::from_str::<Value>(raw).ok()? {
            Value::Object(object) => object,
            Value::Null => Map::new(),
            _ => return None,
        }
    };
    let string = |value: &Value, min: usize, max: usize| {
        value
            .as_str()
            .is_some_and(|s| (min..=max).contains(&s.chars().count()))
    };
    // A day written YYYY-MM-DD.
    let day = |value: &Value| {
        value.as_str().is_some_and(|s| {
            s.len() == 10
                && s.char_indices().all(|(at, c)| {
                    if at == 4 || at == 7 {
                        c == '-'
                    } else {
                        c.is_ascii_digit()
                    }
                })
        })
    };
    let one_of =
        |value: &Value, options: &[&str]| value.as_str().is_some_and(|s| options.contains(&s));
    let fits = |key: &str, value: &Value| match (tool, key) {
        (ChatTool::FindConversations, "query") => string(value, 2, 200),
        (ChatTool::FindConversations, "stream") => {
            one_of(value, &["primary", "updates", "receipts", "newsletters"])
        }
        (ChatTool::FindConversations, "place") => one_of(
            value,
            &["inbox", "sent", "starred", "archive", "spam", "trash"],
        ),
        (ChatTool::ReadConversation | ChatTool::ConversationHistory, "conversation_id")
        | (ChatTool::ReadTask, "task_id") => string(value, 1, 64),
        (ChatTool::FindPeople, "query") => string(value, 2, 100),
        (ChatTool::MyTasks, "due_from" | "due_to") => day(value),
        (ChatTool::WhatNeedsMe, "waiting")
        | (ChatTool::MyTasks, "overdue" | "undated" | "handed_off") => value.is_boolean(),
        _ => false,
    };
    // A null optional argument is as if it were absent.
    let object: Map<String, Value> = object.into_iter().filter(|(_, v)| !v.is_null()).collect();
    if !object.iter().all(|(key, value)| fits(key, value)) {
        return None;
    }
    let needs_id = matches!(
        tool,
        ChatTool::ReadConversation | ChatTool::ConversationHistory
    );
    if needs_id && !object.contains_key("conversation_id") {
        return None;
    }
    if tool == ChatTool::ReadTask && !object.contains_key("task_id") {
        return None;
    }
    Some(object)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_has_a_strict_schema_and_its_operations() {
        for definition in read_definitions() {
            assert_eq!(definition.parameters["type"], "object");
            assert_eq!(definition.parameters["additionalProperties"], false);
            let tool = ChatTool::named(&definition.name).unwrap();
            assert!(
                OPERATIONS
                    .iter()
                    .any(|(t, ops)| *t == tool && !ops.is_empty())
            );
        }
        assert_eq!(read_definitions().len(), ChatTool::ALL.len());
        assert_eq!(
            definitions().len(),
            ChatTool::ALL.len() + Proposing::ALL.len()
        );
        assert_eq!(OPERATIONS.len(), ChatTool::ALL.len());
    }

    #[test]
    fn arguments_outside_the_schema_are_refused() {
        let find = ChatTool::FindConversations;
        assert!(arguments(find, "").is_some());
        assert!(arguments(find, "{\"query\": \"invoice\"}").is_some());
        assert!(arguments(find, "{\"query\": null, \"stream\": \"updates\"}").is_some());
        for bad in [
            "invoice",
            "[1]",
            "{\"query\": 3}",
            "{\"query\": \"x\"}",
            "{\"stream\": \"spam\"}",
            "{\"q\": \"invoice\"}",
            "{\"query\": \"invoice\", \"send\": true}",
        ] {
            assert!(arguments(find, bad).is_none(), "{bad}");
        }
        assert!(arguments(ChatTool::ReadConversation, "{}").is_none());
        assert!(arguments(ChatTool::MyInboxes, "{\"all\": true}").is_none());
    }

    #[test]
    fn the_task_tools_take_days_and_an_id_only() {
        let mine = ChatTool::MyTasks;
        assert!(arguments(mine, "").is_some());
        assert!(
            arguments(
                mine,
                "{\"due_from\": \"2026-10-19\", \"due_to\": \"2026-10-25\", \"overdue\": true}"
            )
            .is_some()
        );
        for bad in [
            "{\"due_to\": \"next friday\"}",
            "{\"due_to\": \"2026-1-25\"}",
            "{\"overdue\": \"yes\"}",
            "{\"state\": \"done\"}",
            "{\"task_id\": \"1\"}",
        ] {
            assert!(arguments(mine, bad).is_none(), "{bad}");
        }
        assert!(arguments(ChatTool::ReadTask, "{}").is_none());
        assert!(arguments(ChatTool::ReadTask, "{\"task_id\": \"0192f0c4\"}").is_some());
        assert!(arguments(ChatTool::ReadTask, "{\"task_id\": \"0192f0c4\", \"x\": 1}").is_none());
    }
}
