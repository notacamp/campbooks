//! The module's vocabulary as Rust types, and the shapes the API returns.

use std::str::FromStr;

use campbooks_kernel::{
    Error,
    tenancy::{Role, UserId, WorkspaceId},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::MESSAGES;

campbooks_kernel::define_id!(
    /// An invitation to join a workspace.
    InvitationId
);

campbooks_kernel::define_id!(
    /// A signup code an administrator minted.
    SignupCodeId
);

campbooks_kernel::define_id!(
    /// An entry of the security log.
    SecurityEventId
);

/// What a workspace is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// A business: a company, a freelancer, a team.
    Business,
    /// A household or one person's own affairs.
    Personal,
}

impl Kind {
    /// The wire and database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Business => "business",
            Self::Personal => "personal",
        }
    }

    /// The accent a new workspace of this kind gets unless one is chosen.
    pub const fn default_accent(self) -> Accent {
        match self {
            Self::Business => Accent::Business,
            Self::Personal => Accent::Personal,
        }
    }
}

impl FromStr for Kind {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "business" => Ok(Self::Business),
            "personal" => Ok(Self::Personal),
            other => Err(Error::internal(format!("unknown workspace kind `{other}`"))),
        }
    }
}

/// The workspace's colour, one of the design system's accent presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Accent {
    /// The business preset.
    Business,
    /// The personal preset.
    Personal,
    /// The forest preset.
    Forest,
}

impl Accent {
    /// The wire and database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Business => "business",
            Self::Personal => "personal",
            Self::Forest => "forest",
        }
    }
}

impl FromStr for Accent {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "business" => Ok(Self::Business),
            "personal" => Ok(Self::Personal),
            "forest" => Ok(Self::Forest),
            other => Err(Error::internal(format!("unknown accent `{other}`"))),
        }
    }
}

/// Longest workspace name, in characters.
pub const MAX_NAME_CHARS: usize = 100;

/// A workspace name, trimmed; 1 to [`MAX_NAME_CHARS`] characters.
pub fn workspace_name(raw: &str) -> Result<String, Error> {
    let name = raw.trim();
    let chars = name.chars().count();
    if chars == 0 || chars > MAX_NAME_CHARS {
        return Err(Error::BadRequest(
            MESSAGES.reason("invalid_name").arg("max", MAX_NAME_CHARS),
        ));
    }
    Ok(name.to_owned())
}

/// A workspace, as its members see it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct WorkspaceView {
    /// Its id.
    pub id: WorkspaceId,
    /// Its name.
    #[schema(example = "Household")]
    pub name: String,
    /// What it is for.
    pub kind: Kind,
    /// Its colour.
    pub accent: Accent,
    /// When it was created.
    pub created_at: DateTime<Utc>,
    /// The caller's role in it; absent for a client token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
}

impl WorkspaceView {
    /// From a row's columns.
    pub fn from_row(
        id: WorkspaceId,
        name: String,
        kind: &str,
        accent: &str,
        created_at: DateTime<Utc>,
        role: Option<Role>,
    ) -> Result<Self, Error> {
        Ok(Self {
            id,
            name,
            kind: kind.parse()?,
            accent: accent.parse()?,
            created_at,
            role,
        })
    }
}

/// A member of a workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MemberView {
    /// The user.
    pub user_id: UserId,
    /// Their name, as their identity provider gives it.
    pub display_name: Option<String>,
    /// Their email address, as their identity provider gives it, when it
    /// verified it (an unverified address is never shown: anyone can claim
    /// one).
    pub email: Option<String>,
    /// Their role.
    pub role: Role,
    /// When they joined.
    pub joined_at: DateTime<Utc>,
    /// It is the caller (by user id, never by email).
    pub you: bool,
}

/// The path of one member: `/api/v1/w/{workspace_id}/members/{user_id}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct MemberPath {
    /// The workspace (checked by the `Workspace` extractor).
    pub workspace_id: WorkspaceId,
    /// The member.
    pub user_id: UserId,
}
