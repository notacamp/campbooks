//! The conflicts of a synced table (campbooks-cloud#377): a column or a cell
//! changed in Campbooks and in Notion between two syncs. Neither change is
//! sent until the member chooses; nothing is deleted on either side.
//!
//! **Keep ours** declares what Notion holds now the base, so the member's
//! version is what differs from it and goes out with the next push (checked
//! against Notion once more before it is written). **Use Notion's** puts
//! Notion's version here and makes it the base. Either way the choice can be
//! undone while nothing has been sent since.

use campbooks_kernel::{
    AppState, Error, Result,
    tenancy::{Access, UserId},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{PgConnection, Postgres, Transaction};
use utoipa::ToSchema;
use uuid::Uuid;

use super::{
    CellBase, apply, cell_bases, config_hash, model::bytes32, open_conflicts, property_bases,
    type_hash, write_cell_base, write_property_base,
};
use crate::{
    domain::{PageId, PropertyId, announce, cells, tables},
    events::{PageChangeKind, TableChangeKind},
    properties::PropertyType,
};

/// Which side a member keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(as = TableConflictKeep)]
pub enum Keep {
    /// Campbooks' version.
    Ours,
    /// Notion's version.
    Theirs,
}

/// What a conflict is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(as = TableConflictKind)]
pub enum Kind {
    /// A column.
    Column,
    /// A cell.
    Cell,
}

/// A conflict, as the API shows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[schema(as = TableConflict)]
pub struct ConflictView {
    /// The conflict.
    pub id: Uuid,
    /// Column or cell.
    pub kind: Kind,
    /// The column.
    pub property: PropertyId,
    /// The row, for a cell.
    pub row: Option<PageId>,
    /// Campbooks' version: a cell's value, or a column's name, type and
    /// config.
    pub ours: Value,
    /// Notion's version.
    pub theirs: Value,
    /// When Campbooks' version was made.
    pub ours_at: DateTime<Utc>,
    /// When Notion's was.
    pub theirs_at: DateTime<Utc>,
    /// What was chosen, once it was.
    pub choice: Option<Keep>,
}

/// A conflict row.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Row {
    pub id: Uuid,
    pub table_id: PageId,
    pub kind: String,
    pub property_id: PropertyId,
    pub row_id: Option<PageId>,
    pub ours: Value,
    pub theirs: Value,
    pub ours_at: DateTime<Utc>,
    pub theirs_at: DateTime<Utc>,
    pub theirs_remote_hash: Vec<u8>,
    pub choice: Option<String>,
    pub undone_base: Option<Value>,
}

const COLUMNS: &str = "id, table_id, kind, property_id, row_id, ours, theirs, ours_at, theirs_at, \
                       theirs_remote_hash, choice, undone_base";

impl Row {
    /// The conflict as the API shows it.
    pub fn view(&self) -> ConflictView {
        ConflictView {
            id: self.id,
            kind: if self.kind == "column" {
                Kind::Column
            } else {
                Kind::Cell
            },
            property: self.property_id,
            row: self.row_id,
            ours: self.ours.clone(),
            theirs: self.theirs.clone(),
            ours_at: self.ours_at,
            theirs_at: self.theirs_at,
            choice: match self.choice.as_deref() {
                Some("ours") => Some(Keep::Ours),
                Some("theirs") => Some(Keep::Theirs),
                _ => None,
            },
        }
    }
}

/// The conflicts of `table` waiting for a choice.
pub async fn open(conn: &mut PgConnection, table: PageId) -> Result<Vec<Row>> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM pages.table_conflicts
          WHERE table_id = $1 AND choice IS NULL ORDER BY raised_at, id"
    )))
    .bind(table)
    .fetch_all(conn)
    .await?)
}

/// Conflict `id` of `table`, locked.
pub async fn lock(conn: &mut PgConnection, table: PageId, id: Uuid) -> Result<Row> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM pages.table_conflicts WHERE table_id = $1 AND id = $2 FOR UPDATE"
    )))
    .bind(table)
    .bind(id)
    .fetch_optional(conn)
    .await?
    .ok_or_else(|| Error::Conflict(crate::MESSAGES.reason("no_table_conflict")))
}

fn remote_of(row: &Row) -> Result<[u8; 32]> {
    bytes32(&row.theirs_remote_hash).ok_or_else(|| Error::internal("a conflict has a bad hash"))
}

/// Settle `conflict` by keeping `keep`. The table is locked.
#[allow(
    clippy::too_many_lines,
    clippy::single_match_else,
    reason = "one choice for a cell and one for a column, each read top to bottom"
)]
pub async fn resolve(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    access: &Access,
    editor: UserId,
    conflict: &Row,
    keep: Keep,
) -> Result<()> {
    let workspace = access.workspace;
    let remote = remote_of(conflict)?;
    let before: Value;
    match (conflict.kind.as_str(), conflict.row_id) {
        ("cell", Some(row)) => {
            let bases = cell_bases(tx, conflict.table_id, Some(&[row])).await?;
            before = bases
                .get(&(row, conflict.property_id))
                .map_or(Value::Null, |b| {
                    json!({"ours": super::hex(&b.ours), "theirs": super::hex(&b.theirs),
                           "edited": b.edited})
                });
            let property =
                tables::property(tx, conflict.table_id, conflict.property_id, false).await?;
            let ty = property.ty()?;
            let theirs = &conflict.theirs;
            match keep {
                Keep::Theirs => {
                    apply::write_value(
                        tx,
                        access,
                        editor,
                        conflict.table_id,
                        row,
                        conflict.property_id,
                        ty,
                        theirs,
                    )
                    .await?;
                    if ty == PropertyType::Title {
                        sqlx::query(
                            "UPDATE pages.external_links SET last_title = $2 WHERE page_id = $1",
                        )
                        .bind(row)
                        .bind(theirs.as_str().unwrap_or_default())
                        .execute(&mut **tx)
                        .await?;
                    }
                    let base = CellBase {
                        ours: apply::hash_of(theirs),
                        theirs: remote,
                        edited: conflict.theirs_at,
                    };
                    write_cell_base(
                        tx,
                        workspace,
                        conflict.table_id,
                        row,
                        conflict.property_id,
                        &base,
                    )
                    .await?;
                    cells::touch(tx, editor, row).await?;
                    announce(
                        state,
                        tx,
                        access,
                        row,
                        vec![PageChangeKind::Values, PageChangeKind::Meta],
                    )
                    .await?;
                }
                Keep::Ours => {
                    // What Notion holds is the base; ours differs from it.
                    let base = CellBase {
                        ours: apply::hash_of(theirs),
                        theirs: remote,
                        edited: conflict.theirs_at,
                    };
                    write_cell_base(
                        tx,
                        workspace,
                        conflict.table_id,
                        row,
                        conflict.property_id,
                        &base,
                    )
                    .await?;
                }
            }
        }
        _ => {
            let bases = property_bases(tx, conflict.table_id).await?;
            let base = bases.iter().find(|b| b.property_id == conflict.property_id);
            before = base.map_or(Value::Null, |b| {
                json!({"name": b.base_name, "type": super::hex(&bytes32(&b.base_type_hash).unwrap_or([0; 32])),
                       "options": super::hex(&bytes32(&b.base_options_hash).unwrap_or([0; 32])),
                       "remote": super::hex(&bytes32(&b.base_remote_hash).unwrap_or([0; 32])),
                       "notion": [b.notion_property_id, b.notion_type]})
            });
            let Some(base) = base else {
                return Err(Error::Conflict(crate::MESSAGES.reason("conflict_gone")));
            };
            let property =
                tables::property(tx, conflict.table_id, conflict.property_id, true).await?;
            let theirs = &conflict.theirs;
            let name = theirs
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or(&property.name)
                .to_owned();
            let ty: PropertyType = theirs
                .get("type")
                .and_then(Value::as_str)
                .and_then(|t| t.parse().ok())
                .unwrap_or(PropertyType::Text);
            let config = theirs.get("config").cloned().unwrap_or_else(|| json!({}));
            match keep {
                Keep::Theirs => {
                    apply::take_theirs(
                        tx,
                        workspace,
                        conflict.table_id,
                        editor,
                        &property,
                        (&base.notion_property_id, &base.notion_type),
                        (&name, ty, &config),
                        &remote,
                    )
                    .await?;
                    tables::changed(
                        state,
                        tx,
                        access,
                        conflict.table_id,
                        &[TableChangeKind::Schema],
                    )
                    .await?;
                }
                Keep::Ours => {
                    write_property_base(
                        tx,
                        workspace,
                        conflict.table_id,
                        conflict.property_id,
                        &base.notion_property_id,
                        &base.notion_type,
                        (&name, &type_hash(ty), &config_hash(&config), &remote),
                    )
                    .await?;
                }
            }
        }
    }
    sqlx::query(
        "UPDATE pages.table_conflicts
            SET choice = $2, resolved_by = $3, resolved_at = now(), undone_base = $4
          WHERE id = $1",
    )
    .bind(conflict.id)
    .bind(if keep == Keep::Ours { "ours" } else { "theirs" })
    .bind(editor)
    .bind(&before)
    .execute(&mut **tx)
    .await?;
    sqlx::query("UPDATE pages.external_tables SET conflicts = $2 WHERE table_id = $1")
        .bind(conflict.table_id)
        .bind(open_conflicts(tx, conflict.table_id).await?)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Take back a choice: the base before it, and ours when Notion's was
/// taken, are put back, and the conflict waits again.
#[allow(
    clippy::too_many_lines,
    clippy::single_match_else,
    reason = "one choice for a cell and one for a column, each read top to bottom"
)]
pub async fn undo(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    access: &Access,
    editor: UserId,
    conflict: &Row,
) -> Result<()> {
    let workspace = access.workspace;
    let Some(choice) = conflict.choice.as_deref() else {
        return Err(Error::Conflict(crate::MESSAGES.reason("nothing_to_undo")));
    };
    let before = conflict.undone_base.clone().unwrap_or(Value::Null);
    match (conflict.kind.as_str(), conflict.row_id) {
        ("cell", Some(row)) => {
            let property =
                tables::property(tx, conflict.table_id, conflict.property_id, false).await?;
            if choice == "theirs" {
                apply::write_value(
                    tx,
                    access,
                    editor,
                    conflict.table_id,
                    row,
                    conflict.property_id,
                    property.ty()?,
                    &conflict.ours,
                )
                .await?;
                cells::touch(tx, editor, row).await?;
                announce(
                    state,
                    tx,
                    access,
                    row,
                    vec![PageChangeKind::Values, PageChangeKind::Meta],
                )
                .await?;
            }
            match before.as_object() {
                Some(b) => {
                    let ours = b
                        .get("ours")
                        .and_then(Value::as_str)
                        .and_then(super::model::unhex);
                    let theirs = b
                        .get("theirs")
                        .and_then(Value::as_str)
                        .and_then(super::model::unhex);
                    let edited = b
                        .get("edited")
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                        .unwrap_or(conflict.ours_at);
                    if let (Some(ours), Some(theirs)) = (ours, theirs) {
                        write_cell_base(
                            tx,
                            workspace,
                            conflict.table_id,
                            row,
                            conflict.property_id,
                            &CellBase {
                                ours,
                                theirs,
                                edited,
                            },
                        )
                        .await?;
                    }
                }
                None => {
                    sqlx::query(
                        "DELETE FROM pages.external_cells WHERE row_id = $1 AND property_id = $2",
                    )
                    .bind(row)
                    .bind(conflict.property_id)
                    .execute(&mut **tx)
                    .await?;
                }
            }
        }
        _ => {
            let property =
                tables::property(tx, conflict.table_id, conflict.property_id, true).await?;
            let ours_name = conflict
                .ours
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or(&property.name)
                .to_owned();
            let ours_ty: PropertyType = conflict
                .ours
                .get("type")
                .and_then(Value::as_str)
                .and_then(|t| t.parse().ok())
                .unwrap_or(property.ty()?);
            let ours_cfg = conflict
                .ours
                .get("config")
                .cloned()
                .unwrap_or_else(|| json!({}));
            if choice == "theirs" {
                if ours_ty == property.ty()? {
                    sqlx::query(
                        "UPDATE pages.table_properties SET name = $2, config = $3 WHERE id = $1",
                    )
                    .bind(conflict.property_id)
                    .bind(&ours_name)
                    .bind(&ours_cfg)
                    .execute(&mut **tx)
                    .await?;
                } else if let (Some(ty), Some(cfg)) =
                    (&property.previous_type, &property.previous_config)
                {
                    // The retype Notion's choice made is taken back like any.
                    sqlx::query(
                        "UPDATE pages.table_properties
                            SET name = $2, type = $3, config = $4, previous_type = NULL,
                                previous_config = NULL, converted_at = NULL
                          WHERE id = $1",
                    )
                    .bind(conflict.property_id)
                    .bind(&ours_name)
                    .bind(ty)
                    .bind(cfg)
                    .execute(&mut **tx)
                    .await?;
                    sqlx::query(
                        "UPDATE pages.row_values SET value = previous, previous = NULL,
                                version = version + 1, updated_by = $2, updated_at = now()
                          WHERE property_id = $1 AND previous IS NOT NULL",
                    )
                    .bind(conflict.property_id)
                    .bind(editor)
                    .execute(&mut **tx)
                    .await?;
                } else {
                    return Err(Error::Conflict(crate::MESSAGES.reason("nothing_to_undo")));
                }
                tables::changed(
                    state,
                    tx,
                    access,
                    conflict.table_id,
                    &[TableChangeKind::Schema],
                )
                .await?;
            }
            if let Some(b) = before.as_object() {
                let get = |key: &str| {
                    b.get(key)
                        .and_then(Value::as_str)
                        .and_then(super::model::unhex)
                };
                let notion = b.get("notion").and_then(Value::as_array);
                if let (Some(t), Some(o), Some(r), Some(notion)) =
                    (get("type"), get("options"), get("remote"), notion)
                {
                    let name = b.get("name").and_then(Value::as_str).unwrap_or(&ours_name);
                    write_property_base(
                        tx,
                        workspace,
                        conflict.table_id,
                        conflict.property_id,
                        notion.first().and_then(Value::as_str).unwrap_or_default(),
                        notion.get(1).and_then(Value::as_str).unwrap_or_default(),
                        (name, &t, &o, &r),
                    )
                    .await?;
                }
            }
        }
    }
    sqlx::query(
        "UPDATE pages.table_conflicts
            SET choice = NULL, resolved_by = NULL, resolved_at = NULL, undone_base = NULL
          WHERE id = $1",
    )
    .bind(conflict.id)
    .execute(&mut **tx)
    .await?;
    sqlx::query("UPDATE pages.external_tables SET conflicts = $2 WHERE table_id = $1")
        .bind(conflict.table_id)
        .bind(open_conflicts(tx, conflict.table_id).await?)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
