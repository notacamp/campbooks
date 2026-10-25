//! Microsoft Graph's mail, as the messages adapter reads it
//! (campbooks-cloud#316): the mailbox's folders, each folder's change log
//! (`messages/delta`), a message's state and its raw MIME (`$value`), and
//! the subscription that has Graph post change notifications.
//!
//! Every URL is the fixed Graph base with validated ids as path segments
//! (encoded by `url`), and of Graph's `@odata.nextLink` and
//! `@odata.deltaLink` only the `$skiptoken` or `$deltatoken` is kept: an
//! answer never chooses where the next request goes. Message ids are
//! Graph's immutable ids (`Prefer: IdType="ImmutableId"`), so a move keeps
//! a message's id. Nothing here changes, moves or deletes mail.

use chrono::{DateTime, SecondsFormat, Utc};
use serde::Deserialize;
use url::Url;

use super::{CALL_TIMEOUT, MAX_ANSWER_BYTES, Microsoft, bounded, graph_failure, safe_code, within};
use crate::provider::{ProviderError, transport, unreadable};

/// The largest listing read (a page of folders or of a folder's delta).
pub const LIST_ANSWER_BYTES: usize = 1024 * 1024;

/// Folders asked for per page of a folder listing.
pub const FOLDER_PAGE: u32 = 100;

/// The most messages Graph lists in a delta round started with a
/// `$filter` ("Applying `$filter` in a delta query returns only up to 5,000
/// messages"): a round that lists this many may have left some out.
pub const FILTER_LIMIT: u32 = 5_000;

/// The longest delta token kept, in characters.
pub const MAX_TOKEN: usize = 4096;

/// The longest id accepted from Graph (a message, a folder, a
/// conversation), in characters.
pub const MAX_ID: usize = 512;

/// The longest category name kept, in characters.
pub const MAX_CATEGORY: usize = 255;

/// Categories kept per message at most.
pub const MAX_CATEGORIES: usize = 50;

/// What a delta page asks Graph to say of each message.
const SELECT: &str = "id,parentFolderId,conversationId,receivedDateTime,isRead,isDraft,flag,\
                      categories";

/// The `Prefer` header of every call about messages.
const PREFER: &str = "odata.maxpagesize=50, IdType=\"ImmutableId\"";

/// Error codes Graph answers when it no longer has a delta's state.
const EXPIRED_CODES: [&str; 3] = ["syncStateNotFound", "resyncRequired", "syncStateInvalid"];

/// Whether `id` is one Graph could have given: 1 to [`MAX_ID`] letters,
/// digits and `-_=+/.`, never starting with `.` nor holding `..` (no
/// segment of it can climb a path, encoded or not).
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID
        && !id.starts_with('.')
        && !id.contains("..")
        && id.bytes().all(|b| {
            b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'=' | b'+' | b'/' | b'.')
        })
}

/// A mail folder, as a listing gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphFolder {
    pub id: String,
    /// Its name, without control characters, at most 255 characters.
    pub name: String,
    /// How many folders it holds.
    pub children: u32,
}

/// One page of a folder listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderPage {
    /// Its folders, search folders and unusable ids left out.
    pub folders: Vec<GraphFolder>,
    /// Graph answered a whole page: there may be more.
    pub full: bool,
}

/// A folder, as Graph answers it.
#[derive(Deserialize)]
struct FolderAnswer {
    #[serde(default)]
    id: Option<String>,
    #[serde(default, rename = "@odata.type")]
    kind: Option<String>,
    #[serde(default, rename = "displayName")]
    display_name: Option<String>,
    #[serde(default, rename = "childFolderCount")]
    child_folder_count: Option<u32>,
}

impl FolderAnswer {
    /// A search folder: a view of other folders' messages, never read.
    fn searches(&self) -> bool {
        self.kind
            .as_deref()
            .is_some_and(|kind| kind.to_ascii_lowercase().contains("mailsearchfolder"))
    }

    fn folder(self) -> Option<GraphFolder> {
        let id = self.id.filter(|id| valid_id(id))?;
        let name: String = self
            .display_name
            .unwrap_or_default()
            .chars()
            .filter(|c| !c.is_control())
            .take(255)
            .collect();
        Some(GraphFolder {
            id,
            name: name.trim().to_owned(),
            children: self.child_folder_count.unwrap_or(0),
        })
    }
}

/// Where a folder's delta read starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position<'a> {
    /// A first round: the folder's messages received since then.
    Start(DateTime<Utc>),
    /// The rest of a round (`$skiptoken`).
    Skip(&'a str),
    /// What changed since the last round (`$deltatoken`).
    Delta(&'a str),
}

/// What a delta read answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Delta {
    /// One page.
    Page(DeltaPage),
    /// Graph no longer has the delta's state (410, `syncStateNotFound`,
    /// `resyncRequired`): read the folder again from scratch.
    Expired,
}

/// One page of a folder's delta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeltaPage {
    pub items: Vec<DeltaItem>,
    pub next: Next,
}

/// Where the next read of the folder starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Next {
    /// The round goes on (`$skiptoken`).
    More(String),
    /// The round is done; later changes start here (`$deltatoken`).
    Done(String),
}

/// A message a delta page names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeltaItem {
    pub id: String,
    /// Its state; `None` when it left the folder (`@removed`): moved or
    /// deleted.
    pub state: Option<MessageState>,
}

/// A message's state at Microsoft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageState {
    /// Its folder.
    pub folder: Option<String>,
    /// Its conversation (the thread key).
    pub conversation: Option<String>,
    /// When Microsoft received it.
    pub received: Option<DateTime<Utc>>,
    pub read: bool,
    pub draft: bool,
    /// Flagged for follow-up (`flag.flagStatus` `flagged`).
    pub flagged: bool,
    /// Its Outlook categories, by name: trimmed, without control
    /// characters, at most [`MAX_CATEGORIES`] of at most [`MAX_CATEGORY`]
    /// characters.
    pub categories: Vec<String>,
}

/// A message's raw MIME, or why there is none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RawFetch {
    /// Its RFC 5322 bytes.
    Message(Vec<u8>),
    /// Larger than the limit: nothing past it was read.
    TooLarge,
}

/// A subscription to change notifications, as Graph answered it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subscription {
    /// Graph's id: letters, digits and `-`, at most 64.
    pub id: String,
    pub expires: DateTime<Utc>,
}

/// Whether `id` is a subscription id Graph could have given.
pub fn valid_subscription_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

impl Microsoft {
    /// The Graph URL of `segments`, each a path segment (encoded).
    fn graph_url(&self, segments: &[&str]) -> Result<Url, ProviderError> {
        let mut url = self.endpoints.graph.clone();
        url.path_segments_mut()
            .map_err(|()| ProviderError::Unexpected("the Graph URL has no path".to_owned()))?
            .pop_if_empty()
            .extend(segments);
        Ok(url)
    }

    /// GET `url`: its body, at most `limit` bytes, or the Graph error.
    async fn get(&self, url: Url, token: &str, limit: usize) -> Result<Vec<u8>, ProviderError> {
        let response = self
            .http
            .get(url)
            .timeout(CALL_TIMEOUT)
            .bearer_auth(token)
            .header(reqwest::header::ACCEPT, "application/json")
            .header("prefer", PREFER)
            .send()
            .await
            .map_err(transport)?;
        let status = response.status();
        let headers = response.headers().clone();
        let body = bounded(response, limit).await?;
        if !status.is_success() {
            return Err(graph_failure(status, &headers, &body));
        }
        Ok(body)
    }

    /// The well-known folder `name` (`inbox`, `sentitems`, …); `None` when
    /// Graph names none (the mailbox has none, or not this time).
    pub async fn well_known(
        &self,
        token: &str,
        name: &str,
    ) -> Result<Option<GraphFolder>, ProviderError> {
        let mut url = self.graph_url(&["me", "mailFolders", name])?;
        url.query_pairs_mut()
            .append_pair("$select", "id,displayName,childFolderCount");
        let body = match self.get(url, token, MAX_ANSWER_BYTES).await {
            Ok(body) => body,
            Err(ProviderError::NotFound) => return Ok(None),
            Err(other) => return Err(other),
        };
        let answer: FolderAnswer = serde_json::from_slice(&body)
            .map_err(|e| ProviderError::Unexpected(unreadable("a Graph folder", &e)))?;
        Ok(answer.folder())
    }

    /// One page of the folders under `parent` (the mailbox's top level when
    /// `None`), by name, from the `skip`-th: at most [`FOLDER_PAGE`], hidden
    /// ones and search folders (views of other folders) left out.
    pub async fn folders(
        &self,
        token: &str,
        parent: Option<&str>,
        skip: u32,
    ) -> Result<FolderPage, ProviderError> {
        #[derive(Deserialize)]
        struct Answer {
            #[serde(default)]
            value: Vec<FolderAnswer>,
        }
        let mut url = match parent {
            None => self.graph_url(&["me", "mailFolders"])?,
            Some(parent) => self.graph_url(&["me", "mailFolders", parent, "childFolders"])?,
        };
        url.query_pairs_mut()
            .append_pair("$top", &FOLDER_PAGE.to_string())
            .append_pair("$skip", &skip.to_string())
            .append_pair("$orderby", "displayName")
            .append_pair("$select", "id,displayName,childFolderCount");
        let body = self.get(url, token, LIST_ANSWER_BYTES).await?;
        let answer: Answer = serde_json::from_slice(&body)
            .map_err(|e| ProviderError::Unexpected(unreadable("a Graph folder listing", &e)))?;
        let full = answer.value.len() >= usize::try_from(FOLDER_PAGE).unwrap_or(usize::MAX);
        Ok(FolderPage {
            folders: answer
                .value
                .into_iter()
                .filter(|folder| !folder.searches())
                .filter_map(FolderAnswer::folder)
                .collect(),
            full,
        })
    }

    /// One page of folder `folder`'s delta from `position`.
    pub async fn delta(
        &self,
        token: &str,
        folder: &str,
        position: Position<'_>,
    ) -> Result<Delta, ProviderError> {
        let mut url = self.graph_url(&["me", "mailFolders", folder, "messages", "delta"])?;
        match position {
            Position::Start(since) => {
                let since = since.to_rfc3339_opts(SecondsFormat::Secs, true);
                // Newest first: Graph lists at most [`FILTER_LIMIT`] messages
                // with a `$filter`, so those it leaves out are the oldest.
                url.query_pairs_mut()
                    .append_pair("$select", SELECT)
                    .append_pair("$filter", &format!("receivedDateTime ge {since}"))
                    .append_pair("$orderby", "receivedDateTime desc");
            }
            Position::Skip(skip) => {
                url.query_pairs_mut().append_pair("$skiptoken", skip);
            }
            Position::Delta(delta) => {
                url.query_pairs_mut().append_pair("$deltatoken", delta);
            }
        }
        let response = self
            .http
            .get(url)
            .timeout(CALL_TIMEOUT)
            .bearer_auth(token)
            .header(reqwest::header::ACCEPT, "application/json")
            .header("prefer", PREFER)
            .send()
            .await
            .map_err(transport)?;
        let status = response.status();
        let headers = response.headers().clone();
        let body = bounded(response, LIST_ANSWER_BYTES).await?;
        if !status.is_success() {
            if expired(status, &body) {
                return Ok(Delta::Expired);
            }
            return Err(graph_failure(status, &headers, &body));
        }
        let answer: DeltaAnswer = serde_json::from_slice(&body)
            .map_err(|e| ProviderError::Unexpected(unreadable("a Graph delta page", &e)))?;
        answer.page().map(Delta::Page)
    }

    /// The state of message `id`; `None` when Microsoft no longer has it.
    pub async fn message(
        &self,
        token: &str,
        id: &str,
    ) -> Result<Option<MessageState>, ProviderError> {
        let mut url = self.graph_url(&["me", "messages", id])?;
        url.query_pairs_mut().append_pair("$select", SELECT);
        let body = match self.get(url, token, MAX_ANSWER_BYTES).await {
            Ok(body) => body,
            Err(ProviderError::NotFound) => return Ok(None),
            Err(other) => return Err(other),
        };
        let item: ItemAnswer = serde_json::from_slice(&body)
            .map_err(|e| ProviderError::Unexpected(unreadable("a Graph message", &e)))?;
        Ok(Some(item.state()))
    }

    /// Message `id`'s raw MIME (`$value`), at most `max_bytes`; `None` when
    /// Microsoft no longer has it.
    pub async fn raw(
        &self,
        token: &str,
        id: &str,
        max_bytes: u64,
    ) -> Result<Option<RawFetch>, ProviderError> {
        let url = self.graph_url(&["me", "messages", id, "$value"])?;
        let response = self
            .http
            .get(url)
            .timeout(RAW_TIMEOUT)
            .bearer_auth(token)
            .header("prefer", PREFER)
            .send()
            .await
            .map_err(transport)?;
        let status = response.status();
        if !status.is_success() {
            let headers = response.headers().clone();
            let body = bounded(response, MAX_ANSWER_BYTES)
                .await
                .unwrap_or_default();
            return match graph_failure(status, &headers, &body) {
                ProviderError::NotFound => Ok(None),
                other => Err(other),
            };
        }
        let limit = usize::try_from(max_bytes).unwrap_or(usize::MAX);
        Ok(Some(match within(response, limit).await? {
            Some(raw) => RawFetch::Message(raw),
            None => RawFetch::TooLarge,
        }))
    }

    /// Subscribe to the mailbox's messages (`created,updated,deleted` on
    /// `me/messages`) until `expires`, Graph posting to this instance's
    /// notification URL with `client_state`. Graph checks the URL before it
    /// answers (the validation handshake).
    pub async fn subscribe(
        &self,
        token: &str,
        client_state: &str,
        expires: DateTime<Utc>,
    ) -> Result<Subscription, ProviderError> {
        let Some(notify) = self.notifications.as_ref() else {
            return Err(ProviderError::Unexpected(
                "change notifications are not configured".to_owned(),
            ));
        };
        let body = serde_json::json!({
            "changeType": "created,updated,deleted",
            "notificationUrl": notify.as_str(),
            "lifecycleNotificationUrl": notify.as_str(),
            "resource": "me/messages",
            "expirationDateTime": expires.to_rfc3339_opts(SecondsFormat::Secs, true),
            "clientState": client_state,
        });
        let url = self.graph_url(&["subscriptions"])?;
        self.send_subscription(self.http.post(url), token, &body)
            .await?
            .ok_or(ProviderError::NotFound)
    }

    /// Renew subscription `id` until `expires`; `None` when Graph no longer
    /// has it.
    pub async fn renew(
        &self,
        token: &str,
        id: &str,
        expires: DateTime<Utc>,
    ) -> Result<Option<Subscription>, ProviderError> {
        let body = serde_json::json!({
            "expirationDateTime": expires.to_rfc3339_opts(SecondsFormat::Secs, true),
        });
        let url = self.graph_url(&["subscriptions", id])?;
        self.send_subscription(self.http.patch(url), token, &body)
            .await
    }

    /// Send a subscription's `body` with `request`: Graph's answer, `None`
    /// on 404.
    async fn send_subscription(
        &self,
        request: reqwest::RequestBuilder,
        token: &str,
        body: &serde_json::Value,
    ) -> Result<Option<Subscription>, ProviderError> {
        #[derive(Deserialize)]
        struct Answer {
            #[serde(default)]
            id: Option<String>,
            #[serde(default, rename = "expirationDateTime")]
            expiration: Option<String>,
        }
        let bytes = serde_json::to_vec(body)
            .map_err(|e| ProviderError::Unexpected(unreadable("a subscription", &e)))?;
        let response = request
            // Graph checks the notification URL before it answers.
            .timeout(SUBSCRIBE_TIMEOUT)
            .bearer_auth(token)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(bytes)
            .send()
            .await
            .map_err(transport)?;
        let status = response.status();
        let headers = response.headers().clone();
        let answer = bounded(response, MAX_ANSWER_BYTES).await?;
        if !status.is_success() {
            return match graph_failure(status, &headers, &answer) {
                ProviderError::NotFound => Ok(None),
                other => Err(other),
            };
        }
        let answer: Answer = serde_json::from_slice(&answer)
            .map_err(|e| ProviderError::Unexpected(unreadable("a Graph subscription", &e)))?;
        let id = answer
            .id
            .filter(|id| valid_subscription_id(id))
            .ok_or_else(|| {
                ProviderError::Unexpected("Graph named no subscription id".to_owned())
            })?;
        let expires = answer
            .expiration
            .as_deref()
            .and_then(|at| DateTime::parse_from_rfc3339(at).ok())
            .map(|at| at.with_timezone(&Utc))
            .ok_or_else(|| ProviderError::Unexpected("Graph named no expiry".to_owned()))?;
        Ok(Some(Subscription { id, expires }))
    }
}

/// The longest a `$value` download may take.
pub const RAW_TIMEOUT: std::time::Duration = std::time::Duration::from_mins(2);

/// The longest a subscription call may take: Graph checks the notification
/// URL within it (up to ten seconds).
pub const SUBSCRIBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Whether a delta's error answer says Graph no longer has its state.
fn expired(status: reqwest::StatusCode, body: &[u8]) -> bool {
    #[derive(Deserialize)]
    struct Answer {
        error: Detail,
    }
    #[derive(Deserialize)]
    struct Detail {
        #[serde(default)]
        code: String,
    }
    if status == reqwest::StatusCode::GONE {
        return true;
    }
    serde_json::from_slice::<Answer>(body).is_ok_and(|answer| {
        let code = safe_code(&answer.error.code);
        EXPIRED_CODES
            .iter()
            .any(|expired| code.eq_ignore_ascii_case(expired))
    })
}

/// A delta page, as Graph answers it.
#[derive(Deserialize)]
struct DeltaAnswer {
    #[serde(default)]
    value: Vec<ItemAnswer>,
    #[serde(default, rename = "@odata.nextLink")]
    next_link: Option<String>,
    #[serde(default, rename = "@odata.deltaLink")]
    delta_link: Option<String>,
}

impl DeltaAnswer {
    fn page(self) -> Result<DeltaPage, ProviderError> {
        let next = match (self.next_link.as_deref(), self.delta_link.as_deref()) {
            (Some(link), _) => token_of(link, "$skiptoken").map(Next::More),
            (None, Some(link)) => token_of(link, "$deltatoken").map(Next::Done),
            (None, None) => None,
        }
        .ok_or_else(|| {
            ProviderError::Unexpected("a Graph delta page has no usable link".to_owned())
        })?;
        let items = self
            .value
            .into_iter()
            .filter_map(|item| {
                let id = item.id.clone().filter(|id| valid_id(id))?;
                let state = item.removed.is_none().then(|| item.state());
                Some(DeltaItem { id, state })
            })
            .collect();
        Ok(DeltaPage { items, next })
    }
}

/// The token `name` of Graph's `link`: only the token is kept, never the
/// link (an answer never chooses where a request goes).
fn token_of(link: &str, name: &str) -> Option<String> {
    let link = Url::parse(link).ok()?;
    let token = link
        .query_pairs()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.into_owned())?;
    (!token.is_empty() && token.len() <= MAX_TOKEN && token.chars().all(|c| c.is_ascii_graphic()))
        .then_some(token)
}

/// A message, as Graph answers it.
#[derive(Deserialize)]
struct ItemAnswer {
    #[serde(default)]
    id: Option<String>,
    #[serde(default, rename = "@removed")]
    removed: Option<serde::de::IgnoredAny>,
    #[serde(default, rename = "parentFolderId")]
    parent_folder_id: Option<String>,
    #[serde(default, rename = "conversationId")]
    conversation_id: Option<String>,
    #[serde(default, rename = "receivedDateTime")]
    received: Option<String>,
    #[serde(default, rename = "isRead")]
    is_read: Option<bool>,
    #[serde(default, rename = "isDraft")]
    is_draft: Option<bool>,
    #[serde(default)]
    flag: Option<FlagAnswer>,
    #[serde(default)]
    categories: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct FlagAnswer {
    #[serde(default, rename = "flagStatus")]
    flag_status: Option<String>,
}

impl ItemAnswer {
    fn state(self) -> MessageState {
        let mut categories: Vec<String> = self
            .categories
            .unwrap_or_default()
            .into_iter()
            .map(|name| {
                name.chars()
                    .filter(|c| !c.is_control())
                    .take(MAX_CATEGORY)
                    .collect::<String>()
                    .trim()
                    .to_owned()
            })
            .filter(|name| !name.is_empty())
            .take(MAX_CATEGORIES)
            .collect();
        categories.sort();
        categories.dedup();
        MessageState {
            folder: self.parent_folder_id.filter(|id| valid_id(id)),
            conversation: self.conversation_id.filter(|id| valid_id(id)),
            received: self
                .received
                .as_deref()
                .and_then(|at| DateTime::parse_from_rfc3339(at).ok())
                .map(|at| at.with_timezone(&Utc)),
            read: self.is_read.unwrap_or(true),
            draft: self.is_draft.unwrap_or(false),
            flagged: self
                .flag
                .and_then(|flag| flag.flag_status)
                .is_some_and(|status| status.eq_ignore_ascii_case("flagged")),
            categories,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_links_token_is_kept() {
        assert_eq!(
            token_of(
                "https://evil.example.com/v1.0/me/mailFolders('x')/messages/delta?$skiptoken=abc-1",
                "$skiptoken"
            )
            .as_deref(),
            Some("abc-1")
        );
        assert_eq!(
            token_of(
                "https://graph.microsoft.com/v1.0/x?%24deltatoken=d%3D1",
                "$deltatoken"
            )
            .as_deref(),
            Some("d=1")
        );
        assert_eq!(
            token_of("https://graph.microsoft.com/v1.0/x?$top=5", "$skiptoken"),
            None
        );
        assert_eq!(token_of("not a url", "$skiptoken"), None);
        assert_eq!(
            token_of(
                "https://graph.microsoft.com/x?$skiptoken=a%0Ab",
                "$skiptoken"
            ),
            None
        );
    }

    #[test]
    fn ids_are_checked() {
        assert!(valid_id("AAMkAGI2TG93AAA="));
        assert!(valid_id(
            "AQMkADAwATM0MDAAMS1iNTcwLWI2NTEtMDACLTAwCgAuAAAD-_x"
        ));
        assert!(!valid_id(""));
        assert!(!valid_id("a b"));
        assert!(!valid_id("../me"));
        assert!(!valid_id(".."));
        assert!(!valid_id("a/../b"));
        assert!(!valid_id(&"a".repeat(MAX_ID + 1)));
        assert!(valid_subscription_id(
            "7f105c7d-2dc5-4530-97cd-4e7ae6534c07"
        ));
        assert!(!valid_subscription_id("7f105c7d/2dc5"));
    }

    #[test]
    fn a_page_reads_removed_and_present_messages() {
        let answer: DeltaAnswer = serde_json::from_value(serde_json::json!({
            "value": [
                {"id": "m1", "parentFolderId": "f1", "conversationId": "c1",
                 "receivedDateTime": "2026-10-20T10:00:00Z", "isRead": false, "isDraft": false,
                 "flag": {"flagStatus": "flagged"}, "categories": ["Clients", " Clients ", "\u{7}"]},
                {"id": "m2", "@removed": {"reason": "deleted"}},
                {"id": "bad id"}
            ],
            "@odata.deltaLink": "https://graph.microsoft.com/v1.0/me/mailFolders/f1/messages/delta?$deltatoken=t1"
        }))
        .unwrap();
        let page = answer.page().unwrap();
        assert_eq!(page.next, Next::Done("t1".to_owned()));
        assert_eq!(page.items.len(), 2);
        let state = page.items[0].state.clone().unwrap();
        assert!(!state.read && state.flagged && !state.draft);
        assert_eq!(state.categories, ["Clients"]);
        assert_eq!(state.folder.as_deref(), Some("f1"));
        assert_eq!(
            page.items[1],
            DeltaItem {
                id: "m2".to_owned(),
                state: None
            }
        );
    }

    #[test]
    fn expired_delta_state_is_recognised() {
        let body =
            |code: &str| serde_json::to_vec(&serde_json::json!({"error": {"code": code}})).unwrap();
        assert!(expired(reqwest::StatusCode::GONE, b""));
        assert!(expired(
            reqwest::StatusCode::BAD_REQUEST,
            &body("SyncStateNotFound")
        ));
        assert!(expired(
            reqwest::StatusCode::BAD_REQUEST,
            &body("resyncRequired")
        ));
        assert!(!expired(
            reqwest::StatusCode::BAD_REQUEST,
            &body("ErrorInvalidRequest")
        ));
    }
}
