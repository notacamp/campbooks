//! Credential public keys: the COSE_Key an authenticator registers (RFC
//! 9052 §7, RFC 9053, RFC 8230), parsed with `coset`, and signature checks
//! with `aws-lc-rs`.
//!
//! Only three shapes are accepted, each with its `alg` stated and matching
//! its key type: OKP/Ed25519 with EdDSA, EC2/P-256 with ES256 (uncompressed
//! coordinates), RSA (2048 bits or more) with RS256.

use aws_lc_rs::signature::{
    ECDSA_P256_SHA256_ASN1, ED25519, ParsedPublicKey, RSA_PKCS1_2048_8192_SHA256,
    RsaPublicKeyComponents, UnparsedPublicKey,
};
use coset::{
    CborSerializable as _, CoseKey, Label, RegisteredLabel, RegisteredLabelWithPrivate,
    cbor::Value,
    iana::{self, EnumI64 as _},
};

use super::{Refusal, refuse};

/// A credential public key Campbooks can verify with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublicKey {
    /// EdDSA over Ed25519: the 32-byte point.
    Ed25519(Vec<u8>),
    /// ES256: the uncompressed P-256 point (`04 || x || y`).
    P256(Vec<u8>),
    /// RS256: modulus and exponent, big-endian, no leading zeros.
    Rsa {
        /// Modulus.
        n: Vec<u8>,
        /// Public exponent.
        e: Vec<u8>,
    },
}

impl PublicKey {
    /// The COSE algorithm this key verifies with.
    pub const fn alg(&self) -> i64 {
        match self {
            Self::Ed25519(_) => iana::Algorithm::EdDSA as i64,
            Self::P256(_) => iana::Algorithm::ES256 as i64,
            Self::Rsa { .. } => iana::Algorithm::RS256 as i64,
        }
    }

    /// Whether `sig` is this key's signature over `message`.
    pub fn verifies(&self, message: &[u8], sig: &[u8]) -> bool {
        match self {
            Self::Ed25519(point) => UnparsedPublicKey::new(&ED25519, point)
                .verify(message, sig)
                .is_ok(),
            Self::P256(point) => UnparsedPublicKey::new(&ECDSA_P256_SHA256_ASN1, point)
                .verify(message, sig)
                .is_ok(),
            Self::Rsa { n, e } => RsaPublicKeyComponents { n, e }
                .verify(&RSA_PKCS1_2048_8192_SHA256, message, sig)
                .is_ok(),
        }
    }
}

/// The parameter `label` of `key`, as bytes.
fn bytes(key: &CoseKey, label: i64) -> Option<&[u8]> {
    key.params.iter().find_map(|(l, v)| match (l, v) {
        (Label::Int(l), Value::Bytes(b)) if *l == label => Some(b.as_slice()),
        _ => None,
    })
}

/// The parameter `label` of `key`, as an integer.
fn int(key: &CoseKey, label: i64) -> Option<i64> {
    key.params.iter().find_map(|(l, v)| match (l, v) {
        (Label::Int(l), Value::Integer(i)) if *l == label => i64::try_from(*i).ok(),
        _ => None,
    })
}

/// The Ed25519 points of small order (libsodium's list, compared on the
/// first 31 bytes and the last without its sign bit): the identity, the
/// points of order 2, 4 and 8, and the non-canonical encodings of 0 and 1.
/// A key among them would verify signatures nobody made.
const ED25519_SMALL_ORDER: [[u8; 32]; 7] = [
    [0; 32],
    [
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ],
    [
        0x26, 0xe8, 0x95, 0x8f, 0xc2, 0xb2, 0x27, 0xb0, 0x45, 0xc3, 0xf4, 0x89, 0xf2, 0xef, 0x98,
        0xf0, 0xd5, 0xdf, 0xac, 0x05, 0xd3, 0xc6, 0x33, 0x39, 0xb1, 0x38, 0x02, 0x88, 0x6d, 0x53,
        0xfc, 0x05,
    ],
    [
        0xc7, 0x17, 0x6a, 0x70, 0x3d, 0x4d, 0xd8, 0x4f, 0xba, 0x3c, 0x0b, 0x76, 0x0d, 0x10, 0x67,
        0x0f, 0x2a, 0x20, 0x53, 0xfa, 0x2c, 0x39, 0xcc, 0xc6, 0x4e, 0xc7, 0xfd, 0x77, 0x92, 0xac,
        0x03, 0x7a,
    ],
    [
        0xec, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ],
    [
        0xed, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ],
    [
        0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ],
];

/// Whether `point` is an acceptable Ed25519 public key: a canonical
/// encoding (y < 2^255 - 19), not of small order, that `aws-lc` parses.
fn ed25519_acceptable(point: &[u8]) -> bool {
    let Ok(point) = <[u8; 32]>::try_from(point) else {
        return false;
    };
    let small_order = ED25519_SMALL_ORDER
        .iter()
        .any(|bad| bad[..31] == point[..31] && bad[31] == point[31] & 0x7f);
    // y >= p: the 254-bit value is p..2^255 - 1, i.e. every byte 0xff but
    // the first (>= 0xed) and the top one (0x7f without the sign bit).
    let non_canonical =
        point[0] >= 0xed && point[1..31].iter().all(|b| *b == 0xff) && point[31] & 0x7f == 0x7f;
    !small_order && !non_canonical && ParsedPublicKey::new(&ED25519, point).is_ok()
}

fn without_leading_zeros(value: &[u8]) -> Vec<u8> {
    let start = value.iter().position(|b| *b != 0).unwrap_or(value.len());
    value[start..].to_vec()
}

/// Parse the COSE_Key `encoded` into a key Campbooks accepts.
pub fn parse(encoded: &[u8]) -> Result<PublicKey, Refusal> {
    let key = CoseKey::from_slice(encoded).map_err(|_| Refusal("cose_key_malformed"))?;
    let alg = match &key.alg {
        Some(RegisteredLabelWithPrivate::Assigned(alg)) => alg.to_i64(),
        _ => return refuse("cose_key_without_algorithm"),
    };
    // A private part (OKP/EC2 `d`; RSA `d`, the primes and CRT values)
    // has no business in a public key.
    let private: &[i64] = match &key.kty {
        RegisteredLabel::Assigned(iana::KeyType::RSA) => {
            &[-3, -4, -5, -6, -7, -8, -9, -10, -11, -12]
        }
        _ => &[-4],
    };
    if key
        .params
        .iter()
        .any(|(l, _)| matches!(l, Label::Int(l) if private.contains(l)))
    {
        return refuse("cose_key_has_private_part");
    }
    match (&key.kty, alg) {
        (RegisteredLabel::Assigned(iana::KeyType::OKP), -8) => {
            let curve = int(&key, iana::OkpKeyParameter::Crv.to_i64());
            let x = bytes(&key, iana::OkpKeyParameter::X.to_i64());
            match (curve, x) {
                (Some(6), Some(x)) if x.len() == 32 => {
                    if ed25519_acceptable(x) {
                        Ok(PublicKey::Ed25519(x.to_vec()))
                    } else {
                        refuse("cose_key_weak_ed25519")
                    }
                }
                _ => refuse("cose_key_bad_okp"),
            }
        }
        (RegisteredLabel::Assigned(iana::KeyType::EC2), -7) => {
            let curve = int(&key, iana::Ec2KeyParameter::Crv.to_i64());
            let x = bytes(&key, iana::Ec2KeyParameter::X.to_i64());
            let y = bytes(&key, iana::Ec2KeyParameter::Y.to_i64());
            match (curve, x, y) {
                (Some(1), Some(x), Some(y)) if x.len() == 32 && y.len() == 32 => {
                    let mut point = Vec::with_capacity(65);
                    point.push(0x04);
                    point.extend_from_slice(x);
                    point.extend_from_slice(y);
                    // On the curve, not the point at infinity: `aws-lc`
                    // parses (and so validates) it.
                    if ParsedPublicKey::new(&ECDSA_P256_SHA256_ASN1, &point).is_ok() {
                        Ok(PublicKey::P256(point))
                    } else {
                        refuse("cose_key_not_on_curve")
                    }
                }
                _ => refuse("cose_key_bad_ec2"),
            }
        }
        (RegisteredLabel::Assigned(iana::KeyType::RSA), -257) => {
            let n = bytes(&key, iana::RsaKeyParameter::N.to_i64()).map(without_leading_zeros);
            let e = bytes(&key, iana::RsaKeyParameter::E.to_i64()).map(without_leading_zeros);
            match (n, e) {
                (Some(n), Some(e)) if n.len() >= 256 && n.len() <= 1024 && !e.is_empty() => {
                    Ok(PublicKey::Rsa { n, e })
                }
                _ => refuse("cose_key_bad_rsa"),
            }
        }
        _ => refuse("cose_key_unsupported"),
    }
}

#[cfg(test)]
mod tests {
    use coset::CborSerializable as _;

    use super::*;

    fn key(
        kty: iana::KeyType,
        alg: Option<iana::Algorithm>,
        params: Vec<(Label, Value)>,
    ) -> Vec<u8> {
        CoseKey {
            kty: RegisteredLabel::Assigned(kty),
            alg: alg.map(RegisteredLabelWithPrivate::Assigned),
            params,
            ..CoseKey::default()
        }
        .to_vec()
        .unwrap()
    }

    fn okp(crv: i64, x: Vec<u8>) -> Vec<(Label, Value)> {
        vec![
            (Label::Int(-1), Value::Integer(crv.into())),
            (Label::Int(-2), Value::Bytes(x)),
        ]
    }

    #[test]
    fn only_the_three_shapes_parse() {
        let ed = key(
            iana::KeyType::OKP,
            Some(iana::Algorithm::EdDSA),
            okp(6, vec![1; 32]),
        );
        assert_eq!(parse(&ed), Ok(PublicKey::Ed25519(vec![1; 32])));
        assert_eq!(parse(&ed).unwrap().alg(), -8);

        let no_alg = key(iana::KeyType::OKP, None, okp(6, vec![9; 32]));
        assert_eq!(parse(&no_alg), refuse("cose_key_without_algorithm"));
        let ed448 = key(
            iana::KeyType::OKP,
            Some(iana::Algorithm::EdDSA),
            okp(7, vec![1; 57]),
        );
        assert_eq!(parse(&ed448), refuse("cose_key_bad_okp"));
        let mismatched = key(
            iana::KeyType::EC2,
            Some(iana::Algorithm::EdDSA),
            okp(6, vec![1; 32]),
        );
        assert_eq!(parse(&mismatched), refuse("cose_key_unsupported"));
        let p384 = key(iana::KeyType::EC2, Some(iana::Algorithm::ES384), Vec::new());
        assert_eq!(parse(&p384), refuse("cose_key_unsupported"));
        let compressed = key(
            iana::KeyType::EC2,
            Some(iana::Algorithm::ES256),
            vec![
                (Label::Int(-1), Value::Integer(1.into())),
                (Label::Int(-2), Value::Bytes(vec![2; 32])),
                (Label::Int(-3), Value::Bool(true)),
            ],
        );
        assert_eq!(parse(&compressed), refuse("cose_key_bad_ec2"));
        let small_rsa = key(
            iana::KeyType::RSA,
            Some(iana::Algorithm::RS256),
            vec![
                (Label::Int(-1), Value::Bytes(vec![0xc3; 128])),
                (Label::Int(-2), Value::Bytes(vec![1, 0, 1])),
            ],
        );
        assert_eq!(parse(&small_rsa), refuse("cose_key_bad_rsa"));
        let mut private = okp(6, vec![9; 32]);
        private.push((Label::Int(-4), Value::Bytes(vec![2; 32])));
        let leaked = key(iana::KeyType::OKP, Some(iana::Algorithm::EdDSA), private);
        assert_eq!(parse(&leaked), refuse("cose_key_has_private_part"));
        assert_eq!(parse(&[0xff]), refuse("cose_key_malformed"));
    }

    #[test]
    fn weak_and_invalid_points_are_refused() {
        let ed = |x: Vec<u8>| key(iana::KeyType::OKP, Some(iana::Algorithm::EdDSA), okp(6, x));
        for weak in ED25519_SMALL_ORDER {
            assert_eq!(
                parse(&ed(weak.to_vec())),
                refuse("cose_key_weak_ed25519"),
                "{weak:?}"
            );
            // The same with the sign bit set.
            let mut signed = weak;
            signed[31] |= 0x80;
            assert_eq!(parse(&ed(signed.to_vec())), refuse("cose_key_weak_ed25519"));
        }
        // y = p + 5: not canonical.
        let mut big = [0xff; 32];
        big[0] = 0xf2;
        big[31] = 0x7f;
        assert_eq!(parse(&ed(big.to_vec())), refuse("cose_key_weak_ed25519"));

        let ec = |x: Vec<u8>, y: Vec<u8>| {
            key(
                iana::KeyType::EC2,
                Some(iana::Algorithm::ES256),
                vec![
                    (Label::Int(-1), Value::Integer(1.into())),
                    (Label::Int(-2), Value::Bytes(x)),
                    (Label::Int(-3), Value::Bytes(y)),
                ],
            )
        };
        let mut one = vec![0; 32];
        one[31] = 1;
        assert_eq!(
            parse(&ec(one.clone(), one)),
            refuse("cose_key_not_on_curve")
        );
        assert_eq!(
            parse(&ec(vec![0; 32], vec![0; 32])),
            refuse("cose_key_not_on_curve")
        );
    }
}
