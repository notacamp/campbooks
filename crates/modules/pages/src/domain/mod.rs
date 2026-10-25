//! The module's vocabulary and what its features share: a page as the API
//! shows it, the words that describe it, and the one way a change is
//! announced (`pages.page_changed`, in the change's own transaction).

use std::str::FromStr;

pub mod access;
pub mod body;
pub mod document;
pub mod position;
pub mod purge;
pub mod resources;
pub mod roles;
pub mod search;
pub mod store;
pub mod tracked;
pub mod view;

use campbooks_kernel::{
    AppState, Error, Result, define_id,
    tenancy::{Access, UserId},
    text,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Transaction};
use utoipa::ToSchema;

use crate::events::{PageChangeKind, PageChanged};

define_id!(
    /// A page.
    PageId
);

/// The deepest a page may be nested: a top-level page is level 1.
pub const MAX_DEPTH: usize = 12;

/// The longest a page's title may be, in characters.
pub const MAX_TITLE: usize = 200;

/// The icons a page may have, by name: the client draws each one.
pub const ICONS: &[&str] = &[
    "document", "tasks", "schedule", "people", "star", "pin", "mail", "money", "home", "inbox",
    "archive", "image", "travel", "dining", "parcel", "ticket",
];

/// A closed set of words kept as text in the database and as `snake_case`
/// on the wire.
macro_rules! words {
    ($(#[$meta:meta])* $name:ident { $($(#[$vmeta:meta])* $variant:ident => $text:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, ToSchema)]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            $($(#[$vmeta])* $variant),+
        }

        impl $name {
            /// The wire and database name.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $text),+
                }
            }
        }

        impl FromStr for $name {
            type Err = Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($text => Ok(Self::$variant),)+
                    other => Err(Error::internal(format!(
                        "unknown {} `{other}`",
                        stringify!($name)
                    ))),
                }
            }
        }
    };
}

words!(
    /// Where a top-level page lives; sub-pages are always in their parent's.
    Place {
        /// Its creator's, and whoever it is shared with.
        Private => "private",
        /// Every member's.
        Workspace => "workspace",
    }
);

words!(
    /// What a member can do with a page, lowest first.
    Role {
        /// Read it.
        Reader => "reader",
        /// Also write it, and make and move pages in it.
        Writer => "writer",
        /// Also share it and delete it.
        Manager => "manager",
    }
);

words!(
    /// How a body points at a record.
    LinkKind {
        /// An inline mention.
        Mention => "mention",
        /// A block that shows a record.
        Embed => "embed",
        /// A link to a sub-page.
        Child => "child",
    }
);

/// The columns of a page row, in [`PageRow`] order.
macro_rules! columns {
    () => {
        "id, parent_id, place, position, title, icon, created_by, created_at, edited_by, \
         edited_at, trashed_at, trashed_by, trash_root, revision, meta_revision"
    };
}

pub(crate) use columns;

/// One page by id.
pub const SELECT_ONE: &str = concat!("SELECT ", columns!(), " FROM pages.pages WHERE id = $1");

/// One page by id, locked for the change.
pub const SELECT_LOCKED: &str = concat!(
    "SELECT ",
    columns!(),
    " FROM pages.pages WHERE id = $1 FOR UPDATE"
);

/// A page row.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PageRow {
    pub id: PageId,
    pub parent_id: Option<PageId>,
    pub place: String,
    pub position: String,
    pub title: String,
    pub icon: Option<String>,
    pub created_by: UserId,
    pub created_at: DateTime<Utc>,
    pub edited_by: UserId,
    pub edited_at: DateTime<Utc>,
    pub trashed_at: Option<DateTime<Utc>>,
    pub trashed_by: Option<UserId>,
    pub trash_root: Option<PageId>,
    pub revision: i64,
    pub meta_revision: i64,
}

impl PageRow {
    /// Whether it is in the trash.
    pub fn is_trashed(&self) -> bool {
        self.trashed_at.is_some()
    }

    /// Its place.
    pub fn place(&self) -> Result<Place> {
        self.place.parse()
    }
}

/// One page of the tree, as `pages.list` shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct PageSummary {
    /// The page.
    pub id: PageId,
    /// Its parent, when it is a sub-page.
    pub parent: Option<PageId>,
    /// Private or workspace.
    pub place: Place,
    /// Where it sits among its siblings: sort by it.
    pub position: String,
    /// Its title (empty when it has none yet).
    pub title: String,
    /// Its icon, one of the page icons.
    pub icon: Option<String>,
    /// Whether it has sub-pages.
    pub has_children: bool,
    /// What the caller can do with it.
    pub role: Role,
    /// The member who made it.
    pub created_by: UserId,
    /// When it was last changed.
    pub edited_at: DateTime<Utc>,
    /// When it was put in the trash, for a page in the trash.
    pub trashed_at: Option<DateTime<Utc>>,
    /// Grows with a change of the title, icon or position.
    pub meta_revision: i64,
}

/// A member a page is shared with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct GrantView {
    /// The member.
    pub member: UserId,
    /// Their role on the page and its sub-pages.
    pub role: Role,
}

/// A page above another, for the breadcrumb: only ones the caller may open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Crumb {
    /// The page.
    pub id: PageId,
    /// Its title.
    pub title: String,
    /// Its icon.
    pub icon: Option<String>,
}

/// A page's body as a Yjs document: what the client loads and merges.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct DocumentState {
    /// The whole document as one Yjs update (v1), in base64.
    pub update: String,
    /// The document's state vector (v1), in base64.
    pub state_vector: String,
}

/// A page, as `pages.get` shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct PageView {
    /// The page.
    pub id: PageId,
    /// Its parent, when it is a sub-page.
    pub parent: Option<PageId>,
    /// Private or workspace.
    pub place: Place,
    /// Where it sits among its siblings.
    pub position: String,
    /// Its title.
    pub title: String,
    /// Its icon.
    pub icon: Option<String>,
    /// What the caller can do with it.
    pub role: Role,
    /// The member who made it.
    pub created_by: UserId,
    /// When it was made.
    pub created_at: DateTime<Utc>,
    /// The member who last changed it.
    pub edited_by: UserId,
    /// When it was last changed.
    pub edited_at: DateTime<Utc>,
    /// When it was put in the trash.
    pub trashed_at: Option<DateTime<Utc>>,
    /// Grows with every change.
    pub revision: i64,
    /// Grows with a change of the title, icon or position: send the one
    /// you saw with `pages.update` and `pages.move`.
    pub meta_revision: i64,
    /// The members it is shared with.
    pub grants: Vec<GrantView>,
    /// The pages above it that the caller may open, outermost first.
    pub ancestors: Vec<Crumb>,
    /// The body.
    pub document: DocumentState,
}

impl PageRow {
    /// The row as a tree entry for a caller with `role`.
    pub fn summary(&self, role: Role, has_children: bool) -> Result<PageSummary> {
        Ok(PageSummary {
            id: self.id,
            parent: self.parent_id,
            place: self.place()?,
            position: self.position.clone(),
            title: self.title.clone(),
            icon: self.icon.clone(),
            has_children,
            role,
            created_by: self.created_by,
            edited_at: self.edited_at,
            trashed_at: self.trashed_at,
            meta_revision: self.meta_revision,
        })
    }
}

/// A title as it is kept: whitespace as plain spaces, no bidirectional,
/// zero-width or control characters, trimmed, at most 200 characters (a
/// page may have none yet).
pub fn clean_title(raw: &str) -> Result<String> {
    let spaced: String = raw
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .collect();
    let cleaned = text::displayable(&spaced);
    let title = cleaned.trim();
    if title.chars().count() > MAX_TITLE {
        return Err(Error::BadRequest(crate::MESSAGES.reason("invalid_title")));
    }
    Ok(title.to_owned())
}

/// An icon name, or none: one of [`ICONS`].
pub fn clean_icon(raw: Option<&str>) -> Result<Option<String>> {
    match raw {
        None => Ok(None),
        Some(name) if ICONS.contains(&name) => Ok(Some(name.to_owned())),
        Some(_) => Err(Error::BadRequest(crate::MESSAGES.reason("invalid_icon"))),
    }
}

/// 409 `page_changed` unless `seen` is the page's metadata revision.
pub fn expect_meta_revision(row: &PageRow, seen: i64) -> Result<()> {
    if row.meta_revision == seen {
        Ok(())
    } else {
        Err(Error::Conflict(crate::MESSAGES.reason("page_changed")))
    }
}

/// 409 `page_trashed` when the page is in the trash.
pub fn expect_live(row: &PageRow) -> Result<()> {
    if row.is_trashed() {
        Err(Error::Conflict(crate::MESSAGES.reason("page_trashed")))
    } else {
        Ok(())
    }
}

/// Announce page `id` as it is now (`pages.page_changed`, in `tx`): its
/// revision, parent, place and whether it is in the trash, with `kinds` of
/// change. Ids and enums only: never a title, never any text.
pub async fn announce(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    access: &Access,
    id: PageId,
    kinds: Vec<PageChangeKind>,
) -> Result<()> {
    let row: PageRow = sqlx::query_as(SELECT_ONE)
        .bind(id)
        .fetch_one(&mut **tx)
        .await?;
    let event = PageChanged {
        workspace: access.workspace.to_string(),
        page: id,
        revision: row.revision,
        parent: row.parent_id,
        place: row.place()?,
        trashed: row.is_trashed(),
        kinds,
    };
    state.events.publish(tx, &event).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_are_cleaned_and_bounded() {
        assert_eq!(
            clean_title("  Weekly\tplan \u{200b}").unwrap(),
            "Weekly plan"
        );
        assert_eq!(clean_title("").unwrap(), "");
        assert_eq!(clean_title("\u{202e}\u{200b}").unwrap(), "");
        assert!(clean_title(&"x".repeat(200)).is_ok());
        assert!(clean_title(&"x".repeat(201)).is_err());
    }

    #[test]
    fn icons_come_from_the_fixed_set() {
        assert_eq!(clean_icon(None).unwrap(), None);
        assert_eq!(clean_icon(Some("star")).unwrap().as_deref(), Some("star"));
        assert!(clean_icon(Some("rocket")).is_err());
    }

    #[test]
    fn roles_rise() {
        assert!(Role::Reader < Role::Writer && Role::Writer < Role::Manager);
        assert_eq!("writer".parse::<Role>().unwrap(), Role::Writer);
    }
}
