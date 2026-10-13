//! Secrets the BFF makes and keeps: random tokens, their hashes, PKCE, and
//! the encryption of tokens at rest.
//!
//! - Session and login cookies hold 256-bit random tokens; the database keeps
//!   only their SHA-256, so a leaked table cannot be replayed as cookies.
//! - Access, refresh and ID tokens are sealed with AES-256-GCM under a key
//!   derived (HKDF-SHA256) from `CAMPBOOKS_SECRET_KEY`, with a fresh random
//!   nonce per value and the row id and column name as associated data, so a
//!   ciphertext cannot be moved to another row or column.

use aws_lc_rs::{
    aead::{AES_256_GCM, Aad, LessSafeKey, NONCE_LEN, Nonce, UnboundKey},
    constant_time, digest, hkdf,
    rand::{SecureRandom as _, SystemRandom},
};
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};

/// Length of `CAMPBOOKS_SECRET_KEY` once decoded.
pub const SECRET_KEY_LEN: usize = 32;

/// HKDF `info` of the BFF's token-encryption key: other uses of the instance
/// secret derive their own keys.
const KEY_INFO: &[u8] = b"campbooks bff token encryption v1";

/// First byte of every sealed value, so the format can change later.
const FORMAT_V1: u8 = 1;

/// Decode `CAMPBOOKS_SECRET_KEY` (standard base64 of 32 random bytes).
pub fn decode_secret_key(encoded: &str) -> Result<[u8; SECRET_KEY_LEN], String> {
    let bytes = STANDARD
        .decode(encoded.trim())
        .map_err(|_| "is not valid base64".to_owned())?;
    bytes.try_into().map_err(|bytes: Vec<u8>| {
        format!(
            "decodes to {} bytes; it must be {SECRET_KEY_LEN} (e.g. `openssl rand -base64 32`)",
            bytes.len()
        )
    })
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

/// SHA-256 of `value`: what the database stores instead of a cookie token.
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

/// Seals and opens tokens kept at rest.
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
#[error("a stored token could not be decrypted")]
pub struct OpenError;

impl Cipher {
    /// The cipher for the instance secret `secret`.
    pub fn new(secret: &[u8; SECRET_KEY_LEN]) -> Self {
        let prk = hkdf::Salt::new(hkdf::HKDF_SHA256, &[]).extract(secret);
        let okm = prk
            .expand(&[KEY_INFO], &AES_256_GCM)
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

    fn key(byte: u8) -> [u8; SECRET_KEY_LEN] {
        [byte; SECRET_KEY_LEN]
    }

    #[test]
    fn seals_and_opens_bound_to_context() {
        let cipher = Cipher::new(&key(7));
        let sealed = cipher.seal("refresh-token", b"row-1:refresh");
        assert!(!String::from_utf8_lossy(&sealed).contains("refresh-token"));
        assert_eq!(
            cipher.open(&sealed, b"row-1:refresh").as_deref(),
            Ok("refresh-token")
        );
        // Another row or column, another key, or tampering: refused.
        assert_eq!(cipher.open(&sealed, b"row-2:refresh"), Err(OpenError));
        assert_eq!(
            Cipher::new(&key(8)).open(&sealed, b"row-1:refresh"),
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
    fn secret_key_must_be_32_bytes_of_base64() {
        assert_eq!(decode_secret_key(&STANDARD.encode(key(1))), Ok(key(1)));
        assert_eq!(
            decode_secret_key(&format!(" {}\n", STANDARD.encode(key(1)))),
            Ok(key(1))
        );
        assert!(
            decode_secret_key("not base64!")
                .unwrap_err()
                .contains("base64")
        );
        assert!(
            decode_secret_key(&STANDARD.encode([0_u8; 16]))
                .unwrap_err()
                .contains("16 bytes")
        );
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
