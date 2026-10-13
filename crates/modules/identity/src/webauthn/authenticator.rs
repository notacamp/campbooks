//! A software authenticator and client for tests: it makes passkeys (Ed25519,
//! P-256 or RSA, with `aws-lc-rs`) and signs assertions exactly as a browser
//! and a platform authenticator would, and every field can be bent to test
//! a refusal.

use aws_lc_rs::{
    digest,
    rand::{SecureRandom as _, SystemRandom},
    rsa::{KeyPair as RsaKeyPair, KeySize},
    signature::{
        ECDSA_P256_SHA256_ASN1_SIGNING, EcdsaKeyPair, Ed25519KeyPair, KeyPair as _,
        RSA_PKCS1_SHA256,
    },
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ciborium::Value;
use coset::{CborSerializable as _, CoseKey, Label, iana};
use serde_json::json;

/// Which algorithm a passkey uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alg {
    /// EdDSA (-8).
    Ed25519,
    /// ES256 (-7).
    P256,
    /// RS256 (-257).
    Rsa,
}

enum Key {
    Ed25519(Ed25519KeyPair),
    P256(EcdsaKeyPair),
    Rsa(RsaKeyPair),
}

impl Key {
    fn generate(alg: Alg) -> Self {
        match alg {
            Alg::Ed25519 => Self::Ed25519(Ed25519KeyPair::generate().expect("ed25519")),
            Alg::P256 => {
                Self::P256(EcdsaKeyPair::generate(&ECDSA_P256_SHA256_ASN1_SIGNING).expect("p256"))
            }
            Alg::Rsa => Self::Rsa(RsaKeyPair::generate(KeySize::Rsa2048).expect("rsa")),
        }
    }

    fn sign(&self, message: &[u8]) -> Vec<u8> {
        let rng = SystemRandom::new();
        match self {
            Self::Ed25519(key) => key.sign(message).as_ref().to_vec(),
            Self::P256(key) => key.sign(&rng, message).expect("sign").as_ref().to_vec(),
            Self::Rsa(key) => {
                let mut sig = vec![0; key.public_modulus_len()];
                key.sign(&RSA_PKCS1_SHA256, &rng, message, &mut sig)
                    .expect("sign");
                sig
            }
        }
    }

    /// The COSE_Key of the public half.
    fn cose(&self) -> Vec<u8> {
        let (kty, alg, params) = match self {
            Self::Ed25519(key) => (
                iana::KeyType::OKP,
                iana::Algorithm::EdDSA,
                vec![
                    (Label::Int(-1), Value::Integer(6.into())),
                    (
                        Label::Int(-2),
                        Value::Bytes(key.public_key().as_ref().to_vec()),
                    ),
                ],
            ),
            Self::P256(key) => {
                let point = key.public_key().as_ref();
                (
                    iana::KeyType::EC2,
                    iana::Algorithm::ES256,
                    vec![
                        (Label::Int(-1), Value::Integer(1.into())),
                        (Label::Int(-2), Value::Bytes(point[1..33].to_vec())),
                        (Label::Int(-3), Value::Bytes(point[33..65].to_vec())),
                    ],
                )
            }
            Self::Rsa(key) => {
                let (n, e) = rsa_components(key.public_key().as_ref());
                (
                    iana::KeyType::RSA,
                    iana::Algorithm::RS256,
                    vec![
                        (Label::Int(-1), Value::Bytes(n)),
                        (Label::Int(-2), Value::Bytes(e)),
                    ],
                )
            }
        };
        CoseKey {
            kty: coset::KeyType::Assigned(kty),
            alg: Some(coset::Algorithm::Assigned(alg)),
            params,
            ..CoseKey::default()
        }
        .to_vec()
        .expect("COSE key")
    }
}

/// `n` and `e` of a PKCS#1 `RSAPublicKey` (DER: a SEQUENCE of two
/// INTEGERs).
fn rsa_components(der: &[u8]) -> (Vec<u8>, Vec<u8>) {
    fn length(bytes: &[u8], at: &mut usize) -> usize {
        let first = bytes[*at];
        *at += 1;
        if first < 0x80 {
            return usize::from(first);
        }
        let mut len = 0;
        for _ in 0..(first & 0x7f) {
            len = (len << 8) | usize::from(bytes[*at]);
            *at += 1;
        }
        len
    }
    fn integer(bytes: &[u8], at: &mut usize) -> Vec<u8> {
        assert_eq!(bytes[*at], 0x02, "INTEGER");
        *at += 1;
        let len = length(bytes, at);
        let value = &bytes[*at..*at + len];
        *at += len;
        let start = value.iter().position(|b| *b != 0).unwrap_or(0);
        value[start..].to_vec()
    }
    let mut at = 0;
    assert_eq!(der[at], 0x30, "SEQUENCE");
    at += 1;
    let _ = length(der, &mut at);
    let n = integer(der, &mut at);
    let e = integer(der, &mut at);
    (n, e)
}

fn b64(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

fn sha256(bytes: &[u8]) -> Vec<u8> {
    digest::digest(&digest::SHA256, bytes).as_ref().to_vec()
}

/// Flags as authenticators set them.
pub const UP: u8 = 1 << 0;
/// User verified.
pub const UV: u8 = 1 << 2;
/// Backup eligible.
pub const BE: u8 = 1 << 3;
/// Backed up.
pub const BS: u8 = 1 << 4;
/// Attested credential data.
pub const AT: u8 = 1 << 6;

/// A passkey the authenticator holds.
pub struct Passkey {
    /// Its credential id.
    pub id: Vec<u8>,
    key: Key,
    /// The user handle it was created for.
    pub user_handle: Vec<u8>,
    /// Its signature counter (0: it keeps none).
    pub count: u32,
    /// Whether it is a synced (multi-device) passkey: BE and BS.
    pub synced: bool,
}

/// What a ceremony puts in the client data and the authenticator data;
/// tests bend these to provoke refusals.
#[derive(Debug, Clone)]
pub struct Ceremony {
    /// `type`.
    pub kind: String,
    /// `challenge`.
    pub challenge: String,
    /// `origin`.
    pub origin: String,
    /// `crossOrigin`, when sent.
    pub cross_origin: Option<bool>,
    /// The relying-party id hashed into the authenticator data.
    pub rp_id: String,
    /// The flags byte (without AT; added for registrations).
    pub flags: u8,
}

/// A software authenticator plus the browser in front of it.
pub struct Authenticator {
    /// The site's origin the browser is on.
    pub origin: String,
    /// The relying-party id.
    pub rp_id: String,
    /// The passkeys it holds.
    pub passkeys: Vec<Passkey>,
}

impl Authenticator {
    /// An authenticator for the site at `origin` with relying-party id
    /// `rp_id`.
    pub fn new(origin: &str, rp_id: &str) -> Self {
        Self {
            origin: origin.to_owned(),
            rp_id: rp_id.to_owned(),
            passkeys: Vec::new(),
        }
    }

    /// The ceremony a well-behaved browser and authenticator would run for
    /// `challenge`: `webauthn.create` or `webauthn.get`, this origin,
    /// UP + UV.
    pub fn ceremony(&self, kind: &str, challenge: &str) -> Ceremony {
        Ceremony {
            kind: kind.to_owned(),
            challenge: challenge.to_owned(),
            origin: self.origin.clone(),
            cross_origin: None,
            rp_id: self.rp_id.clone(),
            flags: UP | UV,
        }
    }

    fn client_data(ceremony: &Ceremony) -> Vec<u8> {
        let mut data = json!({
            "type": ceremony.kind,
            "challenge": ceremony.challenge,
            "origin": ceremony.origin,
        });
        if let Some(cross) = ceremony.cross_origin {
            data["crossOrigin"] = json!(cross);
        }
        data.to_string().into_bytes()
    }

    /// Make a passkey for `options` (a `CreationOptions` as JSON) with
    /// `alg`: the `RegistrationResponseJSON`.
    pub fn create(&mut self, options: &serde_json::Value, alg: Alg) -> serde_json::Value {
        let challenge = options["challenge"].as_str().expect("challenge");
        let ceremony = self.ceremony("webauthn.create", challenge);
        self.create_with(options, alg, false, &ceremony, "none")
    }

    /// [`create`](Self::create), synced or not, for `ceremony`, with
    /// attestation format `fmt`.
    pub fn create_with(
        &mut self,
        options: &serde_json::Value,
        alg: Alg,
        synced: bool,
        ceremony: &Ceremony,
        fmt: &str,
    ) -> serde_json::Value {
        let user_handle = URL_SAFE_NO_PAD
            .decode(options["user"]["id"].as_str().expect("user.id"))
            .expect("user handle");
        let mut id = vec![0_u8; 32];
        SystemRandom::new().fill(&mut id).expect("random");
        let key = Key::generate(alg);
        let mut flags = ceremony.flags | AT;
        if synced {
            flags |= BE | BS;
        }
        let mut auth_data = sha256(ceremony.rp_id.as_bytes());
        auth_data.push(flags);
        auth_data.extend_from_slice(&0_u32.to_be_bytes());
        auth_data.extend_from_slice(&[0; 16]);
        auth_data.extend_from_slice(&u16::try_from(id.len()).expect("id").to_be_bytes());
        auth_data.extend_from_slice(&id);
        auth_data.extend_from_slice(&key.cose());
        let object = Value::Map(vec![
            (Value::Text("fmt".into()), Value::Text(fmt.into())),
            (Value::Text("attStmt".into()), Value::Map(Vec::new())),
            (Value::Text("authData".into()), Value::Bytes(auth_data)),
        ]);
        let mut encoded = Vec::new();
        ciborium::ser::into_writer(&object, &mut encoded).expect("CBOR");
        let client_data = Self::client_data(ceremony);
        self.passkeys.push(Passkey {
            id: id.clone(),
            key,
            user_handle,
            count: 0,
            synced,
        });
        json!({
            "id": b64(&id),
            "rawId": b64(&id),
            "type": "public-key",
            "authenticatorAttachment": "platform",
            "clientExtensionResults": {},
            "response": {
                "clientDataJSON": b64(&client_data),
                "attestationObject": b64(&encoded),
                "transports": ["internal", "hybrid"],
            },
        })
    }

    /// Sign in to `options` (a `RequestOptions` as JSON) with the passkey at
    /// `index`: the `AuthenticationResponseJSON`.
    pub fn get(&mut self, options: &serde_json::Value, index: usize) -> serde_json::Value {
        let challenge = options["challenge"].as_str().expect("challenge");
        let ceremony = self.ceremony("webauthn.get", challenge);
        self.get_with(index, &ceremony)
    }

    /// [`get`](Self::get) for `ceremony`. Counters that are on go up by one.
    pub fn get_with(&mut self, index: usize, ceremony: &Ceremony) -> serde_json::Value {
        let passkey = &mut self.passkeys[index];
        if passkey.count > 0 {
            passkey.count += 1;
        }
        let mut flags = ceremony.flags;
        if passkey.synced {
            flags |= BE | BS;
        }
        let mut auth_data = sha256(ceremony.rp_id.as_bytes());
        auth_data.push(flags);
        auth_data.extend_from_slice(&passkey.count.to_be_bytes());
        let client_data = Self::client_data(ceremony);
        let mut signed = auth_data.clone();
        signed.extend_from_slice(&sha256(&client_data));
        let signature = passkey.key.sign(&signed);
        json!({
            "id": b64(&passkey.id),
            "rawId": b64(&passkey.id),
            "type": "public-key",
            "authenticatorAttachment": "platform",
            "clientExtensionResults": {},
            "response": {
                "clientDataJSON": b64(&client_data),
                "authenticatorData": b64(&auth_data),
                "signature": b64(&signature),
                "userHandle": b64(&passkey.user_handle),
            },
        })
    }
}
