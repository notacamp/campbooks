//! Applying a Notion database snapshot to the table linked to it
//! (campbooks-cloud#377), in the caller's transaction.
//!
//! Three-way, against the base the last sync agreed:
//!
//! - **Columns.** Theirs changed alone: written in (a retype keeps the old
//!   values for 30 days, like any retype). Ours changed alone: left for the
//!   outbound path. Both changed: the same result is just a new base,
//!   anything else is a column conflict and nothing moves. A column new at
//!   Notion is added (or adopted, when this table has one of that name and
//!   type); one gone at Notion and unchanged here is removed (its values
//!   kept 30 days), one changed here is kept and sent again. A column removed
//!   here and changed at Notion is kept.
//! - **Cells.** The same, per cell. Different cells never conflict.
//! - **Read-only kinds** (formula, rollup, people, anything Notion alone
//!   knows) are always Notion's value.
//!
//! No name, option or value leaves this file in an error or a log.

use std::collections::{HashMap, HashSet};

use campbooks_kernel::{
    AppState, Error, Result,
    tenancy::{Access, UserId, WorkspaceId},
};
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use super::{
    CellBase, ExternalTable, PropertyBase, cell_bases, config_hash,
    convert::{self, Mapped},
    model::{NEntry, NProperty, NSchema, Snapshot, bytes32, ours_hash, remote_hash, unhex},
    null_hash, property_bases, type_hash, write_cell_base, write_property_base,
};
use crate::{
    domain::{
        PageId, PageRow, Place, PropertyId, announce, cells, clean_title, position,
        tables::{self, NewPageRow, PropertyRow},
        tracked,
    },
    events::{LinkKind, NotionLinkRequested, PageChangeKind, TableChangeKind},
    properties::{MAX_PROPERTIES, PropertyType, clean_name, config as property_config},
};

/// Most rows of one table whose bodies get a stream of their own (Notion's
/// request budget is shared by a whole connection).
pub const BODY_LINK_LIMIT: i64 = 100;

/// What an apply moved.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Counts {
    /// Columns added, changed or removed.
    pub columns: usize,
    /// Rows added or removed.
    pub rows: usize,
    /// Cells written.
    pub cells: usize,
    /// Column conflicts newly raised.
    pub column_conflicts: usize,
    /// Cell conflicts newly raised.
    pub cell_conflicts: usize,
}

impl Counts {
    /// Everything that moved.
    pub const fn moved(&self) -> usize {
        self.columns + self.rows + self.cells
    }
}

/// Empty and absent are one: `null`.
pub fn canon(value: &Value) -> Value {
    match value {
        Value::String(text) if text.is_empty() => Value::Null,
        Value::Array(items) if items.is_empty() => Value::Null,
        other => other.clone(),
    }
}

/// The ours-form hash of a cell value.
pub fn hash_of(value: &Value) -> [u8; 32] {
    ours_hash(&canon(value))
}

/// Apply `snapshot` to `table` (locked, live) whose link is `ext` (locked).
#[allow(
    clippy::too_many_lines,
    reason = "schema, then rows, then the ones gone"
)]
pub async fn apply(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    table: &PageRow,
    ext: &ExternalTable,
    snapshot: &Snapshot,
) -> Result<Counts> {
    let editor = ext.created_by;
    let access = Access {
        workspace,
        user: Some(editor),
        role: None,
    };
    let mut counts = Counts::default();
    if let Some(schema) = &snapshot.schema {
        apply_schema(tx, workspace, table.id, editor, schema, &mut counts).await?;
    }
    let props = tables::all_properties(tx, table.id).await?;
    let bases = property_bases(tx, table.id).await?;
    let by_notion = mapped_columns(&props, &bases);
    apply_entries(
        state,
        tx,
        &access,
        table,
        ext,
        snapshot,
        &by_notion,
        &mut counts,
    )
    .await?;
    if snapshot.sweep
        && let Some(present) = &snapshot.present
    {
        remove_gone(
            state,
            tx,
            &access,
            table,
            ext,
            present,
            &by_notion,
            &mut counts,
        )
        .await?;
    }
    if counts.columns > 0 {
        tables::changed(state, tx, &access, table.id, &[TableChangeKind::Schema]).await?;
    }
    if counts.rows > 0 {
        tables::changed(state, tx, &access, table.id, &[TableChangeKind::Rows]).await?;
    }
    Ok(counts)
}

/// A column that is mapped to a Notion column, live.
#[derive(Debug, Clone)]
pub struct Column {
    pub row: PropertyRow,
    pub base: PropertyBase,
    pub ty: PropertyType,
}

fn mapped_columns(props: &[PropertyRow], bases: &[PropertyBase]) -> HashMap<String, Column> {
    let mut out = HashMap::new();
    for base in bases {
        let Some(row) = props.iter().find(|p| p.id == base.property_id) else {
            continue;
        };
        if row.removed_at.is_some() {
            continue;
        }
        let Ok(ty) = row.ty() else {
            continue;
        };
        out.insert(
            base.notion_property_id.clone(),
            Column {
                row: row.clone(),
                base: base.clone(),
                ty,
            },
        );
    }
    out
}

fn base_hashes(base: &PropertyBase) -> Option<([u8; 32], [u8; 32], [u8; 32])> {
    Some((
        bytes32(&base.base_type_hash)?,
        bytes32(&base.base_options_hash)?,
        bytes32(&base.base_remote_hash)?,
    ))
}

/// Whether this side changed the column since the base.
pub fn ours_changed(row: &PropertyRow, base: &PropertyBase) -> bool {
    let Ok(ty) = row.ty() else {
        return true;
    };
    let Some((type_h, options_h, _)) = base_hashes(base) else {
        return true;
    };
    row.name != base.base_name || type_hash(ty) != type_h || config_hash(&row.config) != options_h
}

fn theirs_changed(p: &NProperty, base: &PropertyBase) -> bool {
    let Some((_, _, remote)) = base_hashes(base) else {
        return true;
    };
    unhex(&p.hash) != Some(remote)
}

async fn apply_schema(
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    table: PageId,
    editor: UserId,
    schema: &NSchema,
    counts: &mut Counts,
) -> Result<()> {
    let mut props = tables::all_properties(tx, table).await?;
    let mut bases = property_bases(tx, table).await?;
    let mut seen: HashSet<&str> = HashSet::new();
    for p in &schema.properties {
        seen.insert(p.id.as_str());
        let m = convert::mapped(p);
        let base = bases.iter().find(|b| b.notion_property_id == p.id).cloned();
        match base {
            None => {
                if add_property(tx, workspace, table, editor, p, &m, &props, &bases).await? {
                    counts.columns += 1;
                }
                props = tables::all_properties(tx, table).await?;
                bases = property_bases(tx, table).await?;
            }
            Some(base) => {
                let Some(row) = props.iter().find(|r| r.id == base.property_id).cloned() else {
                    continue;
                };
                three_way(tx, workspace, table, editor, &row, &base, p, &m, counts).await?;
            }
        }
    }
    // Columns Notion no longer has.
    for base in bases
        .iter()
        .filter(|b| !seen.contains(b.notion_property_id.as_str()))
    {
        let Some(row) = props.iter().find(|r| r.id == base.property_id) else {
            continue;
        };
        let removed_here = row.removed_at.is_some();
        let keep = !removed_here && ours_changed(row, base);
        sqlx::query("DELETE FROM pages.external_properties WHERE property_id = $1")
            .bind(base.property_id)
            .execute(&mut **tx)
            .await?;
        sqlx::query("DELETE FROM pages.external_cells WHERE property_id = $1")
            .bind(base.property_id)
            .execute(&mut **tx)
            .await?;
        sqlx::query("DELETE FROM pages.table_conflicts WHERE property_id = $1 AND choice IS NULL")
            .bind(base.property_id)
            .execute(&mut **tx)
            .await?;
        if !removed_here && !keep && row.ty()? != PropertyType::Title {
            sqlx::query("UPDATE pages.table_properties SET removed_at = now() WHERE id = $1")
                .bind(base.property_id)
                .execute(&mut **tx)
                .await?;
            counts.columns += 1;
        }
    }
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "one call site; the values are distinct"
)]
async fn add_property(
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    table: PageId,
    editor: UserId,
    p: &NProperty,
    m: &Mapped,
    props: &[PropertyRow],
    bases: &[PropertyBase],
) -> Result<bool> {
    let name = clean_name(&p.name).unwrap_or_else(|_| "Untitled".to_owned());
    let remote = unhex(&p.hash).unwrap_or_else(|| remote_hash(&json!(p.hash)));
    let mapped_ids: HashSet<PropertyId> = bases.iter().map(|b| b.property_id).collect();
    // The title is the table's own; a column of the same name and type that
    // is not mapped yet is the same column.
    let adopted = props.iter().find(|r| {
        r.removed_at.is_none()
            && !mapped_ids.contains(&r.id)
            && if p.kind == "title" {
                r.ty().is_ok_and(|ty| ty == PropertyType::Title)
            } else {
                r.name.trim().eq_ignore_ascii_case(name.trim()) && r.ty().is_ok_and(|ty| ty == m.ty)
            }
    });
    if let Some(row) = adopted {
        let ty = row.ty()?;
        if p.kind == "title" && row.name != name {
            sqlx::query("UPDATE pages.table_properties SET name = $2 WHERE id = $1")
                .bind(row.id)
                .bind(&name)
                .execute(&mut **tx)
                .await?;
        }
        let shown = if p.kind == "title" {
            name.as_str()
        } else {
            row.name.as_str()
        };
        write_property_base(
            tx,
            workspace,
            table,
            row.id,
            &p.id,
            &p.kind,
            (shown, &type_hash(ty), &config_hash(&row.config), &remote),
        )
        .await?;
        return Ok(false);
    }
    let live = props.iter().filter(|r| r.removed_at.is_none()).count();
    if live >= MAX_PROPERTIES {
        return Ok(false);
    }
    let last: Option<String> =
        sqlx::query_scalar("SELECT max(position) FROM pages.table_properties WHERE table_id = $1")
            .bind(table)
            .fetch_one(&mut **tx)
            .await?;
    let key = position::after(last.as_deref());
    let id = PropertyId::new();
    sqlx::query(
        "INSERT INTO pages.table_properties
             (id, table_id, workspace_id, name, type, config, position, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(id)
    .bind(table)
    .bind(workspace.as_uuid())
    .bind(&name)
    .bind(m.ty.as_str())
    .bind(&m.config)
    .bind(&key)
    .bind(editor)
    .execute(&mut **tx)
    .await?;
    write_property_base(
        tx,
        workspace,
        table,
        id,
        &p.id,
        &p.kind,
        (&name, &type_hash(m.ty), &config_hash(&m.config), &remote),
    )
    .await?;
    Ok(true)
}

fn select_like(ty: PropertyType) -> bool {
    matches!(
        ty,
        PropertyType::Select | PropertyType::MultiSelect | PropertyType::Status
    )
}

/// Notion's options with the ids this table already uses for them.
fn merged_options(ty: PropertyType, ours: &Value, theirs: &Value) -> Value {
    let mine = property_config::options_of(ours);
    let listed: Vec<Value> = property_config::options_of(theirs)
        .into_iter()
        .map(|o| {
            let id = mine
                .iter()
                .find(|m| m.id == o.id)
                .or_else(|| {
                    mine.iter()
                        .find(|m| m.name.trim().eq_ignore_ascii_case(o.name.trim()))
                })
                .map_or(o.id.clone(), |m| m.id.clone());
            let mut out = json!({"id": id, "name": o.name, "color": o.color});
            if let Some(group) = o.group {
                out["group"] = serde_json::to_value(group).unwrap_or(Value::Null);
            }
            out
        })
        .collect();
    property_config::normalise(ty, &json!({"options": listed})).unwrap_or_else(|_| theirs.clone())
}

/// The cells of `property` that name an option the config no longer has,
/// cleared.
async fn drop_unknown_options(
    tx: &mut Transaction<'_, Postgres>,
    editor: UserId,
    property: PropertyId,
    ty: PropertyType,
    config: &Value,
) -> Result<()> {
    let known: HashSet<String> = property_config::options_of(config)
        .into_iter()
        .map(|o| o.id)
        .collect();
    let held: Vec<(PageId, Value)> = sqlx::query_as(
        "SELECT row_id, value FROM pages.row_values
          WHERE property_id = $1 AND value IS NOT NULL AND value <> 'null'::jsonb FOR UPDATE",
    )
    .bind(property)
    .fetch_all(&mut **tx)
    .await?;
    for (row, value) in held {
        let next = match (ty, &value) {
            (PropertyType::MultiSelect, Value::Array(items)) => {
                let kept: Vec<Value> = items
                    .iter()
                    .filter(|i| i.as_str().is_some_and(|id| known.contains(id)))
                    .cloned()
                    .collect();
                if kept.len() == items.len() {
                    continue;
                }
                if kept.is_empty() {
                    Value::Null
                } else {
                    Value::Array(kept)
                }
            }
            (_, Value::String(id)) if !known.contains(id) => Value::Null,
            _ => continue,
        };
        sqlx::query(
            "UPDATE pages.row_values SET value = $3, version = version + 1, updated_by = $4,
                    updated_at = now()
              WHERE row_id = $1 AND property_id = $2",
        )
        .bind(row)
        .bind(property)
        .bind(next)
        .bind(editor)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

/// Put Notion's version of a column in place of ours, and make it the base.
#[allow(
    clippy::too_many_arguments,
    reason = "one call site; the values are distinct"
)]
pub async fn take_theirs(
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    table: PageId,
    editor: UserId,
    row: &PropertyRow,
    notion: (&str, &str),
    (name, ty, config): (&str, PropertyType, &Value),
    remote: &[u8; 32],
) -> Result<()> {
    let old_ty = row.ty()?;
    let retyped = old_ty != ty;
    let mut config = config.clone();
    if !retyped && select_like(ty) {
        config = merged_options(ty, &row.config, &config);
    }
    if retyped {
        // Notion's values replace ours (the full read that follows a change
        // of columns brings them); ours are kept 30 days for a restore.
        sqlx::query(
            "UPDATE pages.row_values
                SET previous = value, value = NULL, version = version + 1, updated_by = $2,
                    updated_at = now()
              WHERE property_id = $1 AND value IS NOT NULL AND value <> 'null'::jsonb",
        )
        .bind(row.id)
        .bind(editor)
        .execute(&mut **tx)
        .await?;
        sqlx::query(
            "UPDATE pages.table_properties
                SET name = $2, type = $3, config = $4, previous_type = $5, previous_config = $6,
                    converted_at = now()
              WHERE id = $1",
        )
        .bind(row.id)
        .bind(name)
        .bind(ty.as_str())
        .bind(&config)
        .bind(old_ty.as_str())
        .bind(&row.config)
        .execute(&mut **tx)
        .await?;
        sqlx::query("DELETE FROM pages.external_cells WHERE property_id = $1")
            .bind(row.id)
            .execute(&mut **tx)
            .await?;
    } else {
        sqlx::query("UPDATE pages.table_properties SET name = $2, config = $3 WHERE id = $1")
            .bind(row.id)
            .bind(name)
            .bind(&config)
            .execute(&mut **tx)
            .await?;
        if select_like(ty) {
            drop_unknown_options(tx, editor, row.id, ty, &config).await?;
        }
    }
    write_property_base(
        tx,
        workspace,
        table,
        row.id,
        notion.0,
        notion.1,
        (name, &type_hash(ty), &config_hash(&config), remote),
    )
    .await
}

#[allow(
    clippy::too_many_arguments,
    reason = "one call site; the values are distinct"
)]
async fn three_way(
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    table: PageId,
    editor: UserId,
    row: &PropertyRow,
    base: &PropertyBase,
    p: &NProperty,
    m: &Mapped,
    counts: &mut Counts,
) -> Result<()> {
    let remote = unhex(&p.hash).unwrap_or_else(|| remote_hash(&json!(p.hash)));
    let theirs = theirs_changed(p, base);
    let removed_here = row.removed_at.is_some();
    if !theirs {
        return Ok(());
    }
    let name = clean_name(&p.name).unwrap_or_else(|_| row.name.clone());
    let notion = (p.id.as_str(), p.kind.as_str());
    if removed_here {
        // A column removed here and changed at Notion is kept.
        sqlx::query("UPDATE pages.table_properties SET removed_at = NULL WHERE id = $1")
            .bind(row.id)
            .execute(&mut **tx)
            .await?;
        take_theirs(
            tx,
            workspace,
            table,
            editor,
            row,
            notion,
            (&name, m.ty, &m.config),
            &remote,
        )
        .await?;
        counts.columns += 1;
        return Ok(());
    }
    let ours = ours_changed(row, base);
    if !ours || !m.two_way {
        take_theirs(
            tx,
            workspace,
            table,
            editor,
            row,
            notion,
            (&name, m.ty, &m.config),
            &remote,
        )
        .await?;
        close_column_conflict(tx, row.id).await?;
        counts.columns += 1;
        return Ok(());
    }
    // Both changed.
    let ty = row.ty()?;
    let merged = if ty == m.ty && select_like(ty) {
        merged_options(ty, &row.config, &m.config)
    } else {
        m.config.clone()
    };
    if row.name == name && ty == m.ty && row.config == merged {
        write_property_base(
            tx,
            workspace,
            table,
            row.id,
            notion.0,
            notion.1,
            (&name, &type_hash(ty), &config_hash(&row.config), &remote),
        )
        .await?;
        close_column_conflict(tx, row.id).await?;
        return Ok(());
    }
    let raised: bool = sqlx::query_scalar(
        "INSERT INTO pages.table_conflicts
             (id, table_id, workspace_id, kind, property_id, ours, theirs, ours_at, theirs_at,
              theirs_remote_hash)
         VALUES ($1, $2, $3, 'column', $4, $5, $6, now(), now(), $7)
         ON CONFLICT (property_id) WHERE choice IS NULL AND kind = 'column' DO UPDATE
            SET theirs = EXCLUDED.theirs, theirs_at = EXCLUDED.theirs_at,
                theirs_remote_hash = EXCLUDED.theirs_remote_hash
         RETURNING (xmax = 0)",
    )
    .bind(Uuid::now_v7())
    .bind(table)
    .bind(workspace.as_uuid())
    .bind(row.id)
    .bind(json!({"name": row.name, "type": ty.as_str(), "config": row.config}))
    .bind(
        json!({"name": name, "type": m.ty.as_str(), "config": merged,
                 "notion": [p.id, p.kind]}),
    )
    .bind(remote.as_slice())
    .fetch_one(&mut **tx)
    .await?;
    if raised {
        counts.column_conflicts += 1;
    }
    Ok(())
}

async fn close_column_conflict(
    tx: &mut Transaction<'_, Postgres>,
    property: PropertyId,
) -> Result<()> {
    sqlx::query("DELETE FROM pages.table_conflicts WHERE property_id = $1 AND kind = 'column' AND choice IS NULL")
        .bind(property)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

// --- Rows and cells --------------------------------------------------------

/// A row's link, as the apply reads it.
#[derive(Debug, Clone, sqlx::FromRow)]
struct RowLink {
    page_id: PageId,
    notion_page_id: Uuid,
    state: String,
    stopped_reason: Option<String>,
}

/// A row as the apply reads it.
#[derive(Debug, Clone)]
struct Held {
    title: String,
    edited_at: DateTime<Utc>,
    trashed: bool,
}

async fn load_rows(
    tx: &mut Transaction<'_, Postgres>,
    ids: &[PageId],
) -> Result<HashMap<PageId, Held>> {
    let found: Vec<(PageId, String, DateTime<Utc>, bool)> = sqlx::query_as(
        "SELECT id, title, edited_at, trashed_at IS NOT NULL FROM pages.pages WHERE id = ANY($1)",
    )
    .bind(ids)
    .fetch_all(&mut **tx)
    .await?;
    Ok(found
        .into_iter()
        .map(|(id, title, edited_at, trashed)| {
            (
                id,
                Held {
                    title,
                    edited_at,
                    trashed,
                },
            )
        })
        .collect())
}

#[allow(
    clippy::too_many_arguments,
    reason = "one call site; the values are distinct"
)]
#[allow(
    clippy::too_many_lines,
    reason = "one pass over the entries of a snapshot"
)]
async fn apply_entries(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    access: &Access,
    table: &PageRow,
    ext: &ExternalTable,
    snapshot: &Snapshot,
    columns: &HashMap<String, Column>,
    counts: &mut Counts,
) -> Result<()> {
    if snapshot.entries.is_empty() {
        return Ok(());
    }
    let workspace = access.workspace;
    let editor = ext.created_by;
    let notion_ids: Vec<Uuid> = snapshot.entries.iter().map(|e| e.id).collect();
    let links: HashMap<Uuid, RowLink> = sqlx::query_as::<_, RowLink>(
        "SELECT l.page_id, l.notion_page_id, l.state, l.stopped_reason
           FROM pages.external_links l JOIN pages.table_rows tr ON tr.row_id = l.page_id
          WHERE tr.table_id = $1 AND l.notion_page_id = ANY($2)",
    )
    .bind(table.id)
    .bind(&notion_ids)
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .map(|l| (l.notion_page_id, l))
    .collect();
    // Entries already linked elsewhere (as pages, or in another table) are
    // none of this table's.
    let elsewhere: HashSet<Uuid> = sqlx::query_scalar(
        "SELECT notion_page_id FROM pages.external_links
          WHERE connection_id = $1 AND notion_page_id = ANY($2) AND state <> 'stopped'
            AND page_id NOT IN (SELECT row_id FROM pages.table_rows WHERE table_id = $3)",
    )
    .bind(ext.connection_id)
    .bind(&notion_ids)
    .bind(table.id)
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .collect();
    let mut row_ids: Vec<PageId> = links.values().map(|l| l.page_id).collect();
    let mut held = load_rows(tx, &row_ids).await?;
    let mut bases = cell_bases(tx, table.id, Some(&row_ids)).await?;
    let mut values: HashMap<(PageId, PropertyId), Value> =
        sqlx::query_as::<_, (PageId, PropertyId, Value)>(
            "SELECT row_id, property_id, value FROM pages.row_values
              WHERE table_id = $1 AND row_id = ANY($2)",
        )
        .bind(table.id)
        .bind(&row_ids)
        .fetch_all(&mut **tx)
        .await?
        .into_iter()
        .map(|(r, p, v)| ((r, p), v))
        .collect();
    let mut linked_bodies: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pages.external_links l
           JOIN pages.table_rows tr ON tr.row_id = l.page_id
          WHERE tr.table_id = $1 AND l.state = 'syncing'",
    )
    .bind(table.id)
    .fetch_one(&mut **tx)
    .await?;
    let mut live_rows = tables::live_rows(tx, table.id).await?;
    for entry in &snapshot.entries {
        if elsewhere.contains(&entry.id) {
            continue;
        }
        let row_id = match links.get(&entry.id) {
            Some(link) if link.stopped_reason.as_deref() == Some("trashed_here") => continue,
            Some(link)
                if link.state == "stopped"
                    && link.stopped_reason.as_deref() == Some("trashed_in_notion") =>
            {
                continue;
            }
            Some(link) => link.page_id,
            None => {
                if live_rows >= tables::MAX_ROWS {
                    continue;
                }
                let title = entry_title(entry, columns);
                let id = make_row(
                    state,
                    tx,
                    access,
                    table,
                    ext,
                    entry.id,
                    &title,
                    linked_bodies < BODY_LINK_LIMIT,
                )
                .await?;
                linked_bodies += 1;
                live_rows += 1;
                counts.rows += 1;
                row_ids.push(id);
                held.insert(
                    id,
                    Held {
                        title: String::new(),
                        edited_at: Utc::now(),
                        trashed: false,
                    },
                );
                id
            }
        };
        if held.get(&row_id).is_none_or(|h| h.trashed) {
            continue;
        }
        let moved = merge_row(
            tx,
            access,
            table,
            ext,
            entry,
            row_id,
            columns,
            &mut held,
            &mut bases,
            &mut values,
            counts,
        )
        .await?;
        if moved.0 || moved.1 {
            cells::touch(tx, editor, row_id).await?;
            let mut kinds = vec![PageChangeKind::Values];
            if moved.1 {
                kinds.push(PageChangeKind::Meta);
            }
            announce(state, tx, access, row_id, kinds).await?;
        }
    }
    let _ = workspace;
    Ok(())
}

fn entry_title(entry: &NEntry, columns: &HashMap<String, Column>) -> String {
    columns
        .iter()
        .find(|(_, c)| c.ty == PropertyType::Title)
        .and_then(|(id, _)| entry.cells.iter().find(|c| &c.property == id))
        .map(|c| convert::title_of(&c.value))
        .unwrap_or_default()
}

#[allow(
    clippy::too_many_arguments,
    reason = "one call site; the values are distinct"
)]
async fn make_row(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    access: &Access,
    table: &PageRow,
    ext: &ExternalTable,
    notion_page: Uuid,
    title: &str,
    body_stream: bool,
) -> Result<PageId> {
    let editor = ext.created_by;
    let number: i64 = sqlx::query_scalar(
        "UPDATE pages.tables SET next_number = next_number + 1 WHERE table_id = $1
         RETURNING next_number - 1",
    )
    .bind(table.id)
    .fetch_one(&mut **tx)
    .await?;
    let place: Place = table.place()?;
    let key = position::next(tx, Some(table.id), place, editor).await?;
    let id = PageId::new();
    let title = clean_title(title).unwrap_or_default();
    tables::insert_page(
        tx,
        NewPageRow {
            id,
            workspace: access.workspace,
            parent: Some(table.id),
            place,
            position: &key,
            title: &title,
            icon: None,
            creator: editor,
            kind: crate::domain::Kind::Row,
        },
    )
    .await?;
    sqlx::query(
        "INSERT INTO pages.table_rows (row_id, table_id, workspace_id, number)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(id)
    .bind(table.id)
    .bind(access.workspace.as_uuid())
    .bind(number)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "INSERT INTO pages.external_links
             (page_id, workspace_id, connection_id, notion_page_id, state, created_by, last_title)
         VALUES ($1, $2, $3, $4, 'syncing', $5, $6)",
    )
    .bind(id)
    .bind(access.workspace.as_uuid())
    .bind(ext.connection_id)
    .bind(notion_page)
    .bind(editor)
    .bind(&title)
    .execute(&mut **tx)
    .await?;
    if body_stream {
        state
            .events
            .publish(
                tx,
                &NotionLinkRequested {
                    workspace: access.workspace.to_string(),
                    page: id,
                    connection: ext.connection_id,
                    notion_page,
                    kind: LinkKind::Page,
                },
            )
            .await?;
    }
    announce(state, tx, access, id, vec![PageChangeKind::Meta]).await?;
    tracked::simple(state, tx, access.workspace, id, tracked::Change::Created).await?;
    Ok(id)
}

/// Merge `entry`'s cells into `row`. Whether values and the title moved.
#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "one pass over a row's cells"
)]
async fn merge_row(
    tx: &mut Transaction<'_, Postgres>,
    access: &Access,
    table: &PageRow,
    ext: &ExternalTable,
    entry: &NEntry,
    row: PageId,
    columns: &HashMap<String, Column>,
    held: &mut HashMap<PageId, Held>,
    bases: &mut HashMap<(PageId, PropertyId), CellBase>,
    values: &mut HashMap<(PageId, PropertyId), Value>,
    counts: &mut Counts,
) -> Result<(bool, bool)> {
    let editor = ext.created_by;
    let (mut moved, mut titled) = (false, false);
    let ours_at = held.get(&row).map_or_else(Utc::now, |h| h.edited_at);
    for cell in &entry.cells {
        let Some(column) = columns.get(&cell.property) else {
            continue;
        };
        let property = column.row.id;
        let v_theirs = convert::from_neutral(column.ty, &column.row.config, &cell.value);
        let current = if column.ty == PropertyType::Title {
            Value::String(held.get(&row).map(|h| h.title.clone()).unwrap_or_default())
        } else {
            values.get(&(row, property)).cloned().unwrap_or(Value::Null)
        };
        let remote = unhex(&cell.hash).unwrap_or_else(|| remote_hash(&cell.value));
        if !column.ty.editable() && column.ty != PropertyType::Title {
            // Read only: Notion's value, always.
            if canon(&current) != canon(&v_theirs) {
                cells::store(tx, access, editor, row, table.id, property, &v_theirs).await?;
                values.insert((row, property), v_theirs);
                counts.cells += 1;
                moved = true;
            }
            continue;
        }
        let base = bases.get(&(row, property)).copied();
        let (base_ours, base_theirs) =
            base.map_or((null_hash(), null_hash()), |b| (b.ours, b.theirs));
        let theirs_changed = remote != base_theirs;
        let h_ours = hash_of(&current);
        let ours_changed = h_ours != base_ours;
        let h_theirs = hash_of(&v_theirs);
        if !theirs_changed {
            if !ours_changed {
                close_cell_conflict(tx, row, property).await?;
            }
            continue;
        }
        if !ours_changed {
            write_value(
                tx, access, editor, table.id, row, property, column.ty, &v_theirs,
            )
            .await?;
            if column.ty == PropertyType::Title {
                titled = true;
                if let Some(h) = held.get_mut(&row) {
                    v_theirs
                        .as_str()
                        .unwrap_or_default()
                        .clone_into(&mut h.title);
                }
                sqlx::query("UPDATE pages.external_links SET last_title = $2 WHERE page_id = $1")
                    .bind(row)
                    .bind(v_theirs.as_str().unwrap_or_default())
                    .execute(&mut **tx)
                    .await?;
            } else {
                values.insert((row, property), v_theirs);
            }
            let new = CellBase {
                ours: h_theirs,
                theirs: remote,
                edited: entry.edited,
            };
            write_cell_base(tx, access.workspace, table.id, row, property, &new).await?;
            bases.insert((row, property), new);
            counts.cells += 1;
            moved = true;
            close_cell_conflict(tx, row, property).await?;
            continue;
        }
        if h_ours == h_theirs {
            let new = CellBase {
                ours: h_ours,
                theirs: remote,
                edited: entry.edited,
            };
            write_cell_base(tx, access.workspace, table.id, row, property, &new).await?;
            bases.insert((row, property), new);
            close_cell_conflict(tx, row, property).await?;
            continue;
        }
        let raised: bool = sqlx::query_scalar(
            "INSERT INTO pages.table_conflicts
                 (id, table_id, workspace_id, kind, property_id, row_id, ours, theirs, ours_at,
                  theirs_at, theirs_remote_hash)
             VALUES ($1, $2, $3, 'cell', $4, $5, $6, $7, $8, $9, $10)
             ON CONFLICT (row_id, property_id) WHERE choice IS NULL AND kind = 'cell' DO UPDATE
                SET theirs = EXCLUDED.theirs, theirs_at = EXCLUDED.theirs_at,
                    theirs_remote_hash = EXCLUDED.theirs_remote_hash
             RETURNING (xmax = 0)",
        )
        .bind(Uuid::now_v7())
        .bind(table.id)
        .bind(access.workspace.as_uuid())
        .bind(property)
        .bind(row)
        .bind(&current)
        .bind(&v_theirs)
        .bind(ours_at)
        .bind(entry.edited)
        .bind(remote.as_slice())
        .fetch_one(&mut **tx)
        .await?;
        if raised {
            counts.cell_conflicts += 1;
        }
    }
    Ok((moved, titled))
}

async fn close_cell_conflict(
    tx: &mut Transaction<'_, Postgres>,
    row: PageId,
    property: PropertyId,
) -> Result<()> {
    sqlx::query(
        "DELETE FROM pages.table_conflicts
          WHERE row_id = $1 AND property_id = $2 AND kind = 'cell' AND choice IS NULL",
    )
    .bind(row)
    .bind(property)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Store `value` as the cell of `row`'s `property`.
#[allow(clippy::too_many_arguments, reason = "one row of a cell")]
pub async fn write_value(
    tx: &mut Transaction<'_, Postgres>,
    access: &Access,
    editor: UserId,
    table: PageId,
    row: PageId,
    property: PropertyId,
    ty: PropertyType,
    value: &Value,
) -> Result<()> {
    if ty == PropertyType::Title {
        let title = clean_title(value.as_str().unwrap_or_default()).unwrap_or_default();
        sqlx::query(
            "UPDATE pages.pages SET title = $2, meta_revision = meta_revision + 1 WHERE id = $1",
        )
        .bind(row)
        .bind(title)
        .execute(&mut **tx)
        .await?;
    } else {
        cells::store(tx, access, editor, row, table, property, value).await?;
    }
    Ok(())
}

/// Rows that are not at Notion any more: unchanged here, they go to the
/// trash (30 days); changed here, they are kept and sent again.
#[allow(
    clippy::too_many_arguments,
    reason = "one call site; the values are distinct"
)]
#[allow(
    clippy::too_many_lines,
    reason = "one pass over the rows that are gone"
)]
async fn remove_gone(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    access: &Access,
    table: &PageRow,
    ext: &ExternalTable,
    present: &[Uuid],
    columns: &HashMap<String, Column>,
    counts: &mut Counts,
) -> Result<()> {
    let linked: Vec<(PageId, Uuid)> = sqlx::query_as(
        "SELECT l.page_id, l.notion_page_id FROM pages.external_links l
           JOIN pages.table_rows tr ON tr.row_id = l.page_id
           JOIN pages.pages p ON p.id = l.page_id
          WHERE tr.table_id = $1 AND l.connection_id = $2 AND l.state = 'syncing'
            AND p.trashed_at IS NULL",
    )
    .bind(table.id)
    .bind(ext.connection_id)
    .fetch_all(&mut **tx)
    .await?;
    let present: HashSet<Uuid> = present.iter().copied().collect();
    let gone: Vec<PageId> = linked
        .iter()
        .filter(|(_, notion)| !present.contains(notion))
        .map(|(row, _)| *row)
        .collect();
    if gone.is_empty() {
        return Ok(());
    }
    let titles = load_rows(tx, &gone).await?;
    let bases = cell_bases(tx, table.id, Some(&gone)).await?;
    let values: HashMap<(PageId, PropertyId), Value> =
        sqlx::query_as::<_, (PageId, PropertyId, Value)>(
            "SELECT row_id, property_id, value FROM pages.row_values
              WHERE table_id = $1 AND row_id = ANY($2)",
        )
        .bind(table.id)
        .bind(&gone)
        .fetch_all(&mut **tx)
        .await?
        .into_iter()
        .map(|(r, p, v)| ((r, p), v))
        .collect();
    for row in gone {
        let mut dirty = false;
        for column in columns.values() {
            if !column.ty.editable() && column.ty != PropertyType::Title {
                continue;
            }
            let current = if column.ty == PropertyType::Title {
                Value::String(
                    titles
                        .get(&row)
                        .map(|h| h.title.clone())
                        .unwrap_or_default(),
                )
            } else {
                values
                    .get(&(row, column.row.id))
                    .cloned()
                    .unwrap_or(Value::Null)
            };
            let base = bases
                .get(&(row, column.row.id))
                .map_or_else(null_hash, |b| b.ours);
            if hash_of(&current) != base {
                dirty = true;
                break;
            }
        }
        if dirty {
            // Changed here: kept, and sent again as a row of its own.
            super::super::stop(
                state,
                tx,
                access.workspace,
                row,
                super::super::StopReason::TrashedInNotion,
            )
            .await?;
            sqlx::query("DELETE FROM pages.external_cells WHERE row_id = $1")
                .bind(row)
                .execute(&mut **tx)
                .await?;
            sqlx::query("DELETE FROM pages.table_conflicts WHERE row_id = $1 AND choice IS NULL")
                .bind(row)
                .execute(&mut **tx)
                .await?;
        } else {
            sqlx::query(
                "UPDATE pages.pages
                    SET trashed_at = now(), trashed_by = $2, trash_root = $1, revision = revision + 1
                  WHERE id = $1 AND trashed_at IS NULL",
            )
            .bind(row)
            .bind(ext.created_by)
            .execute(&mut **tx)
            .await?;
            super::super::stop(
                state,
                tx,
                access.workspace,
                row,
                super::super::StopReason::TrashedInNotion,
            )
            .await?;
            announce(state, tx, access, row, vec![PageChangeKind::Trash]).await?;
        }
        counts.rows += 1;
    }
    Ok(())
}

/// The error for an apply that cannot proceed.
pub fn refused(why: &'static str) -> Error {
    Error::internal(why)
}
