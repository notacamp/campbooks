//! Deterministic signals: answers read straight off an item, with no model.
//!
//! [`email`] is the single source of the deterministic triage rules for
//! email. The conversations module calls it and keeps no copy, so the eval
//! harness (`cargo xtask eval decide`) measures the rules that ship.

mod auth_results;
pub mod dates;
pub mod email;
