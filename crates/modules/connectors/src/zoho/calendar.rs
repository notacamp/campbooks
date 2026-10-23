//! The Zoho Calendar REST calls the adapter makes, for one access token.
//! Unverified against a live Zoho grant: the adapter is behind
//! `CAMPBOOKS_CONNECTORS__ZOHO_CALENDAR` (off by default).
//!
//! Answers are read as JSON values and only the fields the adapter uses are
//! taken from them; errors carry a status and Zoho's `errorCode`, never a
//! body.

use serde_json::Value;
use url::Url;

use super::api::classify;
use crate::{google::CalendarError, provider::ProviderError, provider::transport};

/// The Calendar API for one access token.
#[derive(Debug, Clone, Copy)]
pub struct ZohoCalendar<'a> {
    http: &'a reqwest::Client,
    base: &'a Url,
    token: &'a str,
}

impl<'a> ZohoCalendar<'a> {
    pub(super) fn new(http: &'a reqwest::Client, base: &'a Url, token: &'a str) -> Self {
        Self { http, base, token }
    }

    /// The URL of `segments` under the API base, each one path segment.
    fn url(&self, segments: &[&str]) -> Result<Url, CalendarError> {
        let mut url = self.base.clone();
        url.path_segments_mut()
            .map_err(|()| CalendarError::Provider(ProviderError::Unexpected("Zoho URL".into())))?
            .pop_if_empty()
            .extend(segments);
        Ok(url)
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

    async fn call(&self, request: reqwest::RequestBuilder) -> Result<Value, CalendarError> {
        let response = request
            .send()
            .await
            .map_err(|e| CalendarError::Provider(transport(e)))?;
        let status = response.status();
        if !status.is_success() {
            let headers = response.headers().clone();
            let code = response.json::<Value>().await.ok().and_then(|body| {
                body.get("errorCode")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            });
            return Err(CalendarError::Provider(classify(
                status,
                Some(&headers),
                code.as_deref(),
            )));
        }
        // A delete answers with no body.
        let bytes = response
            .bytes()
            .await
            .map_err(|e| CalendarError::Provider(transport(e)))?;
        if bytes.is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_slice(&bytes).map_err(|_| {
            CalendarError::Provider(ProviderError::Unexpected(
                "a Zoho Calendar answer did not parse".into(),
            ))
        })
    }

    /// The account's calendars (`GET calendars`).
    pub async fn calendars(&self) -> Result<Vec<Value>, CalendarError> {
        let answer = self
            .call(self.request(reqwest::Method::GET, self.url(&["calendars"])?))
            .await?;
        Ok(array(&answer, "calendars"))
    }

    /// The events of calendar `uid` between `start` and `end` (Zoho's
    /// `yyyyMMdd'T'HHmmss'Z'`; its range is 31 days at most).
    pub async fn events(
        &self,
        uid: &str,
        start: &str,
        end: &str,
    ) -> Result<Vec<Value>, CalendarError> {
        let mut url = self.url(&["calendars", uid, "events"])?;
        let range = serde_json::json!({ "start": start, "end": end }).to_string();
        url.query_pairs_mut().append_pair("range", &range);
        let answer = self.call(self.request(reqwest::Method::GET, url)).await?;
        Ok(array(&answer, "events"))
    }

    /// Make the event `data` in calendar `uid`; the event Zoho made.
    pub async fn create(&self, uid: &str, data: &Value) -> Result<Value, CalendarError> {
        let mut url = self.url(&["calendars", uid, "events"])?;
        url.query_pairs_mut()
            .append_pair("eventdata", &data.to_string());
        let answer = self.call(self.request(reqwest::Method::POST, url)).await?;
        Ok(array(&answer, "events")
            .into_iter()
            .next()
            .unwrap_or(Value::Null))
    }

    /// Change event `event` of calendar `uid` with `data` (which names the
    /// etag the change is based on); the event Zoho now has.
    pub async fn update(
        &self,
        uid: &str,
        event: &str,
        data: &Value,
    ) -> Result<Value, CalendarError> {
        let mut url = self.url(&["calendars", uid, "events", event])?;
        url.query_pairs_mut()
            .append_pair("eventdata", &data.to_string());
        let answer = self.call(self.request(reqwest::Method::PUT, url)).await?;
        Ok(array(&answer, "events")
            .into_iter()
            .next()
            .unwrap_or(Value::Null))
    }

    /// Delete event `event` of calendar `uid`, if its etag is `etag`.
    pub async fn delete(&self, uid: &str, event: &str, etag: &str) -> Result<(), CalendarError> {
        let value = reqwest::header::HeaderValue::from_str(etag).map_err(|_| {
            CalendarError::Provider(ProviderError::Unexpected("an unusable etag".into()))
        })?;
        let url = self.url(&["calendars", uid, "events", event])?;
        self.call(
            self.request(reqwest::Method::DELETE, url)
                .header("etag", value),
        )
        .await?;
        Ok(())
    }
}

/// The array `key` of an answer (empty when it has none).
fn array(answer: &Value, key: &str) -> Vec<Value> {
    answer
        .get(key)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}
