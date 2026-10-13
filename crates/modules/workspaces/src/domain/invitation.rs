//! Invitation tokens and addresses.
//!
//! A token is `<workspace id>.<secret>`: 32 random bytes, base64url without
//! padding. The workspace prefix lets the accepting request work inside the
//! right workspace (row-level security); only the token's SHA-256 is stored.

use aws_lc_rs::{digest, rand};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use campbooks_kernel::{Error, Result, tenancy::WorkspaceId};
use chrono::TimeDelta;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::MESSAGES;

use super::model::InvitationId;
use campbooks_kernel::tenancy::{Role, UserId};
use chrono::{DateTime, Utc};

/// How long an invitation can be accepted.
pub const VALIDITY: TimeDelta = TimeDelta::days(7);

/// Random bytes in a token's secret.
const SECRET_BYTES: usize = 32;

/// Longest email address accepted (RFC 5321 path limit).
const MAX_EMAIL_LEN: usize = 254;

/// A new token for `workspace`, and the hash to store.
pub fn new_token(workspace: WorkspaceId) -> Result<(String, Vec<u8>)> {
    let mut secret = [0_u8; SECRET_BYTES];
    rand::fill(&mut secret)
        .map_err(|_| Error::internal("no randomness for an invitation token"))?;
    let token = format!("{workspace}.{}", URL_SAFE_NO_PAD.encode(secret));
    let hash = hash(&token);
    Ok((token, hash))
}

/// The SHA-256 of a token, as stored.
pub fn hash(token: &str) -> Vec<u8> {
    digest::digest(&digest::SHA256, token.as_bytes())
        .as_ref()
        .to_vec()
}

/// The workspace a token was issued for; `None` when it is not a token.
pub fn workspace_of(token: &str) -> Option<WorkspaceId> {
    let (workspace, secret) = token.split_once('.')?;
    if secret.is_empty() {
        return None;
    }
    workspace.parse().ok()
}

/// An email address, trimmed and lower-cased, with one `@` between a
/// non-empty local part and domain and no whitespace.
pub fn normalize_email(raw: &str) -> Result<String> {
    let email = raw.trim().to_lowercase();
    let valid = email.len() <= MAX_EMAIL_LEN
        && !email.chars().any(char::is_whitespace)
        && email.split_once('@').is_some_and(|(local, domain)| {
            !local.is_empty() && !domain.is_empty() && !domain.contains('@')
        });
    if valid {
        Ok(email)
    } else {
        Err(Error::BadRequest(MESSAGES.reason("invalid_email")))
    }
}

/// A pending invitation, as owners and admins see it. Never carries the
/// token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct InvitationView {
    /// Its id.
    pub id: InvitationId,
    /// The invited address.
    #[schema(example = "someone@example.com")]
    pub email: String,
    /// The role they get on accepting.
    pub role: Role,
    /// Who invited them.
    pub invited_by: UserId,
    /// When it was sent.
    pub created_at: DateTime<Utc>,
    /// When it stops working.
    pub expires_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_name_their_workspace_and_hash_stably() {
        let workspace = WorkspaceId::new();
        let (token, stored) = new_token(workspace).unwrap();
        assert_eq!(workspace_of(&token), Some(workspace));
        assert_eq!(hash(&token), stored);
        assert_eq!(stored.len(), 32);
        assert_eq!(token.len(), 36 + 1 + 43);
        let (other, _) = new_token(workspace).unwrap();
        assert_ne!(token, other);
        for bad in ["", "x.y", &format!("{workspace}"), &format!("{workspace}.")] {
            assert_eq!(workspace_of(bad), None, "{bad}");
        }
    }

    #[test]
    fn emails_are_normalized_or_refused() {
        assert_eq!(
            normalize_email("  Ana@Example.COM ").unwrap(),
            "ana@example.com"
        );
        for bad in [
            "",
            "ana",
            "@example.com",
            "ana@",
            "a b@example.com",
            "a@b@c",
        ] {
            assert!(normalize_email(bad).is_err(), "{bad}");
        }
    }
}
