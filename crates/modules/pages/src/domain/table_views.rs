//! A table's views: their types and their config, checked.
//!
//! A view's config is stored as given back by [`normalise`]:
//!
//! - `filter`: the tree of [`properties::filter`];
//! - `sorts`: up to five;
//! - `summaries`: a footer per property;
//! - `properties`: `[{property, visible?, width?}]`, in the order shown;
//! - for a board: `group_by` (a select or status property), `hidden_groups`
//!   (option ids, or `none` for the empty group) and `card_properties`.
//!
//! Views are stored generically: `list`, `calendar` and `gallery` are
//! reserved types, refused until they are built, with no data to change.

use std::{collections::HashMap, str::FromStr};

use campbooks_kernel::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::properties::{
    PropertyError, Spec,
    filter::{self, Filter, Sort, Summary},
};

/// The most views a table has.
pub const MAX_VIEWS: usize = 50;
/// The most rows a query answers at once.
pub const PAGE: usize = 100;

/// What a view shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(as = TableViewType)]
pub enum ViewType {
    /// A grid of rows and columns.
    Table,
    /// Cards in a column per option.
    Board,
    /// Reserved.
    List,
    /// Reserved.
    Calendar,
    /// Reserved.
    Gallery,
}

impl ViewType {
    /// The wire and database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Table => "table",
            Self::Board => "board",
            Self::List => "list",
            Self::Calendar => "calendar",
            Self::Gallery => "gallery",
        }
    }

    /// Whether a member may make one now.
    pub const fn available(self) -> bool {
        matches!(self, Self::Table | Self::Board)
    }
}

impl FromStr for ViewType {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self> {
        match text {
            "table" => Ok(Self::Table),
            "board" => Ok(Self::Board),
            "list" => Ok(Self::List),
            "calendar" => Ok(Self::Calendar),
            "gallery" => Ok(Self::Gallery),
            _ => Err(Error::internal("unknown view type")),
        }
    }
}

fn bad(error: PropertyError) -> Error {
    Error::BadRequest(crate::MESSAGES.reason(error.code))
}

fn invalid() -> Error {
    Error::BadRequest(crate::MESSAGES.reason("invalid_view"))
}

fn property_of(raw: &Value, specs: &HashMap<Uuid, Spec>) -> Result<Uuid> {
    raw.as_str()
        .and_then(|text| text.parse::<Uuid>().ok())
        .filter(|id| specs.contains_key(id))
        .ok_or_else(invalid)
}

/// `raw` as the stored config of a view of type `ty` for a table with
/// `specs`, or why not.
pub fn normalise(ty: ViewType, raw: &Value, specs: &HashMap<Uuid, Spec>) -> Result<Value> {
    if !ty.available() {
        return Err(Error::BadRequest(
            crate::MESSAGES.reason("view_type_reserved"),
        ));
    }
    let object = match raw {
        Value::Null => &Map::new(),
        Value::Object(object) => object,
        _ => return Err(invalid()),
    };
    let board = ty == ViewType::Board;
    let allowed: &[&str] = if board {
        &[
            "filter",
            "sorts",
            "summaries",
            "properties",
            "group_by",
            "hidden_groups",
            "card_properties",
        ]
    } else {
        &["filter", "sorts", "summaries", "properties"]
    };
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(invalid());
    }
    let mut out = Map::new();
    if let Some(tree) = object.get("filter").filter(|tree| !tree.is_null()) {
        filter::parse(tree, specs).map_err(bad)?;
        out.insert("filter".into(), tree.clone());
    }
    if let Some(sorts) = object.get("sorts").filter(|sorts| !sorts.is_null()) {
        let sorts = filter::parse_sorts(sorts, specs).map_err(bad)?;
        out.insert("sorts".into(), json!(sorts));
    }
    if let Some(summaries) = object.get("summaries").filter(|s| !s.is_null()) {
        let summaries = filter::parse_summaries(summaries, specs).map_err(bad)?;
        out.insert("summaries".into(), json!(summaries));
    }
    if let Some(columns) = object.get("properties").filter(|p| !p.is_null()) {
        let columns = columns.as_array().ok_or_else(invalid)?;
        let mut seen = std::collections::HashSet::new();
        let mut kept = Vec::new();
        for column in columns {
            let column = column.as_object().ok_or_else(invalid)?;
            if column
                .keys()
                .any(|key| !["property", "visible", "width"].contains(&key.as_str()))
            {
                return Err(invalid());
            }
            let property = property_of(column.get("property").unwrap_or(&Value::Null), specs)?;
            if !seen.insert(property) {
                return Err(invalid());
            }
            let mut entry = json!({"property": property});
            match column.get("visible") {
                None | Some(Value::Null) => {}
                Some(Value::Bool(visible)) => entry["visible"] = json!(visible),
                Some(_) => return Err(invalid()),
            }
            match column.get("width") {
                None | Some(Value::Null) => {}
                Some(width) => {
                    let width = width
                        .as_u64()
                        .filter(|width| (40..=2000).contains(width))
                        .ok_or_else(invalid)?;
                    entry["width"] = json!(width);
                }
            }
            kept.push(entry);
        }
        out.insert("properties".into(), Value::Array(kept));
    }
    if board {
        let group_by = property_of(object.get("group_by").unwrap_or(&Value::Null), specs)?;
        if !specs[&group_by].ty.groupable() {
            return Err(invalid());
        }
        out.insert("group_by".into(), json!(group_by));
        if let Some(hidden) = object.get("hidden_groups").filter(|h| !h.is_null()) {
            let hidden = hidden.as_array().ok_or_else(invalid)?;
            let ids = hidden
                .iter()
                .map(|id| {
                    id.as_str()
                        .filter(|id| {
                            !id.is_empty()
                                && id.len() <= 40
                                && id.chars().all(|c| {
                                    c.is_ascii_alphanumeric() || c == '_' || c == '-'
                                })
                        })
                        .map(ToOwned::to_owned)
                        .ok_or_else(invalid)
                })
                .collect::<Result<Vec<_>>>()?;
            if ids.len() > 101 {
                return Err(invalid());
            }
            out.insert("hidden_groups".into(), json!(ids));
        }
        if let Some(cards) = object.get("card_properties").filter(|c| !c.is_null()) {
            let cards = cards.as_array().ok_or_else(invalid)?;
            if cards.len() > 20 {
                return Err(invalid());
            }
            let ids = cards
                .iter()
                .map(|id| property_of(id, specs))
                .collect::<Result<Vec<_>>>()?;
            out.insert("card_properties".into(), json!(ids));
        }
    }
    Ok(Value::Object(out))
}

/// A stored view read for a query: what it asks, with what no longer fits
/// the table's properties left out.
#[derive(Debug, Clone, Default)]
pub struct Resolved {
    /// Its filter.
    pub filter: Option<Filter>,
    /// Its sorts.
    pub sorts: Vec<Sort>,
    /// Its summaries.
    pub summaries: Vec<Summary>,
    /// For a board, the property it groups by (if it still can).
    pub group_by: Option<Uuid>,
}

/// Read a stored view config against the table's current `specs`.
pub fn resolve(config: &Value, specs: &HashMap<Uuid, Spec>) -> Result<Resolved> {
    let filter = match config.get("filter").filter(|tree| !tree.is_null()) {
        Some(tree) => filter::parse_lenient(tree, specs).map_err(bad)?,
        None => None,
    };
    let sorts: Vec<Sort> = config
        .get("sorts")
        .and_then(|sorts| serde_json::from_value(sorts.clone()).ok())
        .unwrap_or_default();
    let sorts = sorts
        .into_iter()
        .filter(|sort| specs.get(&sort.property).is_some_and(|spec| spec.ty.sortable()))
        .collect();
    let summaries: Vec<Summary> = config
        .get("summaries")
        .and_then(|summaries| serde_json::from_value(summaries.clone()).ok())
        .unwrap_or_default();
    let summaries = summaries
        .into_iter()
        .filter(|summary| {
            specs
                .get(&summary.property)
                .is_some_and(|spec| filter::summary_allowed(spec.ty, summary.kind))
        })
        .collect();
    let group_by = config
        .get("group_by")
        .and_then(Value::as_str)
        .and_then(|id| id.parse::<Uuid>().ok())
        .filter(|id| specs.get(id).is_some_and(|spec| spec.ty.groupable()));
    Ok(Resolved {
        filter,
        sorts,
        summaries,
        group_by,
    })
}
