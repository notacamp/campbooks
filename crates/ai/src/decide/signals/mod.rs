//! What triage reads off an email without a model.
//!
//! [`email`] is the envelope and the triage questions every strategy
//! answers. [`facts`] is the only deterministic strategy: it reads
//! structured facts (addresses, list and automation headers, the receiving
//! provider's authentication results, attachment media types), never what
//! an email says, and may only say "no" (ADR 0018). The conversations
//! module calls it and keeps no copy, so the eval harness
//! (`cargo xtask eval decide`) measures the code that ships. [`inbox`] is
//! the context a model reads before an email.

mod auth_results;
pub mod email;
pub mod facts;
pub mod inbox;
