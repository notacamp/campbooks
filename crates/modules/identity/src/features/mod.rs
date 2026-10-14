//! One folder per use case of the API (the operation catalogue). The issuer's
//! protocol endpoints live in `protocol/`.

pub mod admin_get_account;
pub mod admin_suspend_account;
pub mod admin_unsuspend_account;
pub mod adopt_held_grant;
pub mod discard_held_grant;
pub mod list_clients;
pub mod list_consents;
pub mod list_held_grants;
pub mod list_passkeys;
pub mod list_ways_in;
pub mod member_left;
pub mod notify_way_in_added;
pub mod register_client;
pub mod register_passkey;
pub mod remove_client;
pub mod remove_passkey;
pub mod remove_way_in;
pub mod rename_passkey;
pub mod revoke_consent;
pub mod rotate_client_secret;
pub mod sign_out_everywhere;
pub mod start_link;
pub mod start_passkey_registration;
pub mod workspace_deleting;

use std::sync::Arc;

use campbooks_kernel::{Error, Result, auth::Principal};
use uuid::Uuid;

use crate::deps::{Deps, Issuer};

/// The held grant a path names (by field, so routes nested under the web
/// BFF's `/bff/api/{*path}` still resolve).
#[derive(Debug, serde::Deserialize)]
pub struct HeldGrantPath {
    /// The held grant.
    pub id: Uuid,
}

/// The passkey a path names (by field, so routes nested under the web
/// BFF's `/bff/api/{*path}` still resolve).
#[derive(Debug, serde::Deserialize)]
pub struct PasskeyPath {
    /// The passkey.
    pub id: Uuid,
}

/// The in-process issuer and the caller's account. 404 when another
/// identity provider serves the instance (its subjects are not accounts),
/// and for a token bound to a workspace: an app a person let into one
/// workspace never manages their account, nor that workspace's apps.
fn account(deps: &Deps, principal: &Principal) -> Result<(Arc<Issuer>, Uuid)> {
    if principal.bound_workspace().is_some() {
        return Err(Error::NotFound);
    }
    let issuer = deps.issuer.clone().ok_or(Error::NotFound)?;
    let account = principal.subject.parse().map_err(|_| Error::NotFound)?;
    Ok((issuer, account))
}
