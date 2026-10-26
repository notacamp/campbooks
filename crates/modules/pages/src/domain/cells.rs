//! Writing cells.
//!
//! Every cell is checked against its property's type and config before it
//! is stored (the properties library), and carries a version of its own.
//! Writes to different cells never conflict. A write to a cell that changed
//! since the writer read it (its `version` is behind) is stored all the
//! same, and says so, with the value it replaced, so the writer can offer an
//! undo: the later write wins.
//!
//! The title is the page's title, with the page's `meta_revision` as its
//! version. A paired relation is written on both sides in the same
//! transaction. A Campbooks link holds references only.

use std::collections::{BTreeSet, HashMap, HashSet};

use campbooks_kernel::{
    AppState, Error, Result,
    authz::Action,
    tenancy::{Access, UserId},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};
use utoipa::ToSchema;
use uuid::Uuid;

use super::{
    Kind, PageId, PageRow, PropertyId, access, announce, clean_title, expect_live,
    tables::{PropertyRow, refused},
    tracked,
};
use crate::{
    events::PageChangeKind,
    properties::{
        PropertyType,
        value::{id_list, validate},
    },
};

/// The most cells one call writes.
pub const MAX_CELLS: usize = 100;

/// One cell to write.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CellInput {
    /// The property.
    pub property: PropertyId,
    /// The new value, in the form of the property's type; `null` clears it.
    pub value: Value,
    /// The version of the cell you saw. A stale version still writes, and
    /// the answer says so.
    #[serde(default)]
    pub version: Option<i64>,
}

/// What a write did to one cell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct CellResult {
    /// The property.
    pub property: PropertyId,
    /// The cell's version now.
    pub version: i64,
    /// Whether the cell had changed since the version you sent.
    pub stale: bool,
    /// The value it held before: what an undo writes back.
    pub previous: Value,
    /// The version it had before; 0 for a cell never set.
    pub previous_version: i64,
}

fn invalid() -> Error {
    Error::BadRequest(crate::MESSAGES.reason("invalid_value"))
}

fn relation_ids(value: &Value) -> BTreeSet<String> {
    id_list(value).into_iter().collect()
}

/// The cells of `row` now, by property: value and version.
async fn current(
    tx: &mut Transaction<'_, Postgres>,
    row: PageId,
    properties: &[PropertyId],
) -> Result<HashMap<PropertyId, (Value, i64)>> {
    let found: Vec<(PropertyId, Option<Value>, i64)> = sqlx::query_as(
        "SELECT property_id, value, version FROM pages.row_values
          WHERE row_id = $1 AND property_id = ANY($2) FOR UPDATE",
    )
    .bind(row)
    .bind(properties)
    .fetch_all(&mut **tx)
    .await?;
    Ok(found
        .into_iter()
        .map(|(id, value, version)| (id, (value.unwrap_or(Value::Null), version)))
        .collect())
}

async fn store(
    tx: &mut Transaction<'_, Postgres>,
    access: &Access,
    editor: UserId,
    row: PageId,
    table: PageId,
    property: PropertyId,
    value: &Value,
) -> Result<i64> {
    let version: i64 = sqlx::query_scalar(
        "INSERT INTO pages.row_values
             (row_id, property_id, workspace_id, table_id, value, version, updated_by)
         VALUES ($1, $2, $3, $4, $5, 1, $6)
         ON CONFLICT (row_id, property_id) DO UPDATE
            SET value = EXCLUDED.value, version = pages.row_values.version + 1,
                previous = NULL, updated_by = EXCLUDED.updated_by, updated_at = now()
         RETURNING version",
    )
    .bind(row)
    .bind(property)
    .bind(access.workspace.as_uuid())
    .bind(table)
    .bind(value)
    .bind(editor)
    .fetch_one(&mut **tx)
    .await?;
    Ok(version)
}

/// Mark `row` as edited now by `editor`, so its revision, its "edited"
/// property and its search entry follow.
async fn touch(tx: &mut Transaction<'_, Postgres>, editor: UserId, row: PageId) -> Result<()> {
    sqlx::query(
        "UPDATE pages.pages SET revision = revision + 1, edited_by = $2, edited_at = now()
          WHERE id = $1",
    )
    .bind(row)
    .bind(editor)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[allow(
    clippy::too_many_lines,
    reason = "one pass over the cells of a write, in order"
)]
/// Write `inputs` to `row` of `table`, whose live `properties` are given.
/// Returns what each cell was and became. `row` must be locked and live.
pub async fn write(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    access: &Access,
    row: &PageRow,
    table: PageId,
    properties: &[PropertyRow],
    inputs: &[CellInput],
) -> Result<Vec<CellResult>> {
    let Some(editor) = access.user else {
        return Err(Error::Forbidden);
    };
    if inputs.len() > MAX_CELLS {
        return Err(invalid());
    }
    let by_id: HashMap<PropertyId, &PropertyRow> = properties
        .iter()
        .map(|property| (property.id, property))
        .collect();
    let mut seen = HashSet::new();
    for input in inputs {
        if !by_id.contains_key(&input.property) {
            return Err(Error::BadRequest(
                crate::MESSAGES.reason("unknown_property"),
            ));
        }
        if !seen.insert(input.property) {
            return Err(invalid());
        }
    }
    let ids: Vec<PropertyId> = inputs.iter().map(|input| input.property).collect();
    let existing = current(tx, row.id, &ids).await?;

    let mut results = Vec::with_capacity(inputs.len());
    let mut changed = 0_usize;
    let mut meta_changed = false;
    for input in inputs {
        let property = by_id[&input.property];
        let ty = property.ty()?;
        let (before, before_version) = if ty == PropertyType::Title {
            (Value::String(row.title.clone()), row.meta_revision)
        } else {
            existing
                .get(&input.property)
                .cloned()
                .unwrap_or((Value::Null, 0))
        };
        let behind = input.version.is_some_and(|read| read != before_version);
        let mut result = CellResult {
            property: input.property,
            version: before_version,
            stale: behind,
            previous: before.clone(),
            previous_version: before_version,
        };
        if ty == PropertyType::Title {
            let title = match &input.value {
                Value::String(title) => clean_title(title)?,
                Value::Null => String::new(),
                _ => return Err(invalid()),
            };
            if title != row.title {
                sqlx::query(
                    "UPDATE pages.pages SET title = $2, meta_revision = meta_revision + 1
                      WHERE id = $1",
                )
                .bind(row.id)
                .bind(&title)
                .execute(&mut **tx)
                .await?;
                result.version = before_version + 1;
                changed += 1;
                meta_changed = true;
            }
            results.push(result);
            continue;
        }
        let value = validate(ty, &property.config, &input.value).map_err(refused)?;
        if value == before || (value.is_null() && before.is_null()) {
            results.push(result);
            continue;
        }
        match ty {
            PropertyType::Person => {
                for member in id_list(&value) {
                    let member: UserId = member.parse().map_err(|_| invalid())?;
                    if state
                        .membership
                        .role_held(tx, access.workspace, member)
                        .await?
                        .is_none()
                    {
                        return Err(Error::BadRequest(crate::MESSAGES.reason("not_a_member")));
                    }
                }
            }
            PropertyType::Files => {
                let hashes: Vec<String> = value
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|file| file.get("hash").and_then(Value::as_str))
                    .map(ToOwned::to_owned)
                    .collect();
                let known: i64 = sqlx::query_scalar(
                    "SELECT count(*) FROM pages.files WHERE page_id = $1 AND hash = ANY($2)",
                )
                .bind(row.id)
                .bind(&hashes)
                .fetch_one(&mut **tx)
                .await?;
                if usize::try_from(known).ok() != Some(hashes.len()) {
                    return Err(Error::BadRequest(crate::MESSAGES.reason("foreign_blob")));
                }
            }
            PropertyType::Relation => {
                relate(state, tx, access, editor, row, property, &before, &value).await?;
            }
            _ => {}
        }
        result.version = store(tx, access, editor, row.id, table, input.property, &value).await?;
        changed += 1;
        results.push(result);
    }
    if changed > 0 {
        touch(tx, editor, row.id).await?;
        let mut kinds = vec![PageChangeKind::Values];
        if meta_changed {
            kinds.push(PageChangeKind::Meta);
        }
        announce(state, tx, access, row.id, kinds).await?;
        tracked::cells_edited(state, tx, access.workspace, row.id, changed).await?;
    }
    Ok(results)
}

/// A relation cell of `row` goes from `before` to `after`: the rows it
/// adds must be rows of the related table that the caller may read, and for a
/// paired relation the rows added and removed are written on their side in
/// the same transaction (which needs the right to write them).
#[allow(clippy::too_many_arguments)]
async fn relate(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    access: &Access,
    editor: UserId,
    row: &PageRow,
    property: &PropertyRow,
    before: &Value,
    after: &Value,
) -> Result<()> {
    let related: Uuid = property
        .config
        .get("table")
        .and_then(Value::as_str)
        .and_then(|table| table.parse().ok())
        .ok_or_else(|| Error::internal("a relation has no table"))?;
    let related = PageId::from_uuid(related);
    let mut pair: Option<PropertyId> = property
        .config
        .get("pair")
        .and_then(Value::as_str)
        .and_then(|pair| pair.parse::<Uuid>().ok())
        .map(PropertyId::from_uuid);
    // A pair that was removed is not kept in step: the relation is one way
    // until it is restored.
    if let Some(id) = pair {
        let live: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM pages.table_properties
                             WHERE id = $1 AND removed_at IS NULL)",
        )
        .bind(id)
        .fetch_one(&mut **tx)
        .await?;
        if !live {
            pair = None;
        }
    }
    let (old, new) = (relation_ids(before), relation_ids(after));
    let mut targets: Vec<(PageId, bool)> = new
        .difference(&old)
        .map(|id| (id.clone(), true))
        .chain(old.difference(&new).map(|id| (id.clone(), false)))
        .filter_map(|(id, added)| id.parse::<PageId>().ok().map(|id| (id, added)))
        .collect();
    targets.sort_unstable();
    for (target, added) in targets {
        // The rows an edit adds must be rows of the related table that the
        // caller can read; a row they cannot read is not there for them.
        let page = if added {
            let page = access::require_page(
                state,
                tx,
                access,
                if pair.is_some() {
                    Action::Write
                } else {
                    Action::Read
                },
                target,
                pair.is_some(),
            )
            .await?;
            if page.kind()? != Kind::Row || page.parent_id != Some(related) {
                return Err(invalid());
            }
            expect_live(&page)?;
            Some(page)
        } else if pair.is_some() {
            // A row taken out may be gone, or no longer the caller's to
            // see: the other side is then left as it is.
            match access::require_page(state, tx, access, Action::Write, target, true).await {
                Ok(page) => Some(page),
                Err(Error::NotFound) => None,
                Err(other) => return Err(other),
            }
        } else {
            None
        };
        let (Some(pair), Some(page)) = (pair, page) else {
            continue;
        };
        let theirs = current(tx, page.id, &[pair]).await?;
        let (cell, _) = theirs.get(&pair).cloned().unwrap_or((Value::Null, 0));
        let mut ids = relation_ids(&cell);
        if added {
            ids.insert(row.id.to_string());
        } else {
            ids.remove(&row.id.to_string());
        }
        if ids.len() > crate::properties::MAX_LIST {
            return Err(invalid());
        }
        let value = if ids.is_empty() {
            Value::Null
        } else {
            json!(ids.into_iter().collect::<Vec<_>>())
        };
        store(tx, access, editor, page.id, related, pair, &value).await?;
        touch(tx, editor, page.id).await?;
        announce(state, tx, access, page.id, vec![PageChangeKind::Values]).await?;
    }
    Ok(())
}
