//! The module's own language: a member's proactivity.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Whether rules Scout proposes from a member's own corrections may start
/// on their own (in an inbox only they use) or wait for a yes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Proactivity {
    /// Wait for a yes (the default).
    #[default]
    AskFirst,
    /// Start it and tell the member, with Undo.
    ActThenTell,
}

impl Proactivity {
    /// The database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AskFirst => "ask_first",
            Self::ActThenTell => "act_then_tell",
        }
    }

    /// From the database name; anything unknown is the default.
    pub fn from_db(name: &str) -> Self {
        if name == Self::ActThenTell.as_str() {
            Self::ActThenTell
        } else {
            Self::AskFirst
        }
    }
}
