//! Step-up: an operation that changes how someone signs in (adding or
//! removing a way in, registering a passkey) or hands Campbooks a secret
//! of theirs (a mailbox's password) needs a recent sign-in, not just a live
//! session (ADR 0014). A stolen session that is days old cannot add the
//! thief's passkey, nor connect a mailbox it guessed the password of.
//!
//! The proof is the access token's `auth_time`. When it is too old the
//! operation answers 403 with the caller's reason (`step_up_required`), and
//! the web client signs in again with `/bff/login?max_age=600`.
//!
//! Starting a passkey registration asks more: a sign-in from the last five
//! minutes (`fresh_sign_in_required`), which the web client gets by signing
//! in again at once (`max_age=0`). A passkey outlives every session, so a
//! session that is merely recent is not proof enough.
//!
//! Each module names the reason in its own catalogue, so the message is the
//! module's; the rule is this one.

use chrono::{DateTime, TimeDelta, Utc};

use super::Principal;
use crate::{Error, Result, i18n::Reason};

/// How recent the sign-in must be.
pub const STEP_UP_WINDOW: TimeDelta = TimeDelta::minutes(10);

/// How recent the sign-in must be to start registering a passkey.
pub const FRESH_WINDOW: TimeDelta = TimeDelta::minutes(5);

/// Whether `principal` signed in within `window` of `now` (and not from
/// beyond a minute of clock skew in the future). A token without
/// `auth_time` (a client, an in-process caller) never did.
pub fn signed_in_within(principal: &Principal, now: DateTime<Utc>, window: TimeDelta) -> bool {
    principal.auth_time.is_some_and(|at| {
        let age = now - at;
        age <= window && age >= -TimeDelta::minutes(1)
    })
}

/// `Ok` when `principal` signed in within [`STEP_UP_WINDOW`] of `now`;
/// 403 with `refused` (the module's `step_up_required`) otherwise.
pub fn require_recent(principal: &Principal, now: DateTime<Utc>, refused: Reason) -> Result<()> {
    if signed_in_within(principal, now, STEP_UP_WINDOW) {
        Ok(())
    } else {
        Err(Error::Denied(refused))
    }
}

/// `Ok` when `principal` signed in within [`FRESH_WINDOW`] of `now`; 403
/// with `refused` (the module's `fresh_sign_in_required`) otherwise.
pub fn require_fresh(principal: &Principal, now: DateTime<Utc>, refused: Reason) -> Result<()> {
    if signed_in_within(principal, now, FRESH_WINDOW) {
        Ok(())
    } else {
        Err(Error::Denied(refused))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::{auth::PrincipalKind, i18n::MESSAGES};

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
            sign_in: None,
        }
    }

    fn refused() -> Reason {
        MESSAGES.reason("forbidden")
    }

    #[test]
    fn only_a_recent_sign_in_passes() {
        let now = Utc::now();
        for recent in [now - TimeDelta::minutes(9), now] {
            assert!(require_recent(&signed_in(Some(recent)), now, refused()).is_ok());
        }
        for stale in [
            Some(now - TimeDelta::minutes(11)),
            Some(now + TimeDelta::minutes(5)),
            None,
        ] {
            let error = require_recent(&signed_in(stale), now, refused()).unwrap_err();
            assert!(
                matches!(&error, Error::Denied(reason) if reason.code() == "forbidden"),
                "{stale:?}: {error:?}"
            );
        }
    }

    #[test]
    fn a_fresh_sign_in_is_from_the_last_five_minutes() {
        let now = Utc::now();
        assert!(
            require_fresh(
                &signed_in(Some(now - TimeDelta::minutes(4))),
                now,
                refused()
            )
            .is_ok()
        );
        for stale in [
            Some(now - TimeDelta::minutes(6)),
            Some(now + TimeDelta::minutes(5)),
            None,
        ] {
            assert!(
                require_fresh(&signed_in(stale), now, refused()).is_err(),
                "{stale:?}"
            );
        }
    }
}
