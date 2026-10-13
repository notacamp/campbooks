//! The Gmail API calls the adapter makes (users.me only).

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;
use url::Url;

use super::{GoogleError, failure, json, transport};

/// The Gmail API for one access token.
#[derive(Debug)]
pub struct Gmail<'a> {
    http: &'a reqwest::Client,
    base: &'a Url,
    token: &'a str,
}

/// `users.getProfile`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    /// The mailbox's current history id.
    pub history_id: String,
}

/// One label of `users.labels.list`.
#[derive(Debug, Clone, Deserialize)]
pub struct LabelInfo {
    /// Its id (`INBOX`, `Label_12`).
    pub id: String,
    /// Its name.
    pub name: String,
    /// `system` or `user`.
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
}

/// A message id and thread id, as lists return them.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageRef {
    /// The message id.
    pub id: String,
}

/// One page of `users.messages.list`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPage {
    /// The messages, newest first.
    #[serde(default)]
    pub messages: Vec<MessageRef>,
    /// The next page.
    #[serde(default)]
    pub next_page_token: Option<String>,
}

/// `users.messages.get` with `format=minimal`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageMeta {
    /// The message id.
    pub id: String,
    /// The thread id.
    pub thread_id: String,
    /// Its labels now.
    #[serde(default)]
    pub label_ids: Vec<String>,
    /// When Gmail received it, epoch milliseconds.
    pub internal_date: Option<String>,
}

/// `users.messages.get` with `format=raw`.
#[derive(Debug, Clone)]
pub struct MessageRaw {
    /// The metadata.
    pub meta: MessageMeta,
    /// The RFC 5322 bytes.
    pub raw: Vec<u8>,
    /// Bytes downloaded for it (the encoded answer).
    pub fetched_bytes: u64,
}

/// What a raw download gave.
#[derive(Debug, Clone)]
pub enum RawFetch {
    /// The message.
    Message(MessageRaw),
    /// It is larger than the limit; the download was abandoned.
    TooLarge,
}

/// One page of `users.history.list`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPage {
    /// The changes, oldest first.
    #[serde(default)]
    pub history: Vec<HistoryRecord>,
    /// The next page.
    #[serde(default)]
    pub next_page_token: Option<String>,
    /// The mailbox's history id now.
    pub history_id: String,
}

/// One history record.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryRecord {
    /// Its history id (Gmail's order of writes).
    #[serde(default)]
    pub id: String,
    /// Messages added.
    #[serde(default)]
    pub messages_added: Vec<HistoryMessage>,
    /// Messages deleted.
    #[serde(default)]
    pub messages_deleted: Vec<HistoryMessage>,
    /// Labels added.
    #[serde(default)]
    pub labels_added: Vec<HistoryMessage>,
    /// Labels removed.
    #[serde(default)]
    pub labels_removed: Vec<HistoryMessage>,
}

/// The message a history record is about.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryMessage {
    /// The message.
    pub message: MessageRef,
    /// The labels added or removed (label records only).
    #[serde(default)]
    pub label_ids: Vec<String>,
}

impl<'a> Gmail<'a> {
    pub(super) fn new(http: &'a reqwest::Client, base: &'a Url, token: &'a str) -> Self {
        Self { http, base, token }
    }

    fn url(&self, path: &str) -> Result<Url, GoogleError> {
        self.base
            .join(path)
            .map_err(|e| GoogleError::Unexpected(format!("Gmail URL: {e}")))
    }

    async fn get<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<T, GoogleError> {
        let mut url = self.url(path)?;
        url.query_pairs_mut().extend_pairs(query);
        let response = self
            .http
            .get(url)
            .bearer_auth(self.token)
            .send()
            .await
            .map_err(transport)?;
        json(response).await
    }

    async fn post(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<reqwest::Response, GoogleError> {
        let response = self
            .http
            .post(self.url(path)?)
            .bearer_auth(self.token)
            .json(body)
            .send()
            .await
            .map_err(transport)?;
        if response.status().is_success() {
            Ok(response)
        } else {
            Err(failure(response).await)
        }
    }

    /// `users.messages.modify`: put `add` on message `id` and take `remove`
    /// off. Returns its labels afterwards.
    pub async fn modify(
        &self,
        id: &str,
        add: &[String],
        remove: &[String],
    ) -> Result<MessageMeta, GoogleError> {
        let body = serde_json::json!({"addLabelIds": add, "removeLabelIds": remove});
        let response = self
            .post(&format!("users/me/messages/{id}/modify"), &body)
            .await?;
        json(response).await
    }

    /// `users.messages.batchModify`: the same change for up to 1,000
    /// messages. Gmail answers nothing per message.
    pub async fn batch_modify(
        &self,
        ids: &[String],
        add: &[String],
        remove: &[String],
    ) -> Result<(), GoogleError> {
        let body = serde_json::json!({"ids": ids, "addLabelIds": add, "removeLabelIds": remove});
        self.post("users/me/messages/batchModify", &body).await?;
        Ok(())
    }

    /// `users.getProfile`.
    pub async fn profile(&self) -> Result<Profile, GoogleError> {
        self.get("users/me/profile", &[]).await
    }

    /// `users.labels.list`.
    pub async fn labels(&self) -> Result<Vec<LabelInfo>, GoogleError> {
        #[derive(Deserialize)]
        struct Labels {
            #[serde(default)]
            labels: Vec<LabelInfo>,
        }
        let labels: Labels = self.get("users/me/labels", &[]).await?;
        Ok(labels.labels)
    }

    /// One page of `users.messages.list` for `query`, spam and trash
    /// included.
    pub async fn list(
        &self,
        query: &str,
        page_token: Option<&str>,
        max: u32,
    ) -> Result<ListPage, GoogleError> {
        let max = max.to_string();
        let mut params = vec![
            ("q", query),
            ("includeSpamTrash", "true"),
            ("maxResults", max.as_str()),
        ];
        if let Some(token) = page_token {
            params.push(("pageToken", token));
        }
        self.get("users/me/messages", &params).await
    }

    /// A message's labels (`format=minimal`).
    pub async fn minimal(&self, id: &str) -> Result<MessageMeta, GoogleError> {
        self.get(&format!("users/me/messages/{id}"), &[("format", "minimal")])
            .await
    }

    /// A message's raw RFC 5322 bytes and labels (`format=raw`), unless it
    /// is larger than `max_bytes`: the answer is read as it arrives and
    /// abandoned as soon as it grows past the limit.
    pub async fn raw(&self, id: &str, max_bytes: u64) -> Result<RawFetch, GoogleError> {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(flatten)]
            meta: MessageMeta,
            raw: String,
        }
        let mut url = self.url(&format!("users/me/messages/{id}"))?;
        url.query_pairs_mut().append_pair("format", "raw");
        let mut response = self
            .http
            .get(url)
            .bearer_auth(self.token)
            .send()
            .await
            .map_err(transport)?;
        if !response.status().is_success() {
            return Err(failure(response).await);
        }
        // Base64url grows the message by a third; leave room for the JSON
        // around it.
        let cap = max_bytes.saturating_mul(4) / 3 + 64 * 1024;
        let mut body: Vec<u8> = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(transport)? {
            if u64::try_from(body.len() + chunk.len()).unwrap_or(u64::MAX) > cap {
                return Ok(RawFetch::TooLarge);
            }
            body.extend_from_slice(&chunk);
        }
        let fetched_bytes = u64::try_from(body.len()).unwrap_or(u64::MAX);
        let message: Raw = serde_json::from_slice(&body)
            .map_err(|e| GoogleError::Unexpected(format!("raw message answer: {e}")))?;
        drop(body);
        // Gmail pads base64url; accept it with or without padding.
        let raw = URL_SAFE_NO_PAD
            .decode(message.raw.trim_end_matches('='))
            .map_err(|e| GoogleError::Unexpected(format!("raw message is not base64url: {e}")))?;
        if u64::try_from(raw.len()).unwrap_or(u64::MAX) > max_bytes {
            return Ok(RawFetch::TooLarge);
        }
        Ok(RawFetch::Message(MessageRaw {
            meta: message.meta,
            raw,
            fetched_bytes,
        }))
    }

    /// One page of `users.history.list` from `start`. `NotFound` means the
    /// history id is too old.
    pub async fn history(
        &self,
        start: &str,
        page_token: Option<&str>,
        max: u32,
    ) -> Result<HistoryPage, GoogleError> {
        let max = max.to_string();
        let mut params = vec![
            ("startHistoryId", start),
            ("maxResults", max.as_str()),
            ("historyTypes", "messageAdded"),
            ("historyTypes", "messageDeleted"),
            ("historyTypes", "labelAdded"),
            ("historyTypes", "labelRemoved"),
        ];
        if let Some(token) = page_token {
            params.push(("pageToken", token));
        }
        self.get("users/me/history", &params).await
    }
}
