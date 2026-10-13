//! Authenticator data (WebAuthn L3 §6.1): the bytes the authenticator signs
//! over, after the client data hash.
//!
//! ```text
//! rpIdHash (32) | flags (1) | signCount (4, big-endian)
//! [ attested credential data, when AT: aaguid (16) | credentialIdLength (2)
//!   | credentialId | credentialPublicKey (COSE_Key, CBOR) ]
//! [ extensions (CBOR map), when ED ]
//! ```
//!
//! Parsing is strict: every byte must belong to a field the flags announce.

use super::{Refusal, refuse};

/// Longest credential id WebAuthn allows (§5.8.3 / §6.5.1).
pub const MAX_CREDENTIAL_ID: usize = 1023;

/// The flags byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flags(pub u8);

impl Flags {
    const UP: u8 = 1 << 0;
    const UV: u8 = 1 << 2;
    const BE: u8 = 1 << 3;
    const BS: u8 = 1 << 4;
    const AT: u8 = 1 << 6;
    const ED: u8 = 1 << 7;

    /// User present.
    pub const fn user_present(self) -> bool {
        self.0 & Self::UP != 0
    }
    /// User verified.
    pub const fn user_verified(self) -> bool {
        self.0 & Self::UV != 0
    }
    /// Backup eligible: the credential may be synced (a multi-device
    /// passkey). Fixed for the credential's lifetime.
    pub const fn backup_eligible(self) -> bool {
        self.0 & Self::BE != 0
    }
    /// Backed up: it is synced now.
    pub const fn backed_up(self) -> bool {
        self.0 & Self::BS != 0
    }
    /// Attested credential data follows.
    pub const fn attested(self) -> bool {
        self.0 & Self::AT != 0
    }
    /// Extensions follow.
    pub const fn extensions(self) -> bool {
        self.0 & Self::ED != 0
    }
}

/// The attested credential data of a registration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttestedCredential {
    /// The authenticator model's AAGUID (all zeros under attestation `none`).
    pub aaguid: [u8; 16],
    /// The credential id.
    pub credential_id: Vec<u8>,
    /// The credential's COSE_Key, exactly as encoded.
    pub public_key: Vec<u8>,
}

/// Parsed authenticator data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthData {
    /// SHA-256 of the relying-party id the authenticator used.
    pub rp_id_hash: [u8; 32],
    /// Flags.
    pub flags: Flags,
    /// The signature counter (0 when the authenticator keeps none).
    pub sign_count: u32,
    /// Present when [`Flags::attested`].
    pub attested: Option<AttestedCredential>,
}

/// Read one CBOR data item from the front of `bytes`; how many bytes it
/// took. `None` when `bytes` does not start with a well-formed item.
fn cbor_item_len(bytes: &[u8]) -> Option<usize> {
    let mut rest = bytes;
    let _: ciborium::Value = ciborium::de::from_reader(&mut rest).ok()?;
    Some(bytes.len() - rest.len())
}

/// Parse authenticator data.
pub fn parse(bytes: &[u8]) -> Result<AuthData, Refusal> {
    if bytes.len() < 37 {
        return refuse("auth_data_too_short");
    }
    let (head, mut rest) = bytes.split_at(37);
    let mut rp_id_hash = [0_u8; 32];
    rp_id_hash.copy_from_slice(&head[..32]);
    let flags = Flags(head[32]);
    let sign_count = u32::from_be_bytes([head[33], head[34], head[35], head[36]]);
    let attested = if flags.attested() {
        if rest.len() < 18 {
            return refuse("attested_data_too_short");
        }
        let mut aaguid = [0_u8; 16];
        aaguid.copy_from_slice(&rest[..16]);
        let id_len = usize::from(u16::from_be_bytes([rest[16], rest[17]]));
        rest = &rest[18..];
        if id_len == 0 || id_len > MAX_CREDENTIAL_ID || rest.len() < id_len {
            return refuse("credential_id_length");
        }
        let (credential_id, after_id) = rest.split_at(id_len);
        let key_len = cbor_item_len(after_id).ok_or(Refusal("public_key_not_cbor"))?;
        let (public_key, after_key) = after_id.split_at(key_len);
        rest = after_key;
        Some(AttestedCredential {
            aaguid,
            credential_id: credential_id.to_vec(),
            public_key: public_key.to_vec(),
        })
    } else {
        None
    };
    if flags.extensions() {
        let len = cbor_item_len(rest).ok_or(Refusal("extensions_not_cbor"))?;
        rest = &rest[len..];
    }
    if !rest.is_empty() {
        return refuse("auth_data_trailing_bytes");
    }
    Ok(AuthData {
        rp_id_hash,
        flags,
        sign_count,
        attested,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(flags: u8, count: u32) -> Vec<u8> {
        let mut bytes = vec![7_u8; 32];
        bytes.push(flags);
        bytes.extend_from_slice(&count.to_be_bytes());
        bytes
    }

    #[test]
    fn a_bare_assertion_header_parses() {
        let parsed = parse(&header(0b0001_1101, 42)).unwrap();
        assert_eq!(parsed.rp_id_hash, [7; 32]);
        assert!(parsed.flags.user_present());
        assert!(parsed.flags.user_verified());
        assert!(parsed.flags.backup_eligible());
        assert!(parsed.flags.backed_up());
        assert_eq!(parsed.sign_count, 42);
        assert!(parsed.attested.is_none());
    }

    #[test]
    fn attested_data_is_split_where_the_key_ends() {
        let mut bytes = header(0b1100_0101, 0);
        bytes.extend_from_slice(&[1; 16]);
        bytes.extend_from_slice(&16_u16.to_be_bytes());
        bytes.extend_from_slice(&[9; 16]);
        // A one-entry CBOR map {1: 1} as the "key", then extensions {}.
        bytes.extend_from_slice(&[0xa1, 0x01, 0x01]);
        bytes.push(0xa0);
        let parsed = parse(&bytes).unwrap();
        let attested = parsed.attested.unwrap();
        assert_eq!(attested.aaguid, [1; 16]);
        assert_eq!(attested.credential_id, vec![9; 16]);
        assert_eq!(attested.public_key, vec![0xa1, 0x01, 0x01]);
    }

    #[test]
    fn malformed_data_is_refused() {
        assert_eq!(parse(&[0; 36]), refuse("auth_data_too_short"));
        let mut trailing = header(0b0000_0101, 0);
        trailing.push(0);
        assert_eq!(parse(&trailing), refuse("auth_data_trailing_bytes"));
        let mut no_key = header(0b0100_0101, 0);
        no_key.extend_from_slice(&[0; 16]);
        no_key.extend_from_slice(&4_u16.to_be_bytes());
        no_key.extend_from_slice(&[1, 2, 3, 4]);
        assert_eq!(parse(&no_key), refuse("public_key_not_cbor"));
        let mut long_id = header(0b0100_0101, 0);
        long_id.extend_from_slice(&[0; 16]);
        long_id.extend_from_slice(&1024_u16.to_be_bytes());
        long_id.extend_from_slice(&[0; 1024]);
        assert_eq!(parse(&long_id), refuse("credential_id_length"));
        let mut bad_ext = header(0b1000_0101, 0);
        bad_ext.push(0xff);
        assert_eq!(parse(&bad_ext), refuse("extensions_not_cbor"));
    }
}
