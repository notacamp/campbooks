//! Notion: the OAuth token endpoint and the REST API, as this module calls
//! them. The only code here that speaks HTTP to Notion (campbooks-cloud#376).
//!
//! The consent is this module's own (`features/notion_connect`): Notion
//! returns no ID token, so it is never a way in, and the authorization-code
//! flow runs here. This client exchanges the code, refreshes a connection's
//! tokens (Notion rotates the refresh token: every answer brings a new
//! pair), searches the pages the member shared, reads a page's blocks and
//! changes them (`blocks.rs` is the neutral form, `read.rs` reads a page,
//! `write.rs` makes the changes).
//!
//! The host is a constant (`api.notion.com`), the API version is pinned, and
//! nothing in a request or an answer builds a URL: ids are typed UUIDs as
//! path segments, a cursor is a query value, and of an answer only what is
//! parsed is kept. The one address an answer does name is a file Notion
//! hosts (signed, expiring within the hour); it is fetched only from the
//! hosts in [`FILE_HOSTS`], over `https`, with no credentials, no redirect
//! and at most [`MAX_FILE_BYTES`] of an allowed type. Calls follow no
//! redirect, time out after [`CALL_TIMEOUT`] and read at most
//! [`MAX_ANSWER_BYTES`] (a listing [`LIST_ANSWER_BYTES`]). Errors never
//! carry a token, a code, an address or an answer's words: a status and
//! Notion's short error code at most.

pub mod blocks;
pub mod changes;
pub mod pace;
pub mod read;
pub mod write;

use std::time::Duration;

use chrono::{DateTime, Utc};
use reqwest::Method;
use secrecy::ExposeSecret as _;
use serde::Deserialize;
use serde_json::{Value, json};
use url::Url;
use uuid::Uuid;

use crate::provider::{
    ClientSecret, ProviderError, TokenGrant, http_client, retry_after, transport, unreadable,
};

/// Notion's API, up to its version.
pub const API_BASE: &str = "https://api.notion.com/v1/";

/// The API version every call asks for.
pub const VERSION: &str = "2026-03-11";

/// The longest one call to Notion may take.
pub const CALL_TIMEOUT: Duration = Duration::from_secs(15);

/// The largest answer read from Notion (a token answer or one block is a
/// few kilobytes).
pub const MAX_ANSWER_BYTES: usize = 256 * 1024;

/// The largest listing read (a page of block children or of search results).
pub const LIST_ANSWER_BYTES: usize = 1024 * 1024;

/// The longest `Retry-After` honoured: a longer one waits this long.
pub const MAX_RETRY_AFTER: Duration = Duration::from_hours(1);

/// The largest file downloaded from Notion.
pub const MAX_FILE_BYTES: usize = 20 * 1024 * 1024;

/// The hosts Notion serves its files from. Anything else is not fetched.
pub const FILE_HOSTS: [&str; 4] = [
    "prod-files-secure.s3.us-west-2.amazonaws.com",
    "s3.us-west-2.amazonaws.com",
    "file.notion.so",
    "img.notion.so",
];

/// Where a member's browser is sent to start connecting (under this
/// instance), and where Notion brings it back by default.
pub const START_PATH: &str = "/auth/start/notion";

/// The default redirect path registered with Notion.
pub const CALLBACK_PATH: &str = "/auth/callback/notion";

/// Where Notion posts its webhook events, under the instance's public URL.
pub const HOOK_PATH: &str = "/hooks/notion";

/// The media types of files that come into Pages from Notion.
const FILE_TYPES: [&str; 15] = [
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/webp",
    "application/pdf",
    "text/plain",
    "text/csv",
    "application/zip",
    "application/json",
    "application/msword",
    "application/vnd.ms-excel",
    "application/vnd.ms-powerpoint",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "application/vnd.openxmlformats-officedocument.presentationml.presentation",
];

/// Whether a file of media type `content_type` (without parameters, lower
/// case) comes into Pages.
pub fn file_type_allowed(content_type: &str) -> bool {
    FILE_TYPES.contains(&content_type)
}

/// Where files may be fetched from.
#[derive(Debug, Clone, PartialEq, Eq)]
enum FileHosts {
    /// Notion's own: [`FILE_HOSTS`], over `https`.
    Notion,
    /// A fake in tests and development: this `host:port`, over any scheme.
    Fake(String),
}

/// Where Notion's services live.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoints {
    /// The REST API, up to `/v1/`.
    pub api: Url,
    /// The page a member is sent to consent on.
    pub authorize: Url,
    files: FileHosts,
}

impl Endpoints {
    /// Notion's production endpoints.
    pub fn notion() -> Result<Self, String> {
        Self::laid_out(API_BASE, FileHosts::Notion)
    }

    /// Every endpoint under `base`, laid out like Notion's (a fake in tests
    /// and development): `base/v1/…`, and files served from `base` itself.
    pub fn under(base: &Url) -> Result<Self, String> {
        let host = base
            .host_str()
            .ok_or_else(|| "the Notion base address has no host".to_owned())?;
        let authority = match base.port_or_known_default() {
            Some(port) => format!("{host}:{port}"),
            None => host.to_owned(),
        };
        let root = base.as_str().trim_end_matches('/');
        Self::laid_out(&format!("{root}/v1/"), FileHosts::Fake(authority))
    }

    fn laid_out(api: &str, files: FileHosts) -> Result<Self, String> {
        let parse = |url: &str| Url::parse(url).map_err(|e| format!("{url}: {e}"));
        Ok(Self {
            api: parse(api)?,
            authorize: parse(&format!("{api}oauth/authorize"))?,
            files,
        })
    }

    /// Whether a file at `url` may be fetched.
    fn allows_file(&self, url: &Url) -> bool {
        if !url.username().is_empty() || url.password().is_some() {
            return false;
        }
        match &self.files {
            FileHosts::Notion => {
                url.scheme() == "https"
                    && matches!(url.port(), None | Some(443))
                    && url
                        .host_str()
                        .is_some_and(|host| FILE_HOSTS.contains(&host))
            }
            FileHosts::Fake(authority) => {
                let host = url.host_str().unwrap_or_default();
                let found = match url.port_or_known_default() {
                    Some(port) => format!("{host}:{port}"),
                    None => host.to_owned(),
                };
                matches!(url.scheme(), "http" | "https") && found == *authority
            }
        }
    }
}

/// What Notion answered for a code: the tokens and who they are for.
#[derive(Debug, Clone)]
pub struct Granted {
    /// The access token and the refresh token, with their lifetime.
    pub tokens: TokenGrant,
    /// The authorization's bot, Notion's id of this grant: the connection's
    /// account.
    pub bot_id: String,
    /// The Notion workspace's name, when it gives one.
    pub workspace_name: Option<String>,
}

/// A page, as search and `GET /pages` give it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageInfo {
    /// The page.
    pub id: Uuid,
    /// Its title, without control characters, at most 200 characters.
    pub title: String,
    /// When Notion last edited it.
    pub edited: DateTime<Utc>,
    /// It is in the trash.
    pub trashed: bool,
    /// What it is under.
    pub parent: Parent,
}

/// What a page is under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Parent {
    /// The top level of the workspace.
    Workspace,
    /// Another page.
    Page(Uuid),
    /// A block of another page.
    Block,
    /// A database (a row, not a page).
    Database,
}

/// One page of search results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchPage {
    /// The pages, those that are not rows of a database.
    pub pages: Vec<PageInfo>,
    /// The cursor of the next page.
    pub next: Option<String>,
}

/// One page of a block's children.
#[derive(Debug, Clone, PartialEq)]
pub struct ChildrenPage {
    /// The blocks, as Notion gives them.
    pub blocks: Vec<Value>,
    /// The cursor of the next page.
    pub next: Option<String>,
}

/// A downloaded file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Downloaded {
    /// Its bytes.
    pub bytes: Vec<u8>,
    /// Its media type, lower case, without parameters.
    pub content_type: String,
}

/// The Notion integration this instance is registered as.
#[derive(Debug, Clone)]
pub struct Notion {
    http: reqwest::Client,
    client_id: String,
    client_secret: ClientSecret,
    endpoints: Endpoints,
    redirect_uri: String,
}

impl Notion {
    /// A client with id `client_id` and secret `client_secret`, talking to
    /// `endpoints`, asking Notion to send members back to `redirect_uri`.
    pub fn new(
        client_id: String,
        client_secret: ClientSecret,
        endpoints: Endpoints,
        redirect_uri: String,
    ) -> Result<Self, String> {
        Ok(Self {
            http: http_client("Notion")?,
            client_id,
            client_secret,
            endpoints,
            redirect_uri,
        })
    }

    /// Where Notion sends members back to.
    pub fn redirect_uri(&self) -> &str {
        &self.redirect_uri
    }

    /// The address a member consents at: `state` is ours to check on the way
    /// back, `challenge` the PKCE challenge of the verifier kept for the
    /// exchange (Notion ignores it today).
    pub fn authorize_url(&self, state: &str, challenge: &str) -> Url {
        let mut url = self.endpoints.authorize.clone();
        url.query_pairs_mut()
            .append_pair("client_id", &self.client_id)
            .append_pair("response_type", "code")
            .append_pair("owner", "user")
            .append_pair("redirect_uri", &self.redirect_uri)
            .append_pair("state", state)
            .append_pair("code_challenge", challenge)
            .append_pair("code_challenge_method", "S256");
        url
    }

    /// The integration's secret now (a file is read each time, so a rotated
    /// secret needs no restart).
    fn secret(&self) -> Result<secrecy::SecretString, ProviderError> {
        self.client_secret
            .read("CAMPBOOKS_NOTION__CLIENT_SECRET_FILE")
    }

    /// Exchange the `code` Notion sent back for the grant's tokens.
    pub async fn exchange(&self, code: &str, verifier: &str) -> Result<Granted, ProviderError> {
        let answer = self
            .token_call(&json!({
                "grant_type": "authorization_code",
                "code": code,
                "redirect_uri": self.redirect_uri,
                "code_verifier": verifier,
            }))
            .await?;
        let parsed: TokenAnswer = serde_json::from_slice(&answer)
            .map_err(|e| ProviderError::Unexpected(unreadable("the token answer", &e)))?;
        let tokens = parsed.tokens()?;
        let bot_id = parsed
            .bot_id
            .filter(|id| !id.is_empty() && id.len() <= 64 && id.is_ascii())
            .ok_or_else(|| ProviderError::Unexpected("the token answer names no bot".into()))?;
        let workspace_name = parsed
            .workspace_name
            .map(|name| {
                name.chars()
                    .filter(|c| !c.is_control())
                    .take(120)
                    .collect::<String>()
                    .trim()
                    .to_owned()
            })
            .filter(|name| !name.is_empty());
        Ok(Granted {
            tokens,
            bot_id,
            workspace_name,
        })
    }

    /// A fresh pair of tokens for `refresh_token`. Notion rotates the
    /// refresh token: the answer's replaces it.
    pub async fn refresh(&self, refresh_token: &str) -> Result<TokenGrant, ProviderError> {
        let answer = self
            .token_call(&json!({
                "grant_type": "refresh_token",
                "refresh_token": refresh_token,
            }))
            .await?;
        let parsed: TokenAnswer = serde_json::from_slice(&answer)
            .map_err(|e| ProviderError::Unexpected(unreadable("the token answer", &e)))?;
        parsed.tokens()
    }

    /// POST the token endpoint with the integration's credentials.
    async fn token_call(&self, body: &Value) -> Result<Vec<u8>, ProviderError> {
        let secret = self.secret()?;
        let url = self.api_url(&["oauth", "token"])?;
        let response = self
            .http
            .post(url)
            .timeout(CALL_TIMEOUT)
            .basic_auth(&self.client_id, Some(secret.expose_secret()))
            .header(reqwest::header::ACCEPT, "application/json")
            .header("notion-version", VERSION)
            .json(body)
            .send()
            .await
            .map_err(transport)?;
        let status = response.status();
        let headers = response.headers().clone();
        let answer = bounded(response, MAX_ANSWER_BYTES).await?;
        if !status.is_success() {
            return Err(token_failure(status, &headers, &answer));
        }
        Ok(answer)
    }

    /// The API's URL of `segments`, each a path segment (encoded).
    fn api_url(&self, segments: &[&str]) -> Result<Url, ProviderError> {
        let mut url = self.endpoints.api.clone();
        url.path_segments_mut()
            .map_err(|()| ProviderError::Unexpected("the Notion URL has no path".to_owned()))?
            .pop_if_empty()
            .extend(segments);
        Ok(url)
    }

    /// One call to the API: its answer as JSON, or what the failure means.
    async fn call(
        &self,
        method: Method,
        url: Url,
        token: &str,
        body: Option<&Value>,
        limit: usize,
    ) -> Result<Value, ProviderError> {
        let mut request = self
            .http
            .request(method, url)
            .timeout(CALL_TIMEOUT)
            .bearer_auth(token)
            .header(reqwest::header::ACCEPT, "application/json")
            .header("notion-version", VERSION);
        if let Some(body) = body {
            request = request.json(body);
        }
        let response = request.send().await.map_err(transport)?;
        let status = response.status();
        let headers = response.headers().clone();
        let answer = bounded(response, limit).await?;
        if !status.is_success() {
            return Err(api_failure(status, &headers, &answer));
        }
        serde_json::from_slice(&answer)
            .map_err(|e| ProviderError::Unexpected(unreadable("a Notion answer", &e)))
    }

    /// The pages `token` can see, most recently edited first, a hundred at a
    /// time. Rows of databases are left out (they wait for Tables).
    pub async fn search(
        &self,
        token: &str,
        cursor: Option<&str>,
    ) -> Result<SearchPage, ProviderError> {
        let mut body = json!({
            "filter": {"property": "object", "value": "page"},
            "sort": {"direction": "descending", "timestamp": "last_edited_time"},
            "page_size": 100,
        });
        if let Some(cursor) = cursor {
            body["start_cursor"] = Value::String(cursor.to_owned());
        }
        let url = self.api_url(&["search"])?;
        let answer = self
            .call(Method::POST, url, token, Some(&body), LIST_ANSWER_BYTES)
            .await?;
        let pages = answer
            .get("results")
            .and_then(Value::as_array)
            .map(|items| items.iter().filter_map(page_info).collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .filter(|page| page.parent != Parent::Database)
            .collect();
        Ok(SearchPage {
            pages,
            next: next_cursor(&answer),
        })
    }

    /// Page `id`.
    pub async fn page(&self, token: &str, id: Uuid) -> Result<PageInfo, ProviderError> {
        let url = self.api_url(&["pages", &id.to_string()])?;
        let answer = self
            .call(Method::GET, url, token, None, MAX_ANSWER_BYTES)
            .await?;
        page_info(&answer)
            .ok_or_else(|| ProviderError::Unexpected("a Notion page did not parse".into()))
    }

    /// One page of block `id`'s children (a page is a block too).
    pub async fn children(
        &self,
        token: &str,
        id: Uuid,
        cursor: Option<&str>,
    ) -> Result<ChildrenPage, ProviderError> {
        let mut url = self.api_url(&["blocks", &id.to_string(), "children"])?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("page_size", "100");
            if let Some(cursor) = cursor {
                query.append_pair("start_cursor", cursor);
            }
        }
        let answer = self
            .call(Method::GET, url, token, None, LIST_ANSWER_BYTES)
            .await?;
        let blocks = answer
            .get("results")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        Ok(ChildrenPage {
            blocks,
            next: next_cursor(&answer),
        })
    }

    /// Block `id`, as Notion gives it.
    pub async fn block(&self, token: &str, id: Uuid) -> Result<Value, ProviderError> {
        let url = self.api_url(&["blocks", &id.to_string()])?;
        self.call(Method::GET, url, token, None, MAX_ANSWER_BYTES)
            .await
    }

    /// Rewrite block `id` with `body` (`{"<type>": {...}}`); the block as it
    /// is now.
    pub async fn update_block(
        &self,
        token: &str,
        id: Uuid,
        body: &Value,
    ) -> Result<Value, ProviderError> {
        let url = self.api_url(&["blocks", &id.to_string()])?;
        self.call(Method::PATCH, url, token, Some(body), MAX_ANSWER_BYTES)
            .await
    }

    /// Add `children` under block (or page) `parent`, after block `after`
    /// (first when none), by the `position` object of API version
    /// 2026-03-11 (`after_block` or `start`); the blocks Notion made, in
    /// order.
    pub async fn append(
        &self,
        token: &str,
        parent: Uuid,
        children: &[Value],
        after: Option<Uuid>,
    ) -> Result<Vec<Value>, ProviderError> {
        let url = self.api_url(&["blocks", &parent.to_string(), "children"])?;
        let mut body = json!({"children": children});
        body["position"] = match after {
            Some(after) => json!({"type": "after_block", "after_block": {"id": after}}),
            None => json!({"type": "start"}),
        };
        let answer = self
            .call(Method::PATCH, url, token, Some(&body), LIST_ANSWER_BYTES)
            .await?;
        Ok(answer
            .get("results")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default())
    }

    /// Move block `id` to the trash. Only for a block this module made the
    /// pages' counterpart of: a page is never trashed (`write.rs` refuses a
    /// block that is one before it asks).
    pub async fn delete_block(&self, token: &str, id: Uuid) -> Result<Value, ProviderError> {
        let url = self.api_url(&["blocks", &id.to_string()])?;
        self.call(Method::DELETE, url, token, None, MAX_ANSWER_BYTES)
            .await
    }

    /// The file at `url`, when it is on one of Notion's file hosts, within
    /// [`MAX_FILE_BYTES`] and of an allowed type; `None` otherwise, and when
    /// the address has expired.
    pub async fn download(&self, url: &str) -> Result<Option<Downloaded>, ProviderError> {
        let Ok(url) = Url::parse(url) else {
            return Ok(None);
        };
        if !self.endpoints.allows_file(&url) {
            tracing::info!("a Notion file on an unlisted host was not fetched");
            return Ok(None);
        }
        let response = self
            .http
            .get(url)
            .timeout(CALL_TIMEOUT.saturating_mul(4))
            .send()
            .await
            .map_err(transport)?;
        let status = response.status();
        if matches!(status.as_u16(), 403 | 404 | 410) {
            return Ok(None);
        }
        if !status.is_success() {
            let headers = response.headers().clone();
            return Err(api_failure(status, &headers, b""));
        }
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(|value| {
                value
                    .split(';')
                    .next()
                    .unwrap_or_default()
                    .trim()
                    .to_ascii_lowercase()
            })
            .unwrap_or_default();
        if !file_type_allowed(&content_type) {
            return Ok(None);
        }
        Ok(within(response, MAX_FILE_BYTES)
            .await?
            .map(|bytes| Downloaded {
                bytes,
                content_type,
            }))
    }
}

/// The tokens answer, as Notion gives it.
#[derive(Deserialize)]
struct TokenAnswer {
    #[serde(default)]
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
    #[serde(default)]
    bot_id: Option<String>,
    #[serde(default)]
    workspace_name: Option<String>,
}

impl TokenAnswer {
    fn tokens(&self) -> Result<TokenGrant, ProviderError> {
        if self.access_token.is_empty() {
            return Err(ProviderError::Unexpected(
                "the token answer has no access token".to_owned(),
            ));
        }
        Ok(TokenGrant {
            access_token: self.access_token.clone(),
            expires_in: self.expires_in,
            refresh_token: self.refresh_token.clone().filter(|token| !token.is_empty()),
            scope: None,
        })
    }
}

/// A page object, as much of it as is used.
fn page_info(raw: &Value) -> Option<PageInfo> {
    if raw.get("object").and_then(Value::as_str) != Some("page") {
        return None;
    }
    let id = raw
        .get("id")
        .and_then(Value::as_str)
        .and_then(|id| Uuid::parse_str(id).ok())?;
    let edited = raw
        .get("last_edited_time")
        .and_then(Value::as_str)
        .and_then(|time| DateTime::parse_from_rfc3339(time).ok())?
        .with_timezone(&Utc);
    let flag = |key: &str| raw.get(key).and_then(Value::as_bool).unwrap_or(false);
    let parent = match raw.pointer("/parent/type").and_then(Value::as_str) {
        Some("workspace") => Parent::Workspace,
        Some("page_id") => raw
            .pointer("/parent/page_id")
            .and_then(Value::as_str)
            .and_then(|id| Uuid::parse_str(id).ok())
            .map_or(Parent::Block, Parent::Page),
        Some("database_id" | "data_source_id") => Parent::Database,
        _ => Parent::Block,
    };
    let title = raw
        .get("properties")
        .and_then(Value::as_object)
        .and_then(|properties| {
            properties
                .values()
                .find(|property| property.get("type").and_then(Value::as_str) == Some("title"))
        })
        .and_then(|property| property.get("title"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("plain_text").and_then(Value::as_str))
                .collect::<String>()
        })
        .unwrap_or_default();
    Some(PageInfo {
        id,
        title: campbooks_kernel::text::displayable(&title)
            .trim()
            .chars()
            .take(200)
            .collect(),
        edited,
        trashed: flag("in_trash"),
        parent,
    })
}

/// A listing's cursor for the next page, when there is one.
fn next_cursor(answer: &Value) -> Option<String> {
    if answer.get("has_more").and_then(Value::as_bool) != Some(true) {
        return None;
    }
    answer
        .get("next_cursor")
        .and_then(Value::as_str)
        .filter(|cursor| !cursor.is_empty() && cursor.len() <= 512)
        .map(ToOwned::to_owned)
}

/// An answer's body, refused above `limit` bytes.
async fn bounded(response: reqwest::Response, limit: usize) -> Result<Vec<u8>, ProviderError> {
    within(response, limit)
        .await?
        .ok_or_else(|| ProviderError::Unexpected("the answer is too large".to_owned()))
}

/// An answer's body, or `None` once it is over `limit` bytes (stated or
/// sent): what is over is never read.
async fn within(
    mut response: reqwest::Response,
    limit: usize,
) -> Result<Option<Vec<u8>>, ProviderError> {
    if response
        .content_length()
        .is_some_and(|length| length > u64::try_from(limit).unwrap_or(u64::MAX))
    {
        return Ok(None);
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(transport)? {
        if body.len().saturating_add(chunk.len()) > limit {
            return Ok(None);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(Some(body))
}

/// The `Retry-After` of an answer, at most [`MAX_RETRY_AFTER`].
fn capped_retry_after(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    retry_after(headers).map(|wait| wait.min(MAX_RETRY_AFTER))
}

/// A short, safe code from an answer (letters, digits and `_` only, at most
/// 40): never more of the answer than that.
fn safe_code(code: &str) -> String {
    code.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .take(40)
        .collect()
}

/// Notion's error answer, as much as is used: the API's `code` or the token
/// endpoint's `error`.
#[derive(Deserialize, Default)]
struct ErrorAnswer {
    #[serde(default)]
    code: String,
    #[serde(default)]
    error: String,
}

impl ErrorAnswer {
    fn code(&self) -> String {
        if self.code.is_empty() {
            safe_code(&self.error)
        } else {
            safe_code(&self.code)
        }
    }
}

/// What a token endpoint's non-success answer means.
fn token_failure(
    status: reqwest::StatusCode,
    headers: &reqwest::header::HeaderMap,
    body: &[u8],
) -> ProviderError {
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return ProviderError::RateLimited {
            retry_after: capped_retry_after(headers),
        };
    }
    let code = serde_json::from_slice::<ErrorAnswer>(body)
        .unwrap_or_default()
        .code();
    if code == "invalid_grant" {
        return ProviderError::InvalidGrant;
    }
    if status.is_server_error() {
        return ProviderError::Unavailable(format!("HTTP {}", status.as_u16()));
    }
    ProviderError::Unexpected(if code.is_empty() {
        format!("HTTP {}", status.as_u16())
    } else {
        format!("HTTP {} ({code})", status.as_u16())
    })
}

/// What an API non-success answer means.
fn api_failure(
    status: reqwest::StatusCode,
    headers: &reqwest::header::HeaderMap,
    body: &[u8],
) -> ProviderError {
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return ProviderError::RateLimited {
            retry_after: capped_retry_after(headers),
        };
    }
    match status {
        reqwest::StatusCode::UNAUTHORIZED => ProviderError::Unauthorized,
        // A page or block the integration cannot reach is the same as one
        // that is not there: Notion answers 404 `object_not_found`. A 403
        // (`restricted_resource`) may be a capability the integration lacks,
        // not a page that is no longer shared: it is trouble, never a
        // reason to end a link.
        reqwest::StatusCode::NOT_FOUND => ProviderError::NotFound,
        // Notion could not take the write now (`conflict_error`).
        reqwest::StatusCode::CONFLICT => ProviderError::Unavailable("HTTP 409".to_owned()),
        s if s.is_server_error() => ProviderError::Unavailable(format!("HTTP {}", s.as_u16())),
        s => {
            let code = serde_json::from_slice::<ErrorAnswer>(body)
                .unwrap_or_default()
                .code();
            ProviderError::Unexpected(if code.is_empty() {
                format!("HTTP {}", s.as_u16())
            } else {
                format!("HTTP {} ({code})", s.as_u16())
            })
        }
    }
}

#[cfg(test)]
mod tests;
