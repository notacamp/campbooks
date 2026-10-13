//! What the system module is composed with: the AI gateway (to show each
//! role's health) and the facts only the server knows (which identity
//! provider and which sign-in and mail providers this instance runs with).

use campbooks_ai::Gateway;
use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;

/// The system module's dependencies.
#[derive(Debug, Clone)]
pub struct Deps {
    /// The AI gateway, when any role is configured.
    pub ai: Option<Gateway>,
    /// What the server composed.
    pub facts: Facts,
    /// When this server process started.
    pub started_at: DateTime<Utc>,
}

impl Default for Deps {
    fn default() -> Self {
        Self {
            ai: None,
            facts: Facts::default(),
            started_at: Utc::now(),
        }
    }
}

/// Who signs people in, and the outside providers set up, by name only.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, ToSchema)]
pub struct Facts {
    /// Which identity provider issues tokens.
    pub identity_provider: IdentityProvider,
    /// The providers people can sign in with (`google`, …).
    pub sign_in_providers: Vec<String>,
    /// The mail providers mailboxes can be connected from (`google`, …).
    pub mail_providers: Vec<String>,
}

/// Which identity provider issues this instance's tokens (ADR 0014).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum IdentityProvider {
    /// Campbooks' own, in this process.
    BuiltIn,
    /// Another one (`CAMPBOOKS_AUTH__ISSUER`).
    External,
    /// None: nobody can sign in.
    #[default]
    None,
}
