//! Microsoft Graph's mail, as the messages adapter changes and sends it
//! (campbooks-cloud#317): a message's state with its version, a patch of
//! its read state, flag and categories checked against that version
//! (`If-Match`), a move to another folder, `sendMail` with the message as
//! MIME, and the Sent Items copy of a sent message by its
//! `internetMessageId`.
//!
//! As for reading (`graph.rs`): the fixed Graph base, ids validated and
//! encoded as path segments, no redirect, a timeout, bounded answers, and
//! errors with a status and a sanitised code only. Messages are named by
//! their immutable ids (`Prefer: IdType="ImmutableId"`), so a move keeps
//! the id. There is no call here that deletes a message or a folder, or
//! empties one: a message is only ever patched or moved, and "delete" is a
//! move to Deleted Items.

use serde::Deserialize;

use super::{
    CALL_TIMEOUT, MAX_ANSWER_BYTES, Microsoft, bounded,
    graph::{ItemAnswer, LIST_ANSWER_BYTES, MessageState, PREFER, SELECT, valid_id},
    graph_failure, within,
};
use crate::provider::{HTTP_TIMEOUT, ProviderError, transport, unreadable};

/// The largest `sendMail` body: Graph takes at most 4 MB in a request, and
/// a MIME message goes as base64.
pub const MAX_SEND_BODY: usize = 4_000_000;

/// The longest version (`@odata.etag`) kept.
pub const MAX_ETAG: usize = 1024;

/// The longest `Message-ID` looked up.
pub const MAX_INTERNET_ID: usize = 250;

/// A message's state at Graph and its version, what a write is checked
/// against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Current {
    pub state: MessageState,
    /// Its `@odata.etag` (`W/"<changeKey>"`); `None` when Graph gave
    /// neither it nor a `changeKey` usable as one.
    pub etag: Option<String>,
}

/// What a patch changes: each field only when given.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Patch {
    pub read: Option<bool>,
    pub flagged: Option<bool>,
    /// Every category the message is to have.
    pub categories: Option<Vec<String>>,
}

impl Patch {
    /// Whether it changes nothing.
    pub fn is_empty(&self) -> bool {
        self.read.is_none() && self.flagged.is_none() && self.categories.is_none()
    }

    /// Its JSON body.
    fn body(&self) -> serde_json::Value {
        let mut body = serde_json::Map::new();
        if let Some(read) = self.read {
            body.insert("isRead".into(), read.into());
        }
        if let Some(flagged) = self.flagged {
            let status = if flagged { "flagged" } else { "notFlagged" };
            body.insert("flag".into(), serde_json::json!({ "flagStatus": status }));
        }
        if let Some(categories) = &self.categories {
            body.insert("categories".into(), categories.clone().into());
        }
        body.into()
    }

    /// `state` with this patch made.
    pub fn apply(&self, state: &mut MessageState) {
        if let Some(read) = self.read {
            state.read = read;
        }
        if let Some(flagged) = self.flagged {
            state.flagged = flagged;
        }
        if let Some(categories) = &self.categories {
            let mut categories = categories.clone();
            categories.sort();
            categories.dedup();
            state.categories = categories;
        }
    }
}

/// How a patch ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Patched {
    /// Graph made it.
    Done,
    /// The message changed since its version was read (412, or 409
    /// `ErrorIrresolvableConflict`): nothing was made.
    Conflict,
    /// Graph no longer has the message.
    Gone,
}

/// How a move ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Moved {
    /// Graph moved it, and answered with this id (the message's own, an
    /// immutable id).
    To(String),
    /// Graph moved it, but its answer named no id that could be read.
    Unknown,
    /// Graph no longer has the message.
    Gone,
}

/// How a `sendMail` ended, when Graph answered for sure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendAnswer {
    /// Taken (202): Graph sends it and saves it to Sent Items.
    Accepted,
    /// Larger than Graph takes (413): not sent.
    TooLarge,
}

/// A sent message's copy in Sent Items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentCopy {
    pub id: String,
    pub folder: Option<String>,
    pub conversation: Option<String>,
}

/// What looking a sent message up in Sent Items found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SentLookup {
    /// Exactly one copy.
    Found(SentCopy),
    /// None.
    Missing,
    /// More than one: nothing can be concluded.
    Several,
}

/// The body of a `sendMail` of `raw`: its base64.
pub fn send_body(raw: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(raw)
}

/// Whether `etag` can be sent back in `If-Match`: printable ASCII without
/// spaces, at most [`MAX_ETAG`].
fn valid_etag(etag: &str) -> bool {
    !etag.is_empty() && etag.len() <= MAX_ETAG && etag.bytes().all(|b| b.is_ascii_graphic())
}

/// Whether `id` is a `Message-ID` (without angle brackets) that can go in
/// a quoted `$filter` value: printable ASCII with an `@`, no quote, bracket
/// or backslash, at most [`MAX_INTERNET_ID`].
pub fn valid_internet_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_INTERNET_ID
        && id.contains('@')
        && id
            .bytes()
            .all(|b| b.is_ascii_graphic() && !matches!(b, b'\'' | b'"' | b'<' | b'>' | b'\\'))
}

/// An id that is not one Graph could have given: never put in a URL.
fn not_an_id() -> ProviderError {
    ProviderError::Unexpected("not a Graph id".to_owned())
}

impl Microsoft {
    /// Message `id`'s state and version; `None` when Graph no longer has it.
    pub async fn current(&self, token: &str, id: &str) -> Result<Option<Current>, ProviderError> {
        if !valid_id(id) {
            return Err(not_an_id());
        }
        let mut url = self.graph_url(&["me", "messages", id])?;
        url.query_pairs_mut()
            .append_pair("$select", &format!("{SELECT},changeKey"));
        let body = match self.get(url, token, MAX_ANSWER_BYTES).await {
            Ok(body) => body,
            Err(ProviderError::NotFound) => return Ok(None),
            Err(other) => return Err(other),
        };
        let mut item: ItemAnswer = serde_json::from_slice(&body)
            .map_err(|e| ProviderError::Unexpected(unreadable("a Graph message", &e)))?;
        let etag = item
            .etag
            .take()
            .filter(|etag| valid_etag(etag))
            .or_else(|| {
                item.change_key
                    .take()
                    .filter(|key| valid_id(key))
                    .map(|key| format!("W/\"{key}\""))
            });
        Ok(Some(Current {
            state: item.state(),
            etag,
        }))
    }

    /// Patch message `id` with `patch`, only if it is still at version
    /// `etag`.
    pub async fn patch(
        &self,
        token: &str,
        id: &str,
        etag: &str,
        patch: &Patch,
    ) -> Result<Patched, ProviderError> {
        if !valid_id(id) {
            return Err(not_an_id());
        }
        if !valid_etag(etag) {
            return Err(ProviderError::Unexpected(
                "not a version Graph gave".to_owned(),
            ));
        }
        let url = self.graph_url(&["me", "messages", id])?;
        let bytes = serde_json::to_vec(&patch.body())
            .map_err(|e| ProviderError::Unexpected(unreadable("a patch", &e)))?;
        let response = self
            .http
            .patch(url)
            .timeout(CALL_TIMEOUT)
            .bearer_auth(token)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(reqwest::header::IF_MATCH, etag)
            .header("prefer", PREFER)
            .body(bytes)
            .send()
            .await
            .map_err(transport)?;
        let status = response.status();
        if status.is_success() {
            // The message as patched: never read (it carries its body).
            return Ok(Patched::Done);
        }
        if matches!(
            status,
            reqwest::StatusCode::PRECONDITION_FAILED | reqwest::StatusCode::CONFLICT
        ) {
            return Ok(Patched::Conflict);
        }
        let headers = response.headers().clone();
        let body = bounded(response, MAX_ANSWER_BYTES)
            .await
            .unwrap_or_default();
        match graph_failure(status, &headers, &body) {
            ProviderError::NotFound => Ok(Patched::Gone),
            other => Err(other),
        }
    }

    /// Move message `id` to folder `folder` (an id from the stream's
    /// cursor).
    pub async fn move_message(
        &self,
        token: &str,
        id: &str,
        folder: &str,
    ) -> Result<Moved, ProviderError> {
        #[derive(Deserialize)]
        struct Answer {
            #[serde(default)]
            id: Option<String>,
        }
        if !valid_id(id) || !valid_id(folder) {
            return Err(not_an_id());
        }
        let url = self.graph_url(&["me", "messages", id, "move"])?;
        let bytes = serde_json::to_vec(&serde_json::json!({ "destinationId": folder }))
            .map_err(|e| ProviderError::Unexpected(unreadable("a move", &e)))?;
        let response = self
            .http
            .post(url)
            .timeout(CALL_TIMEOUT)
            .bearer_auth(token)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header("prefer", PREFER)
            .body(bytes)
            .send()
            .await
            .map_err(transport)?;
        let status = response.status();
        if status.is_success() {
            // The message as moved: only its id is read, from at most a
            // listing's bytes (it carries its body).
            let answered = within(response, LIST_ANSWER_BYTES)
                .await
                .ok()
                .flatten()
                .and_then(|body| serde_json::from_slice::<Answer>(&body).ok())
                .and_then(|answer| answer.id)
                .filter(|id| valid_id(id));
            return Ok(answered.map_or(Moved::Unknown, Moved::To));
        }
        let headers = response.headers().clone();
        let body = bounded(response, MAX_ANSWER_BYTES)
            .await
            .unwrap_or_default();
        match graph_failure(status, &headers, &body) {
            ProviderError::NotFound => Ok(Moved::Gone),
            other => Err(other),
        }
    }

    /// Send `raw` (an RFC 5322 message) as the signed-in mailbox
    /// (`sendMail`, MIME): Graph saves it to Sent Items itself. A message
    /// whose base64 is larger than [`MAX_SEND_BODY`] is never sent
    /// ([`SendAnswer::TooLarge`], no call).
    pub async fn send_mime(&self, token: &str, raw: &[u8]) -> Result<SendAnswer, ProviderError> {
        let body = send_body(raw);
        if body.len() > MAX_SEND_BODY {
            return Ok(SendAnswer::TooLarge);
        }
        let url = self.graph_url(&["me", "sendMail"])?;
        let response = self
            .http
            .post(url)
            .timeout(HTTP_TIMEOUT)
            .bearer_auth(token)
            .header(reqwest::header::CONTENT_TYPE, "text/plain")
            .body(body)
            .send()
            .await
            .map_err(transport)?;
        let status = response.status();
        if status.is_success() {
            return Ok(SendAnswer::Accepted);
        }
        if status == reqwest::StatusCode::PAYLOAD_TOO_LARGE {
            return Ok(SendAnswer::TooLarge);
        }
        let headers = response.headers().clone();
        let body = bounded(response, MAX_ANSWER_BYTES)
            .await
            .unwrap_or_default();
        Err(graph_failure(status, &headers, &body))
    }

    /// The copy in Sent Items of the message whose `Message-ID` is
    /// `internet_id` (without angle brackets).
    pub async fn sent_copy(
        &self,
        token: &str,
        internet_id: &str,
    ) -> Result<SentLookup, ProviderError> {
        #[derive(Deserialize)]
        struct Answer {
            #[serde(default)]
            value: Vec<Found>,
        }
        #[derive(Deserialize)]
        struct Found {
            #[serde(default)]
            id: Option<String>,
            #[serde(default, rename = "parentFolderId")]
            folder: Option<String>,
            #[serde(default, rename = "conversationId")]
            conversation: Option<String>,
        }
        if !valid_internet_id(internet_id) {
            return Ok(SentLookup::Missing);
        }
        let mut url = self.graph_url(&["me", "mailFolders", "sentitems", "messages"])?;
        url.query_pairs_mut()
            .append_pair(
                "$filter",
                &format!("internetMessageId eq '<{internet_id}>'"),
            )
            .append_pair("$select", "id,parentFolderId,conversationId")
            .append_pair("$top", "2");
        let body = match self.get(url, token, MAX_ANSWER_BYTES).await {
            Ok(body) => body,
            // No Sent Items (yet): nothing found.
            Err(ProviderError::NotFound) => return Ok(SentLookup::Missing),
            Err(other) => return Err(other),
        };
        let answer: Answer = serde_json::from_slice(&body)
            .map_err(|e| ProviderError::Unexpected(unreadable("a Graph listing", &e)))?;
        let mut found = answer.value.into_iter();
        Ok(match (found.next(), found.next()) {
            (None, _) => SentLookup::Missing,
            (Some(_), Some(_)) => SentLookup::Several,
            (Some(one), None) => match one.id.filter(|id| valid_id(id)) {
                Some(id) => SentLookup::Found(SentCopy {
                    id,
                    folder: one.folder.filter(|f| valid_id(f)),
                    conversation: one.conversation.filter(|c| valid_id(c)),
                }),
                None => SentLookup::Several,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_patch_says_only_what_it_changes() {
        let patch = Patch {
            read: Some(true),
            ..Patch::default()
        };
        assert_eq!(patch.body(), serde_json::json!({"isRead": true}));
        let patch = Patch {
            flagged: Some(false),
            categories: Some(vec!["Red".into(), "Campbooks/Receipts".into()]),
            ..Patch::default()
        };
        assert_eq!(
            patch.body(),
            serde_json::json!({
                "flag": {"flagStatus": "notFlagged"},
                "categories": ["Red", "Campbooks/Receipts"]
            })
        );
        assert!(Patch::default().is_empty());
    }

    #[test]
    fn versions_and_message_ids_are_checked() {
        assert!(valid_etag("W/\"CQAAABYAAAD+abc==\""));
        assert!(!valid_etag("W/\"a b\""));
        assert!(!valid_etag("W/\"a\"\r\nX-Evil: 1"));
        assert!(!valid_etag(""));
        assert!(valid_internet_id("0192f3e4-send@example.com"));
        assert!(!valid_internet_id("a' or 1 eq 1 or 'b@example.com"));
        assert!(!valid_internet_id("<a@example.com>"));
        assert!(!valid_internet_id("no-at-sign"));
        assert!(!valid_internet_id(&format!(
            "{}@example.com",
            "a".repeat(300)
        )));
    }
}
