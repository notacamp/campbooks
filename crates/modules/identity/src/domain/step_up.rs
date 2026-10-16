//! Step-up: changing how someone signs in (adding or removing a way in,
//! registering a passkey) needs a recent sign-in, not just a live session
//! (ADR 0014). A stolen session that is days old cannot add the thief's
//! passkey.
//!
//! The proof is the access token's `auth_time`. When it is too old the
//! operation answers 403 `step_up_required`, and the web client signs in
//! again with `/bff/login?max_age=600`.
//!
//! Starting a passkey registration asks more: a sign-in from the last five
//! minutes (403 `fresh_sign_in_required`), which the web client gets by
//! signing in again at once (`max_age=0`). A passkey outlives every session,
//! so a session that is merely recent is not proof enough.

use campbooks_kernel::{Error, Result, auth::Principal};
use chrono::{DateTime, TimeDelta, Utc};

use crate::MESSAGES;

/// How recent the sign-in must be.
pub const STEP_UP_WINDOW: TimeDelta = TimeDelta::minutes(10);

/// How recent the sign-in must be to start registering a passkey.
pub const FRESH_WINDOW: TimeDelta = TimeDelta::minutes(5);

/// `Ok` when `principal` signed in within [`FRESH_WINDOW`] of `now`; 403
/// `fresh_sign_in_required` otherwise (same rules as [`require_recent`]).
pub fn require_fresh(principal: &Principal, now: DateTime<Utc>) -> Result<()> {
    if signed_in_within(principal, now, FRESH_WINDOW) {
        Ok(())
    } else {
        Err(Error::Denied(MESSAGES.reason("fresh_sign_in_required")))
    }
}

/// Whether `principal` signed in within `window` of `now` (and not from
/// beyond a minute of clock skew in the future).
fn signed_in_within(principal: &Principal, now: DateTime<Utc>, window: TimeDelta) -> bool {
    principal.auth_time.is_some_and(|at| {
        let age = now - at;
        age <= window && age >= -TimeDelta::minutes(1)
    })
}

/// `Ok` when `principal` signed in within [`STEP_UP_WINDOW`] of `now`.
/// A token without `auth_time` (a client, an in-process caller) never
/// passes; nor does one from the future beyond a minute of clock skew.
pub fn require_recent(principal: &Principal, now: DateTime<Utc>) -> Result<()> {
    if signed_in_within(principal, now, STEP_UP_WINDOW) {
        Ok(())
    } else {
        Err(Error::Denied(MESSAGES.reason("step_up_required")))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use campbooks_kernel::auth::PrincipalKind;

    use super::*;

    fn signed_in(at: Option<DateTime<Utc>>) -> Principal {
        Principal {
            subject: "user-1".to_owned(),
            kind: PrincipalKind::User,
            client_id: None,
            scopes: BTreeSet::new(),
            workspace: None,
            email: None,
            email_verified: false,
            name: None,
            expires_at: None,
            issued_at: None,
            auth_time: at,
        }
    }

    #[test]
    fn only_a_recent_sign_in_passes() {
        let now = Utc::now();
        assert!(require_recent(&signed_in(Some(now - TimeDelta::minutes(9))), now).is_ok());
        assert!(require_recent(&signed_in(Some(now)), now).is_ok());
        for stale in [
            Some(now - TimeDelta::minutes(11)),
            Some(now + TimeDelta::minutes(5)),
            None,
        ] {
            let refused = require_recent(&signed_in(stale), now).unwrap_err();
            assert!(matches!(refused, Error::Denied(_)), "{stale:?}");
        }
    }

    #[test]
    fn a_passkey_needs_a_sign_in_from_the_last_five_minutes() {
        let now = Utc::now();
        assert!(require_fresh(&signed_in(Some(now - TimeDelta::minutes(4))), now).is_ok());
        for stale in [
            Some(now - TimeDelta::minutes(6)),
            Some(now + TimeDelta::minutes(5)),
            None,
        ] {
            let refused = require_fresh(&signed_in(stale), now).unwrap_err();
            assert!(
                matches!(&refused, Error::Denied(reason) if reason.code() == "fresh_sign_in_required"),
                "{stale:?}: {refused:?}"
            );
        }
    }
}
