//! One folder per use case of the API (the operation catalogue). The issuer's
//! protocol endpoints live in `protocol/`.

pub mod adopt_held_grant;
pub mod discard_held_grant;
pub mod list_held_grants;

use std::sync::Arc;

use campbooks_kernel::{Error, Result, auth::Principal};
use uuid::Uuid;

use crate::deps::{Deps, Issuer};

/// The in-process issuer and the caller's account. 404 when another
/// identity provider serves the instance (its subjects are not accounts).
fn account(deps: &Deps, principal: &Principal) -> Result<(Arc<Issuer>, Uuid)> {
    let issuer = deps.issuer.clone().ok_or(Error::NotFound)?;
    let account = principal.subject.parse().map_err(|_| Error::NotFound)?;
    Ok((issuer, account))
}
