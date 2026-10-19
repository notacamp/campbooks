//! Access tokens that stop before they expire: a person's account can be
//! suspended, or its owner can sign out everywhere, while clients still
//! hold access tokens issued to it; and one sign-in can end on its own (its
//! session signed out, its refresh token revoked, the way in it used
//! removed, or the app's consent revoked: campbooks-cloud#143). The
//! [`TokenRevocation`] port says where a token's account and sign-in
//! stand; [`TokenVerifier`](super::TokenVerifier) asks it on every
//! verification of a person's token once the server attaches it
//! ([`with_revocation`](super::TokenVerifier::with_revocation)).
//!
//! Only the in-process issuer's verifier gets one: its tokens' `sub` is an
//! identity account. An outside identity provider's subjects are not, and
//! client-credentials tokens are never looked up (the client directory
//! resolves those on every request).

use std::{future::Future, pin::Pin, sync::Arc};

use chrono::{DateTime, Utc};
use sqlx::PgPool;

use super::{Principal, PrincipalKind};
use crate::Error;

/// Where an account stands for the access tokens issued to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TokenStanding {
    /// The account is suspended: none of its tokens stand.
    pub suspended: bool,
    /// The instant of the account's last revocation (suspension, sign out
    /// everywhere), if any: only tokens issued in a later second stand.
    pub tokens_not_before: Option<DateTime<Utc>>,
    /// The token's own sign-in ended since it was issued: its session was
    /// signed out (or its way in removed), its refresh family revoked, or
    /// the consent of its app revoked. None of its tokens stand.
    pub sign_in_ended: bool,
}

impl TokenStanding {
    /// Whether a token issued at `issued_at` (its `iat`, whole seconds)
    /// still stands. A token without `iat` never does.
    ///
    /// `iat` has one-second resolution, so a token issued earlier in the
    /// revocation's own second carries that second too: it must be refused,
    /// so only a **later** second stands. (The issuer stamps a token of a
    /// sign-in newer than the revocation, minted in that same second, with
    /// the next second.)
    pub fn admits(&self, issued_at: Option<DateTime<Utc>>) -> bool {
        if self.suspended || self.sign_in_ended {
            return false;
        }
        let Some(issued_at) = issued_at else {
            return false;
        };
        self.tokens_not_before
            .is_none_or(|epoch| issued_at.timestamp() > epoch.timestamp())
    }
}

/// A future returned by [`TokenRevocation::standing`].
pub type StandingFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Option<TokenStanding>, Error>> + Send + 'a>>;

/// Says where a person's token stands: its account, and its own sign-in.
/// Implemented by the `identity` module (which owns accounts, sessions,
/// refresh families and consents) and registered by the server for the
/// in-process issuer only.
pub trait TokenRevocation: std::fmt::Debug + Send + Sync + 'static {
    /// The standing of the token `principal` was verified from (its `sub`,
    /// sign-in, client and `iat`), or `None` when there is no such account
    /// (its tokens stand no more).
    fn standing<'a>(&'a self, db: &'a PgPool, principal: &'a Principal) -> StandingFuture<'a>;
}

/// A registered revocation port.
pub type TokenRevocationPort = Arc<dyn TokenRevocation>;

/// The port and the pool it reads with, as the verifier holds them.
#[derive(Debug, Clone)]
pub(super) struct Revocation {
    pub(super) db: PgPool,
    pub(super) port: TokenRevocationPort,
}

impl Revocation {
    /// Whether `principal`'s token still stands. Clients are not looked up.
    /// Errors (the database) are returned, never taken as "stands".
    pub(super) async fn stands(&self, principal: &Principal) -> Result<bool, Error> {
        if principal.kind != PrincipalKind::User {
            return Ok(true);
        }
        let standing = self.port.standing(&self.db, principal).await?;
        Ok(standing.is_some_and(|standing| standing.admits(principal.issued_at)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: i64, millis: u32) -> DateTime<Utc> {
        DateTime::from_timestamp(seconds, millis * 1_000_000).unwrap()
    }

    #[test]
    fn nothing_revoked_admits_any_token_with_an_iat() {
        let standing = TokenStanding::default();
        assert!(standing.admits(Some(at(1_700_000_000, 0))));
        assert!(!standing.admits(None), "no iat, no standing");
    }

    #[test]
    fn only_a_later_second_than_the_revocation_stands() {
        let standing = TokenStanding {
            tokens_not_before: Some(at(1_700_000_000, 500)),
            ..TokenStanding::default()
        };
        // Before, and earlier in the same second (`iat` is whole seconds).
        assert!(!standing.admits(Some(at(1_699_999_999, 0))));
        assert!(!standing.admits(Some(at(1_700_000_000, 0))));
        // The next second on.
        assert!(standing.admits(Some(at(1_700_000_001, 0))));
        assert!(standing.admits(Some(at(1_700_000_600, 0))));
        assert!(!standing.admits(None));
    }

    #[test]
    fn a_revocation_on_a_whole_second_refuses_that_second() {
        let standing = TokenStanding {
            tokens_not_before: Some(at(1_700_000_000, 0)),
            ..TokenStanding::default()
        };
        assert!(!standing.admits(Some(at(1_700_000_000, 0))));
        assert!(standing.admits(Some(at(1_700_000_001, 0))));
    }

    #[test]
    fn a_suspended_account_admits_nothing() {
        let standing = TokenStanding {
            suspended: true,
            ..TokenStanding::default()
        };
        assert!(!standing.admits(Some(at(1_700_000_000, 0))));
    }

    #[test]
    fn an_ended_sign_in_admits_nothing() {
        let standing = TokenStanding {
            sign_in_ended: true,
            ..TokenStanding::default()
        };
        assert!(!standing.admits(Some(at(1_700_000_000, 0))));
    }
}
