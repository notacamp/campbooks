//! What the table features share: a table's properties and views as the API
//! shows them, the rows with their cells, the one door to a table or a row
//! (the page chain, ADR 0017), and the announcement of a change.
//!
//! A table is a page of kind `table`; its rows are pages of kind `row`
//! under it. Who may do what with a table, its schema, its views and its
//! rows is who may do it with the page: the table for the schema and the
//! views, the row for its values. Nothing in the table tables stores who may
//! read what.

use std::collections::{BTreeMap, HashMap};

use campbooks_kernel::{
    AppState, Error, Result,
    authz::Action,
    tenancy::{Access, UserId, WorkspaceId},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgConnection, Postgres, Transaction};
use utoipa::ToSchema;
use uuid::Uuid;

use super::{
    Kind, PageId, PageRow, PageSummary, Place, PropertyId, ViewId, access, announce,
    table_views::ViewType, view,
};
use crate::{
    events::{PageChangeKind, TableChangeKind, TableChanged},
    properties::{PropertyError, PropertyType, Spec},
};

/// The most rows a table holds.
pub const MAX_ROWS: i64 = 20_000;

/// `/api/v1/w/{workspace_id}/tables/{table_id}…`
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct TablePath {
    /// The table.
    pub table_id: PageId,
}

/// `…/tables/{table_id}/properties/{property_id}…`
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct PropertyPath {
    /// The table.
    pub table_id: PageId,
    /// The property.
    pub property_id: PropertyId,
}

/// `…/tables/{table_id}/views/{view_id}`
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct ViewPath {
    /// The table.
    pub table_id: PageId,
    /// The view.
    pub view_id: ViewId,
}

/// `…/tables/{table_id}/rows/{row_id}…`
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct RowPath {
    /// The table.
    pub table_id: PageId,
    /// The row (a page).
    pub row_id: PageId,
}

/// A page to insert.
pub struct NewPageRow<'a> {
    pub id: PageId,
    pub workspace: WorkspaceId,
    pub parent: Option<PageId>,
    pub place: Place,
    pub position: &'a str,
    pub title: &'a str,
    pub icon: Option<&'a str>,
    pub creator: UserId,
    pub kind: Kind,
}

/// Insert a page of any kind.
pub async fn insert_page(tx: &mut Transaction<'_, Postgres>, new: NewPageRow<'_>) -> Result<()> {
    sqlx::query(
        "INSERT INTO pages.pages
             (id, workspace_id, parent_id, place, position, title, icon, created_by, edited_by,
              kind)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8, $9)",
    )
    .bind(new.id)
    .bind(new.workspace.as_uuid())
    .bind(new.parent)
    .bind(new.place.as_str())
    .bind(new.position)
    .bind(new.title)
    .bind(new.icon)
    .bind(new.creator)
    .bind(new.kind.as_str())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// A property error as the 400 it answers.
pub fn refused(error: PropertyError) -> Error {
    Error::BadRequest(crate::MESSAGES.reason(error.code))
}

/// 409 `not_a_table` unless the page is a table.
pub fn expect_table(row: &PageRow) -> Result<()> {
    if row.kind()? == Kind::Table {
        Ok(())
    } else {
        Err(Error::Conflict(crate::MESSAGES.reason("not_a_table")))
    }
}

/// Table `id` as `access` may take `action` on it: 404 when they may not
/// see the page, 403 when they see it and may not do that, 409 when it is
/// not a table.
pub async fn require_table(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    access: &Access,
    action: Action,
    id: PageId,
    lock: bool,
) -> Result<PageRow> {
    let row = access::require_page(state, tx, access, action, id, lock).await?;
    expect_table(&row)?;
    Ok(row)
}

/// Row `row_id` of table `table_id` as `access` may take `action` on it
/// (the row is a page: its values follow its table's access, or a grant of
/// its own). 404 when the row is not in that table.
pub async fn require_row(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    access: &Access,
    action: Action,
    table_id: PageId,
    row_id: PageId,
    lock: bool,
) -> Result<PageRow> {
    let row = access::require_page(state, tx, access, action, row_id, lock).await?;
    if row.kind()? != Kind::Row {
        return Err(Error::Conflict(crate::MESSAGES.reason("not_a_row")));
    }
    if row.parent_id != Some(table_id) {
        return Err(Error::NotFound);
    }
    Ok(row)
}

/// A property (a column).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PropertyRow {
    pub id: PropertyId,
    pub table_id: PageId,
    pub name: String,
    pub property_type: String,
    pub config: Value,
    pub position: String,
    pub created_by: UserId,
    pub removed_at: Option<DateTime<Utc>>,
    pub previous_type: Option<String>,
    pub previous_config: Option<Value>,
    pub converted_at: Option<DateTime<Utc>>,
}

/// The columns of a [`PropertyRow`].
macro_rules! property_columns {
    () => {
        "id, table_id, name, type AS property_type, config, position, created_by, removed_at, \
         previous_type, previous_config, converted_at"
    };
}

impl PropertyRow {
    /// Its type.
    pub fn ty(&self) -> Result<PropertyType> {
        self.property_type
            .parse()
            .map_err(|_| Error::internal("a stored property has an unknown type"))
    }

    /// The library's view of it.
    pub fn spec(&self) -> Result<Spec> {
        Ok(Spec::new(
            *self.id.as_uuid(),
            self.ty()?,
            self.config.clone(),
        ))
    }

    /// As the API shows it.
    pub fn view(&self) -> Result<PropertyView> {
        Ok(PropertyView {
            id: self.id,
            name: self.name.clone(),
            property_type: self.ty()?,
            config: self.config.clone(),
            position: self.position.clone(),
            can_revert: self.previous_type.is_some(),
        })
    }
}

/// A property of a table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct PropertyView {
    /// The property.
    pub id: PropertyId,
    /// Its name.
    pub name: String,
    /// Its type.
    #[serde(rename = "type")]
    pub property_type: PropertyType,
    /// Its config, per type: options for a select, the format of a number,
    /// whether a date has a time, the table of a relation.
    pub config: Value,
    /// Where it sits among the table's properties: sort by it.
    pub position: String,
    /// Whether its last change of type can be taken back
    /// (`pages.restore_property`).
    pub can_revert: bool,
}

/// A view of a table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct TableViewView {
    /// The view.
    pub id: ViewId,
    /// Its name.
    pub name: String,
    /// `table` or `board`.
    #[serde(rename = "type")]
    pub view_type: ViewType,
    /// Where it sits among the table's views.
    pub position: String,
    /// Its filter, sorts, summaries, visible properties and, for a board, its
    /// groups and card properties.
    pub config: Value,
}

/// A view row.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ViewRow {
    pub id: ViewId,
    pub name: String,
    pub view_type: String,
    pub position: String,
    pub config: Value,
}

/// The columns of a [`ViewRow`].
macro_rules! view_columns {
    () => {
        "id, name, type AS view_type, position, config"
    };
}

impl ViewRow {
    /// As the API shows it.
    pub fn view(&self) -> Result<TableViewView> {
        Ok(TableViewView {
            id: self.id,
            name: self.name.clone(),
            view_type: self.view_type.parse()?,
            position: self.position.clone(),
            config: self.config.clone(),
        })
    }
}

/// A table: its page, its properties and its views.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct TableDetail {
    /// The table as a page: title, icon, place, role.
    pub page: PageSummary,
    /// Grows with every change of its properties, views or set of rows.
    pub revision: i64,
    /// Its properties, in order.
    pub properties: Vec<PropertyView>,
    /// Its views, in order.
    pub views: Vec<TableViewView>,
    /// How many rows it has (those in the trash excluded).
    pub rows: i64,
}

/// The live properties of a table, in order.
pub async fn properties(conn: &mut PgConnection, table: PageId) -> Result<Vec<PropertyRow>> {
    Ok(sqlx::query_as(concat!(
        "SELECT ",
        property_columns!(),
        " FROM pages.table_properties
          WHERE table_id = $1 AND removed_at IS NULL ORDER BY position, id"
    ))
    .bind(table)
    .fetch_all(conn)
    .await?)
}

/// Property `id` of `table`, live or removed, locked when `lock`.
pub async fn property(
    conn: &mut PgConnection,
    table: PageId,
    id: PropertyId,
    lock: bool,
) -> Result<PropertyRow> {
    sqlx::query_as(if lock {
        concat!(
            "SELECT ",
            property_columns!(),
            " FROM pages.table_properties WHERE table_id = $1 AND id = $2 FOR UPDATE"
        )
    } else {
        concat!(
            "SELECT ",
            property_columns!(),
            " FROM pages.table_properties WHERE table_id = $1 AND id = $2"
        )
    })
    .bind(table)
    .bind(id)
    .fetch_optional(conn)
    .await?
    .ok_or(Error::NotFound)
}

/// The live properties as the library sees them, by id.
pub fn specs(properties: &[PropertyRow]) -> Result<HashMap<Uuid, Spec>> {
    properties
        .iter()
        .map(|property| Ok((*property.id.as_uuid(), property.spec()?)))
        .collect()
}

/// The views of a table, in order.
pub async fn views(conn: &mut PgConnection, table: PageId) -> Result<Vec<ViewRow>> {
    Ok(sqlx::query_as(concat!(
        "SELECT ",
        view_columns!(),
        " FROM pages.table_views WHERE table_id = $1 ORDER BY position, id"
    ))
    .bind(table)
    .fetch_all(conn)
    .await?)
}

/// How many rows the table has, those in the trash excluded.
pub async fn live_rows(conn: &mut PgConnection, table: PageId) -> Result<i64> {
    Ok(sqlx::query_scalar(
        "SELECT count(*) FROM pages.table_rows tr JOIN pages.pages p ON p.id = tr.row_id
          WHERE tr.table_id = $1 AND p.trashed_at IS NULL",
    )
    .bind(table)
    .fetch_one(conn)
    .await?)
}

/// `table` as `access` sees it.
pub async fn detail(
    conn: &mut PgConnection,
    access: &Access,
    table: &PageRow,
) -> Result<TableDetail> {
    let page = view::summary_of(&mut *conn, access, table).await?;
    let revision: i64 = sqlx::query_scalar("SELECT revision FROM pages.tables WHERE table_id = $1")
        .bind(table.id)
        .fetch_optional(&mut *conn)
        .await?
        .ok_or_else(|| Error::internal("a table has no counters"))?;
    let properties = properties(&mut *conn, table.id)
        .await?
        .iter()
        .map(PropertyRow::view)
        .collect::<Result<Vec<_>>>()?;
    let views = views(&mut *conn, table.id)
        .await?
        .iter()
        .map(ViewRow::view)
        .collect::<Result<Vec<_>>>()?;
    let rows = live_rows(conn, table.id).await?;
    Ok(TableDetail {
        page,
        revision,
        properties,
        views,
        rows,
    })
}

/// One cell of a row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct CellView {
    /// Its value, in the form of the property's type.
    pub value: Value,
    /// Grows with every change of this cell: send the one you saw.
    pub version: i64,
}

/// A row with its cells.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct RowView {
    /// The row (a page).
    pub id: PageId,
    /// The table.
    pub table: PageId,
    /// Its number in the table: what an ID property shows.
    pub number: i64,
    /// Its title, the table's title property.
    pub title: String,
    /// The cells that hold a value, by property id.
    pub cells: BTreeMap<String, CellView>,
    /// Who made it.
    pub created_by: UserId,
    /// When.
    pub created_at: DateTime<Utc>,
    /// Who changed it last.
    pub edited_by: UserId,
    /// When.
    pub edited_at: DateTime<Utc>,
    /// Grows with every change of the row, a cell included.
    pub revision: i64,
    /// Grows with a change of the title: the version of the title cell.
    pub meta_revision: i64,
}

type RowHead = (
    PageId,
    PageId,
    i64,
    String,
    UserId,
    DateTime<Utc>,
    UserId,
    DateTime<Utc>,
    i64,
    i64,
);

/// The rows `ids` of a table with their cells, in the order of `ids`. Rows
/// that are not in the table are left out.
pub async fn rows(conn: &mut PgConnection, table: PageId, ids: &[PageId]) -> Result<Vec<RowView>> {
    let heads: Vec<RowHead> = sqlx::query_as(
        "SELECT p.id, tr.table_id, tr.number, p.title, p.created_by, p.created_at,
                p.edited_by, p.edited_at, p.revision, p.meta_revision
           FROM pages.table_rows tr JOIN pages.pages p ON p.id = tr.row_id
          WHERE tr.table_id = $1 AND tr.row_id = ANY($2)",
    )
    .bind(table)
    .bind(ids)
    .fetch_all(&mut *conn)
    .await?;
    let cells: Vec<(PageId, PropertyId, Value, i64)> = sqlx::query_as(
        "SELECT v.row_id, v.property_id, v.value, v.version
           FROM pages.row_values v
           JOIN pages.table_properties tp ON tp.id = v.property_id AND tp.removed_at IS NULL
          WHERE v.row_id = ANY($1) AND v.value IS NOT NULL AND v.value <> 'null'::jsonb",
    )
    .bind(ids)
    .fetch_all(conn)
    .await?;
    let mut by_row: HashMap<PageId, BTreeMap<String, CellView>> = HashMap::new();
    for (row, property, value, version) in cells {
        by_row
            .entry(row)
            .or_default()
            .insert(property.to_string(), CellView { value, version });
    }
    let mut views: HashMap<PageId, RowView> = heads
        .into_iter()
        .map(
            |(
                id,
                table,
                number,
                title,
                created_by,
                created_at,
                edited_by,
                edited_at,
                revision,
                meta_revision,
            )| {
                (
                    id,
                    RowView {
                        id,
                        table,
                        number,
                        title,
                        cells: by_row.remove(&id).unwrap_or_default(),
                        created_by,
                        created_at,
                        edited_by,
                        edited_at,
                        revision,
                        meta_revision,
                    },
                )
            },
        )
        .collect();
    Ok(ids.iter().filter_map(|id| views.remove(id)).collect())
}

/// Announce a change to a table (`pages.page_changed` of the table page and
/// `pages.table_changed`, in `tx`) and move its revisions on: what a
/// property, an option, a view or the set of rows did. Ids and enums only.
pub async fn changed(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    access: &Access,
    table: PageId,
    kinds: &[TableChangeKind],
) -> Result<()> {
    let Some(editor) = access.user else {
        return Err(Error::Forbidden);
    };
    let revision: i64 = sqlx::query_scalar(
        "UPDATE pages.tables SET revision = revision + 1 WHERE table_id = $1 RETURNING revision",
    )
    .bind(table)
    .fetch_one(&mut **tx)
    .await?;
    sqlx::query(
        "UPDATE pages.pages SET revision = revision + 1, edited_by = $2, edited_at = now()
          WHERE id = $1",
    )
    .bind(table)
    .bind(editor)
    .execute(&mut **tx)
    .await?;
    let page_kinds = kinds
        .iter()
        .map(|kind| match kind {
            TableChangeKind::Schema => PageChangeKind::Schema,
            TableChangeKind::Views => PageChangeKind::Views,
            TableChangeKind::Rows => PageChangeKind::Values,
        })
        .collect();
    announce(state, tx, access, table, page_kinds).await?;
    state
        .events
        .publish(
            tx,
            &TableChanged {
                workspace: access.workspace.to_string(),
                table,
                revision,
                kinds: kinds.to_vec(),
            },
        )
        .await?;
    Ok(())
}

/// A row was added, trashed, restored or deleted: the table it is in asks
/// again. Nothing happens for a page that is not a row.
pub async fn row_set_changed(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    access: &Access,
    page: &PageRow,
) -> Result<()> {
    if page.kind()? == Kind::Row
        && let Some(table) = page.parent_id
    {
        changed(state, tx, access, table, &[TableChangeKind::Rows]).await?;
    }
    Ok(())
}
