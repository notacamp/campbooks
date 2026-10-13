//! The Zoho Mail REST calls the adapter makes, for one access token and
//! one mail account. Answers are `{ "status": {…}, "data": … }`; ids are
//! 64-bit and come as numbers or strings, so they are kept as strings and
//! sent back as numbers.

use serde::{Deserialize, Deserializer, de::DeserializeOwned};
use url::Url;

use super::html;
use crate::provider::{ProviderError, retry_after, transport};

/// The Mail API for one access token and mail account.
#[derive(Debug, Clone, Copy)]
pub struct ZohoMail<'a> {
    http: &'a reqwest::Client,
    base: &'a Url,
    token: &'a str,
    account: &'a str,
}

/// A mail account of the login (`GET accounts`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailAccount {
    /// Zoho's id of the mail account.
    #[serde(deserialize_with = "id")]
    pub account_id: String,
    /// Its primary address.
    #[serde(default)]
    pub primary_email_address: Option<String>,
    /// The mailbox's address.
    #[serde(default)]
    pub mailbox_address: Option<String>,
    /// The name shown for it.
    #[serde(default)]
    pub display_name: Option<String>,
    /// Another name Zoho gives it.
    #[serde(default)]
    pub account_display_name: Option<String>,
    /// `ZOHO_ACCOUNT` for a mailbox Zoho hosts; other kinds fetch an
    /// outside mailbox into Zoho.
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
}

impl MailAccount {
    /// Whether Zoho hosts this mailbox (and so vouches for its address).
    pub fn is_zoho_mailbox(&self) -> bool {
        self.kind.as_deref() == Some("ZOHO_ACCOUNT")
    }

    /// Its address, lower-cased.
    pub fn address(&self) -> Option<String> {
        self.primary_email_address
            .as_deref()
            .or(self.mailbox_address.as_deref())
            .map(|a| html::unescape(a).trim().to_lowercase())
            .filter(|a| a.contains('@'))
    }

    /// Its display name.
    pub fn name(&self) -> Option<String> {
        self.display_name
            .as_deref()
            .or(self.account_display_name.as_deref())
            .map(|n| html::unescape(n).trim().to_owned())
            .filter(|n| !n.is_empty())
    }
}

/// A folder (`GET accounts/{a}/folders`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    /// Its id.
    #[serde(deserialize_with = "id", rename = "folderId")]
    pub id: String,
    /// `Inbox`, `Drafts`, `Templates`, `Snoozed`, `Sent`, `Spam`, `Trash`,
    /// `Outbox`, or another kind for the member's own.
    #[serde(default, rename = "folderType")]
    pub kind: String,
    /// Its path (`/Inbox`).
    #[serde(default)]
    pub path: String,
}

/// A message as a listing (or `details`) describes it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Listed {
    /// Its id.
    #[serde(deserialize_with = "id")]
    pub message_id: String,
    /// Its folder.
    #[serde(deserialize_with = "id")]
    pub folder_id: String,
    /// When Zoho received it, epoch milliseconds.
    #[serde(
        default,
        deserialize_with = "number",
        rename = "receivedTime",
        alias = "receivedtime"
    )]
    pub received_time: Option<i64>,
    /// `0`/`unread` or `1`/`read`.
    #[serde(default, deserialize_with = "text")]
    pub status: String,
    /// `info`, `important`, `followup` or `flag_not_set` (or 0–3).
    #[serde(default, deserialize_with = "text")]
    pub flagid: String,
    /// Its size in bytes.
    #[serde(default, deserialize_with = "number")]
    pub size: Option<i64>,
    /// Its subject, HTML-escaped.
    #[serde(default)]
    pub subject: String,
    /// Its recipients, HTML-escaped.
    #[serde(default)]
    pub to_address: String,
}

impl Listed {
    /// Whether it has been read.
    pub fn read(&self) -> bool {
        matches!(self.status.as_str(), "1" | "read")
    }

    /// Whether it carries a flag (any of Zoho's three).
    pub fn flagged(&self) -> bool {
        !matches!(self.flagid.as_str(), "" | "0" | "flag_not_set")
    }

    /// Its subject, decoded.
    pub fn subject(&self) -> String {
        html::unescape(&self.subject).trim().to_owned()
    }

    /// Its recipients' addresses, decoded and lower-cased.
    pub fn recipients(&self) -> Vec<String> {
        html::addresses(&html::unescape(&self.to_address))
    }
}

/// What downloading a raw message gave.
#[derive(Debug, Clone)]
pub enum OriginalFetch {
    /// The RFC 5322 bytes and the bytes downloaded for them.
    Message {
        /// The message.
        raw: Vec<u8>,
        /// Bytes downloaded (the encoded answer).
        fetched_bytes: u64,
    },
    /// It is larger than the limit; the download was abandoned.
    TooLarge,
}

/// What a send or reply answered.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendAnswer {
    /// The sent message's id, when Zoho says.
    #[serde(default, deserialize_with = "optional_id")]
    pub message_id: Option<String>,
}

/// Zoho's answer envelope.
#[derive(Deserialize)]
struct Envelope<T> {
    data: T,
}

/// An error answer's body.
#[derive(Deserialize)]
struct ErrorBody {
    #[serde(default)]
    data: Option<ErrorData>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ErrorData {
    #[serde(default)]
    error_code: Option<String>,
}

impl<'a> ZohoMail<'a> {
    pub(super) fn new(
        http: &'a reqwest::Client,
        base: &'a Url,
        token: &'a str,
        account: &'a str,
    ) -> Self {
        Self {
            http,
            base,
            token,
            account,
        }
    }

    /// `accounts/{a}/{path}` (or `{path}` alone for an empty account).
    fn url(&self, path: &str) -> Result<Url, ProviderError> {
        let path = if self.account.is_empty() {
            path.to_owned()
        } else {
            format!("accounts/{}/{path}", self.account)
        };
        self.base
            .join(path.trim_end_matches('/'))
            .map_err(|e| ProviderError::Unexpected(format!("Zoho URL: {e}")))
    }

    fn request(&self, method: reqwest::Method, url: Url) -> reqwest::RequestBuilder {
        self.http
            .request(method, url)
            .header(
                reqwest::header::AUTHORIZATION,
                format!("Zoho-oauthtoken {}", self.token),
            )
            .header(reqwest::header::ACCEPT, "application/json")
    }

    async fn call<T: DeserializeOwned>(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<T, ProviderError> {
        let response = request.send().await.map_err(transport)?;
        let status = response.status();
        if !status.is_success() {
            return Err(failure(response).await);
        }
        let envelope: Envelope<T> = response
            .json()
            .await
            .map_err(|e| ProviderError::Unexpected(format!("Zoho answer: {}", e.without_url())))?;
        Ok(envelope.data)
    }

    /// The login's mail accounts (`GET accounts`; call on an account-less
    /// client).
    pub async fn accounts(&self) -> Result<Vec<MailAccount>, ProviderError> {
        let url = self.url("accounts")?;
        self.call(self.request(reqwest::Method::GET, url)).await
    }

    /// The account's folders.
    pub async fn folders(&self) -> Result<Vec<Folder>, ProviderError> {
        let url = self.url("folders")?;
        self.call(self.request(reqwest::Method::GET, url)).await
    }

    /// Up to `limit` (≤ 200) messages of `folder` from position `start`
    /// (1-based), newest first; archived ones are not listed.
    pub async fn list(
        &self,
        folder: &str,
        start: u32,
        limit: u32,
    ) -> Result<Vec<Listed>, ProviderError> {
        let mut url = self.url("messages/view")?;
        url.query_pairs_mut()
            .append_pair("folderId", folder)
            .append_pair("start", &start.to_string())
            .append_pair("limit", &limit.min(200).to_string())
            .append_pair("sortorder", "false")
            .append_pair("includeto", "true");
        self.call(self.request(reqwest::Method::GET, url)).await
    }

    /// Message `message` of folder `folder` (where it is, read, flag).
    pub async fn details(&self, folder: &str, message: &str) -> Result<Listed, ProviderError> {
        let url = self.url(&format!("folders/{folder}/messages/{message}/details"))?;
        self.call(self.request(reqwest::Method::GET, url)).await
    }

    /// Message `message` as raw MIME, unless it is larger than `max_bytes`:
    /// the answer is read as it arrives and abandoned once it grows past
    /// the limit.
    pub async fn original(
        &self,
        message: &str,
        max_bytes: u64,
    ) -> Result<OriginalFetch, ProviderError> {
        #[derive(Deserialize)]
        struct Original {
            content: String,
        }
        let url = self.url(&format!("messages/{message}/originalmessage"))?;
        let mut response = self
            .request(reqwest::Method::GET, url)
            .send()
            .await
            .map_err(transport)?;
        if !response.status().is_success() {
            return Err(failure(response).await);
        }
        // JSON escaping can double a message's size; leave room for the
        // envelope.
        let cap = max_bytes.saturating_mul(2) + 64 * 1024;
        let mut body: Vec<u8> = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(transport)? {
            if u64::try_from(body.len() + chunk.len()).unwrap_or(u64::MAX) > cap {
                return Ok(OriginalFetch::TooLarge);
            }
            body.extend_from_slice(&chunk);
        }
        let fetched_bytes = u64::try_from(body.len()).unwrap_or(u64::MAX);
        let envelope: Envelope<Original> = serde_json::from_slice(&body)
            .map_err(|e| ProviderError::Unexpected(format!("raw message answer: {e}")))?;
        let raw = envelope.data.content.into_bytes();
        if u64::try_from(raw.len()).unwrap_or(u64::MAX) > max_bytes {
            return Ok(OriginalFetch::TooLarge);
        }
        Ok(OriginalFetch::Message { raw, fetched_bytes })
    }

    /// `PUT updatemessage` with `mode` for `messages`, plus `extra` fields.
    pub async fn update(
        &self,
        mode: &str,
        messages: &[String],
        extra: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<(), ProviderError> {
        let ids: Vec<serde_json::Value> = messages.iter().map(|id| id_value(id)).collect();
        let mut body = serde_json::Map::new();
        body.insert("mode".into(), mode.into());
        body.insert("messageId".into(), ids.into());
        body.extend(extra.clone());
        let url = self.url("updatemessage")?;
        let response = self
            .request(reqwest::Method::PUT, url)
            .json(&body)
            .send()
            .await
            .map_err(transport)?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(failure(response).await)
        }
    }

    /// Upload one attachment for a message to send: what the send names it
    /// by (`storeName`, `attachmentPath`, `attachmentName`).
    pub async fn upload(
        &self,
        file_name: &str,
        content_type: &str,
        bytes: Vec<u8>,
    ) -> Result<serde_json::Value, ProviderError> {
        let mut url = self.url("messages/attachments")?;
        url.query_pairs_mut().append_pair("fileName", file_name);
        let request = self
            .request(reqwest::Method::POST, url)
            .header(reqwest::header::CONTENT_TYPE, content_type)
            .body(bytes);
        let uploaded: serde_json::Value = self.call(request).await?;
        // One file answers an object; some answers wrap it in a list.
        let uploaded = match uploaded {
            serde_json::Value::Array(mut list) if !list.is_empty() => list.swap_remove(0),
            other => other,
        };
        let mut reference = serde_json::Map::new();
        for key in ["storeName", "attachmentPath", "attachmentName"] {
            let value = uploaded
                .get(key)
                .cloned()
                .ok_or_else(|| ProviderError::Unexpected(format!("upload answer without {key}")))?;
            reference.insert(key.into(), value);
        }
        Ok(serde_json::Value::Object(reference))
    }

    /// Send `message` (`POST messages`), or reply to `parent` with it
    /// (`POST messages/{parent}`, `action: reply`). Never retried here: the
    /// caller decides, knowing whether Zoho may have it. An accepted send
    /// whose answer cannot be read is `Unavailable` (it may be out), never a
    /// refusal.
    pub async fn send(
        &self,
        message: &serde_json::Value,
        parent: Option<&str>,
    ) -> Result<SendAnswer, ProviderError> {
        let (url, body) = match parent {
            Some(parent) => {
                let mut body = message.clone();
                body["action"] = "reply".into();
                (self.url(&format!("messages/{parent}"))?, body)
            }
            None => (self.url("messages")?, message.clone()),
        };
        let response = self
            .request(reqwest::Method::POST, url)
            .json(&body)
            .send()
            .await
            .map_err(transport)?;
        if !response.status().is_success() {
            return Err(failure(response).await);
        }
        let envelope: Envelope<SendAnswer> = response.json().await.map_err(|e| {
            ProviderError::Unavailable(format!("unreadable answer to a send: {}", e.without_url()))
        })?;
        Ok(envelope.data)
    }
}

/// An id as Zoho wants it back: a number when it is one.
fn id_value(id: &str) -> serde_json::Value {
    id.parse::<u64>()
        .map_or_else(|_| id.into(), serde_json::Value::from)
}

/// What a non-success answer means.
async fn failure(response: reqwest::Response) -> ProviderError {
    let status = response.status();
    let headers = response.headers().clone();
    let code = response
        .json::<ErrorBody>()
        .await
        .ok()
        .and_then(|b| b.data)
        .and_then(|d| d.error_code);
    classify(status, Some(&headers), code.as_deref())
}

/// What a non-success status (with Zoho's `errorCode`) means.
pub(super) fn classify(
    status: reqwest::StatusCode,
    headers: Option<&reqwest::header::HeaderMap>,
    code: Option<&str>,
) -> ProviderError {
    let code = code.unwrap_or_default().to_ascii_uppercase();
    let throttled = code.contains("TOO_MANY") || code.contains("THROTTL") || code.contains("LIMIT");
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS || throttled {
        return ProviderError::RateLimited {
            retry_after: headers.and_then(retry_after),
        };
    }
    if code.contains("OAUTHTOKEN") || code.contains("INVALID_TICKET") {
        return ProviderError::Unauthorized;
    }
    match status {
        reqwest::StatusCode::UNAUTHORIZED => ProviderError::Unauthorized,
        reqwest::StatusCode::NOT_FOUND => ProviderError::NotFound,
        s if s.is_server_error() => ProviderError::Unavailable(format!("HTTP {}", s.as_u16())),
        s => ProviderError::Unexpected(format!(
            "HTTP {}{}",
            s.as_u16(),
            if code.is_empty() {
                String::new()
            } else {
                format!(" ({code})")
            }
        )),
    }
}

/// A JSON string or number, as a string.
fn id<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    optional_id(deserializer)?.ok_or_else(|| serde::de::Error::custom("an id is missing"))
}

fn optional_id<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    Ok(match serde_json::Value::deserialize(deserializer)? {
        serde_json::Value::String(s) if !s.is_empty() => Some(s),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    })
}

/// A JSON string or number, as text (empty for anything else).
fn text<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    Ok(optional_id(deserializer)?.unwrap_or_default())
}

/// A JSON number or numeric string.
fn number<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<i64>, D::Error> {
    Ok(optional_id(deserializer)?.and_then(|s| s.trim().parse().ok()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listings_read_as_zoho_writes_them() {
        let listed: Vec<Listed> = serde_json::from_value(serde_json::json!([
            {"messageId": "1709887058769100001", "folderId": "9000000002014",
             "receivedTime": "1709887053409", "status": "1", "flagid": "flag_not_set",
             "size": "1190", "subject": "Re: Or&#231;amento", "toAddress": "&quot;Sam&quot;&lt;sam@example.com&gt;"},
            {"messageId": 1_711_540_357_880_100_000_u64, "folderId": 9_000_000_000_905_u64,
             "receivedtime": 1_425_388_373_920_u64, "status": "unread", "flagid": 2}
        ]))
        .unwrap();
        assert_eq!(listed[0].message_id, "1709887058769100001");
        assert_eq!(listed[0].received_time, Some(1_709_887_053_409));
        assert!(listed[0].read() && !listed[0].flagged());
        assert_eq!(listed[0].subject(), "Re: Orçamento");
        assert_eq!(listed[0].recipients(), ["sam@example.com"]);
        assert_eq!(listed[1].message_id, "1711540357880100000");
        assert_eq!(listed[1].received_time, Some(1_425_388_373_920));
        assert!(!listed[1].read() && listed[1].flagged());
        assert_eq!(
            id_value("1711540357880100000"),
            serde_json::json!(1_711_540_357_880_100_000_u64)
        );
    }

    #[test]
    fn statuses_are_classified() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::RETRY_AFTER, "30".parse().unwrap());
        assert_eq!(
            classify(reqwest::StatusCode::TOO_MANY_REQUESTS, Some(&headers), None),
            ProviderError::RateLimited {
                retry_after: Some(std::time::Duration::from_secs(30))
            }
        );
        assert_eq!(
            classify(
                reqwest::StatusCode::BAD_REQUEST,
                None,
                Some("TOO_MANY_REQUESTS")
            ),
            ProviderError::RateLimited { retry_after: None }
        );
        assert_eq!(
            classify(
                reqwest::StatusCode::BAD_REQUEST,
                None,
                Some("INVALID_OAUTHTOKEN")
            ),
            ProviderError::Unauthorized
        );
        assert_eq!(
            classify(reqwest::StatusCode::NOT_FOUND, None, None),
            ProviderError::NotFound
        );
        assert!(matches!(
            classify(reqwest::StatusCode::BAD_GATEWAY, None, None),
            ProviderError::Unavailable(_)
        ));
        assert!(matches!(
            classify(
                reqwest::StatusCode::BAD_REQUEST,
                None,
                Some("JSON_PARSE_ERROR")
            ),
            ProviderError::Unexpected(_)
        ));
    }

    #[test]
    fn mail_accounts_have_an_address_and_a_name() {
        let accounts: Vec<MailAccount> = serde_json::from_value(serde_json::json!([
            {"accountId": "2560636000000008002", "primaryEmailAddress": "Sam@Example.net",
             "displayName": "Sam Rivera"},
            {"accountId": 42, "mailboxAddress": "billing@example.net", "accountDisplayName": "Billing"}
        ]))
        .unwrap();
        assert_eq!(accounts[0].address().as_deref(), Some("sam@example.net"));
        assert_eq!(accounts[0].name().as_deref(), Some("Sam Rivera"));
        assert_eq!(accounts[1].account_id, "42");
        assert_eq!(
            accounts[1].address().as_deref(),
            Some("billing@example.net")
        );
    }
}
