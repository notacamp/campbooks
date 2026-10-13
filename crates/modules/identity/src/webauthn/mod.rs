//! A WebAuthn relying party (Web Authentication Level 3), narrowed to what
//! Campbooks asks of passkeys:
//!
//! - **discoverable credentials with user verification**, always;
//! - **attestation `none`**: the authenticator's make and model are not our
//!   business, so no attestation statement is trusted or needed;
//! - **three algorithms**: EdDSA (-8, Ed25519), ES256 (-7, P-256) and RS256
//!   (-257), verified with `aws-lc-rs`; the COSE key is parsed with `coset`
//!   and the attestation object with `ciborium`.
//!
//! The relying-party id is the host of the public URL and the only accepted
//! origin is that URL's origin. Nothing here touches the database: callers
//! store challenges (single use, [`CEREMONY_TIMEOUT`]) and credentials, and
//! hand them back for verification.
//!
//! Every refusal is a [`Refusal`]: callers answer one generic reason, so a
//! probing client learns nothing about which check failed. The refusal's
//! label is for the server's own logs and tests and never holds the
//! credential, the challenge or any other input.

mod auth_data;
mod cbor;
mod cose;
mod options;
mod verify;

#[cfg(test)]
pub(crate) mod authenticator;
#[cfg(test)]
mod tests;

use chrono::TimeDelta;
use url::Url;

pub use auth_data::Flags;
pub use options::{
    AuthenticatorSelection, CreationOptions, CredentialDescriptor, CredentialParameter,
    RelyingPartyEntity, RequestOptions, UserEntity, descriptor,
};
pub use verify::{
    AssertionResponse, AttestationResponse, AuthenticationCredential, NewCredential,
    RegistrationCredential, Stored, Verified,
};

/// How long a ceremony's challenge lives, and the `timeout` the browser is
/// given.
pub const CEREMONY_TIMEOUT: TimeDelta = TimeDelta::minutes(5);

/// The COSE algorithms accepted, in order of preference: EdDSA, ES256,
/// RS256.
pub const ALGORITHMS: [i64; 3] = [-8, -7, -257];

/// Why a credential or an assertion was refused. The label names the check
/// (for logs and tests), never the input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("webauthn refused: {0}")]
pub struct Refusal(pub &'static str);

/// Shorthand for a refusal.
pub(crate) const fn refuse<T>(label: &'static str) -> Result<T, Refusal> {
    Err(Refusal(label))
}

/// This instance as a WebAuthn relying party.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelyingParty {
    /// The relying-party id: the public URL's host.
    pub id: String,
    /// The only origin a ceremony may come from.
    pub origin: String,
    /// SHA-256 of [`id`](Self::id), which authenticators sign over.
    id_hash: [u8; 32],
}

impl RelyingParty {
    /// The relying party for public origin `origin` (`scheme://host[:port]`).
    /// `None` where browsers offer no passkeys: an IP address (not a valid
    /// relying-party id) or plain http on any host but `localhost`.
    pub fn for_origin(origin: &str) -> Option<Self> {
        let url = Url::parse(origin).ok()?;
        let host = match url.host()? {
            url::Host::Domain(host) => host.to_ascii_lowercase(),
            url::Host::Ipv4(_) | url::Host::Ipv6(_) => return None,
        };
        let secure = url.scheme() == "https"
            || (url.scheme() == "http" && (host == "localhost" || host.ends_with(".localhost")));
        if !secure {
            return None;
        }
        let digest = aws_lc_rs::digest::digest(&aws_lc_rs::digest::SHA256, host.as_bytes());
        let mut id_hash = [0_u8; 32];
        id_hash.copy_from_slice(digest.as_ref());
        Some(Self {
            id: host,
            origin: url.origin().ascii_serialization(),
            id_hash,
        })
    }

    /// SHA-256 of the relying-party id.
    pub const fn id_hash(&self) -> &[u8; 32] {
        &self.id_hash
    }
}
