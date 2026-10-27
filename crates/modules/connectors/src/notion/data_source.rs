//! The calls that read and write a Notion database (a data source, API
//! version 2026-03-11) (campbooks-cloud#377). Like every call of this
//! module they go through `Notion::call`: the fixed host, the pinned
//! version, typed UUID path segments, no redirects, a time and size bound.
//! A database is never trashed from here: no call below can do it.

use chrono::{DateTime, SecondsFormat, Utc};
use reqwest::Method;
use serde_json::{Value, json};
use uuid::Uuid;

use super::{LIST_ANSWER_BYTES, MAX_ANSWER_BYTES, Notion, databases, next_cursor};
use crate::provider::ProviderError;

/// A data source, as much of it as is used.
#[derive(Debug, Clone, PartialEq)]
pub struct DataSourceInfo {
    /// The data source.
    pub id: Uuid,
    /// Its title, without control characters, at most 200 characters.
    pub title: String,
    /// It is in the trash.
    pub trashed: bool,
    /// Its properties.
    pub schema: databases::Schema,
}

/// One page of a data source's entries.
#[derive(Debug, Clone, PartialEq)]
pub struct EntriesPage {
    /// The entries, as Notion gives them (page objects).
    pub entries: Vec<Value>,
    /// The cursor of the next page.
    pub next: Option<String>,
}

impl Notion {
    /// Data source `id`.
    pub async fn data_source(
        &self,
        token: &str,
        id: Uuid,
    ) -> Result<DataSourceInfo, ProviderError> {
        let url = self.api_url(&["data_sources", &id.to_string()])?;
        let answer = self
            .call(Method::GET, url, token, None, LIST_ANSWER_BYTES)
            .await?;
        if answer.get("object").and_then(Value::as_str) != Some("data_source") {
            return Err(ProviderError::Unexpected(
                "a Notion data source did not parse".into(),
            ));
        }
        let title = campbooks_kernel::text::displayable(&databases::plain(answer.get("title")))
            .trim()
            .chars()
            .take(200)
            .collect();
        Ok(DataSourceInfo {
            id,
            title,
            trashed: answer
                .get("in_trash")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            schema: databases::schema_of(&answer),
        })
    }

    /// One page of the entries of data source `id` edited at or after
    /// `since` (oldest first), a hundred at a time; with `only`, just that
    /// property (an id) in each.
    pub async fn query_entries(
        &self,
        token: &str,
        id: Uuid,
        since: Option<DateTime<Utc>>,
        only: Option<&str>,
        cursor: Option<&str>,
    ) -> Result<EntriesPage, ProviderError> {
        let mut url = self.api_url(&["data_sources", &id.to_string(), "query"])?;
        if let Some(only) = only {
            url.query_pairs_mut().append_pair("filter_properties", only);
        }
        let mut body = json!({
            "sorts": [{"timestamp": "last_edited_time", "direction": "ascending"}],
            "page_size": 100,
        });
        if let Some(since) = since {
            body["filter"] = json!({
                "timestamp": "last_edited_time",
                "last_edited_time": {"on_or_after": since.to_rfc3339_opts(SecondsFormat::Secs, true)},
            });
        }
        if let Some(cursor) = cursor {
            body["start_cursor"] = Value::String(cursor.to_owned());
        }
        let answer = self
            .call(Method::POST, url, token, Some(&body), LIST_ANSWER_BYTES)
            .await?;
        Ok(EntriesPage {
            entries: answer
                .get("results")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter(|item| item.get("object").and_then(Value::as_str) == Some("page"))
                        .cloned()
                        .collect()
                })
                .unwrap_or_default(),
            next: next_cursor(&answer),
        })
    }

    /// Page `page`'s property `property` in full: one page of its items
    /// (Notion lists at most 25 of a long list in the page object).
    pub async fn property_items(
        &self,
        token: &str,
        page: Uuid,
        property: &str,
        cursor: Option<&str>,
    ) -> Result<(Value, Option<String>), ProviderError> {
        let mut url = self.api_url(&["pages", &page.to_string(), "properties", property])?;
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
        let next = next_cursor(&answer);
        Ok((answer, next))
    }

    /// Change the properties of data source `id` with `properties` (the
    /// body's `properties` object); the data source as it is now. Never
    /// anything else of the data source: not its title, not its trash.
    pub async fn update_data_source_properties(
        &self,
        token: &str,
        id: Uuid,
        properties: &Value,
    ) -> Result<Value, ProviderError> {
        let url = self.api_url(&["data_sources", &id.to_string()])?;
        let body = json!({"properties": properties});
        self.call(Method::PATCH, url, token, Some(&body), LIST_ANSWER_BYTES)
            .await
    }

    /// Add an entry to data source `id` with `properties`; the page made.
    pub async fn create_entry(
        &self,
        token: &str,
        id: Uuid,
        properties: &Value,
    ) -> Result<Value, ProviderError> {
        let url = self.api_url(&["pages"])?;
        let body = json!({
            "parent": {"type": "data_source_id", "data_source_id": id},
            "properties": properties,
        });
        self.call(Method::POST, url, token, Some(&body), MAX_ANSWER_BYTES)
            .await
    }

    /// Set properties of entry `page`; the page as it is now.
    pub async fn update_entry(
        &self,
        token: &str,
        page: Uuid,
        properties: &Value,
    ) -> Result<Value, ProviderError> {
        let url = self.api_url(&["pages", &page.to_string()])?;
        let body = json!({"properties": properties});
        self.call(Method::PATCH, url, token, Some(&body), MAX_ANSWER_BYTES)
            .await
    }

    /// Move entry `page` to the trash (the only trashing this module does,
    /// and only of an entry of the data source a stream reads: the caller
    /// checks that first).
    pub async fn trash_entry(&self, token: &str, page: Uuid) -> Result<Value, ProviderError> {
        let url = self.api_url(&["pages", &page.to_string()])?;
        let body = json!({"in_trash": true});
        self.call(Method::PATCH, url, token, Some(&body), MAX_ANSWER_BYTES)
            .await
    }

    /// Entry `page` as Notion gives it.
    pub async fn entry(&self, token: &str, page: Uuid) -> Result<Value, ProviderError> {
        let url = self.api_url(&["pages", &page.to_string()])?;
        self.call(Method::GET, url, token, None, MAX_ANSWER_BYTES)
            .await
    }
}
