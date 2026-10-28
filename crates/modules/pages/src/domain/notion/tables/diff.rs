//! What a synced table has that Notion does not (campbooks-cloud#377): the
//! columns, rows and cells that differ from the bases the last sync agreed,
//! as a request for the connectors module.
//!
//! - **Never sent:** columns only Campbooks has (a Campbooks link, people of
//!   this workspace, files, relations, created, edited, ID, and the kinds
//!   Notion's API cannot make), columns and cells of Notion's alone
//!   (formula, rollup, people, everything read only), and anything in
//!   conflict until the member chooses.
//! - **A column removed here** is sent only when the member confirmed it
//!   (`remove_confirmed`), and what Notion holds for it must still be what
//!   the last sync saw.
//! - **A row trashed here** is sent as a trash at Notion, never a deletion.

use std::collections::{HashMap, HashSet};

use campbooks_kernel::Result;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgConnection;
use uuid::Uuid;

use super::{
    ExternalTable, MAX_ITEMS, PropertyBase,
    apply::hash_of,
    cell_bases, config_hash, convert, hex,
    model::{ChangeDocument, ChangeItem, NewCell, bytes32},
    null_hash, property_bases,
    requests::StoredItem,
    type_hash,
};
use crate::{
    domain::{
        PageId, PropertyId,
        tables::{self, PropertyRow},
    },
    properties::PropertyType,
};

#[derive(sqlx::FromRow)]
struct RowFacts {
    id: PageId,
    title: String,
    trashed: bool,
    notion_page: Option<Uuid>,
    link_state: Option<String>,
    link_reason: Option<String>,
}

/// What to ask for.
#[derive(Debug, Default)]
pub struct Plan {
    /// The changes.
    pub items: Vec<StoredItem>,
    /// What they write.
    pub document: ChangeDocument,
}

impl Plan {
    fn push(&mut self, mut item: StoredItem, content: ChangeItem) -> bool {
        if self.items.len() >= MAX_ITEMS {
            return false;
        }
        let seq = self.items.len();
        item.seq = u32::try_from(seq).unwrap_or(u32::MAX);
        let mut content = content;
        content.seq = item.seq;
        self.items.push(item);
        self.document.items.push(content);
        true
    }
}

/// The plan for `table`, whose link is `ext`.
#[allow(
    clippy::too_many_lines,
    reason = "schema, then rows, then cells, then trashed rows"
)]
pub async fn plan(conn: &mut PgConnection, ext: &ExternalTable) -> Result<Plan> {
    let table = ext.table_id;
    let mut plan = Plan {
        items: Vec::new(),
        document: ChangeDocument {
            version: 1,
            items: Vec::new(),
        },
    };
    let props = tables::all_properties(conn, table).await?;
    let bases = property_bases(conn, table).await?;
    let schema_hash = ext.schema_hash.clone();
    let in_conflict: HashSet<PropertyId> = sqlx::query_scalar(
        "SELECT property_id FROM pages.table_conflicts
          WHERE table_id = $1 AND choice IS NULL AND kind = 'column'",
    )
    .bind(table)
    .fetch_all(&mut *conn)
    .await?
    .into_iter()
    .collect();
    let base_of: HashMap<PropertyId, &PropertyBase> =
        bases.iter().map(|b| (b.property_id, b)).collect();

    // --- Columns ---
    for row in &props {
        let Ok(ty) = row.ty() else { continue };
        match (base_of.get(&row.id), row.removed_at.is_some()) {
            (Some(base), true) => {
                if base.remove_confirmed && ty != PropertyType::Title {
                    let mut item = StoredItem::new(0, "schema_remove", *row.id.as_uuid());
                    item.external_property = Some(base.notion_property_id.clone());
                    item.base_hash = bytes32(&base.base_remote_hash).map(|h| hex(&h));
                    item.schema_hash.clone_from(&schema_hash);
                    item.confirmed = true;
                    plan.push(item, ChangeItem::default());
                }
            }
            (Some(base), false) => {
                if in_conflict.contains(&row.id)
                    || !crate::domain::notion::tables::apply::ours_changed(row, base)
                {
                    continue;
                }
                // Only the kinds that go both ways are ever changed there.
                if !is_two_way(&base.notion_type) {
                    continue;
                }
                let Some((kind, format)) = convert::notion_kind(ty, &row.config) else {
                    continue;
                };
                let retyped = bytes32(&base.base_type_hash) != Some(type_hash(ty));
                let mut item = StoredItem::new(0, "schema_update", *row.id.as_uuid());
                item.external_property = Some(base.notion_property_id.clone());
                item.base_hash = bytes32(&base.base_remote_hash).map(|h| hex(&h));
                item.schema_hash.clone_from(&schema_hash);
                item.name = Some(row.name.clone());
                item.notion_kind = Some(kind.to_owned());
                item.type_hash = Some(hex(&type_hash(ty)));
                item.options_hash = Some(hex(&config_hash(&row.config)));
                let content = ChangeItem {
                    name: (row.name != base.base_name).then(|| row.name.clone()),
                    kind: Some(kind.to_owned()),
                    format: format.filter(|_| ty == PropertyType::Number),
                    options: if retyped
                        || bytes32(&base.base_options_hash) != Some(config_hash(&row.config))
                    {
                        convert::options_to_add(&row.config)
                    } else {
                        Vec::new()
                    },
                    ..ChangeItem::default()
                };
                plan.push(item, content);
            }
            (None, false) => {
                let Some((kind, format)) = convert::notion_kind(ty, &row.config) else {
                    continue;
                };
                if ty == PropertyType::Title || !convert::creatable(kind) {
                    continue;
                }
                let mut item = StoredItem::new(0, "schema_add", *row.id.as_uuid());
                item.name = Some(row.name.clone());
                item.notion_kind = Some(kind.to_owned());
                item.type_hash = Some(hex(&type_hash(ty)));
                item.options_hash = Some(hex(&config_hash(&row.config)));
                item.schema_hash.clone_from(&schema_hash);
                let content = ChangeItem {
                    name: Some(row.name.clone()),
                    kind: Some(kind.to_owned()),
                    format,
                    options: convert::options_to_add(&row.config),
                    ..ChangeItem::default()
                };
                plan.push(item, content);
            }
            (None, true) => {}
        }
    }

    // --- Rows and cells ---
    let live: Vec<(&PropertyRow, &PropertyBase, PropertyType)> = props
        .iter()
        .filter(|r| r.removed_at.is_none() && !in_conflict.contains(&r.id))
        .filter_map(|r| {
            let base = base_of.get(&r.id)?;
            let ty = r.ty().ok()?;
            (is_two_way(&base.notion_type) && convert::notion_kind(ty, &r.config).is_some())
                .then_some((r, *base, ty))
        })
        .collect();
    if live.is_empty() {
        return Ok(plan);
    }
    let rows: Vec<RowFacts> = sqlx::query_as(
        "SELECT p.id, p.title, p.trashed_at IS NOT NULL AS trashed, l.notion_page_id AS notion_page,
                l.state AS link_state, l.stopped_reason AS link_reason
           FROM pages.table_rows tr
           JOIN pages.pages p ON p.id = tr.row_id
           LEFT JOIN pages.external_links l ON l.page_id = p.id
          WHERE tr.table_id = $1
          ORDER BY tr.number",
    )
    .bind(table)
    .fetch_all(&mut *conn)
    .await?;
    let ids: Vec<PageId> = rows.iter().map(|r| r.id).collect();
    let values: HashMap<(PageId, PropertyId), Value> =
        sqlx::query_as::<_, (PageId, PropertyId, Value)>(
            "SELECT row_id, property_id, value FROM pages.row_values
              WHERE table_id = $1 AND row_id = ANY($2)",
        )
        .bind(table)
        .bind(&ids)
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(|(r, p, v)| ((r, p), v))
        .collect();
    let cell_base = cell_bases(conn, table, None).await?;
    let held: HashSet<(PageId, PropertyId)> = sqlx::query_as::<_, (PageId, PropertyId)>(
        "SELECT row_id, property_id FROM pages.table_conflicts
          WHERE table_id = $1 AND choice IS NULL AND kind = 'cell'",
    )
    .bind(table)
    .fetch_all(&mut *conn)
    .await?
    .into_iter()
    .collect();
    // What a row holds in a column, in this side's form.
    let ours = |row: &RowFacts, column: &PropertyRow, ty: PropertyType| -> Value {
        if ty == PropertyType::Title {
            Value::String(row.title.clone())
        } else {
            values
                .get(&(row.id, column.id))
                .cloned()
                .unwrap_or(Value::Null)
        }
    };
    for row in &rows {
        let syncing = row.link_state.as_deref() == Some("syncing");
        let gone_there = row.link_state.as_deref() == Some("stopped")
            && row.link_reason.as_deref() == Some("trashed_in_notion");
        let unlinked = row.link_state.is_none() || gone_there;
        if row.trashed {
            // A row put in the trash here: trashed at Notion, once.
            let has_no_bases = !cell_base.keys().any(|(r, _)| *r == row.id);
            if row.link_reason.as_deref() == Some("trashed_here")
                && let Some(page) = row.notion_page
                && !has_no_bases
            {
                let edited: Option<DateTime<Utc>> = cell_base
                    .iter()
                    .filter(|((r, _), _)| *r == row.id)
                    .map(|(_, b)| b.edited)
                    .max();
                let mut item = StoredItem::new(0, "row_trash", *row.id.as_uuid());
                item.external_page = Some(page);
                item.base_edited = edited;
                plan.push(item, ChangeItem::default());
            }
            continue;
        }
        if unlinked {
            // A row only Campbooks has: sent whole.
            let mut cells = Vec::new();
            for (column, base, ty) in &live {
                let value = ours(row, column, *ty);
                if hash_of(&value) == null_hash() {
                    continue;
                }
                let Some((kind, _)) = convert::notion_kind(*ty, &column.config) else {
                    continue;
                };
                cells.push(NewCell {
                    property: base.notion_property_id.clone(),
                    kind: kind.to_owned(),
                    value: convert::to_neutral(*ty, &column.config, &value),
                });
            }
            let mut item = StoredItem::new(0, "row_create", *row.id.as_uuid());
            item.notion_kind = None;
            plan.push(
                item,
                ChangeItem {
                    cells,
                    ..ChangeItem::default()
                },
            );
            continue;
        }
        let (true, Some(page)) = (syncing, row.notion_page) else {
            continue;
        };
        for (column, base, ty) in &live {
            if held.contains(&(row.id, column.id)) {
                continue;
            }
            let value = ours(row, column, *ty);
            let h = hash_of(&value);
            let known = cell_base.get(&(row.id, column.id));
            if h == known.map_or_else(null_hash, |b| b.ours) {
                continue;
            }
            let Some((kind, _)) = convert::notion_kind(*ty, &column.config) else {
                continue;
            };
            let mut item = StoredItem::new(0, "cell_update", *row.id.as_uuid());
            item.property = Some(*column.id.as_uuid());
            item.external_property = Some(base.notion_property_id.clone());
            item.external_page = Some(page);
            item.base_hash = Some(hex(&known.map_or_else(null_hash, |b| b.theirs)));
            item.notion_kind = Some(kind.to_owned());
            item.hash = Some(hex(&h));
            let content = ChangeItem {
                kind: Some(kind.to_owned()),
                value: Some(convert::to_neutral(*ty, &column.config, &value)),
                ..ChangeItem::default()
            };
            if !plan.push(item, content) {
                break;
            }
        }
    }
    Ok(plan)
}

fn is_two_way(notion_type: &str) -> bool {
    matches!(
        notion_type,
        "title"
            | "rich_text"
            | "number"
            | "select"
            | "multi_select"
            | "status"
            | "date"
            | "checkbox"
            | "url"
            | "email"
            | "phone_number"
    )
}
