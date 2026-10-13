//! One folder per use case of the API (the operation catalogue). The issuer's
//! protocol endpoints live in `protocol/`.

pub mod admin_get_account;
pub mod admin_suspend_account;
pub mod admin_unsuspend_account;
pub mod adopt_held_grant;
pub mod discard_held_grant;
pub mod list_held_grants;
pub mod list_ways_in;
pub mod remove_way_in;
pub mod start_link;

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

/// The in-process issuer and the caller's account. 404 when another
/// identity provider serves the instance (its subjects are not accounts).
fn account(deps: &Deps, principal: &Principal) -> Result<(Arc<Issuer>, Uuid)> {
    let issuer = deps.issuer.clone().ok_or(Error::NotFound)?;
    let account = principal.subject.parse().map_err(|_| Error::NotFound)?;
    Ok((issuer, account))
}
