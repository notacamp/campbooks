//! Step-up: changing how someone signs in (adding or removing a way in,
//! registering a passkey) needs a recent sign-in, not just a live session
//! (ADR 0014). A stolen session that is days old cannot add the thief's
//! passkey.
//!
//! The rule is the kernel's ([`campbooks_kernel::auth::step_up`], shared
//! with the connectors module); the messages are this module's:
//! `step_up_required` when the sign-in is more than ten minutes old, and
//! `fresh_sign_in_required` when starting a passkey registration more than
//! five minutes after it.

use campbooks_kernel::{Result, auth::Principal, auth::step_up};
use chrono::{DateTime, Utc};

use crate::MESSAGES;

pub use step_up::{FRESH_WINDOW, STEP_UP_WINDOW};

/// `Ok` when `principal` signed in within [`FRESH_WINDOW`] of `now`; 403
/// `fresh_sign_in_required` otherwise (same rules as [`require_recent`]).
pub fn require_fresh(principal: &Principal, now: DateTime<Utc>) -> Result<()> {
    step_up::require_fresh(principal, now, MESSAGES.reason("fresh_sign_in_required"))
}

/// `Ok` when `principal` signed in within [`STEP_UP_WINDOW`] of `now`; 403
/// `step_up_required` otherwise. A token without `auth_time` (a client, an
/// in-process caller) never passes; nor does one from the future beyond a
/// minute of clock skew.
pub fn require_recent(principal: &Principal, now: DateTime<Utc>) -> Result<()> {
    step_up::require_recent(principal, now, MESSAGES.reason("step_up_required"))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use campbooks_kernel::{Error, auth::PrincipalKind};
    use chrono::TimeDelta;

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
            sign_in: None,
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
            assert!(
                matches!(&refused, Error::Denied(reason) if reason.code() == "step_up_required"),
                "{stale:?}: {refused:?}"
            );
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
