//! Credential public keys: the COSE_Key an authenticator registers (RFC
//! 9052 §7, RFC 9053, RFC 8230), parsed with `coset`, and signature checks
//! with `aws-lc-rs`.
//!
//! Only three shapes are accepted, each with its `alg` stated and matching
//! its key type: OKP/Ed25519 with EdDSA, EC2/P-256 with ES256 (uncompressed
//! coordinates), RSA (2048 bits or more) with RS256.

use aws_lc_rs::signature::{
    ECDSA_P256_SHA256_ASN1, ED25519, RSA_PKCS1_2048_8192_SHA256, RsaPublicKeyComponents,
    UnparsedPublicKey,
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
                (Some(6), Some(x)) if x.len() == 32 => Ok(PublicKey::Ed25519(x.to_vec())),
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
                    Ok(PublicKey::P256(point))
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

        let no_alg = key(iana::KeyType::OKP, None, okp(6, vec![1; 32]));
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
        let es384 = key(iana::KeyType::EC2, Some(iana::Algorithm::ES384), Vec::new());
        assert_eq!(parse(&es384), refuse("cose_key_unsupported"));
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
        let mut private = okp(6, vec![1; 32]);
        private.push((Label::Int(-4), Value::Bytes(vec![2; 32])));
        let leaked = key(iana::KeyType::OKP, Some(iana::Algorithm::EdDSA), private);
        assert_eq!(parse(&leaked), refuse("cose_key_has_private_part"));
        assert_eq!(parse(&[0xff]), refuse("cose_key_malformed"));
    }
}
