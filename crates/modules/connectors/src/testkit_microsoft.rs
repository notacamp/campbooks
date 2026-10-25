//! A fake Microsoft (on wiremock): the mail tenant's token endpoint (refresh
//! only, each refresh token spent once and rotated), Graph `/me`, and one
//! fictional mailbox through Graph (campbooks-cloud#316): its folders (the
//! well-known ones by name, the tree), each folder's delta (pages joined by
//! `@odata.nextLink`, a round ending in `@odata.deltaLink`), a message's
//! state and its `$value`, and subscriptions. Faults are switched on one by
//! one: a refresh refused with an answer of the test's choosing, throttling
//! (of refreshes, or of the next Graph call), access tokens that stop
//! working, a folder's delta state forgotten (410), a subscription refused.
//!
//! The fake's delta follows Graph's: a message's every change bumps its
//! version; a folder's `$deltatoken` is the version it was read up to, and
//! a round from it lists the folder's messages changed since and those that
//! left it (`@removed`). Its links point at `graph.microsoft.com`, which the
//! client never calls: it keeps only their token.
//!
//! No test talks to Microsoft.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use chrono::{DateTime, SecondsFormat, Utc};
use url::Url;
use wiremock::{Mock, MockServer, Respond, ResponseTemplate, matchers::any};

/// The tenant the fake's mail connections are in.
pub const TENANT: &str = "common";

/// The client secret the fake accepts.
pub const CLIENT_SECRET: &str = "test-microsoft-secret";

/// The fictional work account's Graph user id.
pub const GRAPH_ID: &str = "0d9ed3e4-1c2b-4f6a-9e8d-7c6b5a4f3e2d";

/// Its primary address.
pub const ADDRESS: &str = "ana.rivera@example.com";

/// Its ID token subject (pairwise, per app).
pub const SUB: &str = "AAAAAAAAAAAAAAAAAAAAAMicrosoftSub1";

/// The well-known folders of the fake mailbox: (Graph's name, id, shown
/// name).
pub const WELL_KNOWN: [(&str, &str, &str); 7] = [
    ("inbox", "fld-inbox", "Inbox"),
    ("sentitems", "fld-sent", "Sent Items"),
    ("junkemail", "fld-junk", "Junk Email"),
    ("deleteditems", "fld-deleted", "Deleted Items"),
    ("archive", "fld-archive", "Archive"),
    ("drafts", "fld-drafts", "Drafts"),
    ("outbox", "fld-outbox", "Outbox"),
];

/// Where the fake's links point (never called).
const LINK_BASE: &str = "https://graph.microsoft.com/v1.0";

/// A folder of the fake mailbox.
#[derive(Debug, Clone)]
struct FakeFolder {
    id: String,
    name: String,
    parent: Option<String>,
    /// A search folder (a view of other folders' messages).
    search: bool,
}

/// A message of the fake mailbox.
#[derive(Debug, Clone)]
pub struct FakeGraphMessage {
    pub id: String,
    pub folder: String,
    pub raw: Vec<u8>,
    pub received: DateTime<Utc>,
    pub read: bool,
    pub flagged: bool,
    pub draft: bool,
    pub categories: Vec<String>,
    pub conversation: String,
    /// Bumped by every change.
    version: u64,
}

/// A subscription the fake holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeSubscription {
    pub id: String,
    pub client_state: String,
    pub notification_url: String,
    pub resource: String,
    pub expires: DateTime<Utc>,
}

#[derive(Debug)]
struct State {
    next_id: u64,
    access_tokens: Vec<String>,
    refresh_tokens: Vec<String>,
    /// What `/me` answers.
    me: serde_json::Value,
    /// Answer the next refreshes so (status, body, `Retry-After`).
    refresh_refusal: Option<(u16, serde_json::Value, Option<u64>)>,
    /// Requests received, as `METHOD path`.
    calls: Vec<String>,
    folders: Vec<FakeFolder>,
    messages: Vec<FakeGraphMessage>,
    /// (folder, message, version): a message left a folder.
    departures: Vec<(String, String, u64)>,
    version: u64,
    /// Messages per delta page, whatever the client prefers.
    page_size: Option<usize>,
    /// The next delta of these folders answers 410.
    expire: Vec<String>,
    /// The next Graph call answers 429 with this `Retry-After`.
    throttle_next: Option<u64>,
    subscriptions: Vec<FakeSubscription>,
    /// Subscribing answers 400 (Graph could not reach the URL).
    refuse_subscriptions: bool,
    /// Well-known names answered 404 although the folder is there.
    hidden_well_known: Vec<String>,
    /// The most messages a first round lists.
    filter_limit: Option<usize>,
}

/// Microsoft's token endpoint and Graph, faked.
#[derive(Clone)]
pub struct FakeMicrosoft {
    server: Arc<MockServer>,
    state: Arc<Mutex<State>>,
}

struct Responder(Arc<Mutex<State>>);

impl FakeMicrosoft {
    /// A fake answering for [`GRAPH_ID`] at [`ADDRESS`], its mailbox
    /// holding the [`WELL_KNOWN`] folders and no message.
    pub async fn start() -> Self {
        let server = MockServer::start().await;
        let folders = WELL_KNOWN
            .iter()
            .map(|(_, id, name)| FakeFolder {
                id: (*id).to_owned(),
                name: (*name).to_owned(),
                parent: None,
                search: false,
            })
            .collect();
        let state = Arc::new(Mutex::new(State {
            next_id: 1,
            access_tokens: Vec::new(),
            refresh_tokens: Vec::new(),
            me: serde_json::json!({
                "id": GRAPH_ID,
                "mail": "Ana.Rivera@Example.com",
                "userPrincipalName": "ana@example.onmicrosoft.com",
                "displayName": "Ana Rivera"
            }),
            refresh_refusal: None,
            calls: Vec::new(),
            folders,
            messages: Vec::new(),
            departures: Vec::new(),
            version: 0,
            page_size: None,
            expire: Vec::new(),
            throttle_next: None,
            subscriptions: Vec::new(),
            refuse_subscriptions: false,
            hidden_well_known: Vec::new(),
            filter_limit: None,
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

    /// Where it is served: Microsoft's endpoints are laid out under it.
    pub fn base(&self) -> Url {
        Url::parse(&format!("{}/", self.server.uri())).unwrap()
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// A fresh grant's (access token, refresh token).
    pub fn grant(&self) -> (String, String) {
        let mut state = self.lock();
        let n = state.next_id;
        state.next_id += 1;
        let access = format!("fake-ms-access-{n}");
        let refresh = format!("fake-ms-refresh-{n}");
        state.access_tokens.push(access.clone());
        state.refresh_tokens.push(refresh.clone());
        (access, refresh)
    }

    /// `/me` answers `me` from now on.
    pub fn answer_me(&self, me: serde_json::Value) {
        self.lock().me = me;
    }

    /// Every refresh from now on is refused with `status` and `body`.
    pub fn refuse_refresh(&self, status: u16, body: serde_json::Value) {
        self.lock().refresh_refusal = Some((status, body, None));
    }

    /// Every refresh from now on answers 429 with `Retry-After: seconds`.
    pub fn throttle_refresh(&self, seconds: u64) {
        self.lock().refresh_refusal = Some((
            429,
            serde_json::json!({"error": "temporarily_unavailable"}),
            Some(seconds),
        ));
    }

    /// Refreshes work again.
    pub fn allow_refresh(&self) {
        self.lock().refresh_refusal = None;
    }

    /// Every access token handed out so far stops working.
    pub fn expire_access_tokens(&self) {
        self.lock().access_tokens.clear();
    }

    /// The requests received, as `METHOD path`.
    pub fn calls(&self) -> Vec<String> {
        self.lock().calls.clone()
    }

    /// Forget the requests received so far.
    pub fn clear_calls(&self) {
        self.lock().calls.clear();
    }

    /// How many refreshes were asked for.
    pub fn token_calls(&self) -> usize {
        self.lock()
            .calls
            .iter()
            .filter(|call| call.ends_with("/oauth2/v2.0/token"))
            .count()
    }

    /// How many `$value` downloads of `id` were asked for.
    pub fn raw_fetches(&self, id: &str) -> usize {
        let path = format!("GET /v1.0/me/messages/{id}/$value");
        self.lock()
            .calls
            .iter()
            .filter(|call| **call == path)
            .count()
    }

    /// A member's folder named `name` under `parent` (a folder id; the top
    /// level when `None`): its id.
    pub fn add_folder(&self, name: &str, parent: Option<&str>) -> String {
        let mut state = self.lock();
        let id = format!("fld-{}", state.next_id);
        state.next_id += 1;
        state.folders.push(FakeFolder {
            id: id.clone(),
            name: name.to_owned(),
            parent: parent.map(ToOwned::to_owned),
            search: false,
        });
        id
    }

    /// A search folder named `name` under `parent`: its id.
    pub fn add_search_folder(&self, name: &str, parent: Option<&str>) -> String {
        let id = self.add_folder(name, parent);
        if let Some(folder) = self.lock().folders.iter_mut().find(|f| f.id == id) {
            folder.search = true;
        }
        id
    }

    /// Graph names the well-known folder `name` again, or (`hidden`) answers
    /// 404 for it while it is there (a passing fault).
    pub fn hide_well_known(&self, name: &str, hidden: bool) {
        let mut state = self.lock();
        state.hidden_well_known.retain(|n| n != name);
        if hidden {
            state.hidden_well_known.push(name.to_owned());
        }
    }

    /// A first round (with a `$filter`) lists at most `limit` messages, the
    /// newest, from now on; all when `None`.
    pub fn limit_filtered(&self, limit: Option<usize>) {
        self.lock().filter_limit = limit;
    }

    /// Delete folder `id` (its messages go with it).
    pub fn remove_folder(&self, id: &str) {
        let mut state = self.lock();
        state.folders.retain(|f| f.id != id);
        let gone: Vec<String> = state
            .messages
            .iter()
            .filter(|m| m.folder == id)
            .map(|m| m.id.clone())
            .collect();
        for message in gone {
            remove(&mut state, &message);
        }
    }

    /// A message `raw` received at `at` into folder `folder` (an id): its id.
    pub fn deliver(&self, folder: &str, raw: &[u8], at: DateTime<Utc>) -> String {
        let mut state = self.lock();
        let id = format!("msg-{}", state.next_id);
        state.next_id += 1;
        state.version += 1;
        let version = state.version;
        state.messages.push(FakeGraphMessage {
            id: id.clone(),
            folder: folder.to_owned(),
            raw: raw.to_vec(),
            received: at,
            read: false,
            flagged: false,
            draft: false,
            categories: Vec::new(),
            conversation: format!("conv-{id}"),
            version,
        });
        id
    }

    /// Change message `id` with `change`, as Outlook would.
    pub fn change(&self, id: &str, change: impl FnOnce(&mut FakeGraphMessage)) {
        let mut state = self.lock();
        state.version += 1;
        let version = state.version;
        let Some(message) = state.messages.iter_mut().find(|m| m.id == id) else {
            return;
        };
        let before = message.folder.clone();
        change(message);
        message.version = version;
        let after = message.folder.clone();
        if before != after {
            state.departures.push((before, id.to_owned(), version));
        }
    }

    /// Move message `id` to folder `folder` (its id stays: immutable ids).
    pub fn move_to(&self, id: &str, folder: &str) {
        self.change(id, |m| folder.clone_into(&mut m.folder));
    }

    /// Delete message `id` for good.
    pub fn delete(&self, id: &str) {
        remove(&mut self.lock(), id);
    }

    /// Message `id` as the fake holds it.
    pub fn message(&self, id: &str) -> Option<FakeGraphMessage> {
        self.lock().messages.iter().find(|m| m.id == id).cloned()
    }

    /// Delta pages of at most `size` messages from now on.
    pub fn page_size(&self, size: usize) {
        self.lock().page_size = Some(size);
    }

    /// The next delta read of folder `id` answers 410 `syncStateNotFound`.
    pub fn expire_delta(&self, id: &str) {
        self.lock().expire.push(id.to_owned());
    }

    /// The next Graph call answers 429 with `Retry-After: seconds`.
    pub fn throttle_next(&self, seconds: u64) {
        self.lock().throttle_next = Some(seconds);
    }

    /// Subscribing answers 400 from now on (Graph could not reach the
    /// notification URL), or works again.
    pub fn refuse_subscriptions(&self, refuse: bool) {
        self.lock().refuse_subscriptions = refuse;
    }

    /// The subscriptions the fake holds.
    pub fn subscriptions(&self) -> Vec<FakeSubscription> {
        self.lock().subscriptions.clone()
    }

    /// Graph forgets every subscription (they expired, or were removed).
    pub fn drop_subscriptions(&self) {
        self.lock().subscriptions.clear();
    }
}

/// Delete message `id` from `state`: it leaves its folder.
fn remove(state: &mut State, id: &str) {
    state.version += 1;
    let version = state.version;
    if let Some(at) = state.messages.iter().position(|m| m.id == id) {
        let message = state.messages.remove(at);
        state
            .departures
            .push((message.folder, id.to_owned(), version));
    }
}

fn json(status: u16, body: &serde_json::Value) -> ResponseTemplate {
    ResponseTemplate::new(status).set_body_json(body)
}

fn graph_error(status: u16, code: &str) -> ResponseTemplate {
    json(
        status,
        &serde_json::json!({"error": {"code": code, "message": "A message the client never shows."}}),
    )
}

impl Respond for Responder {
    fn respond(&self, request: &wiremock::Request) -> ResponseTemplate {
        let mut state = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let path = request.url.path().to_owned();
        state.calls.push(format!("{} {path}", request.method));
        let token_path = format!("/{TENANT}/oauth2/v2.0/token");
        if request.method.as_str() == "POST" && path == token_path {
            return refresh(&mut state, &request.body);
        }
        let Some(rest) = path.strip_prefix("/v1.0/") else {
            return ResponseTemplate::new(404);
        };
        let known = request
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .is_some_and(|token| state.access_tokens.iter().any(|t| t == token));
        if !known {
            return graph_error(401, "InvalidAuthenticationToken");
        }
        if let Some(seconds) = state.throttle_next.take() {
            return graph_error(429, "ApplicationThrottled")
                .insert_header("retry-after", seconds.to_string().as_str());
        }
        let segments: Vec<String> = rest.split('/').map(percent_decode).collect();
        let segments: Vec<&str> = segments.iter().map(String::as_str).collect();
        let query: HashMap<String, String> = request.url.query_pairs().into_owned().collect();
        match (request.method.as_str(), segments.as_slice()) {
            ("GET", ["me"]) => json(200, &state.me),
            ("GET", ["me", "mailFolders"]) => folders(&state, None, &query),
            ("GET", ["me", "mailFolders", id, "childFolders"]) => {
                folders(&state, Some(*id), &query)
            }
            ("GET", ["me", "mailFolders", id, "messages", "delta"]) => {
                delta(&mut state, id, &query, request)
            }
            ("GET", ["me", "mailFolders", name]) => {
                let hidden = state.hidden_well_known.iter().any(|h| h == name);
                let found = WELL_KNOWN
                    .iter()
                    .find(|(known, _, _)| known == name)
                    .and_then(|(_, id, _)| state.folders.iter().find(|f| f.id == *id))
                    .filter(|_| !hidden);
                match found {
                    Some(folder) => json(200, &folder_json(&state, folder)),
                    None => graph_error(404, "ErrorItemNotFound"),
                }
            }
            ("GET", ["me", "messages", id]) => match state.messages.iter().find(|m| m.id == *id) {
                Some(message) => json(200, &item(message)),
                None => graph_error(404, "ErrorItemNotFound"),
            },
            ("GET", ["me", "messages", id, "$value"]) => {
                match state.messages.iter().find(|m| m.id == *id) {
                    Some(message) => ResponseTemplate::new(200)
                        .insert_header("content-type", "text/plain")
                        .set_body_bytes(message.raw.clone()),
                    None => graph_error(404, "ErrorItemNotFound"),
                }
            }
            ("POST", ["subscriptions"]) => subscribe(&mut state, &request.body),
            ("PATCH", ["subscriptions", id]) => renew(&mut state, id, &request.body),
            _ => graph_error(404, "ResourceNotFound"),
        }
    }
}

fn percent_decode(segment: &str) -> String {
    url::form_urlencoded::parse(format!("x={segment}").as_bytes())
        .next()
        .map(|(_, value)| value.into_owned())
        .unwrap_or_default()
}

/// A folder listing page.
fn folders(
    state: &State,
    parent: Option<&str>,
    query: &HashMap<String, String>,
) -> ResponseTemplate {
    let top: usize = query.get("$top").and_then(|t| t.parse().ok()).unwrap_or(10);
    let skip: usize = query.get("$skip").and_then(|t| t.parse().ok()).unwrap_or(0);
    if query.get("$orderby").map(String::as_str) != Some("displayName") {
        // Graph's order is not stable without one.
        return graph_error(400, "OrderByRequired");
    }
    let mut under: Vec<&FakeFolder> = state
        .folders
        .iter()
        .filter(|f| f.parent.as_deref() == parent)
        .collect();
    under.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id)));
    let value: Vec<serde_json::Value> = under
        .into_iter()
        .skip(skip)
        .take(top)
        .map(|f| folder_json(state, f))
        .collect();
    json(200, &serde_json::json!({"value": value}))
}

/// A folder as Graph answers it.
fn folder_json(state: &State, folder: &FakeFolder) -> serde_json::Value {
    let children = state
        .folders
        .iter()
        .filter(|c| c.parent.as_deref() == Some(folder.id.as_str()))
        .count();
    let mut answer = serde_json::json!({
        "id": folder.id, "displayName": folder.name, "childFolderCount": children
    });
    if folder.search {
        answer["@odata.type"] = "#microsoft.graph.mailSearchFolder".into();
    }
    answer
}

/// A message as a delta page or a GET lists it.
fn item(message: &FakeGraphMessage) -> serde_json::Value {
    serde_json::json!({
        "id": message.id,
        "parentFolderId": message.folder,
        "conversationId": message.conversation,
        "receivedDateTime": message.received.to_rfc3339_opts(SecondsFormat::Secs, true),
        "isRead": message.read,
        "isDraft": message.draft,
        "flag": {"flagStatus": if message.flagged { "flagged" } else { "notFlagged" }},
        "categories": message.categories,
    })
}

/// One delta page of folder `folder`.
///
/// Tokens: a round's `$skiptoken` is `<since>.<from version>.<offset>`
/// (`since` the round's `$filter` start in epoch seconds, or `-`), a
/// `$deltatoken` is `v<version>`.
#[allow(
    clippy::too_many_lines,
    reason = "one fake endpoint read top to bottom"
)]
fn delta(
    state: &mut State,
    folder: &str,
    query: &HashMap<String, String>,
    request: &wiremock::Request,
) -> ResponseTemplate {
    if !state.folders.iter().any(|f| f.id == folder) {
        return graph_error(404, "ErrorItemNotFound");
    }
    if let Some(at) = state.expire.iter().position(|f| f == folder) {
        state.expire.remove(at);
        return graph_error(410, "syncStateNotFound");
    }
    let prefer = request
        .headers
        .get("prefer")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    if !prefer.contains("IdType=\"ImmutableId\"") {
        return graph_error(400, "ImmutableIdRequired");
    }
    let preferred = prefer
        .split(',')
        .find_map(|p| p.trim().strip_prefix("odata.maxpagesize="))
        .and_then(|n| n.trim().parse().ok())
        .unwrap_or(10);
    let size = state.page_size.unwrap_or(preferred);
    // (since, from, offset): the round.
    let (since, from, offset): (Option<i64>, u64, usize) =
        if let Some(skip) = query.get("$skiptoken") {
            let parts: Vec<&str> = skip.split('.').collect();
            let [since, from, offset] = parts.as_slice() else {
                return graph_error(400, "BadSkipToken");
            };
            (
                since.parse().ok(),
                from.parse().unwrap_or(0),
                offset.parse().unwrap_or(0),
            )
        } else if let Some(token) = query.get("$deltatoken") {
            let Some(from) = token.strip_prefix('v').and_then(|v| v.parse().ok()) else {
                return graph_error(410, "syncStateNotFound");
            };
            (None, from, 0)
        } else {
            let since = query
                .get("$filter")
                .and_then(|f| f.strip_prefix("receivedDateTime ge "))
                .and_then(|at| DateTime::parse_from_rfc3339(at).ok())
                .map(|at| at.timestamp());
            if query
                .get("$select")
                .is_none_or(|s| !s.contains("parentFolderId"))
                || query.get("$orderby").map(String::as_str) != Some("receivedDateTime desc")
            {
                return graph_error(400, "SelectAndOrderRequired");
            }
            (since.or(Some(i64::MIN)), 0, 0)
        };
    let round = since.is_some();
    let mut listed: Vec<serde_json::Value> = Vec::new();
    let mut present: Vec<&FakeGraphMessage> = state
        .messages
        .iter()
        .filter(|m| m.folder == folder && m.version > from)
        .filter(|m| since.is_none_or(|s| m.received.timestamp() >= s))
        .collect();
    if round {
        // `$orderby=receivedDateTime desc`, and at most the filter's limit.
        present.sort_by(|a, b| b.received.cmp(&a.received).then_with(|| a.id.cmp(&b.id)));
        if let Some(limit) = state.filter_limit {
            present.truncate(limit);
        }
    } else {
        present.sort_by(|a, b| a.id.cmp(&b.id));
    }
    listed.extend(present.iter().map(|m| item(m)));
    if !round {
        let mut left: Vec<&str> = state
            .departures
            .iter()
            .filter(|(from_folder, id, version)| {
                from_folder == folder
                    && *version > from
                    && !state
                        .messages
                        .iter()
                        .any(|m| m.id == *id && m.folder == folder)
            })
            .map(|(_, id, _)| id.as_str())
            .collect();
        left.sort_unstable();
        left.dedup();
        listed.extend(
            left.into_iter()
                .map(|id| serde_json::json!({"id": id, "@removed": {"reason": "deleted"}})),
        );
    }
    let page: Vec<serde_json::Value> = listed.iter().skip(offset).take(size).cloned().collect();
    let base = format!("{LINK_BASE}/me/mailFolders/{folder}/messages/delta");
    let mut answer = serde_json::json!({"value": page});
    if offset + size < listed.len() {
        let since = since.map_or_else(|| "-".to_owned(), |s| s.to_string());
        answer["@odata.nextLink"] =
            format!("{base}?$skiptoken={since}.{from}.{}", offset + size).into();
    } else {
        answer["@odata.deltaLink"] = format!("{base}?$deltatoken=v{}", state.version).into();
    }
    json(200, &answer)
}

fn subscribe(state: &mut State, body: &[u8]) -> ResponseTemplate {
    if state.refuse_subscriptions {
        return graph_error(400, "ValidationError");
    }
    let Ok(asked) = serde_json::from_slice::<serde_json::Value>(body) else {
        return graph_error(400, "BadRequest");
    };
    let text = |key: &str| asked[key].as_str().unwrap_or_default().to_owned();
    let Ok(expires) = DateTime::parse_from_rfc3339(&text("expirationDateTime")) else {
        return graph_error(400, "BadRequest");
    };
    let n = state.next_id;
    state.next_id += 1;
    let subscription = FakeSubscription {
        id: format!("7f105c7d-2dc5-4530-97cd-{n:012}"),
        client_state: text("clientState"),
        notification_url: text("notificationUrl"),
        resource: text("resource"),
        expires: expires.with_timezone(&Utc),
    };
    let answer = serde_json::json!({
        "id": subscription.id,
        "expirationDateTime": subscription.expires.to_rfc3339_opts(SecondsFormat::Secs, true),
    });
    state.subscriptions.push(subscription);
    json(201, &answer)
}

fn renew(state: &mut State, id: &str, body: &[u8]) -> ResponseTemplate {
    let Ok(asked) = serde_json::from_slice::<serde_json::Value>(body) else {
        return graph_error(400, "BadRequest");
    };
    let Some(expires) = asked["expirationDateTime"]
        .as_str()
        .and_then(|at| DateTime::parse_from_rfc3339(at).ok())
    else {
        return graph_error(400, "BadRequest");
    };
    let Some(subscription) = state.subscriptions.iter_mut().find(|s| s.id == id) else {
        return graph_error(404, "ResourceNotFound");
    };
    subscription.expires = expires.with_timezone(&Utc);
    json(
        200,
        &serde_json::json!({
            "id": subscription.id,
            "expirationDateTime": subscription.expires.to_rfc3339_opts(SecondsFormat::Secs, true),
        }),
    )
}

fn refresh(state: &mut State, body: &[u8]) -> ResponseTemplate {
    let form: HashMap<String, String> = url::form_urlencoded::parse(body).into_owned().collect();
    if form.get("client_secret").map(String::as_str) != Some(CLIENT_SECRET) {
        return json(
            401,
            &serde_json::json!({"error": "invalid_client", "error_codes": [7_000_215]}),
        );
    }
    if let Some((status, answer, retry)) = state.refresh_refusal.clone() {
        let template = json(status, &answer);
        return match retry {
            Some(seconds) => template.insert_header("retry-after", seconds.to_string().as_str()),
            None => template,
        };
    }
    let known = form.get("grant_type").map(String::as_str) == Some("refresh_token")
        && form
            .get("refresh_token")
            .is_some_and(|t| state.refresh_tokens.contains(t));
    if !known {
        return json(
            400,
            &serde_json::json!({"error": "invalid_grant", "error_codes": [70000],
                                "error_description": "AADSTS70000: The provided grant is invalid."}),
        );
    }
    // The refresh token used is spent: only the rotated one works next.
    if let Some(used) = form.get("refresh_token") {
        state.refresh_tokens.retain(|t| t != used);
    }
    let n = state.next_id;
    state.next_id += 1;
    let access = format!("fake-ms-access-{n}");
    let rotated = format!("fake-ms-refresh-{n}");
    state.access_tokens.push(access.clone());
    state.refresh_tokens.push(rotated.clone());
    json(
        200,
        &serde_json::json!({
            "token_type": "Bearer",
            "access_token": access,
            "refresh_token": rotated,
            "expires_in": 3599,
            "scope": "https://graph.microsoft.com/Mail.ReadWrite https://graph.microsoft.com/Mail.Send https://graph.microsoft.com/User.Read"
        }),
    )
}
