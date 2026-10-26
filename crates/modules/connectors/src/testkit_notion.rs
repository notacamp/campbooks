//! A fake Notion (on wiremock): the OAuth token endpoint (a code is spent
//! once; a refresh token is spent once and rotated), search, pages, a
//! page's blocks (children listed in pages, appended after a block,
//! rewritten, trashed) and the files Notion hosts (campbooks-cloud#376).
//!
//! Faults are switched on one by one: throttling (the next API call
//! answers 429 with a `Retry-After`), a refresh refused with an answer of
//! the test's choosing, access tokens that stop working, a page that is not
//! shared any more, and listings in small pages.
//!
//! Notion's own edits are made by [`FakeNotion::edit_block`] and friends:
//! they move the block's edit time, as a real edit does. Everything the
//! client does to a block is kept to read back: the calls, the patches, the
//! blocks trashed.
//!
//! No test talks to Notion.

use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::{Value, json};
use url::Url;
use uuid::Uuid;
use wiremock::{Mock, MockServer, Respond, ResponseTemplate, matchers::any};

/// The integration's client id.
pub const CLIENT_ID: &str = "notion-client-1";

/// The client secret the fake accepts.
pub const CLIENT_SECRET: &str = "test-notion-secret";

/// The authorization's bot, Notion's id of the grant.
pub const BOT_ID: &str = "bot-7f3a9c10-2b4d-4e6f-8a1b-5c7d9e0f1a2b";

/// The Notion workspace's name.
pub const WORKSPACE_NAME: &str = "Acme";

/// Where the fake's clock starts: 2026-10-25T09:00:00Z.
const BASE_TIME: i64 = 1_792_918_800;

/// A page of the fake workspace.
#[derive(Debug, Clone)]
struct FakePage {
    title: String,
    /// The page it is under, when it is a sub-page.
    parent: Option<Uuid>,
    edited: i64,
    archived: bool,
    /// Shared with the integration.
    shared: bool,
}

/// A block of the fake workspace.
#[derive(Debug, Clone)]
struct FakeBlock {
    /// The page or block it is under.
    parent: Uuid,
    kind: String,
    body: Value,
    edited: i64,
    archived: bool,
}

#[derive(Debug)]
struct State {
    next: u64,
    /// The fake's clock, in minutes after the start.
    clock: i64,
    access_tokens: Vec<String>,
    refresh_tokens: Vec<String>,
    codes: Vec<String>,
    pages: BTreeMap<Uuid, FakePage>,
    blocks: BTreeMap<Uuid, FakeBlock>,
    /// The children of each page and block, in order.
    order: HashMap<Uuid, Vec<Uuid>>,
    files: HashMap<String, (Vec<u8>, String)>,
    /// Requests received, as `METHOD path`.
    calls: Vec<String>,
    /// Blocks `DELETE` was asked for.
    deleted: Vec<Uuid>,
    /// Blocks `PATCH`ed (their body), and appends (the parent and children).
    patches: Vec<(Uuid, Value)>,
    appends: Vec<(Uuid, Value)>,
    /// The next API call answers 429 with this `Retry-After`.
    throttle_next: Option<u64>,
    /// Answer the next refreshes so.
    refresh_refusal: Option<(u16, Value)>,
    /// Listings are this big, whatever the client asks.
    page_size: Option<usize>,
    /// Where the fake is served (files point back at it).
    base: String,
}

/// Notion, faked.
#[derive(Clone)]
pub struct FakeNotion {
    server: Arc<MockServer>,
    state: Arc<Mutex<State>>,
}

struct Responder(Arc<Mutex<State>>);

impl FakeNotion {
    /// A fake with an empty workspace.
    pub async fn start() -> Self {
        let server = MockServer::start().await;
        let state = Arc::new(Mutex::new(State {
            next: 1,
            clock: 0,
            access_tokens: Vec::new(),
            refresh_tokens: Vec::new(),
            codes: Vec::new(),
            pages: BTreeMap::new(),
            blocks: BTreeMap::new(),
            order: HashMap::new(),
            files: HashMap::new(),
            calls: Vec::new(),
            deleted: Vec::new(),
            patches: Vec::new(),
            appends: Vec::new(),
            throttle_next: None,
            refresh_refusal: None,
            page_size: None,
            base: server.uri(),
        }));
        Mock::given(any())
            .respond_with(Responder(state.clone()))
            .mount(&server)
            .await;
        Self {
            server: Arc::new(server),
            state,
        }
    }

    /// Where it is served: Notion's endpoints are laid out under it.
    pub fn base(&self) -> Url {
        Url::parse(&format!("{}/", self.server.uri())).unwrap()
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// A code the browser brings back from the consent screen.
    pub fn issue_code(&self) -> String {
        let mut state = self.lock();
        let n = state.next;
        state.next += 1;
        let code = format!("fake-notion-code-{n}");
        state.codes.push(code.clone());
        code
    }

    /// A fresh grant's (access token, refresh token).
    pub fn grant(&self) -> (String, String) {
        let mut state = self.lock();
        issue(&mut state)
    }

    /// Every access token stops working (the member revoked the integration).
    pub fn revoke_access(&self) {
        self.lock().access_tokens.clear();
    }

    /// The next refreshes are answered `status` with `body`.
    pub fn refuse_refresh(&self, status: u16, body: Value) {
        self.lock().refresh_refusal = Some((status, body));
    }

    /// The next API call answers 429 with this `Retry-After`.
    pub fn throttle_next(&self, seconds: u64) {
        self.lock().throttle_next = Some(seconds);
    }

    /// Listings come in pages of `size`, whatever the client asks.
    pub fn page_size(&self, size: usize) {
        self.lock().page_size = Some(size);
    }

    /// The fake's clock moves on (a minute, as Notion rounds).
    fn tick(state: &mut State) -> i64 {
        state.clock += 1;
        state.clock
    }

    /// A page of the workspace, shared with the integration.
    pub fn add_page(&self, title: &str, parent: Option<Uuid>) -> Uuid {
        let mut state = self.lock();
        let id = Uuid::now_v7();
        let edited = Self::tick(&mut state);
        state.pages.insert(
            id,
            FakePage {
                title: title.to_owned(),
                parent,
                edited,
                archived: false,
                shared: true,
            },
        );
        state.order.insert(id, Vec::new());
        // A sub-page is a `child_page` block of its parent, with the page's id.
        if let Some(parent) = parent {
            state.blocks.insert(
                id,
                FakeBlock {
                    parent,
                    kind: "child_page".to_owned(),
                    body: json!({"title": title}),
                    edited,
                    archived: false,
                },
            );
            state.order.entry(parent).or_default().push(id);
        }
        id
    }

    /// Add a block of `kind` with `body` at the end of `parent`'s children.
    pub fn add_block(&self, parent: Uuid, kind: &str, body: &Value) -> Uuid {
        let mut state = self.lock();
        insert_block(&mut state, parent, kind, body.clone(), None)
    }

    /// Add a paragraph.
    pub fn paragraph(&self, parent: Uuid, text: &str) -> Uuid {
        self.add_block(parent, "paragraph", &json!({"rich_text": [rich(text)]}))
    }

    /// Notion edits block `id`: `change` rewrites its type body, and the
    /// block's and its page's edit times move.
    pub fn edit_block(&self, id: Uuid, change: impl FnOnce(&mut Value)) {
        let mut state = self.lock();
        let edited = Self::tick(&mut state);
        if let Some(block) = state.blocks.get_mut(&id) {
            change(&mut block.body);
            block.edited = edited;
        }
        touch_page(&mut state, id, edited);
    }

    /// Notion changes the text of block `id` to `text`.
    pub fn set_text(&self, id: Uuid, text: &str) {
        self.edit_block(id, |body| body["rich_text"] = json!([rich(text)]));
    }

    /// Block `id` goes to the trash in Notion.
    pub fn remove_block(&self, id: Uuid) {
        let mut state = self.lock();
        let edited = Self::tick(&mut state);
        if let Some(block) = state.blocks.get_mut(&id) {
            block.archived = true;
            block.edited = edited;
        }
        touch_page(&mut state, id, edited);
    }

    /// The member stops sharing page `id` with the integration.
    pub fn unshare(&self, id: Uuid) {
        if let Some(page) = self.lock().pages.get_mut(&id) {
            page.shared = false;
        }
    }

    /// Page `id` is trashed in Notion.
    pub fn trash_page(&self, id: Uuid) {
        let mut state = self.lock();
        let edited = Self::tick(&mut state);
        if let Some(page) = state.pages.get_mut(&id) {
            page.archived = true;
            page.edited = edited;
        }
    }

    /// A file Notion hosts at `name`: its address.
    pub fn add_file(&self, name: &str, bytes: &[u8], content_type: &str) -> String {
        let mut state = self.lock();
        state
            .files
            .insert(name.to_owned(), (bytes.to_vec(), content_type.to_owned()));
        format!("{}/files/{name}?sig=1", state.base)
    }

    /// Block `id` as the API shows it now.
    pub fn block(&self, id: Uuid) -> Option<Value> {
        let state = self.lock();
        state
            .blocks
            .contains_key(&id)
            .then(|| block_json(&state, id))
    }

    /// The children of `id`, in order, those in the trash left out.
    pub fn children_of(&self, id: Uuid) -> Vec<Uuid> {
        let state = self.lock();
        live_children(&state, id)
    }

    /// When Notion last edited block or page `id`.
    pub fn edited(&self, id: Uuid) -> Option<DateTime<Utc>> {
        let state = self.lock();
        state
            .blocks
            .get(&id)
            .map(|b| b.edited)
            .or_else(|| state.pages.get(&id).map(|p| p.edited))
            .map(time_of)
    }

    /// The plain text of block `id`.
    pub fn text(&self, id: Uuid) -> Option<String> {
        let state = self.lock();
        let block = state.blocks.get(&id)?;
        Some(plain(block.body.get("rich_text")))
    }

    /// The requests received, as `METHOD path`.
    pub fn calls(&self) -> Vec<String> {
        self.lock().calls.clone()
    }

    /// The blocks `DELETE` was asked for.
    pub fn deleted(&self) -> Vec<Uuid> {
        self.lock().deleted.clone()
    }

    /// The blocks `PATCH`ed, with what was sent.
    pub fn patches(&self) -> Vec<(Uuid, Value)> {
        self.lock().patches.clone()
    }

    /// The appends received: the parent and what was sent.
    pub fn appends(&self) -> Vec<(Uuid, Value)> {
        self.lock().appends.clone()
    }

    /// Forget the calls received so far.
    pub fn forget_calls(&self) {
        self.lock().calls.clear();
    }
}

/// A rich-text item of plain text.
pub fn rich(text: &str) -> Value {
    json!({
        "type": "text",
        "text": {"content": text, "link": null},
        "plain_text": text,
        "annotations": {"bold": false, "italic": false, "strikethrough": false,
                        "underline": false, "code": false, "color": "default"},
        "href": null,
    })
}

fn plain(rich_text: Option<&Value>) -> String {
    rich_text
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    item.pointer("/text/content")
                        .or_else(|| item.get("plain_text"))
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                })
                .collect::<String>()
        })
        .unwrap_or_default()
}

fn time_of(n: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(BASE_TIME + n * 60, 0).unwrap_or_default()
}

fn time(n: i64) -> String {
    time_of(n).to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn issue(state: &mut State) -> (String, String) {
    let n = state.next;
    state.next += 1;
    let access = format!("fake-notion-access-{n}");
    let refresh = format!("fake-notion-refresh-{n}");
    state.access_tokens.push(access.clone());
    state.refresh_tokens.push(refresh.clone());
    (access, refresh)
}

/// The page block or page `id` belongs to.
fn page_of(state: &State, id: Uuid) -> Option<Uuid> {
    let mut at = id;
    for _ in 0..64 {
        if state.pages.contains_key(&at) {
            return Some(at);
        }
        at = state.blocks.get(&at)?.parent;
    }
    None
}

fn touch_page(state: &mut State, id: Uuid, edited: i64) {
    if let Some(page) = page_of(state, id)
        && let Some(page) = state.pages.get_mut(&page)
    {
        page.edited = edited;
    }
}

fn insert_block(
    state: &mut State,
    parent: Uuid,
    kind: &str,
    body: Value,
    after: Option<Uuid>,
) -> Uuid {
    let id = Uuid::now_v7();
    let edited = FakeNotion::tick(state);
    state.blocks.insert(
        id,
        FakeBlock {
            parent,
            kind: kind.to_owned(),
            body,
            edited,
            archived: false,
        },
    );
    state.order.entry(id).or_default();
    let siblings = state.order.entry(parent).or_default();
    let at = after
        .and_then(|after| siblings.iter().position(|s| *s == after))
        .map_or(siblings.len(), |at| at + 1);
    siblings.insert(at, id);
    touch_page(state, parent, edited);
    id
}

fn live_children(state: &State, id: Uuid) -> Vec<Uuid> {
    state
        .order
        .get(&id)
        .map(|children| {
            children
                .iter()
                .copied()
                .filter(|child| state.blocks.get(child).is_some_and(|b| !b.archived))
                .collect()
        })
        .unwrap_or_default()
}

fn block_json(state: &State, id: Uuid) -> Value {
    let Some(block) = state.blocks.get(&id) else {
        return Value::Null;
    };
    let parent = if state.pages.contains_key(&block.parent) {
        json!({"type": "page_id", "page_id": block.parent})
    } else {
        json!({"type": "block_id", "block_id": block.parent})
    };
    let mut answer = json!({
        "object": "block",
        "id": id,
        "parent": parent,
        "created_time": time(block.edited),
        "last_edited_time": time(block.edited),
        "has_children": !live_children(state, id).is_empty() && block.kind != "child_page",
        "archived": block.archived,
        "in_trash": block.archived,
        "type": block.kind,
    });
    answer[block.kind.as_str()] = block.body.clone();
    answer
}

fn page_json(state: &State, id: Uuid) -> Value {
    let Some(page) = state.pages.get(&id) else {
        return Value::Null;
    };
    let parent = match page.parent {
        Some(parent) => json!({"type": "page_id", "page_id": parent}),
        None => json!({"type": "workspace", "workspace": true}),
    };
    json!({
        "object": "page",
        "id": id,
        "created_time": time(page.edited),
        "last_edited_time": time(page.edited),
        "archived": page.archived,
        "in_trash": page.archived,
        "parent": parent,
        "properties": {"title": {"id": "title", "type": "title",
            "title": [{"type": "text", "text": {"content": page.title},
                       "plain_text": page.title}]}},
        "url": format!("https://www.notion.so/{}", id.simple()),
    })
}

fn json_response(status: u16, value: &Value) -> ResponseTemplate {
    ResponseTemplate::new(status).set_body_json(value)
}

fn error(status: u16, code: &str) -> ResponseTemplate {
    json_response(
        status,
        &json!({"object": "error", "status": status, "code": code,
                "message": "A message the client never keeps."}),
    )
}

fn oauth_error(status: u16, code: &str) -> ResponseTemplate {
    json_response(status, &json!({"error": code}))
}

impl Respond for Responder {
    fn respond(&self, request: &wiremock::Request) -> ResponseTemplate {
        let mut state = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let path = request.url.path().to_owned();
        state.calls.push(format!("{} {path}", request.method));
        if request.method.as_str() == "GET"
            && let Some(name) = path.strip_prefix("/files/")
        {
            // Notion's file hosts are not the API: no credentials.
            return match state.files.get(name) {
                Some((bytes, content_type)) => ResponseTemplate::new(200)
                    .insert_header("content-type", content_type.as_str())
                    .set_body_bytes(bytes.clone()),
                None => ResponseTemplate::new(404),
            };
        }
        let Some(rest) = path.strip_prefix("/v1/") else {
            return ResponseTemplate::new(404);
        };
        if request.method.as_str() == "POST" && rest == "oauth/token" {
            return token(&mut state, request);
        }
        if request
            .headers
            .get("notion-version")
            .and_then(|v| v.to_str().ok())
            != Some("2022-06-28")
        {
            return error(400, "invalid_request_url");
        }
        let known = request
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .is_some_and(|token| state.access_tokens.iter().any(|t| t == token));
        if !known {
            return error(401, "unauthorized");
        }
        if let Some(seconds) = state.throttle_next.take() {
            return error(429, "rate_limited")
                .insert_header("retry-after", seconds.to_string().as_str());
        }
        let segments: Vec<&str> = rest.split('/').collect();
        let query: HashMap<String, String> = request.url.query_pairs().into_owned().collect();
        let body: Value = serde_json::from_slice(&request.body).unwrap_or(Value::Null);
        match (request.method.as_str(), segments.as_slice()) {
            ("POST", ["search"]) => search(&state, &body),
            ("GET", ["pages", id]) => match shared_page(&state, id) {
                Some(id) => json_response(200, &page_json(&state, id)),
                None => error(404, "object_not_found"),
            },
            ("GET", ["blocks", id, "children"]) => children(&state, id, &query),
            ("GET", ["blocks", id]) => match visible_block(&state, id) {
                Some(id) => json_response(200, &block_json(&state, id)),
                None => error(404, "object_not_found"),
            },
            ("PATCH", ["blocks", id, "children"]) => append(&mut state, id, &body),
            ("PATCH", ["blocks", id]) => patch(&mut state, id, &body),
            ("DELETE", ["blocks", id]) => delete(&mut state, id),
            _ => error(404, "invalid_request_url"),
        }
    }
}

/// Page `id` when it exists and is shared.
fn shared_page(state: &State, id: &str) -> Option<Uuid> {
    let id = Uuid::parse_str(id).ok()?;
    state.pages.get(&id).filter(|page| page.shared)?;
    Some(id)
}

/// Block `id` (or a page, which is a block too) when its page is shared.
fn visible_block(state: &State, id: &str) -> Option<Uuid> {
    let id = Uuid::parse_str(id).ok()?;
    if state.pages.contains_key(&id) {
        return shared_page(state, &id.to_string());
    }
    state.blocks.get(&id)?;
    let page = page_of(state, id)?;
    state.pages.get(&page).filter(|page| page.shared)?;
    Some(id)
}

fn token(state: &mut State, request: &wiremock::Request) -> ResponseTemplate {
    let credentials = request
        .headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Basic "))
        .and_then(|v| STANDARD.decode(v).ok())
        .and_then(|v| String::from_utf8(v).ok());
    if credentials.as_deref() != Some(format!("{CLIENT_ID}:{CLIENT_SECRET}").as_str()) {
        return oauth_error(401, "unauthorized_client");
    }
    let body: Value = serde_json::from_slice(&request.body).unwrap_or(Value::Null);
    match body["grant_type"].as_str() {
        Some("authorization_code") => {
            let code = body["code"].as_str().unwrap_or_default().to_owned();
            let Some(at) = state.codes.iter().position(|c| *c == code) else {
                return oauth_error(400, "invalid_grant");
            };
            state.codes.remove(at);
            let (access, refresh) = issue(state);
            json_response(
                200,
                &json!({
                    "access_token": access,
                    "refresh_token": refresh,
                    "token_type": "bearer",
                    "bot_id": BOT_ID,
                    "workspace_id": "5c7d9e0f-1a2b-4c3d-8e4f-6a7b8c9d0e1f",
                    "workspace_name": WORKSPACE_NAME,
                    "owner": {"type": "user", "user": {"object": "user", "id": "u-1"}},
                }),
            )
        }
        Some("refresh_token") => {
            if let Some((status, answer)) = state.refresh_refusal.clone() {
                return json_response(status, &answer);
            }
            let spent = body["refresh_token"]
                .as_str()
                .unwrap_or_default()
                .to_owned();
            let Some(at) = state.refresh_tokens.iter().position(|t| *t == spent) else {
                return oauth_error(400, "invalid_grant");
            };
            state.refresh_tokens.remove(at);
            let (access, refresh) = issue(state);
            json_response(
                200,
                &json!({
                    "access_token": access,
                    "refresh_token": refresh,
                    "token_type": "bearer",
                    "bot_id": BOT_ID,
                    "workspace_name": WORKSPACE_NAME,
                }),
            )
        }
        _ => oauth_error(400, "unsupported_grant_type"),
    }
}

/// Start index of a listing from its cursor.
fn start_of(cursor: Option<&str>) -> usize {
    cursor.and_then(|c| c.parse().ok()).unwrap_or(0)
}

fn listing(state: &State, results: Vec<Value>, start: usize, size: usize) -> Value {
    let size = state.page_size.unwrap_or(size).max(1);
    let end = (start + size).min(results.len());
    let more = end < results.len();
    json!({
        "object": "list",
        "results": results.get(start..end).unwrap_or_default(),
        "has_more": more,
        "next_cursor": more.then(|| end.to_string()),
    })
}

fn search(state: &State, body: &Value) -> ResponseTemplate {
    let mut pages: Vec<(&Uuid, &FakePage)> = state
        .pages
        .iter()
        .filter(|(_, page)| page.shared && !page.archived)
        .collect();
    pages.sort_by(|a, b| b.1.edited.cmp(&a.1.edited).then_with(|| a.0.cmp(b.0)));
    let results: Vec<Value> = pages.iter().map(|(id, _)| page_json(state, **id)).collect();
    let size = body["page_size"]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or(100);
    json_response(
        200,
        &listing(
            state,
            results,
            start_of(body["start_cursor"].as_str()),
            size,
        ),
    )
}

fn children(state: &State, id: &str, query: &HashMap<String, String>) -> ResponseTemplate {
    let Some(id) = visible_block(state, id) else {
        return error(404, "object_not_found");
    };
    let results: Vec<Value> = state
        .order
        .get(&id)
        .map(|ids| {
            ids.iter()
                .filter(|child| state.blocks.get(child).is_some_and(|b| !b.archived))
                .map(|child| block_json(state, *child))
                .collect()
        })
        .unwrap_or_default();
    let size = query
        .get("page_size")
        .and_then(|n| n.parse().ok())
        .unwrap_or(100);
    json_response(
        200,
        &listing(
            state,
            results,
            start_of(query.get("start_cursor").map(String::as_str)),
            size,
        ),
    )
}

fn append(state: &mut State, id: &str, body: &Value) -> ResponseTemplate {
    let Some(parent) = visible_block(state, id) else {
        return error(404, "object_not_found");
    };
    let mut after = body["after"].as_str().and_then(|a| Uuid::parse_str(a).ok());
    let Some(children) = body["children"].as_array() else {
        return error(400, "validation_error");
    };
    state.appends.push((parent, body.clone()));
    let mut made = Vec::new();
    for child in children {
        let Some(kind) = child["type"].as_str() else {
            return error(400, "validation_error");
        };
        let made_id = insert_block(state, parent, kind, child[kind].clone(), after);
        after = Some(made_id);
        made.push(made_id);
    }
    let results: Vec<Value> = made.iter().map(|id| block_json(state, *id)).collect();
    json_response(
        200,
        &json!({"object": "list", "results": results, "has_more": false}),
    )
}

fn patch(state: &mut State, id: &str, body: &Value) -> ResponseTemplate {
    let Some(id) = visible_block(state, id) else {
        return error(404, "object_not_found");
    };
    let Some(block) = state.blocks.get(&id) else {
        return error(404, "object_not_found");
    };
    let kind = block.kind.clone();
    let Some(change) = body.get(kind.as_str()).and_then(Value::as_object) else {
        return error(400, "validation_error");
    };
    state.patches.push((id, body.clone()));
    let edited = FakeNotion::tick(state);
    if let Some(block) = state.blocks.get_mut(&id) {
        for (key, value) in change {
            block.body[key.as_str()] = value.clone();
        }
        block.edited = edited;
    }
    touch_page(state, id, edited);
    json_response(200, &block_json(state, id))
}

fn delete(state: &mut State, id: &str) -> ResponseTemplate {
    let Some(id) = visible_block(state, id) else {
        return error(404, "object_not_found");
    };
    state.deleted.push(id);
    let edited = FakeNotion::tick(state);
    if let Some(block) = state.blocks.get_mut(&id) {
        block.archived = true;
        block.edited = edited;
    }
    touch_page(state, id, edited);
    json_response(200, &block_json(state, id))
}
