//! The search-source port (ADR 0016): how a module hands the `search`
//! module the current searchable form of its records.
//!
//! Search is a projection (ADR 0011). It learns *that* something changed from
//! the owning module's ordinary events (ids only), then asks the module's
//! [`SearchSource`] for the record's current state. Reading current state
//! makes repeated and out-of-order deliveries converge, and lets search build
//! or rebuild its index by paging through [`SearchSource::page`] instead of
//! replaying the event log.
//!
//! The shape is two-level:
//!
//! - a **group** is what a result is shown as and what access applies to (a
//!   conversation): it lives in a **container** (an inbox) whose visibility
//!   the [`Visibility`](crate::changes::Visibility) port decides;
//! - an **entry** is text inside a group (a message).
//!
//! Each entry has a **stamp** that changes exactly when its text does, so
//! search re-reads text ([`SearchSource::texts`]) only for entries whose
//! stamp moved; a flag change costs one [`SearchSource::group`] call.
//!
//! The server registers the modules' sources with
//! [`AppState::with_search_sources`](crate::AppState::with_search_sources).
//! Nothing here is persisted or published: personal data (names, addresses,
//! text) crosses in-process only.

use std::{collections::BTreeMap, future::Future, pin::Pin, sync::Arc};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;

use crate::{Error, blobs::BlobStore, records::RecordRef, tenancy::WorkspaceId};

/// Facet names the `search` module's query operators read. A source sets the
/// ones that apply to its records; others are ignored.
pub mod facet {
    /// Group: the place the group is sorted into (text).
    pub const STREAM: &str = "stream";
    /// Group: something in it is unread (bool).
    pub const UNREAD: &str = "unread";
    /// Group: starred (bool).
    pub const STARRED: &str = "starred";
    /// Group: out of the inbox (bool).
    pub const ARCHIVED: &str = "archived";
    /// Group: the ids of its labels (texts).
    pub const LABELS: &str = "labels";
    /// Group: how many entries it has (count).
    pub const COUNT: &str = "count";
    /// Entry: where it sits, `inbox`, `archive`, `spam` or `trash` (text).
    pub const MAILBOX: &str = "mailbox";
    /// Entry: `inbound` or `outbound` (text).
    pub const DIRECTION: &str = "direction";
    /// Entry: how many files it carries (count).
    pub const ATTACHMENTS: &str = "attachments";
}

/// One non-personal value filters can test: never names, addresses or text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Facet {
    /// A flag.
    Bool(bool),
    /// A count.
    Count(i64),
    /// One value of a small set (a stream, a mailbox).
    Text(String),
    /// Several values (label ids).
    Texts(Vec<String>),
}

/// A record's facets, by name ([`facet`]).
pub type Facets = BTreeMap<String, Facet>;

/// The current searchable form of a group, without its entries' text.
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    /// Where it lives: the container whose visibility decides who sees it.
    pub container: RecordRef,
    /// Its time (a conversation's last message).
    pub occurred_at: DateTime<Utc>,
    /// Its title (a conversation's subject).
    pub title: String,
    /// Words that name it without being text in it (label names).
    pub keywords: Vec<String>,
    /// Its facets.
    pub facets: Facets,
    /// Its entries, every one of them.
    pub entries: Vec<EntryHead>,
}

/// An entry as a group lists it: enough to keep its facets current and to
/// tell whether its text changed.
#[derive(Debug, Clone, PartialEq)]
pub struct EntryHead {
    /// The entry (a message).
    pub record: RecordRef,
    /// Its time.
    pub occurred_at: DateTime<Utc>,
    /// Its facets.
    pub facets: Facets,
    /// Changes exactly when its text, people or attachment names do.
    pub stamp: String,
}

/// Who an entry is from or to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersonRole {
    /// The author.
    From,
    /// A recipient.
    To,
    /// A copied recipient.
    Cc,
}

/// A person on an entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Person {
    /// From, to or cc.
    pub role: PersonRole,
    /// Their display name, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Their address, lower-cased.
    pub address: String,
}

/// An entry's text.
#[derive(Debug, Clone, PartialEq)]
pub struct EntryText {
    /// The entry.
    pub record: RecordRef,
    /// Its people.
    pub people: Vec<Person>,
    /// Its plain text: no markup, no quoted history, bounded by the source.
    pub text: String,
    /// The names of the files it carries.
    pub attachments: Vec<String>,
}

/// A page of group ids, for building an index from scratch.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Page {
    /// Group ids, in the source's stable order.
    pub groups: Vec<String>,
    /// Pass it as `after` for the next page; `None` on the last one.
    pub next: Option<String>,
}

/// What a source reads with: the caller's transaction, whose workspace is
/// set (row-level security applies), and the blob store.
#[derive(Debug)]
pub struct SourceContext<'a> {
    /// The transaction's connection.
    pub conn: &'a mut PgConnection,
    /// Bodies and documents.
    pub blobs: &'a BlobStore,
    /// The transaction's workspace.
    pub workspace: WorkspaceId,
}

/// A future returned by a [`SearchSource`].
pub type SourceFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, Error>> + Send + 'a>>;

/// A module's records, as search reads them. Implemented by the module that
/// owns the group type; registered by the server.
pub trait SearchSource: std::fmt::Debug + Send + Sync + 'static {
    /// The group record type it serves (`<module>.<record>`).
    fn group_type(&self) -> &'static str;

    /// The container record type of its groups (`<module>.<record>`).
    fn container_type(&self) -> &'static str;

    /// Group `id` as it is now, or `None` once it is gone.
    fn group<'a>(&'a self, cx: SourceContext<'a>, id: &'a str) -> SourceFuture<'a, Option<Group>>;

    /// The text of `entries` (ones it listed); entries that are gone are
    /// left out.
    fn texts<'a>(
        &'a self,
        cx: SourceContext<'a>,
        entries: &'a [RecordRef],
    ) -> SourceFuture<'a, Vec<EntryText>>;

    /// Up to `limit` group ids after `after`, in a stable order.
    fn page<'a>(
        &'a self,
        cx: SourceContext<'a>,
        after: Option<&'a str>,
        limit: u32,
    ) -> SourceFuture<'a, Page>;
}

/// The registered sources, by group type. Cheap to clone. Empty until the
/// server registers them: search then indexes nothing (fails closed).
#[derive(Debug, Clone, Default)]
pub struct SearchSources(Arc<Vec<Arc<dyn SearchSource>>>);

impl SearchSources {
    /// The sources in `sources`.
    pub fn new(sources: Vec<Arc<dyn SearchSource>>) -> Self {
        Self(Arc::new(sources))
    }

    /// The source of `group_type`.
    pub fn get(&self, group_type: &str) -> Option<&Arc<dyn SearchSource>> {
        self.0
            .iter()
            .find(|source| source.group_type() == group_type)
    }

    /// Every source.
    pub fn iter(&self) -> impl Iterator<Item = &Arc<dyn SearchSource>> {
        self.0.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facets_serialise_as_plain_json() {
        let facets: Facets = [
            (facet::UNREAD.to_owned(), Facet::Bool(true)),
            (facet::COUNT.to_owned(), Facet::Count(3)),
            (facet::STREAM.to_owned(), Facet::Text("receipts".into())),
            (facet::LABELS.to_owned(), Facet::Texts(vec!["a".into()])),
        ]
        .into();
        let json = serde_json::to_value(&facets).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"unread": true, "count": 3, "stream": "receipts", "labels": ["a"]})
        );
        assert_eq!(serde_json::from_value::<Facets>(json).unwrap(), facets);
    }

    #[test]
    fn sources_are_found_by_group_type() {
        assert!(SearchSources::default().get("x.y").is_none());
    }
}
