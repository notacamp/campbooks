//! Bindings: MACs over the requests that make the server act at an outside
//! account (send a message, change mail at the provider), keyed outside the
//! database (campbooks-cloud#227).
//!
//! A row in the database is not proof that a member asked for anything: a
//! transaction working in a workspace can write that workspace's rows. So
//! the request that asked (an authenticated member's call) is bound to what
//! it asked for with a MAC under a key the database never holds, and the
//! code that acts verifies the MAC against the stored request first. SQL
//! alone can then change or plant a request, but not make it verify.
//!
//! - **The key** is derived (HKDF-SHA256) from the instance secret
//!   ([`SecretKey`], `CAMPBOOKS_SECRET_KEY[_FILE]`) with the fixed label
//!   [`KEY_PURPOSE`]. It is never stored, logged or sent anywhere. Without
//!   an instance secret the binding is [`Binding::disabled`]: nothing is
//!   signed and nothing verifies, so nothing is sent (fail closed).
//! - **The input** is a [`Fields`]: a context label naming the use (and
//!   its version), then each field length-prefixed, so two different
//!   requests never encode to the same bytes.
//! - **A tag** is [`TAG_VERSION`] followed by the HMAC-SHA256, compared in
//!   constant time.

use std::sync::Arc;

use aws_lc_rs::{digest, hmac};
use uuid::Uuid;

use crate::crypto::{SecretKey, derive_bytes};

/// The HKDF label of the binding key. Never change it: every request bound
/// in flight would stop verifying (see `design.md` of campbooks-cloud#227
/// for rotation).
pub const KEY_PURPOSE: &[u8] = b"campbooks/send-binding/v1";

/// First byte of every tag, so the format can change later.
pub const TAG_VERSION: u8 = 1;

/// Length of a tag: the version byte and an HMAC-SHA256.
pub const TAG_LEN: usize = 1 + 32;

/// The canonical encoding of a bound request: a context label, then each
/// field as an 8-byte big-endian length and its bytes. Options and lists
/// encode their shape too (a presence byte, a count), so no two requests
/// share an encoding.
#[derive(Clone, PartialEq, Eq)]
pub struct Fields {
    bytes: Vec<u8>,
}

impl std::fmt::Debug for Fields {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The fields can hold addresses and message text: never printed.
        f.debug_struct("Fields").finish_non_exhaustive()
    }
}

impl Fields {
    /// Fields for `context`: the use and its version
    /// (`campbooks/<module>.<what>/v1`). A new encoding takes a new label.
    pub fn new(context: &str) -> Self {
        let mut fields = Self { bytes: Vec::new() };
        fields.str(context);
        fields
    }

    /// Append `value`, length-prefixed.
    pub fn bytes(&mut self, value: &[u8]) -> &mut Self {
        let len = u64::try_from(value.len()).unwrap_or(u64::MAX);
        self.bytes.extend_from_slice(&len.to_be_bytes());
        self.bytes.extend_from_slice(value);
        self
    }

    /// Append a string (its UTF-8 bytes).
    pub fn str(&mut self, value: &str) -> &mut Self {
        self.bytes(value.as_bytes())
    }

    /// Append an optional string: a presence byte, then the string.
    pub fn opt_str(&mut self, value: Option<&str>) -> &mut Self {
        match value {
            None => self.bytes(&[0]),
            Some(value) => self.bytes(&[1]).str(value),
        }
    }

    /// Append a UUID (its 16 bytes).
    pub fn uuid(&mut self, value: &Uuid) -> &mut Self {
        self.bytes(value.as_bytes())
    }

    /// Append an optional UUID: a presence byte, then the UUID.
    pub fn opt_uuid(&mut self, value: Option<&Uuid>) -> &mut Self {
        match value {
            None => self.bytes(&[0]),
            Some(value) => self.bytes(&[1]).uuid(value),
        }
    }

    /// Append a signed integer (8 bytes, big-endian).
    pub fn int(&mut self, value: i64) -> &mut Self {
        self.bytes(&value.to_be_bytes())
    }

    /// Append an unsigned integer (8 bytes, big-endian).
    pub fn uint(&mut self, value: u64) -> &mut Self {
        self.bytes(&value.to_be_bytes())
    }

    /// Append the length of a list whose items follow.
    pub fn count(&mut self, len: usize) -> &mut Self {
        self.uint(u64::try_from(len).unwrap_or(u64::MAX))
    }

    /// SHA-256 of the encoding: a content hash to bind in other fields.
    pub fn digest(&self) -> [u8; 32] {
        let mut out = [0_u8; 32];
        out.copy_from_slice(digest::digest(&digest::SHA256, &self.bytes).as_ref());
        out
    }
}

/// Signs and verifies bound requests under the key derived from the
/// instance secret. Cheap to clone. [`Binding::disabled`] (the default,
/// until the server installs one) signs nothing and verifies nothing.
#[derive(Clone, Default)]
pub struct Binding {
    key: Option<Arc<hmac::Key>>,
    /// The previous instance secret's key, during a rotation: it verifies
    /// what was bound before, and signs nothing (campbooks-cloud#191).
    previous: Option<Arc<hmac::Key>>,
}

impl std::fmt::Debug for Binding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.is_enabled() {
            "Binding(<key>)"
        } else {
            "Binding(disabled)"
        })
    }
}

impl Binding {
    /// The binding keyed by `secret` (the instance secret): every process
    /// of the install agrees.
    /// A previous key set on `secret` verifies what it bound too.
    pub fn new(secret: &SecretKey) -> Self {
        let key = |secret: &SecretKey| {
            Arc::new(hmac::Key::new(
                hmac::HMAC_SHA256,
                &derive_bytes(secret, KEY_PURPOSE),
            ))
        };
        Self {
            key: Some(key(secret)),
            previous: secret.previous_key().map(|previous| key(&previous)),
        }
    }

    /// No key (no instance secret): nothing is signed or verifies.
    pub fn disabled() -> Self {
        Self::default()
    }

    /// Whether there is a key to sign and verify with.
    pub fn is_enabled(&self) -> bool {
        self.key.is_some()
    }

    /// The tag of `fields`: [`TAG_VERSION`] and the HMAC. `None` without a
    /// key.
    pub fn tag(&self, fields: &Fields) -> Option<Vec<u8>> {
        let key = self.key.as_ref()?;
        let mut tag = Vec::with_capacity(TAG_LEN);
        tag.push(TAG_VERSION);
        tag.extend_from_slice(hmac::sign(key, &fields.bytes).as_ref());
        Some(tag)
    }

    /// Whether `tag` is the tag of `fields` under this key (the HMAC
    /// compared in constant time). `false` without a key, without a tag,
    /// or for a tag of another version.
    pub fn verify(&self, fields: &Fields, tag: Option<&[u8]>) -> bool {
        let (Some(key), Some(tag)) = (self.key.as_ref(), tag) else {
            return false;
        };
        let Some((&version, mac)) = tag.split_first() else {
            return false;
        };
        version == TAG_VERSION
            && (hmac::verify(key, &fields.bytes, mac).is_ok()
                || self
                    .previous
                    .as_ref()
                    .is_some_and(|previous| hmac::verify(previous, &fields.bytes, mac).is_ok()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::SECRET_KEY_LEN;

    fn binding(byte: u8) -> Binding {
        Binding::new(&SecretKey::from_bytes([byte; SECRET_KEY_LEN]))
    }

    fn hex(bytes: &[u8]) -> String {
        use std::fmt::Write as _;
        bytes.iter().fold(String::new(), |mut out, b| {
            let _ = write!(out, "{b:02x}");
            out
        })
    }

    fn sample() -> Fields {
        let mut fields = Fields::new("campbooks/test/v1");
        fields
            .str("hello")
            .uint(42)
            .int(-1)
            .opt_str(None)
            .opt_uuid(Some(
                &"0190a5e2-0000-7000-8000-000000000001".parse().unwrap(),
            ));
        fields
    }

    /// The key derivation and the encoding, pinned against an independent
    /// implementation (HKDF and HMAC from Python's standard library): a
    /// change to either would make every bound request in flight fail.
    #[test]
    fn the_format_is_pinned() {
        let fields = sample();
        assert_eq!(
            hex(&binding(7).tag(&fields).unwrap()),
            "01987593624348446a4e787ee572a8ace6df1a644eae67c83d5ce1e95bc793cacf"
        );
        assert_eq!(
            hex(&fields.digest()),
            "b7853750d960ac901bb48d54e782864588234519293ad7e024bdf5a91a9be976"
        );
    }

    #[test]
    fn a_tag_verifies_only_for_its_fields_and_key() {
        let binding = binding(7);
        let fields = sample();
        let tag = binding.tag(&fields).unwrap();
        assert_eq!(tag.len(), TAG_LEN);
        assert!(binding.verify(&fields, Some(&tag)));
        // Another key (another instance), no key, no tag.
        assert!(!self::binding(8).verify(&fields, Some(&tag)));
        assert!(!Binding::disabled().verify(&fields, Some(&tag)));
        assert!(Binding::disabled().tag(&fields).is_none());
        assert!(!binding.verify(&fields, None));
        assert!(!binding.verify(&fields, Some(&[])));
        // Another field, another context.
        let mut other = sample();
        other.str("more");
        assert!(!binding.verify(&other, Some(&tag)));
        let mut context = Fields::new("campbooks/other/v1");
        context
            .str("hello")
            .uint(42)
            .int(-1)
            .opt_str(None)
            .opt_uuid(Some(
                &"0190a5e2-0000-7000-8000-000000000001".parse().unwrap(),
            ));
        assert!(!binding.verify(&context, Some(&tag)));
        // Altered, truncated, or of another version.
        let mut altered = tag.clone();
        altered[TAG_LEN - 1] ^= 1;
        assert!(!binding.verify(&fields, Some(&altered)));
        assert!(!binding.verify(&fields, Some(&tag[..TAG_LEN - 1])));
        let mut version = tag.clone();
        version[0] = 2;
        assert!(!binding.verify(&fields, Some(&version)));
    }

    /// Length prefixes, presence bytes and counts: moving bytes between
    /// fields, or an empty value for a missing one, changes the encoding.
    /// During a rotation a tag of the previous key still verifies, new tags
    /// are the new key's, and without the previous key an old tag fails
    /// closed (campbooks-cloud#191).
    #[test]
    fn a_previous_key_verifies_what_it_bound() {
        let fields = sample();
        let old = binding(7).tag(&fields).unwrap();
        let rotated = Binding::new(
            &SecretKey::from_bytes([8; SECRET_KEY_LEN])
                .with_previous(&SecretKey::from_bytes([7; SECRET_KEY_LEN])),
        );
        assert!(rotated.verify(&fields, Some(&old)));
        let fresh = rotated.tag(&fields).unwrap();
        assert_eq!(
            fresh,
            binding(8).tag(&fields).unwrap(),
            "signed with the new key"
        );
        assert!(!binding(7).verify(&fields, Some(&fresh)));
        assert!(!binding(8).verify(&fields, Some(&old)));
        let mut other = Fields::new("campbooks/test/v1");
        other.str("hello");
        assert!(!rotated.verify(&other, Some(&old)));
    }

    #[test]
    fn the_encoding_is_unambiguous() {
        let encode = |build: fn(&mut Fields)| {
            let mut fields = Fields::new("campbooks/test/v1");
            build(&mut fields);
            fields
        };
        let split = [
            encode(|f| {
                f.str("ab").str("c");
            }),
            encode(|f| {
                f.str("a").str("bc");
            }),
            encode(|f| {
                f.str("abc");
            }),
            encode(|f| {
                f.opt_str(None);
            }),
            encode(|f| {
                f.opt_str(Some(""));
            }),
            encode(|f| {
                f.str("");
            }),
            encode(|f| {
                f.count(0).str("a");
            }),
            encode(|f| {
                f.count(1).str("a");
            }),
        ];
        for (i, a) in split.iter().enumerate() {
            for b in &split[i + 1..] {
                assert_ne!(a, b);
                assert_ne!(a.digest(), b.digest());
            }
        }
        assert_eq!(format!("{:?}", sample()), "Fields { .. }");
        assert_eq!(format!("{:?}", binding(7)), "Binding(<key>)");
        assert_eq!(format!("{:?}", Binding::disabled()), "Binding(disabled)");
    }
}
