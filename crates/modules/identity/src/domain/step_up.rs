//! Step-up: changing how someone signs in (adding or removing a way in,
//! registering a passkey) needs a recent sign-in, not just a live session
//! (ADR 0014). A stolen session that is days old cannot add the thief's
//! passkey.
//!
//! The proof is the access token's `auth_time`. When it is too old the
//! operation answers 403 `step_up_required`, and the web client signs in
//! again with `/bff/login?max_age=600`.

use campbooks_kernel::{Error, Result, auth::Principal};
use chrono::{DateTime, TimeDelta, Utc};

use crate::MESSAGES;

/// How recent the sign-in must be.
pub const STEP_UP_WINDOW: TimeDelta = TimeDelta::minutes(10);

/// `Ok` when `principal` signed in within [`STEP_UP_WINDOW`] of `now`.
/// A token without `auth_time` (a client, an in-process caller) never
/// passes; nor does one from the future beyond a minute of clock skew.
pub fn require_recent(principal: &Principal, now: DateTime<Utc>) -> Result<()> {
    let recent = principal.auth_time.is_some_and(|at| {
        let age = now - at;
        age <= STEP_UP_WINDOW && age >= -TimeDelta::minutes(1)
    });
    if recent {
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
}
