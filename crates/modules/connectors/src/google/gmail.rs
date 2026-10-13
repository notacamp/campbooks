//! The Gmail API calls the adapter makes (users.me only).

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;
use url::Url;

use super::{GoogleError, json, transport};

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
pub struct HistoryMessage {
    /// The message.
    pub message: MessageRef,
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

    /// A message's raw RFC 5322 bytes and labels (`format=raw`).
    pub async fn raw(&self, id: &str) -> Result<MessageRaw, GoogleError> {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(flatten)]
            meta: MessageMeta,
            raw: String,
        }
        let message: Raw = self
            .get(&format!("users/me/messages/{id}"), &[("format", "raw")])
            .await?;
        // Gmail pads base64url; accept it with or without padding.
        let raw = URL_SAFE_NO_PAD
            .decode(message.raw.trim_end_matches('='))
            .map_err(|e| GoogleError::Unexpected(format!("raw message is not base64url: {e}")))?;
        Ok(MessageRaw {
            meta: message.meta,
            raw,
        })
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
