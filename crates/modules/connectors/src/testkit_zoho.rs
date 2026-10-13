//! A fake Zoho (one data centre, on wiremock): the accounts server's token
//! and revocation endpoints and the Mail API the adapter calls, over
//! mailboxes of fictional mail. Like Zoho, listings hide archived messages,
//! metadata is HTML-escaped, ids are 64-bit numbers, and a send sets its own
//! `Message-ID` (a reply's `In-Reply-To` names the message replied to).
//!
//! No test talks to Zoho.

use std::{
    collections::{BTreeMap, HashMap},
    fmt::Write as _,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use campbooks_kernel::clock::{Clock as _, FixedClock};
use chrono::{DateTime, Utc};
use url::Url;
use wiremock::{Mock, MockServer, Respond, ResponseTemplate, matchers::any};

/// The fictional Zoho login's address.
pub const LOGIN: &str = "sam@example.net";

/// A message in a fake mailbox.
#[derive(Debug, Clone)]
pub struct ZohoMessage {
    pub id: String,
    pub folder: String,
    pub read: bool,
    pub flag: String,
    pub archived: bool,
    pub received: DateTime<Utc>,
    pub raw: Vec<u8>,
}

/// One mail account of the login.
#[derive(Debug, Clone)]
struct Account {
    id: String,
    address: String,
    messages: BTreeMap<String, ZohoMessage>,
}

/// What goes wrong with the next send.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SendFault {
    /// Answer this status and send nothing.
    Refuse(u16),
    /// Send it, but answer 503, as when the answer is lost.
    LoseAnswer,
}

#[derive(Debug)]
struct State {
    /// The time sends are stamped with (the module's clock).
    clock: Arc<FixedClock>,
    accounts: Vec<Account>,
    next_id: u64,
    access_tokens: Vec<String>,
    refresh_tokens: Vec<String>,
    refresh_invalid: bool,
    revoked: Vec<String>,
    calls: Vec<String>,
    rate_limit_next: Option<u64>,
    send_fault: Option<SendFault>,
    /// Bodies of the sends Zoho received (JSON), in order.
    sends: Vec<(Option<String>, serde_json::Value)>,
    refusing: Vec<String>,
}

/// Folder ids of every fake account: Inbox, Drafts, Sent, Spam, Trash,
/// and the member's own "Clients".
pub const INBOX: &str = "9000000002014";
pub const DRAFTS: &str = "9000000002016";
pub const SENT: &str = "9000000002022";
pub const SPAM: &str = "9000000002024";
pub const TRASH: &str = "9000000002026";
pub const CLIENTS: &str = "9000000075001";

/// Zoho's OAuth and Mail endpoints, faked over mutable mailboxes.
#[derive(Clone)]
pub struct FakeZoho {
    server: Arc<MockServer>,
    state: Arc<Mutex<State>>,
}

struct Responder(Arc<Mutex<State>>);

impl FakeZoho {
    pub async fn start(clock: Arc<FixedClock>) -> Self {
        let server = MockServer::start().await;
        let state = Arc::new(Mutex::new(State {
            clock,
            accounts: vec![Account {
                id: "2560636000000008002".to_owned(),
                address: LOGIN.to_owned(),
                messages: BTreeMap::new(),
            }],
            next_id: 1,
            access_tokens: Vec::new(),
            refresh_tokens: Vec::new(),
            refresh_invalid: false,
            revoked: Vec::new(),
            calls: Vec::new(),
            rate_limit_next: None,
            send_fault: None,
            sends: Vec::new(),
            refusing: Vec::new(),
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

    /// The base (the fake's accounts server and Mail API host).
    pub fn base(&self) -> Url {
        Url::parse(&format!("{}/", self.server.uri())).unwrap()
    }

    /// The issuer a grant from this data centre names.
    pub fn issuer(&self) -> String {
        self.server.uri()
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// A consent: fresh tokens Zoho will accept.
    pub fn grant(&self) -> (String, String) {
        let mut state = self.lock();
        let access = format!("zoho-access-{}", state.next_id);
        let refresh = format!("zoho-refresh-{}", state.next_id + 1);
        state.next_id += 2;
        state.access_tokens.push(access.clone());
        state.refresh_tokens.push(refresh.clone());
        state.refresh_invalid = false;
        (access, refresh)
    }

    /// The login gains a second mail account at `address`.
    pub fn add_account(&self, address: &str) -> String {
        let mut state = self.lock();
        let id = format!("25606360000000{:05}", 10_000 + state.next_id);
        state.next_id += 1;
        state.accounts.push(Account {
            id: id.clone(),
            address: address.to_owned(),
            messages: BTreeMap::new(),
        });
        id
    }

    /// Put a message in the first account's `folder`; its id.
    pub fn deliver(&self, raw: &[u8], folder: &str, read: bool, at: DateTime<Utc>) -> String {
        self.deliver_to(0, raw, folder, read, at)
    }

    /// Put a message in account number `account`'s `folder`; its id.
    pub fn deliver_to(
        &self,
        account: usize,
        raw: &[u8],
        folder: &str,
        read: bool,
        at: DateTime<Utc>,
    ) -> String {
        let mut state = self.lock();
        let id = format!("17098870587691{:05}", state.next_id);
        state.next_id += 1;
        state.accounts[account].messages.insert(
            id.clone(),
            ZohoMessage {
                id: id.clone(),
                folder: folder.to_owned(),
                read,
                flag: "flag_not_set".to_owned(),
                archived: false,
                received: at,
                raw: raw.to_vec(),
            },
        );
        id
    }

    /// Change message `id` at Zoho.
    pub fn change(&self, id: &str, f: impl FnOnce(&mut ZohoMessage)) {
        let mut state = self.lock();
        let message = state
            .accounts
            .iter_mut()
            .find_map(|a| a.messages.get_mut(id))
            .expect("a message of the fake");
        f(message);
    }

    /// Delete message `id` for good.
    pub fn purge(&self, id: &str) {
        for account in &mut self.lock().accounts {
            account.messages.remove(id);
        }
    }

    /// Message `id` as Zoho holds it now.
    pub fn message(&self, id: &str) -> ZohoMessage {
        self.lock()
            .accounts
            .iter()
            .find_map(|a| a.messages.get(id).cloned())
            .expect("a message of the fake")
    }

    /// The messages of the first account's Sent folder, oldest first.
    pub fn sent(&self) -> Vec<ZohoMessage> {
        let mut sent: Vec<ZohoMessage> = self.lock().accounts[0]
            .messages
            .values()
            .filter(|m| m.folder == SENT)
            .cloned()
            .collect();
        sent.sort_by_key(|m| m.received);
        sent
    }

    /// Each send Zoho received: the message replied to, and the JSON.
    pub fn sends(&self) -> Vec<(Option<String>, serde_json::Value)> {
        self.lock().sends.clone()
    }

    /// The next Mail API call answers 429 with `Retry-After: seconds`.
    pub fn rate_limit_next(&self, seconds: u64) {
        self.lock().rate_limit_next = Some(seconds);
    }

    /// The next send answers `status` and sends nothing.
    pub fn refuse_next_send(&self, status: u16) {
        self.lock().send_fault = Some(SendFault::Refuse(status));
    }

    /// The next send goes out, but its answer is lost (503).
    pub fn lose_next_send_answer(&self) {
        self.lock().send_fault = Some(SendFault::LoseAnswer);
    }

    /// Zoho refuses every change to message `id` (400).
    pub fn refuse_changes(&self, id: &str) {
        self.lock().refusing.push(id.to_owned());
    }

    /// Refreshing answers `invalid_code` with a 200, as Zoho does.
    pub fn revoke_grant(&self) {
        let mut state = self.lock();
        state.refresh_invalid = true;
        state.access_tokens.clear();
    }

    /// Tokens revoked through the revocation endpoint.
    pub fn revoked(&self) -> Vec<String> {
        self.lock().revoked.clone()
    }

    /// Calls seen, as `METHOD path` plus a write's mode (no tokens).
    pub fn calls(&self) -> Vec<String> {
        self.lock().calls.clone()
    }

    /// How many raw downloads of `id` were made.
    pub fn raw_fetches(&self, id: &str) -> usize {
        let needle = format!("/messages/{id}/originalmessage");
        self.lock()
            .calls
            .iter()
            .filter(|c| c.ends_with(&needle))
            .count()
    }
}

fn json(status: u16, body: &serde_json::Value) -> ResponseTemplate {
    ResponseTemplate::new(status).set_body_json(body)
}

fn ok(data: &serde_json::Value) -> ResponseTemplate {
    json(
        200,
        &serde_json::json!({"status": {"code": 200, "description": "success"}, "data": data}),
    )
}

fn error(status: u16, code: &str) -> ResponseTemplate {
    json(
        status,
        &serde_json::json!({"status": {"code": status, "description": "failure"},
                            "data": {"errorCode": code}}),
    )
}

/// HTML-escape as Zoho does in metadata.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// A header of a raw message, as written.
fn header(raw: &[u8], name: &str) -> String {
    let text = String::from_utf8_lossy(raw);
    let prefix = format!("{}:", name.to_ascii_lowercase());
    text.lines()
        .take_while(|l| !l.is_empty())
        .find(|l| l.to_ascii_lowercase().starts_with(&prefix))
        .map(|l| l[prefix.len()..].trim().to_owned())
        .unwrap_or_default()
}

fn listed(message: &ZohoMessage) -> serde_json::Value {
    serde_json::json!({
        "messageId": message.id,
        "folderId": message.folder,
        "receivedTime": message.received.timestamp_millis().to_string(),
        "status": if message.read { "1" } else { "0" },
        "flagid": message.flag,
        "size": message.raw.len().to_string(),
        "subject": escape(&header(&message.raw, "Subject")),
        "toAddress": escape(&header(&message.raw, "To")),
        "sender": "Someone",
    })
}

fn folders() -> serde_json::Value {
    serde_json::json!([
        {"folderId": INBOX, "folderName": "Inbox", "folderType": "Inbox", "path": "/Inbox"},
        {"folderId": DRAFTS, "folderName": "Drafts", "folderType": "Drafts", "path": "/Drafts"},
        {"folderId": SENT, "folderName": "Sent", "folderType": "Sent", "path": "/Sent"},
        {"folderId": SPAM, "folderName": "Spam", "folderType": "Spam", "path": "/Spam"},
        {"folderId": TRASH, "folderName": "Trash", "folderType": "Trash", "path": "/Trash"},
        {"folderId": CLIENTS, "folderName": "Clients", "folderType": "Inbox", "path": "/Clients"}
    ])
}

impl Respond for Responder {
    fn respond(&self, request: &wiremock::Request) -> ResponseTemplate {
        let mut state = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let path = request.url.path().to_owned();
        let query: HashMap<String, String> = request.url.query_pairs().into_owned().collect();
        let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap_or_default();
        let mode = body["mode"]
            .as_str()
            .map(|m| format!(" {m}"))
            .unwrap_or_default();
        state.calls.push(format!("{} {path}{mode}", request.method));
        match (request.method.as_str(), path.as_str()) {
            ("POST", "/oauth/v2/token") => return token(&mut state, &request.body),
            ("POST", "/oauth/v2/token/revoke") => {
                let token = query.get("token").cloned().unwrap_or_default();
                state.revoked.push(token);
                return ResponseTemplate::new(200);
            }
            _ => {}
        }
        let Some(api) = path.strip_prefix("/api/") else {
            return ResponseTemplate::new(404);
        };
        let token = request
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Zoho-oauthtoken "));
        if !token.is_some_and(|t| state.access_tokens.iter().any(|a| a == t)) {
            return error(401, "INVALID_OAUTHTOKEN");
        }
        if let Some(seconds) = state.rate_limit_next.take() {
            return error(429, "TOO_MANY_REQUESTS")
                .insert_header("retry-after", seconds.to_string().as_str());
        }
        if api == "accounts" {
            let accounts: Vec<_> = state
                .accounts
                .iter()
                .map(|a| {
                    serde_json::json!({"accountId": a.id, "primaryEmailAddress": a.address,
                                       "displayName": "Sam Rivera", "type": "ZOHO_ACCOUNT"})
                })
                .collect();
            return ok(&accounts.into());
        }
        let Some(rest) = api.strip_prefix("accounts/") else {
            return ResponseTemplate::new(404);
        };
        let (account_id, rest) = rest.split_once('/').unwrap_or((rest, ""));
        let Some(index) = state.accounts.iter().position(|a| a.id == account_id) else {
            return error(404, "ACCOUNT_NOT_FOUND");
        };
        mail(
            &mut state,
            index,
            request.method.as_str(),
            rest,
            &query,
            &body,
            &request.body,
        )
    }
}

fn token(state: &mut State, body: &[u8]) -> ResponseTemplate {
    let form: HashMap<String, String> = url::form_urlencoded::parse(body).into_owned().collect();
    if form.get("client_secret").map(String::as_str) != Some("test-zoho-secret") {
        return json(200, &serde_json::json!({"error": "invalid_client"}));
    }
    let known = form
        .get("refresh_token")
        .is_some_and(|t| state.refresh_tokens.contains(t));
    if form.get("grant_type").map(String::as_str) != Some("refresh_token")
        || state.refresh_invalid
        || !known
    {
        // Zoho answers a refused refresh with a 200.
        return json(200, &serde_json::json!({"error": "invalid_code"}));
    }
    let access = format!("zoho-access-{}", state.next_id);
    state.next_id += 1;
    state.access_tokens.push(access.clone());
    json(
        200,
        &serde_json::json!({"access_token": access, "expires_in": 3600,
                            "api_domain": "https://www.zohoapis.example", "token_type": "Bearer"}),
    )
}

#[allow(
    clippy::too_many_lines,
    reason = "one fake API, one match over its routes"
)]
fn mail(
    state: &mut State,
    index: usize,
    method: &str,
    path: &str,
    query: &HashMap<String, String>,
    body: &serde_json::Value,
    raw_body: &[u8],
) -> ResponseTemplate {
    let parts: Vec<&str> = path.split('/').collect();
    match (method, parts.as_slice()) {
        ("GET", ["folders"]) => ok(&folders()),
        ("GET", ["messages", "view"]) => {
            let folder = query.get("folderId").cloned().unwrap_or_default();
            let start: usize = query.get("start").and_then(|s| s.parse().ok()).unwrap_or(1);
            let limit: usize = query
                .get("limit")
                .and_then(|s| s.parse().ok())
                .unwrap_or(10);
            let mut messages: Vec<&ZohoMessage> = state.accounts[index]
                .messages
                .values()
                .filter(|m| m.folder == folder && !m.archived)
                .collect();
            messages.sort_by(|a, b| b.received.cmp(&a.received).then(b.id.cmp(&a.id)));
            let page: Vec<_> = messages
                .into_iter()
                .skip(start.saturating_sub(1))
                .take(limit)
                .map(listed)
                .collect();
            ok(&page.into())
        }
        ("GET", ["folders", _, "messages", id, "details"]) => {
            match state.accounts[index].messages.get(*id) {
                Some(message) => ok(&listed(message)),
                None => error(404, "MESSAGE_NOT_FOUND"),
            }
        }
        ("GET", ["messages", id, "originalmessage"]) => {
            match state.accounts[index].messages.get(*id) {
                Some(message) => ok(&serde_json::json!({
                    "messageId": message.id.parse::<u64>().unwrap_or_default(),
                    "content": String::from_utf8_lossy(&message.raw),
                })),
                None => error(404, "MESSAGE_NOT_FOUND"),
            }
        }
        ("PUT", ["updatemessage"]) => update(state, index, body),
        ("POST", ["messages", "attachments"]) => {
            let name = query.get("fileName").cloned().unwrap_or_default();
            ok(&serde_json::json!({"storeName": "52882865",
                                  "attachmentPath": format!("/compose/{}", raw_body.len()),
                                  "attachmentName": name}))
        }
        ("POST", ["messages"]) => send(state, index, None, body),
        ("POST", ["messages", parent]) => {
            if body["action"] != "reply" {
                return error(400, "INVALID_ACTION");
            }
            send(state, index, Some((*parent).to_owned()), body)
        }
        _ => ResponseTemplate::new(404),
    }
}

fn update(state: &mut State, index: usize, body: &serde_json::Value) -> ResponseTemplate {
    let ids: Vec<String> = body["messageId"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|v| v.as_u64().map(|n| n.to_string()))
                .collect()
        })
        .unwrap_or_default();
    if ids
        .iter()
        .any(|id| !state.accounts[index].messages.contains_key(id))
    {
        return error(404, "MESSAGE_NOT_FOUND");
    }
    if ids.iter().any(|id| state.refusing.contains(id)) {
        return error(400, "OPERATION_NOT_PERMITTED");
    }
    let account = &mut state.accounts[index];
    let mode = body["mode"].as_str().unwrap_or_default();
    for id in &ids {
        let Some(message) = account.messages.get_mut(id) else {
            continue;
        };
        match mode {
            "markAsRead" => message.read = true,
            "markAsUnread" => message.read = false,
            "setFlag" => message.flag = body["flagid"].as_str().unwrap_or_default().to_owned(),
            "archiveMails" => message.archived = true,
            "unArchiveMails" => message.archived = false,
            "moveMessage" => {
                message.folder = body["destfolderId"]
                    .as_u64()
                    .map(|n| n.to_string())
                    .unwrap_or_default();
            }
            _ => return error(400, "INVALID_MODE"),
        }
    }
    ok(&serde_json::Value::Null)
}

fn send(
    state: &mut State,
    index: usize,
    parent: Option<String>,
    body: &serde_json::Value,
) -> ResponseTemplate {
    let fault = state.send_fault.take();
    if let Some(SendFault::Refuse(status)) = fault {
        return error(status, "EXTRA_KEY_FOUND_IN_JSON");
    }
    let field = |key: &str| body[key].as_str().unwrap_or_default().to_owned();
    let parent_id = match &parent {
        Some(parent) => match state.accounts[index].messages.get(parent) {
            Some(message) => Some(header(&message.raw, "Message-ID")),
            None => return error(404, "MESSAGE_NOT_FOUND"),
        },
        None => None,
    };
    let number = state.next_id;
    state.next_id += 1;
    let id = format!("17115403578801{number:05}");
    let mut raw = format!(
        "From: {}\r\nTo: {}\r\nSubject: {}\r\nMessage-ID: <zoho-{number}@zoho.example>\r\n",
        field("fromAddress"),
        field("toAddress"),
        field("subject"),
    );
    if let Some(parent_id) = parent_id {
        write!(
            raw,
            "In-Reply-To: {parent_id}\r\nReferences: {parent_id}\r\n"
        )
        .unwrap();
    }
    let kind = if field("mailFormat") == "plaintext" {
        "text/plain"
    } else {
        "text/html"
    };
    write!(
        raw,
        "MIME-Version: 1.0\r\nContent-Type: {kind}; charset=utf-8\r\n\r\n{}\r\n",
        field("content")
    )
    .unwrap();
    state.accounts[index].messages.insert(
        id.clone(),
        ZohoMessage {
            id: id.clone(),
            folder: SENT.to_owned(),
            read: true,
            flag: "flag_not_set".to_owned(),
            archived: false,
            received: state.clock.now(),
            raw: raw.into_bytes(),
        },
    );
    state.sends.push((parent, body.clone()));
    if fault == Some(SendFault::LoseAnswer) {
        return error(503, "INTERNAL_ERROR");
    }
    ok(&serde_json::json!({"messageId": id, "subject": field("subject")}))
}
