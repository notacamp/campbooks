//! The fake Notion's databases (data sources, API version 2026-03-11):
//! their properties, their entries, the query by edit time, the schema
//! patch (add, rename, change, remove; options matched by id or name), the
//! entries made, set and trashed, and a page's long property lists.
//!
//! Notion's own edits are made by [`FakeNotion::edit_entry`] and friends:
//! they move the entry's edit time, as a real edit does. Everything the
//! client does is kept to read back.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde_json::{Map, Value, json};
use uuid::Uuid;
use wiremock::ResponseTemplate;

use super::{FakeNotion, State, error, json_response, listing, start_of, time, time_of};

/// A database of the fake workspace.
#[derive(Debug, Clone)]
pub(super) struct FakeDs {
    title: String,
    shared: bool,
    trashed: bool,
    edited: i64,
    /// By property id.
    props: Vec<FakeProp>,
}

/// A property of a database.
#[derive(Debug, Clone)]
struct FakeProp {
    id: String,
    name: String,
    kind: String,
    /// The type's settings (`options`, `format`, …).
    config: Value,
}

/// An entry (a row).
#[derive(Debug, Clone)]
pub(super) struct FakeEntry {
    ds: Uuid,
    edited: i64,
    trashed: bool,
    /// The type body of each property's value, by property id.
    values: BTreeMap<String, Value>,
}

/// What the fake received.
#[derive(Debug, Default)]
pub(super) struct Received {
    /// Schema patches: the data source and the `properties` sent.
    pub patches: Vec<(Uuid, Value)>,
    /// Entries made: the data source and the `properties` sent.
    pub created: Vec<(Uuid, Value)>,
    /// Entries set: the page and the `properties` sent.
    pub sets: Vec<(Uuid, Value)>,
    /// Entries trashed.
    pub trashed: Vec<Uuid>,
    /// `POST …/query` bodies, with their query strings.
    pub queries: Vec<(Uuid, Value, String)>,
}

/// The databases of the fake.
#[derive(Debug, Default)]
pub(super) struct Tables {
    pub sources: BTreeMap<Uuid, FakeDs>,
    pub entries: BTreeMap<Uuid, FakeEntry>,
    pub received: Received,
    next_prop: u32,
}

fn option(name: &str, color: &str) -> Value {
    json!({"id": Uuid::now_v7().to_string(), "name": name, "color": color})
}

/// The value body of a property of `kind` holding `text` (or the choice
/// `text`, the number `text`, `true`): a convenience for tests.
pub fn value(kind: &str, text: &str) -> Value {
    match kind {
        "title" | "rich_text" => json!([{
            "type": "text", "text": {"content": text, "link": null}, "plain_text": text,
            "annotations": {"bold": false, "italic": false, "strikethrough": false,
                            "underline": false, "code": false, "color": "default"},
            "href": null,
        }]),
        "number" => text.parse::<f64>().map_or(Value::Null, |n| json!(n)),
        "select" | "status" => json!({"id": format!("o-{text}"), "name": text, "color": "default"}),
        "multi_select" => Value::Array(
            text.split(',')
                .filter(|name| !name.is_empty())
                .map(|name| json!({"id": format!("o-{name}"), "name": name, "color": "default"}))
                .collect(),
        ),
        "date" => json!({"start": text, "end": null, "time_zone": null}),
        "checkbox" => json!(text == "true"),
        "url" | "email" | "phone_number" => json!(text),
        "people" => json!([{"object": "user", "id": "u-1", "name": text}]),
        "formula" => json!({"type": "string", "string": text}),
        _ => Value::Null,
    }
}

fn settings_of(kind: &str, requested: &Value) -> Value {
    match kind {
        "select" | "multi_select" | "status" => {
            let options: Vec<Value> = requested
                .get("options")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .map(|o| {
                            let name = o.get("name").and_then(Value::as_str).unwrap_or_default();
                            let color = o.get("color").and_then(Value::as_str).unwrap_or("default");
                            json!({"id": o.get("id").cloned()
                                .unwrap_or_else(|| json!(Uuid::now_v7().to_string())),
                                "name": name, "color": color})
                        })
                        .collect()
                })
                .unwrap_or_default();
            json!({"options": options})
        }
        "number" => json!({"format": requested.get("format").cloned().unwrap_or(json!("number"))}),
        _ => json!({}),
    }
}

impl FakeNotion {
    /// A database of the workspace, shared with the integration, with its
    /// title property "Name" (id `title`).
    pub fn add_data_source(&self, title: &str) -> Uuid {
        let mut state = self.lock();
        let id = Uuid::now_v7();
        let edited = Self::tick(&mut state);
        state.tables.sources.insert(
            id,
            FakeDs {
                title: title.to_owned(),
                shared: true,
                trashed: false,
                edited,
                props: vec![FakeProp {
                    id: "title".into(),
                    name: "Name".into(),
                    kind: "title".into(),
                    config: json!({}),
                }],
            },
        );
        id
    }

    /// Notion adds a property to database `ds`: its id.
    pub fn add_property(&self, ds: Uuid, name: &str, kind: &str, config: &Value) -> String {
        let mut state = self.lock();
        let edited = Self::tick(&mut state);
        state.tables.next_prop += 1;
        let id = format!("p{}", state.tables.next_prop);
        if let Some(source) = state.tables.sources.get_mut(&ds) {
            source.edited = edited;
            source.props.push(FakeProp {
                id: id.clone(),
                name: name.to_owned(),
                kind: kind.to_owned(),
                config: settings_of(kind, config),
            });
        }
        id
    }

    /// Notion renames property `prop` of `ds`.
    pub fn rename_property(&self, ds: Uuid, prop: &str, name: &str) {
        let mut state = self.lock();
        let edited = Self::tick(&mut state);
        if let Some(source) = state.tables.sources.get_mut(&ds) {
            source.edited = edited;
            if let Some(p) = source.props.iter_mut().find(|p| p.id == prop) {
                p.name = name.to_owned();
            }
        }
    }

    /// Notion removes property `prop` of `ds` (and its values).
    pub fn remove_property(&self, ds: Uuid, prop: &str) {
        let mut state = self.lock();
        let edited = Self::tick(&mut state);
        remove_prop(&mut state, ds, prop, edited);
    }

    /// Notion adds an entry to `ds` with its title and other values (a
    /// property id and the value body, see [`value`]).
    pub fn add_entry(&self, ds: Uuid, title: &str, values: &[(&str, Value)]) -> Uuid {
        let mut state = self.lock();
        let edited = Self::tick(&mut state);
        let id = Uuid::now_v7();
        let mut map = BTreeMap::new();
        map.insert("title".to_owned(), value("title", title));
        for (prop, body) in values {
            map.insert((*prop).to_owned(), body.clone());
        }
        state.tables.entries.insert(
            id,
            FakeEntry {
                ds,
                edited,
                trashed: false,
                values: map,
            },
        );
        id
    }

    /// Notion sets one value of entry `id`.
    pub fn edit_entry(&self, id: Uuid, prop: &str, body: Value) {
        let mut state = self.lock();
        let edited = Self::tick(&mut state);
        if let Some(entry) = state.tables.entries.get_mut(&id) {
            entry.values.insert(prop.to_owned(), body);
            entry.edited = edited;
        }
    }

    /// Entry `id` goes to the trash in Notion.
    pub fn trash_data_entry(&self, id: Uuid) {
        let mut state = self.lock();
        let edited = Self::tick(&mut state);
        if let Some(entry) = state.tables.entries.get_mut(&id) {
            entry.trashed = true;
            entry.edited = edited;
        }
    }

    /// The member stops sharing database `ds` with the integration.
    pub fn unshare_data_source(&self, ds: Uuid) {
        if let Some(source) = self.lock().tables.sources.get_mut(&ds) {
            source.shared = false;
        }
    }

    /// Database `ds` is trashed in Notion.
    pub fn trash_data_source(&self, ds: Uuid) {
        if let Some(source) = self.lock().tables.sources.get_mut(&ds) {
            source.trashed = true;
        }
    }

    /// When Notion last edited entry `id`.
    pub fn entry_edited(&self, id: Uuid) -> Option<DateTime<Utc>> {
        self.lock()
            .tables
            .entries
            .get(&id)
            .map(|e| time_of(e.edited))
    }

    /// Entry `id`'s value of property `prop`.
    pub fn entry_value(&self, id: Uuid, prop: &str) -> Option<Value> {
        self.lock()
            .tables
            .entries
            .get(&id)
            .and_then(|e| e.values.get(prop).cloned())
    }

    /// Whether entry `id` is in the trash.
    pub fn entry_trashed(&self, id: Uuid) -> bool {
        self.lock()
            .tables
            .entries
            .get(&id)
            .is_some_and(|e| e.trashed)
    }

    /// The live entries of `ds`.
    pub fn entries_of(&self, ds: Uuid) -> Vec<Uuid> {
        self.lock()
            .tables
            .entries
            .iter()
            .filter(|(_, e)| e.ds == ds && !e.trashed)
            .map(|(id, _)| *id)
            .collect()
    }

    /// The names of the properties of `ds`, as Notion has them now.
    pub fn property_names(&self, ds: Uuid) -> Vec<String> {
        self.lock()
            .tables
            .sources
            .get(&ds)
            .map(|s| s.props.iter().map(|p| p.name.clone()).collect())
            .unwrap_or_default()
    }

    /// The id of the property of `ds` named `name`.
    pub fn property_id(&self, ds: Uuid, name: &str) -> Option<String> {
        self.lock().tables.sources.get(&ds).and_then(|s| {
            s.props
                .iter()
                .find(|p| p.name == name)
                .map(|p| p.id.clone())
        })
    }

    /// The schema patches received: the database and the properties sent.
    pub fn schema_patches(&self) -> Vec<(Uuid, Value)> {
        self.lock().tables.received.patches.clone()
    }

    /// The entries made: the database and the properties sent.
    pub fn entries_created(&self) -> Vec<(Uuid, Value)> {
        self.lock().tables.received.created.clone()
    }

    /// The entries set: the page and the properties sent.
    pub fn entry_sets(&self) -> Vec<(Uuid, Value)> {
        self.lock().tables.received.sets.clone()
    }

    /// The entries the client trashed.
    pub fn entries_trashed_by_client(&self) -> Vec<Uuid> {
        self.lock().tables.received.trashed.clone()
    }

    /// The query bodies received, with their query strings.
    pub fn queries(&self) -> Vec<(Uuid, Value, String)> {
        self.lock().tables.received.queries.clone()
    }

    /// Whether Notion still has database `ds` (not in the trash).
    pub fn data_source_live(&self, ds: Uuid) -> bool {
        self.lock()
            .tables
            .sources
            .get(&ds)
            .is_some_and(|s| !s.trashed)
    }
}

fn remove_prop(state: &mut State, ds: Uuid, prop: &str, edited: i64) {
    if let Some(source) = state.tables.sources.get_mut(&ds) {
        source.edited = edited;
        source.props.retain(|p| p.id != prop);
    }
    for entry in state.tables.entries.values_mut() {
        if entry.ds == ds && entry.values.remove(prop).is_some() {
            entry.edited = edited;
        }
    }
}

fn prop_json(p: &FakeProp) -> Value {
    let mut out = json!({"id": p.id, "name": p.name, "type": p.kind, "description": null});
    out[p.kind.as_str()] = p.config.clone();
    out
}

fn ds_json(id: Uuid, source: &FakeDs) -> Value {
    let mut props = Map::new();
    for p in &source.props {
        props.insert(p.name.clone(), prop_json(p));
    }
    json!({
        "object": "data_source",
        "id": id,
        "created_time": time(source.edited),
        "last_edited_time": time(source.edited),
        "in_trash": source.trashed,
        "title": [{"type": "text", "text": {"content": source.title}, "plain_text": source.title}],
        "properties": props,
        "parent": {"type": "database_id", "database_id": Uuid::nil()},
    })
}

fn entry_json(state: &State, id: Uuid, only: Option<&[String]>) -> Option<Value> {
    let entry = state.tables.entries.get(&id)?;
    let source = state.tables.sources.get(&entry.ds)?;
    let mut props = Map::new();
    for p in &source.props {
        if only.is_some_and(|only| !only.contains(&p.id) && !only.contains(&p.name)) {
            continue;
        }
        let body = entry
            .values
            .get(&p.id)
            .cloned()
            .unwrap_or_else(|| match p.kind.as_str() {
                "checkbox" => json!(false),
                "title" | "rich_text" | "multi_select" | "people" => json!([]),
                _ => Value::Null,
            });
        let mut v = json!({"id": p.id, "type": p.kind});
        v[p.kind.as_str()] = body;
        props.insert(p.name.clone(), v);
    }
    Some(json!({
        "object": "page",
        "id": id,
        "created_time": time(entry.edited),
        "last_edited_time": time(entry.edited),
        "in_trash": entry.trashed,
        "parent": {"type": "data_source_id", "data_source_id": entry.ds,
                   "database_id": Uuid::nil()},
        "properties": props,
        "url": format!("https://www.notion.so/{}", id.simple()),
    }))
}

fn shared_ds(state: &State, id: &str) -> Option<Uuid> {
    let id = Uuid::parse_str(id).ok()?;
    state.tables.sources.get(&id).filter(|s| s.shared)?;
    Some(id)
}

/// The entry `id` when its database is shared.
pub(super) fn visible_entry(state: &State, id: &str) -> Option<Uuid> {
    let id = Uuid::parse_str(id).ok()?;
    let entry = state.tables.entries.get(&id)?;
    state.tables.sources.get(&entry.ds).filter(|s| s.shared)?;
    Some(id)
}

/// Entry `id` as a page object, when it is one.
pub(super) fn page_of_entry(state: &State, id: &str) -> Option<Value> {
    entry_json(state, visible_entry(state, id)?, None)
}

/// Answer a request for a database or an entry, or `None` when it is none.
pub(super) fn respond(
    state: &mut State,
    method: &str,
    segments: &[&str],
    query: &std::collections::HashMap<String, String>,
    raw_query: &str,
    body: &Value,
) -> Option<ResponseTemplate> {
    match (method, segments) {
        ("GET", ["data_sources", id]) => Some(match shared_ds(state, id) {
            Some(id) => json_response(200, &ds_json(id, &state.tables.sources[&id])),
            None => error(404, "object_not_found"),
        }),
        ("POST", ["data_sources", id, "query"]) => {
            Some(query_entries(state, id, query, raw_query, body))
        }
        ("PATCH", ["data_sources", id]) => Some(patch_ds(state, id, body)),
        ("POST", ["pages"])
            if body.pointer("/parent/type").and_then(Value::as_str) == Some("data_source_id") =>
        {
            Some(create_entry(state, body))
        }
        ("PATCH", ["pages", id]) if visible_entry(state, id).is_some() => {
            Some(patch_entry(state, id, body))
        }
        ("GET", ["pages", id, "properties", prop]) if visible_entry(state, id).is_some() => {
            Some(property_item(state, id, prop))
        }
        _ => None,
    }
}

fn query_entries(
    state: &mut State,
    id: &str,
    query: &std::collections::HashMap<String, String>,
    raw_query: &str,
    body: &Value,
) -> ResponseTemplate {
    let Some(ds) = shared_ds(state, id) else {
        return error(404, "object_not_found");
    };
    state
        .tables
        .received
        .queries
        .push((ds, body.clone(), raw_query.to_owned()));
    let since = body
        .pointer("/filter/last_edited_time/on_or_after")
        .and_then(Value::as_str)
        .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
        .map(|t| t.with_timezone(&Utc));
    let only: Option<Vec<String>> = {
        let list: Vec<String> = url::form_urlencoded::parse(raw_query.as_bytes())
            .filter(|(k, _)| k == "filter_properties")
            .map(|(_, v)| v.into_owned())
            .collect();
        (!list.is_empty()).then_some(list)
    };
    let mut ids: Vec<(i64, Uuid)> = state
        .tables
        .entries
        .iter()
        .filter(|(_, e)| e.ds == ds && !e.trashed)
        .filter(|(_, e)| since.is_none_or(|since| time_of(e.edited) >= since))
        .map(|(id, e)| (e.edited, *id))
        .collect();
    ids.sort();
    let results: Vec<Value> = ids
        .iter()
        .filter_map(|(_, id)| entry_json(state, *id, only.as_deref()))
        .collect();
    let size = body["page_size"]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or(100);
    let _ = query;
    json_response(
        200,
        &listing(
            state,
            &results,
            start_of(body["start_cursor"].as_str()),
            size,
        ),
    )
}

#[allow(
    clippy::too_many_lines,
    clippy::single_match_else,
    reason = "one pass over the properties sent, each a small state change"
)]
fn patch_ds(state: &mut State, id: &str, body: &Value) -> ResponseTemplate {
    let Some(ds) = shared_ds(state, id) else {
        return error(404, "object_not_found");
    };
    // Only the properties are changed through this endpoint; a data source
    // is never trashed by it.
    if body.get("in_trash").is_some() || body.get("archived").is_some() {
        return error(400, "validation_error");
    }
    let Some(changes) = body.get("properties").and_then(Value::as_object) else {
        return error(400, "validation_error");
    };
    state
        .tables
        .received
        .patches
        .push((ds, body["properties"].clone()));
    let edited = FakeNotion::tick(state);
    for (key, change) in changes {
        let found = state.tables.sources[&ds]
            .props
            .iter()
            .position(|p| p.id == *key || p.name.eq_ignore_ascii_case(key));
        match (found, change) {
            (Some(at), Value::Null) => {
                if state.tables.sources[&ds].props[at].kind == "title" {
                    return error(400, "validation_error");
                }
                let prop = state.tables.sources[&ds].props[at].id.clone();
                remove_prop(state, ds, &prop, edited);
            }
            (None, Value::Null) => {}
            (found, Value::Object(change)) => {
                let kind = change
                    .keys()
                    .find(|k| !["name", "description"].contains(&k.as_str()))
                    .cloned();
                let Some(source) = state.tables.sources.get_mut(&ds) else {
                    return error(404, "object_not_found");
                };
                source.edited = edited;
                match found {
                    Some(at) => {
                        let prop = &mut source.props[at];
                        if let Some(name) = change.get("name").and_then(Value::as_str) {
                            prop.name = name.to_owned();
                        }
                        if let Some(kind) = kind {
                            if kind == prop.kind
                                && matches!(kind.as_str(), "select" | "multi_select" | "status")
                            {
                                // The full list to keep: options matched by
                                // id or name; new names are added.
                                let wanted = change[kind.as_str()]["options"]
                                    .as_array()
                                    .cloned()
                                    .unwrap_or_default();
                                let old = prop.config["options"]
                                    .as_array()
                                    .cloned()
                                    .unwrap_or_default();
                                let mut next = Vec::new();
                                for w in wanted {
                                    let by_id = w.get("id").and_then(Value::as_str).and_then(|i| {
                                        old.iter().find(|o| o["id"].as_str() == Some(i)).cloned()
                                    });
                                    let by_name =
                                        w.get("name").and_then(Value::as_str).and_then(|n| {
                                            old.iter()
                                                .find(|o| {
                                                    o["name"]
                                                        .as_str()
                                                        .is_some_and(|x| x.eq_ignore_ascii_case(n))
                                                })
                                                .cloned()
                                        });
                                    next.push(by_id.or(by_name).unwrap_or_else(|| {
                                        option(
                                            w.get("name").and_then(Value::as_str).unwrap_or("?"),
                                            w.get("color")
                                                .and_then(Value::as_str)
                                                .unwrap_or("default"),
                                        )
                                    }));
                                }
                                prop.config = json!({"options": next});
                            } else {
                                prop.kind = kind.clone();
                                prop.config = settings_of(&kind, &change[kind.as_str()]);
                            }
                        }
                    }
                    None => {
                        let Some(kind) = kind else {
                            return error(400, "validation_error");
                        };
                        if source
                            .props
                            .iter()
                            .any(|p| p.name.eq_ignore_ascii_case(key))
                        {
                            return error(400, "validation_error");
                        }
                        state.tables.next_prop += 1;
                        let pid = format!("p{}", state.tables.next_prop);
                        let settings = settings_of(&kind, &change[kind.as_str()]);
                        if let Some(source) = state.tables.sources.get_mut(&ds) {
                            source.props.push(FakeProp {
                                id: pid,
                                name: key.clone(),
                                kind,
                                config: settings,
                            });
                        }
                    }
                }
            }
            _ => return error(400, "validation_error"),
        }
    }
    json_response(200, &ds_json(ds, &state.tables.sources[&ds]))
}

/// Notion keeps a value body the way the API would show it back.
fn stored_value(kind: &str, sent: &Value, config: &Value) -> Value {
    match kind {
        "title" | "rich_text" => Value::Array(
            sent.as_array()
                .into_iter()
                .flatten()
                .map(|item| {
                    let content = item
                        .pointer("/text/content")
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    let mut out = value("rich_text", content)[0].clone();
                    out["annotations"] = item
                        .get("annotations")
                        .cloned()
                        .unwrap_or(out["annotations"].clone());
                    out["href"] = item
                        .pointer("/text/link/url")
                        .cloned()
                        .unwrap_or(Value::Null);
                    out
                })
                .collect(),
        ),
        "select" | "status" => match sent.get("name").and_then(Value::as_str) {
            None => Value::Null,
            Some(name) => option_of(config, name),
        },
        "multi_select" => Value::Array(
            sent.as_array()
                .into_iter()
                .flatten()
                .filter_map(|o| o.get("name").and_then(Value::as_str))
                .map(|name| option_of(config, name))
                .collect(),
        ),
        _ => sent.clone(),
    }
}

fn option_of(config: &Value, name: &str) -> Value {
    config["options"]
        .as_array()
        .and_then(|options| {
            options
                .iter()
                .find(|o| o["name"].as_str().is_some_and(|n| n.eq_ignore_ascii_case(name)))
                .cloned()
        })
        // Notion makes the option on the way, for a select.
        .unwrap_or_else(|| option(name, "default"))
}

fn apply_properties(state: &mut State, ds: Uuid, entry: Uuid, properties: &Value) -> bool {
    let Some(map) = properties.as_object() else {
        return false;
    };
    let Some(source) = state.tables.sources.get(&ds).cloned() else {
        return false;
    };
    let mut changes: Vec<(String, Value)> = Vec::new();
    for (key, sent) in map {
        let Some(prop) = source.props.iter().find(|p| p.id == *key || p.name == *key) else {
            return false;
        };
        let body = sent.get(prop.kind.as_str()).cloned().unwrap_or(Value::Null);
        if sent.get(prop.kind.as_str()).is_none() {
            return false;
        }
        changes.push((
            prop.id.clone(),
            stored_value(&prop.kind, &body, &prop.config),
        ));
        // A new option on a select is Notion's to remember.
        if matches!(prop.kind.as_str(), "select" | "multi_select") {
            let names: Vec<String> = match &body {
                Value::Array(items) => items
                    .iter()
                    .filter_map(|o| o["name"].as_str().map(str::to_owned))
                    .collect(),
                Value::Object(_) => body["name"]
                    .as_str()
                    .map(str::to_owned)
                    .into_iter()
                    .collect(),
                _ => Vec::new(),
            };
            for name in names {
                if let Some(source) = state.tables.sources.get_mut(&ds)
                    && let Some(p) = source.props.iter_mut().find(|p| p.id == prop.id)
                    && option_of(&p.config, &name)["id"].as_str().is_some_and(|i| {
                        !p.config["options"]
                            .as_array()
                            .is_some_and(|o| o.iter().any(|x| x["id"].as_str() == Some(i)))
                    })
                {
                    let made = option(&name, "default");
                    if let Some(options) = p.config["options"].as_array_mut() {
                        options.push(made);
                    }
                }
            }
        }
    }
    let edited = FakeNotion::tick(state);
    // Re-resolve the stored options now that new ones exist.
    if let Some(e) = state.tables.entries.get_mut(&entry) {
        for (id, body) in changes {
            e.values.insert(id, body);
        }
        e.edited = edited;
    }
    true
}

fn create_entry(state: &mut State, body: &Value) -> ResponseTemplate {
    let Some(ds) = body
        .pointer("/parent/data_source_id")
        .and_then(Value::as_str)
        .and_then(|id| shared_ds(state, id))
    else {
        return error(404, "object_not_found");
    };
    state
        .tables
        .received
        .created
        .push((ds, body["properties"].clone()));
    let id = Uuid::now_v7();
    let edited = FakeNotion::tick(state);
    state.tables.entries.insert(
        id,
        FakeEntry {
            ds,
            edited,
            trashed: false,
            values: BTreeMap::new(),
        },
    );
    if !apply_properties(state, ds, id, &body["properties"]) {
        state.tables.entries.remove(&id);
        return error(400, "validation_error");
    }
    json_response(200, &entry_json(state, id, None).unwrap_or(Value::Null))
}

fn patch_entry(state: &mut State, id: &str, body: &Value) -> ResponseTemplate {
    let Some(entry) = visible_entry(state, id) else {
        return error(404, "object_not_found");
    };
    let ds = state.tables.entries[&entry].ds;
    if body.get("in_trash").and_then(Value::as_bool) == Some(true) {
        state.tables.received.trashed.push(entry);
        let edited = FakeNotion::tick(state);
        if let Some(e) = state.tables.entries.get_mut(&entry) {
            e.trashed = true;
            e.edited = edited;
        }
    }
    if let Some(properties) = body.get("properties") {
        state.tables.received.sets.push((entry, properties.clone()));
        if !apply_properties(state, ds, entry, properties) {
            return error(400, "validation_error");
        }
    }
    json_response(200, &entry_json(state, entry, None).unwrap_or(Value::Null))
}

fn property_item(state: &State, id: &str, prop: &str) -> ResponseTemplate {
    let Some(entry) = visible_entry(state, id) else {
        return error(404, "object_not_found");
    };
    let page = entry_json(state, entry, None).unwrap_or(Value::Null);
    let found = page["properties"]
        .as_object()
        .and_then(|m| m.values().find(|v| v["id"].as_str() == Some(prop)).cloned());
    let Some(found) = found else {
        return error(404, "object_not_found");
    };
    let kind = found["type"].as_str().unwrap_or_default().to_owned();
    let items: Vec<Value> = match found[kind.as_str()].as_array() {
        Some(list) => list
            .iter()
            .map(|item| json!({"object": "property_item", "type": kind, kind.as_str(): item}))
            .collect(),
        None => vec![
            json!({"object": "property_item", "type": kind, kind.as_str(): found[kind.as_str()]}),
        ],
    };
    json_response(
        200,
        &json!({"object": "list", "results": items, "has_more": false, "next_cursor": null,
                "type": "property_item"}),
    )
}
