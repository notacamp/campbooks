//! How search reads pages (ADR 0016): a page is a group of one entry, its
//! body, in a container of its own whose visibility the module answers.
//! Title weighs more than text (the index does that); a page in the trash is
//! no group at all, so it leaves the index and comes back when restored.

use std::sync::Arc;

use campbooks_kernel::{
    RecordRef,
    search::{
        EntryHead, EntryText, Facets, Group, Page, SearchSource, SourceContext, SourceFuture,
    },
};
use chrono::{DateTime, Utc};
use serde_json::Value;

use super::{PageId, document, resources::PAGE_RECORD, store};

/// The record type of a page's body, the entry of its group.
pub const BODY_RECORD: &str = "pages.body";

/// A page as the index needs it: title, edited at, trashed at, revision, and
/// for a row its table.
type Head = (
    String,
    DateTime<Utc>,
    Option<DateTime<Utc>>,
    i64,
    Option<PageId>,
);

/// The source, for the server to register.
pub fn source() -> Arc<dyn SearchSource> {
    Arc::new(PageSource)
}

#[derive(Debug, Clone, Copy)]
struct PageSource;

impl SearchSource for PageSource {
    fn group_type(&self) -> &'static str {
        PAGE_RECORD
    }

    fn container_type(&self) -> &'static str {
        PAGE_RECORD
    }

    fn group<'a>(&'a self, cx: SourceContext<'a>, id: &'a str) -> SourceFuture<'a, Option<Group>> {
        Box::pin(async move {
            let Ok(page) = id.parse::<PageId>() else {
                return Ok(None);
            };
            let row: Option<Head> = sqlx::query_as(
                "SELECT title, edited_at, trashed_at, revision,
                        CASE WHEN kind = 'row' THEN parent_id END
                   FROM pages.pages WHERE id = $1",
            )
            .bind(page)
            .fetch_optional(cx.conn)
            .await?;
            let Some((title, edited_at, None, revision, table)) = row else {
                return Ok(None);
            };
            Ok(Some(Group {
                // A row is found by whoever may read its table: its container.
                container: RecordRef::new(PAGE_RECORD, &table.unwrap_or(page)),
                occurred_at: edited_at,
                title,
                keywords: Vec::new(),
                facets: Facets::new(),
                entries: vec![EntryHead {
                    record: RecordRef::new(BODY_RECORD, &page),
                    occurred_at: edited_at,
                    facets: Facets::new(),
                    stamp: revision.to_string(),
                }],
            }))
        })
    }

    fn texts<'a>(
        &'a self,
        cx: SourceContext<'a>,
        entries: &'a [RecordRef],
    ) -> SourceFuture<'a, Vec<EntryText>> {
        Box::pin(async move {
            let conn = cx.conn;
            let mut texts = Vec::new();
            for entry in entries {
                let Ok(page) = entry.id.parse::<PageId>() else {
                    continue;
                };
                let loaded = store::load(&mut *conn, page).await?;
                let mut text = document::inspect(&loaded.doc)
                    .map(|read| read.text)
                    .unwrap_or_default();
                // A row's text values are searched with its title and body.
                let values: Vec<Value> = sqlx::query_scalar(
                    "SELECT v.value FROM pages.row_values v
                       JOIN pages.table_properties p ON p.id = v.property_id
                      WHERE v.row_id = $1 AND p.type = 'text' AND p.removed_at IS NULL
                        AND jsonb_typeof(v.value) = 'array'
                      ORDER BY p.position, p.id",
                )
                .bind(page)
                .fetch_all(&mut *conn)
                .await?;
                for value in values {
                    text.push('\n');
                    text.push_str(&crate::properties::value::runs_text(&value));
                }
                texts.push(EntryText {
                    record: entry.clone(),
                    people: Vec::new(),
                    text,
                    attachments: Vec::new(),
                });
            }
            Ok(texts)
        })
    }

    fn page<'a>(
        &'a self,
        cx: SourceContext<'a>,
        after: Option<&'a str>,
        limit: u32,
    ) -> SourceFuture<'a, Page> {
        Box::pin(async move {
            let after: Option<PageId> = after.and_then(|cursor| cursor.parse().ok());
            let ids: Vec<PageId> = sqlx::query_scalar(
                "SELECT id FROM pages.pages
                  WHERE trashed_at IS NULL AND ($1::uuid IS NULL OR id > $1)
                  ORDER BY id LIMIT $2",
            )
            .bind(after)
            .bind(i64::from(limit))
            .fetch_all(cx.conn)
            .await?;
            let full = ids.len() >= limit as usize;
            let next = ids.last().filter(|_| full).map(ToString::to_string);
            Ok(Page {
                groups: ids.iter().map(ToString::to_string).collect(),
                next,
            })
        })
    }
}
