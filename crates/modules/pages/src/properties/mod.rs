//! `pages::properties` — the property types of a table (ADR 0022), as a
//! self-contained library: what each type's config and value look like and
//! how they are checked, compared, filtered, sorted and summarised in SQL,
//! and converted when a property changes type.
//!
//! It depends on nothing else in `pages` (no database, no access, no
//! events): a type's rules are written here once, so the `records` module of
//! ADR 0008 can adopt them later, moving this directory to the kernel if two
//! modules need it. The module that stores the values asks it three things:
//!
//! - [`config::normalise`] and [`value::validate`]: what may be stored. A
//!   value is checked against its property's type and config *before* it is
//!   stored, and what comes back is the canonical form (so two equal values
//!   are stored equal).
//! - [`filter`], [`sql`]: what a query may ask, and the SQL that answers it,
//!   cast by type, with every operand bound (never spliced into the text).
//! - [`convert`]: what a column becomes when its property changes type.
//!
//! [`eval`] answers the same questions in memory; it is the oracle the tests
//! compare the SQL with.
//!
//! Errors are [`PropertyError`]s: a stable code, never the value.

pub mod config;
pub mod convert;
pub mod eval;
pub mod filter;
pub mod sql;
pub mod value;

#[cfg(test)]
pub(crate) mod tests;

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// The most properties a table holds.
pub const MAX_PROPERTIES: usize = 200;
/// The most options a select, multi-select or status property holds.
pub const MAX_OPTIONS: usize = 100;
/// The longest a property or option name may be, in characters.
pub const MAX_NAME: usize = 100;
/// The longest a text value may be, in characters.
pub const MAX_TEXT: usize = 2_000;
/// The most related rows (or people, or links) one cell holds.
pub const MAX_LIST: usize = 100;
/// The largest a stored value may be, as JSON, in bytes.
pub const MAX_VALUE_BYTES: usize = 16 * 1024;

/// Why a config, a value, a filter or a sort was refused: a stable code the
/// module turns into a message. Never holds the value that was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PropertyError {
    /// The code (`invalid_value`, `invalid_config`, `invalid_filter`, ...).
    pub code: &'static str,
}

impl PropertyError {
    /// The value does not fit its property's type or config.
    pub const VALUE: Self = Self {
        code: "invalid_value",
    };
    /// The config does not fit its type.
    pub const CONFIG: Self = Self {
        code: "invalid_config",
    };
    /// The filter is not valid for the table's properties.
    pub const FILTER: Self = Self {
        code: "invalid_filter",
    };
    /// The sort is not valid.
    pub const SORT: Self = Self {
        code: "invalid_sort",
    };
    /// The summary is not valid for its property.
    pub const SUMMARY: Self = Self {
        code: "invalid_summary",
    };
    /// A member may not write this property's cells.
    pub const NOT_EDITABLE: Self = Self {
        code: "property_not_editable",
    };
    /// A member may not create this type.
    pub const RESERVED: Self = Self {
        code: "property_type_reserved",
    };
    /// The name is empty or too long.
    pub const NAME: Self = Self {
        code: "invalid_property_name",
    };
}

impl fmt::Display for PropertyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code)
    }
}

impl std::error::Error for PropertyError {}

/// Result of the library's checks.
pub type Result<T, E = PropertyError> = std::result::Result<T, E>;

macro_rules! property_types {
    ($($(#[$meta:meta])* $variant:ident => $text:literal),+ $(,)?) => {
        /// The type of a property. Every type has a Notion counterpart
        /// (design.md, "Property types").
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
        #[serde(rename_all = "snake_case")]
        #[schema(as = PropertyType)]
        pub enum PropertyType {
            $($(#[$meta])* $variant),+
        }

        impl PropertyType {
            /// Every type.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// The wire and database name.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $text),+
                }
            }
        }

        impl FromStr for PropertyType {
            type Err = PropertyError;

            fn from_str(s: &str) -> Result<Self> {
                match s {
                    $($text => Ok(Self::$variant),)+
                    _ => Err(PropertyError::CONFIG),
                }
            }
        }
    };
}

property_types!(
    /// The row's page title; exactly one per table.
    Title => "title",
    /// Rich text: runs with marks and links.
    Text => "text",
    /// A decimal number, shown as a number, a percent or a currency.
    Number => "number",
    /// One option.
    Select => "select",
    /// Several options.
    MultiSelect => "multi_select",
    /// One option, each in a to-do, doing or done group.
    Status => "status",
    /// A date or a range, optionally with a time.
    Date => "date",
    /// Yes or no.
    Checkbox => "checkbox",
    /// Teammates.
    Person => "person",
    /// Files.
    Files => "files",
    /// A web address.
    Url => "url",
    /// An email address.
    Email => "email",
    /// A phone number.
    Phone => "phone",
    /// Rows of another table (or of the same one).
    Relation => "relation",
    /// References to Campbooks records (threads, tasks, events, people).
    CampbooksLink => "campbooks_link",
    /// When the row was made, or by whom.
    Created => "created",
    /// When the row was last edited, or by whom.
    Edited => "edited",
    /// A prefix and the row's number in its table.
    UniqueId => "unique_id",
    /// A Notion formula's last value, read only (reserved).
    NotionFormula => "notion_formula",
    /// A Notion rollup's last value, read only (reserved).
    NotionRollup => "notion_rollup",
    /// A Notion people property, names only, read only (reserved).
    NotionPeople => "notion_people",
    /// A Notion property kept at Notion (reserved).
    External => "external",
);

impl PropertyType {
    /// Whether a member may create a property of this type. The types
    /// reserved for Notion (step 3) are created by the sync only, and the
    /// title exists once, with the table.
    pub const fn creatable(self) -> bool {
        !matches!(
            self,
            Self::Title
                | Self::NotionFormula
                | Self::NotionRollup
                | Self::NotionPeople
                | Self::External
        )
    }

    /// Whether the type is one of those reserved for Notion.
    pub const fn reserved(self) -> bool {
        matches!(
            self,
            Self::NotionFormula | Self::NotionRollup | Self::NotionPeople | Self::External
        )
    }

    /// Whether the value comes from the page, not from `row_values`.
    pub const fn computed(self) -> bool {
        matches!(
            self,
            Self::Title | Self::Created | Self::Edited | Self::UniqueId
        )
    }

    /// Whether a member may write the cell. The title is written through
    /// the page; computed and reserved types are never written.
    pub const fn editable(self) -> bool {
        !(self.computed() || self.reserved())
    }

    /// Whether the cell is stored in `row_values`.
    pub const fn stored(self) -> bool {
        !self.computed()
    }

    /// Whether rows can be ordered by it.
    pub const fn sortable(self) -> bool {
        matches!(
            self,
            Self::Title
                | Self::Text
                | Self::Number
                | Self::Select
                | Self::Status
                | Self::Date
                | Self::Checkbox
                | Self::Url
                | Self::Email
                | Self::Phone
                | Self::Created
                | Self::Edited
                | Self::UniqueId
        )
    }

    /// Whether a board can group by it.
    pub const fn groupable(self) -> bool {
        matches!(self, Self::Select | Self::Status)
    }
}

/// A property as the library sees it: enough to check a value, a filter or
/// a sort against it.
#[derive(Debug, Clone, PartialEq)]
pub struct Spec {
    /// The property's id.
    pub id: uuid::Uuid,
    /// Its type.
    pub ty: PropertyType,
    /// Its config, as [`config::normalise`] made it.
    pub config: serde_json::Value,
}

impl Spec {
    /// A property.
    pub fn new(id: uuid::Uuid, ty: PropertyType, config: serde_json::Value) -> Self {
        Self { id, ty, config }
    }
}

/// A property name as it is kept: whitespace as plain spaces, no hidden or
/// control characters, trimmed, 1 to 100 characters.
pub fn clean_name(raw: &str) -> Result<String> {
    let spaced: String = raw
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .collect();
    let cleaned: String = spaced
        .chars()
        .filter(|c| campbooks_kernel::text::is_displayable(*c))
        .collect();
    let name = cleaned.trim();
    let length = name.chars().count();
    if length == 0 || length > MAX_NAME {
        return Err(PropertyError::NAME);
    }
    Ok(name.to_owned())
}
