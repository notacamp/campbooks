//! Secrets the server makes and keeps: random tokens, their hashes, PKCE,
//! and the encryption of secrets at rest.
//!
//! - **The instance secret** ([`SecretKey`]) is 32 random bytes from
//!   `CAMPBOOKS_SECRET_KEY_FILE` or `CAMPBOOKS_SECRET_KEY` (standard base64),
//!   loaded with [`SecretKey::from_env`].
//! - **Sealing** ([`Cipher`]): AES-256-GCM under a key derived (HKDF-SHA256)
//!   from the instance secret and a **purpose label**, so every use (the web
//!   BFF's session tokens, connectors' credentials) has its own key. Each
//!   value gets a fresh random nonce, and the caller's context (table,
//!   column, row) is the associated data, so a ciphertext cannot be moved to
//!   another row, column or table.
//! - Cookie and invitation tokens are 256-bit random values
//!   ([`random_token`]); the database keeps only their SHA-256 ([`hash`]),
//!   compared in constant time ([`same_hash`]).

use std::path::PathBuf;

use aws_lc_rs::{
    aead::{AES_256_GCM, Aad, LessSafeKey, NONCE_LEN, Nonce, UnboundKey},
    constant_time, digest, hkdf,
    rand::{SecureRandom as _, SystemRandom},
};
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use secrecy::{ExposeSecret as _, SecretString};
use serde::Deserialize;

/// Length of the instance secret once decoded.
pub const SECRET_KEY_LEN: usize = 32;

/// First byte of every sealed value, so the format can change later.
const FORMAT_V1: u8 = 1;

/// The instance secret, `CAMPBOOKS_SECRET_KEY[_FILE]`. Never printed.
#[derive(Clone, PartialEq, Eq)]
pub struct SecretKey([u8; SECRET_KEY_LEN]);

impl std::fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretKey(<redacted>)")
    }
}

/// Where the instance secret may be set.
#[derive(Debug, Default, Deserialize)]
struct SecretEnv {
    secret_key: Option<SecretString>,
    secret_key_file: Option<PathBuf>,
}

impl SecretKey {
    /// A key made of `bytes` (tests, or a key read elsewhere).
    pub const fn from_bytes(bytes: [u8; SECRET_KEY_LEN]) -> Self {
        Self(bytes)
    }

    /// Decode standard base64 of 32 random bytes.
    pub fn decode(encoded: &str) -> Result<Self, String> {
        let bytes = STANDARD
            .decode(encoded.trim())
            .map_err(|_| "is not valid base64".to_owned())?;
        bytes.try_into().map(Self).map_err(|bytes: Vec<u8>| {
            format!(
                "decodes to {} bytes; it must be {SECRET_KEY_LEN} (e.g. `openssl rand -base64 32`)",
                bytes.len()
            )
        })
    }

    /// The instance secret from `CAMPBOOKS_SECRET_KEY_FILE` (preferred) or
    /// `CAMPBOOKS_SECRET_KEY`; `None` when neither is set. An error names the
    /// variable at fault.
    pub fn from_env() -> Result<Option<Self>, String> {
        let env: SecretEnv = crate::config::env_source()
            .extract()
            .map_err(|e| format!("invalid configuration: {e}"))?;
        match (env.secret_key_file, env.secret_key) {
            (Some(path), _) => std::fs::read_to_string(&path)
                .map_err(|e| format!("CAMPBOOKS_SECRET_KEY_FILE={}: {e}", path.display()))
                .and_then(|key| {
                    Self::decode(&key)
                        .map_err(|e| format!("CAMPBOOKS_SECRET_KEY_FILE={}: {e}", path.display()))
                })
                .map(Some),
            (None, Some(key)) => Self::decode(key.expose_secret())
                .map(Some)
                .map_err(|e| format!("CAMPBOOKS_SECRET_KEY {e}")),
            (None, None) => Ok(None),
        }
    }
}

/// A 256-bit secret derived from the instance secret for `purpose`
/// (HKDF-SHA256, `purpose` as `info`), URL-safe base64. Two parts of the
/// server that agree on `purpose` derive the same value without storing or
/// exchanging it (the web BFF's client secret at the in-process issuer).
/// Never change a label in use.
pub fn derive_secret(secret: &SecretKey, purpose: &'static [u8]) -> SecretString {
    struct Len32;
    impl hkdf::KeyType for Len32 {
        fn len(&self) -> usize {
            32
        }
    }
    let prk = hkdf::Salt::new(hkdf::HKDF_SHA256, &[]).extract(&secret.0);
    let info = [purpose];
    let mut bytes = [0_u8; 32];
    prk.expand(&info, Len32)
        .and_then(|okm| okm.fill(&mut bytes))
        .unwrap_or_else(|_| unreachable!("HKDF can expand to 32 bytes"));
    SecretString::from(URL_SAFE_NO_PAD.encode(bytes))
}

/// A URL-safe random token of 256 bits.
pub fn random_token() -> String {
    let mut bytes = [0_u8; 32];
    // The system RNG failing means the OS cannot provide entropy at all.
    SystemRandom::new()
        .fill(&mut bytes)
        .unwrap_or_else(|_| unreachable!("the system random number generator failed"));
    URL_SAFE_NO_PAD.encode(bytes)
}

/// SHA-256 of `value`: what the database stores instead of a token.
pub fn hash(value: &str) -> Vec<u8> {
    digest::digest(&digest::SHA256, value.as_bytes())
        .as_ref()
        .to_vec()
}

/// Whether two hashes are equal, in constant time.
pub fn same_hash(a: &[u8], b: &[u8]) -> bool {
    constant_time::verify_slices_are_equal(a, b).is_ok()
}

/// The PKCE S256 challenge of `verifier` (RFC 7636 §4.2).
pub fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(digest::digest(&digest::SHA256, verifier.as_bytes()))
}

/// Seals and opens secrets kept at rest, for one purpose.
pub struct Cipher {
    key: LessSafeKey,
    rng: SystemRandom,
}

impl std::fmt::Debug for Cipher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Cipher(AES-256-GCM)")
    }
}

/// A sealed value could not be opened: wrong key (e.g. after the secret key
/// was rotated) or tampered data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a stored secret could not be decrypted")]
pub struct OpenError;

impl Cipher {
    /// The cipher for `purpose` under the instance secret `secret`.
    /// `purpose` is the HKDF `info`: a fixed, versioned label per use
    /// (`b"campbooks bff token encryption v1"`); never change a label in
    /// use, or what it sealed cannot be opened.
    pub fn new(secret: &SecretKey, purpose: &'static [u8]) -> Self {
        let prk = hkdf::Salt::new(hkdf::HKDF_SHA256, &[]).extract(&secret.0);
        let info = [purpose];
        let okm = prk
            .expand(&info, &AES_256_GCM)
            .unwrap_or_else(|_| unreachable!("HKDF can expand to one AES-256 key"));
        Self {
            key: LessSafeKey::new(UnboundKey::from(okm)),
            rng: SystemRandom::new(),
        }
    }

    /// `FORMAT_V1 || nonce || ciphertext || tag`, bound to `context`.
    pub fn seal(&self, plaintext: &str, context: &[u8]) -> Vec<u8> {
        let mut nonce = [0_u8; NONCE_LEN];
        self.rng
            .fill(&mut nonce)
            .unwrap_or_else(|_| unreachable!("the system random number generator failed"));
        let mut sealed = Vec::with_capacity(1 + NONCE_LEN + plaintext.len() + 16);
        sealed.push(FORMAT_V1);
        sealed.extend_from_slice(&nonce);
        let mut body = plaintext.as_bytes().to_vec();
        self.key
            .seal_in_place_append_tag(
                Nonce::assume_unique_for_key(nonce),
                Aad::from(context),
                &mut body,
            )
            .unwrap_or_else(|_| unreachable!("AES-GCM seals any value shorter than 64 GiB"));
        sealed.extend_from_slice(&body);
        sealed
    }

    /// The plaintext of `sealed`, which must have been sealed for `context`.
    pub fn open(&self, sealed: &[u8], context: &[u8]) -> Result<String, OpenError> {
        let (&format, rest) = sealed.split_first().ok_or(OpenError)?;
        if format != FORMAT_V1 || rest.len() < NONCE_LEN {
            return Err(OpenError);
        }
        let (nonce, body) = rest.split_at(NONCE_LEN);
        let nonce = Nonce::try_assume_unique_for_key(nonce).map_err(|_| OpenError)?;
        let mut body = body.to_vec();
        let plaintext = self
            .key
            .open_in_place(nonce, Aad::from(context), &mut body)
            .map_err(|_| OpenError)?;
        String::from_utf8(plaintext.to_vec()).map_err(|_| OpenError)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PURPOSE: &[u8] = b"campbooks test v1";

    fn key(byte: u8) -> SecretKey {
        SecretKey::from_bytes([byte; SECRET_KEY_LEN])
    }

    #[test]
    fn seals_and_opens_bound_to_context() {
        let cipher = Cipher::new(&key(7), PURPOSE);
        let sealed = cipher.seal("refresh-token", b"row-1:refresh");
        assert!(!String::from_utf8_lossy(&sealed).contains("refresh-token"));
        assert_eq!(
            cipher.open(&sealed, b"row-1:refresh").as_deref(),
            Ok("refresh-token")
        );
        // Another row or column, another key, or tampering: refused.
        assert_eq!(cipher.open(&sealed, b"row-2:refresh"), Err(OpenError));
        assert_eq!(
            Cipher::new(&key(8), PURPOSE).open(&sealed, b"row-1:refresh"),
            Err(OpenError)
        );
        let mut tampered = sealed.clone();
        if let Some(last) = tampered.last_mut() {
            *last ^= 1;
        }
        assert_eq!(cipher.open(&tampered, b"row-1:refresh"), Err(OpenError));
        assert_eq!(cipher.open(&[], b"row-1:refresh"), Err(OpenError));
        // A fresh nonce every time.
        assert_ne!(sealed, cipher.seal("refresh-token", b"row-1:refresh"));
    }

    #[test]
    fn derived_secrets_are_stable_and_per_purpose() {
        let one = derive_secret(&key(7), b"campbooks one v1");
        assert_eq!(
            one.expose_secret(),
            derive_secret(&key(7), b"campbooks one v1").expose_secret()
        );
        assert_ne!(
            one.expose_secret(),
            derive_secret(&key(7), b"campbooks two v1").expose_secret()
        );
        assert_ne!(
            one.expose_secret(),
            derive_secret(&key(8), b"campbooks one v1").expose_secret()
        );
        assert_eq!(one.expose_secret().len(), 43);
    }

    #[test]
    fn each_purpose_has_its_own_key() {
        let sealed = Cipher::new(&key(7), b"campbooks one v1").seal("secret", b"ctx");
        assert_eq!(
            Cipher::new(&key(7), b"campbooks two v1").open(&sealed, b"ctx"),
            Err(OpenError)
        );
    }

    /// A value sealed by the web BFF before this code moved into the kernel
    /// (key `[7; 32]`, the BFF's label, a fixed nonce; computed with an
    /// independent HKDF + AES-GCM implementation) still opens: the move
    /// changed neither the derivation nor the format.
    #[test]
    fn values_sealed_before_the_move_still_open() {
        const SEALED: &str =
            "AQABAgMEBQYHCAkKC11jeA3Oce/KZFp9ipw2oDDCwlNIoRJttTNbf3IyECfIC857231hue+RFXEdzEmNplCn";
        let cipher = Cipher::new(&key(7), b"campbooks bff token encryption v1");
        let sealed = STANDARD.decode(SEALED).unwrap();
        assert_eq!(
            cipher.open(&sealed, b"session-1:refresh_token").as_deref(),
            Ok("refresh-token-from-before-the-move")
        );
    }

    #[test]
    fn secret_key_must_be_32_bytes_of_base64() {
        assert_eq!(SecretKey::decode(&STANDARD.encode([1_u8; 32])), Ok(key(1)));
        assert_eq!(
            SecretKey::decode(&format!(" {}\n", STANDARD.encode([1_u8; 32]))),
            Ok(key(1))
        );
        assert!(
            SecretKey::decode("not base64!")
                .unwrap_err()
                .contains("base64")
        );
        assert!(
            SecretKey::decode(&STANDARD.encode([0_u8; 16]))
                .unwrap_err()
                .contains("16 bytes")
        );
        assert_eq!(format!("{:?}", key(1)), "SecretKey(<redacted>)");
    }

    #[test]
    fn tokens_are_random_and_hashed() {
        let a = random_token();
        assert_eq!(a.len(), 43);
        assert_ne!(a, random_token());
        assert_eq!(hash(&a).len(), 32);
        assert!(same_hash(&hash(&a), &hash(&a)));
        assert!(!same_hash(&hash(&a), &hash("other")));
    }

    #[test]
    fn pkce_s256_matches_rfc_7636_appendix_b() {
        assert_eq!(
            pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }
}
