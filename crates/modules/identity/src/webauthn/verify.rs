//! Verifying a registration (WebAuthn L3 §7.1) and an authentication
//! assertion (§7.2), from the browser's `PublicKeyCredential.toJSON()`.

use aws_lc_rs::{constant_time, digest};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

use super::{
    ALGORITHMS, Refusal, RelyingParty,
    auth_data::{self, AuthData},
    cose, refuse,
};

/// The transports worth remembering (`AuthenticatorTransport`); others are
/// dropped.
const TRANSPORTS: [&str; 6] = ["usb", "nfc", "ble", "smart-card", "hybrid", "internal"];

/// `AuthenticatorAttestationResponseJSON`: what the authenticator made.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct AttestationResponse {
    /// The client data, base64url.
    #[serde(rename = "clientDataJSON")]
    pub client_data_json: String,
    /// The attestation object (CBOR), base64url.
    #[serde(rename = "attestationObject")]
    pub attestation_object: String,
    /// How the browser reached the authenticator.
    #[serde(default)]
    pub transports: Vec<String>,
}

/// `RegistrationResponseJSON`: a new passkey, as the browser returns it.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct RegistrationCredential {
    /// The credential id, base64url.
    pub id: String,
    /// The same, as the raw id.
    #[serde(rename = "rawId")]
    pub raw_id: String,
    /// `public-key`.
    #[serde(rename = "type")]
    pub kind: String,
    /// The authenticator's response.
    pub response: AttestationResponse,
}

/// `AuthenticatorAssertionResponseJSON`: the authenticator's signature.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct AssertionResponse {
    /// The client data, base64url.
    #[serde(rename = "clientDataJSON")]
    pub client_data_json: String,
    /// The authenticator data, base64url.
    #[serde(rename = "authenticatorData")]
    pub authenticator_data: String,
    /// The signature, base64url.
    pub signature: String,
    /// The user handle the passkey was created with, base64url.
    #[serde(rename = "userHandle")]
    pub user_handle: Option<String>,
}

/// `AuthenticationResponseJSON`: a sign-in with a passkey, as the browser
/// returns it.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct AuthenticationCredential {
    /// The credential id, base64url.
    pub id: String,
    /// The same, as the raw id.
    #[serde(rename = "rawId")]
    pub raw_id: String,
    /// `public-key`.
    #[serde(rename = "type")]
    pub kind: String,
    /// The authenticator's response.
    pub response: AssertionResponse,
}

/// `CollectedClientData` (§5.8.1), the fields checked.
#[derive(Debug, Deserialize)]
struct ClientData {
    #[serde(rename = "type")]
    kind: String,
    challenge: String,
    origin: String,
    #[serde(rename = "crossOrigin")]
    cross_origin: Option<bool>,
    #[serde(rename = "topOrigin")]
    top_origin: Option<String>,
}

fn decode(value: &str, label: &'static str) -> Result<Vec<u8>, Refusal> {
    URL_SAFE_NO_PAD.decode(value).map_err(|_| Refusal(label))
}

fn sha256(bytes: &[u8]) -> Vec<u8> {
    digest::digest(&digest::SHA256, bytes).as_ref().to_vec()
}

fn same(a: &[u8], b: &[u8]) -> bool {
    constant_time::verify_slices_are_equal(a, b).is_ok()
}

fn client_data(encoded: &str) -> Result<(ClientData, Vec<u8>), Refusal> {
    let raw = decode(encoded, "client_data_not_base64url")?;
    let parsed = serde_json::from_slice(&raw).map_err(|_| Refusal("client_data_malformed"))?;
    Ok((parsed, raw))
}

/// The challenge a credential's client data carries (to find its stored
/// ceremony), before anything else is checked.
pub fn challenge_of(client_data_json: &str) -> Result<String, Refusal> {
    client_data(client_data_json).map(|(data, _)| data.challenge)
}

impl RegistrationCredential {
    /// The challenge its client data carries.
    pub fn challenge(&self) -> Result<String, Refusal> {
        challenge_of(&self.response.client_data_json)
    }
}

impl AuthenticationCredential {
    /// The challenge its client data carries.
    pub fn challenge(&self) -> Result<String, Refusal> {
        challenge_of(&self.response.client_data_json)
    }

    /// The credential id it names.
    pub fn credential_id(&self) -> Result<Vec<u8>, Refusal> {
        credential_id(&self.kind, &self.id, &self.raw_id)
    }

    /// The account its user handle names, when it is an account id.
    pub fn user_handle(&self) -> Result<Uuid, Refusal> {
        let handle = self
            .response
            .user_handle
            .as_deref()
            .ok_or(Refusal("user_handle_missing"))?;
        let bytes = decode(handle, "user_handle_not_base64url")?;
        Uuid::from_slice(&bytes).map_err(|_| Refusal("user_handle_not_an_account"))
    }
}

fn credential_id(kind: &str, id: &str, raw_id: &str) -> Result<Vec<u8>, Refusal> {
    if kind != "public-key" {
        return refuse("credential_type");
    }
    if id != raw_id {
        return refuse("credential_id_mismatch");
    }
    let raw = decode(raw_id, "credential_id_not_base64url")?;
    if raw.is_empty() || raw.len() > auth_data::MAX_CREDENTIAL_ID {
        return refuse("credential_id_length");
    }
    Ok(raw)
}

/// The client-data checks shared by both ceremonies (§7.1 steps 7–10,
/// §7.2 steps 11–14).
fn check_client_data(
    rp: &RelyingParty,
    data: &ClientData,
    ceremony: &str,
    challenge: &str,
) -> Result<(), Refusal> {
    if data.kind != ceremony {
        return refuse("client_data_type");
    }
    if !same(data.challenge.as_bytes(), challenge.as_bytes()) {
        return refuse("challenge_mismatch");
    }
    if data.origin != rp.origin {
        return refuse("origin_mismatch");
    }
    // Campbooks is never embedded in another site's frame.
    if data.cross_origin == Some(true) || data.top_origin.is_some() {
        return refuse("cross_origin");
    }
    Ok(())
}

/// The authenticator-data checks shared by both ceremonies (§7.1 steps
/// 13–16, §7.2 steps 15–17): our relying party, a present and verified
/// user, and BS only with BE.
fn check_auth_data(rp: &RelyingParty, data: &AuthData) -> Result<(), Refusal> {
    if !same(&data.rp_id_hash, rp.id_hash()) {
        return refuse("rp_id_mismatch");
    }
    if !data.flags.user_present() {
        return refuse("user_not_present");
    }
    if !data.flags.user_verified() {
        return refuse("user_not_verified");
    }
    if data.flags.backed_up() && !data.flags.backup_eligible() {
        return refuse("backup_state_without_eligibility");
    }
    Ok(())
}

/// A verified new passkey, ready to store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCredential {
    /// Its credential id.
    pub credential_id: Vec<u8>,
    /// Its COSE_Key, as registered.
    pub public_key: Vec<u8>,
    /// Its COSE algorithm.
    pub alg: i64,
    /// The signature counter at registration.
    pub sign_count: u32,
    /// The authenticator model's AAGUID (zeros when not said).
    pub aaguid: Uuid,
    /// Known transports.
    pub transports: Vec<String>,
    /// BE: it may be synced.
    pub backup_eligible: bool,
    /// BS: it is synced.
    pub backup_state: bool,
    /// The attestation statement format the authenticator used (not
    /// trusted; recorded only).
    pub attestation_format: String,
}

/// A text-keyed entry of a CBOR map.
fn entry<'a>(
    map: &'a [(ciborium::Value, ciborium::Value)],
    key: &str,
) -> Option<&'a ciborium::Value> {
    map.iter()
        .find_map(|(k, v)| (k.as_text() == Some(key)).then_some(v))
}

/// An attestation object's parts (§6.5).
struct AttestationObject {
    /// `fmt`.
    fmt: String,
    /// `attStmt`, a CBOR map.
    statement: Vec<(ciborium::Value, ciborium::Value)>,
    /// `authData`.
    auth_data: Vec<u8>,
}

/// The attestation object's `fmt`, `attStmt` and `authData` (§6.5).
fn attestation_object(bytes: &[u8]) -> Result<AttestationObject, Refusal> {
    let mut rest = bytes;
    let value: ciborium::Value =
        ciborium::de::from_reader(&mut rest).map_err(|_| Refusal("attestation_not_cbor"))?;
    if !rest.is_empty() {
        return refuse("attestation_trailing_bytes");
    }
    let ciborium::Value::Map(map) = value else {
        return refuse("attestation_not_a_map");
    };
    let fmt = entry(&map, "fmt")
        .and_then(ciborium::Value::as_text)
        .ok_or(Refusal("attestation_without_fmt"))?
        .to_owned();
    let statement = entry(&map, "attStmt")
        .and_then(ciborium::Value::as_map)
        .ok_or(Refusal("attestation_without_statement"))?
        .clone();
    let auth_data = entry(&map, "authData")
        .and_then(ciborium::Value::as_bytes)
        .ok_or(Refusal("attestation_without_auth_data"))?
        .clone();
    Ok(AttestationObject {
        fmt,
        statement,
        auth_data,
    })
}

/// The attestation statement formats WebAuthn registers (§8, IANA). Their
/// statements are not verified: the options ask for attestation `none`,
/// and nothing here depends on the authenticator's provenance, so such a
/// credential is treated as unattested (§7.1 step 24's note).
const KNOWN_FORMATS: [&str; 7] = [
    "none",
    "packed",
    "tpm",
    "android-key",
    "android-safetynet",
    "fido-u2f",
    "apple",
];

impl RelyingParty {
    /// Verify a new passkey created for `challenge` (§7.1).
    pub fn verify_registration(
        &self,
        challenge: &str,
        credential: &RegistrationCredential,
    ) -> Result<NewCredential, Refusal> {
        let raw_id = credential_id(&credential.kind, &credential.id, &credential.raw_id)?;
        let (data, _) = client_data(&credential.response.client_data_json)?;
        check_client_data(self, &data, "webauthn.create", challenge)?;
        let object = decode(
            &credential.response.attestation_object,
            "attestation_not_base64url",
        )?;
        let AttestationObject {
            fmt,
            statement,
            auth_data,
        } = attestation_object(&object)?;
        if !KNOWN_FORMATS.contains(&fmt.as_str()) {
            return refuse("attestation_format_unknown");
        }
        if fmt == "none" && !statement.is_empty() {
            return refuse("attestation_none_with_statement");
        }
        let parsed = auth_data::parse(&auth_data)?;
        check_auth_data(self, &parsed)?;
        let attested = parsed.attested.ok_or(Refusal("attested_data_missing"))?;
        if !same(&attested.credential_id, &raw_id) {
            return refuse("credential_id_mismatch");
        }
        let key = cose::parse(&attested.public_key)?;
        if !ALGORITHMS.contains(&key.alg()) {
            return refuse("algorithm_not_allowed");
        }
        let mut transports: Vec<String> = credential
            .response
            .transports
            .iter()
            .filter(|t| TRANSPORTS.contains(&t.as_str()))
            .cloned()
            .collect();
        transports.sort();
        transports.dedup();
        Ok(NewCredential {
            credential_id: attested.credential_id,
            public_key: attested.public_key,
            alg: key.alg(),
            sign_count: parsed.sign_count,
            aaguid: Uuid::from_bytes(attested.aaguid),
            transports,
            backup_eligible: parsed.flags.backup_eligible(),
            backup_state: parsed.flags.backed_up(),
            attestation_format: fmt,
        })
    }

    /// Verify a sign-in assertion for `challenge` by the stored credential
    /// `stored` (§7.2). The caller has found `stored` by the assertion's
    /// credential id and checked that its user handle names the passkey's
    /// account; it then records the [`Verified`] counter and backup state.
    pub fn verify_assertion(
        &self,
        challenge: &str,
        credential: &AuthenticationCredential,
        stored: &Stored<'_>,
    ) -> Result<Verified, Refusal> {
        let raw_id = credential.credential_id()?;
        if !same(&raw_id, stored.credential_id) {
            return refuse("credential_id_mismatch");
        }
        let (data, client_data_json) = client_data(&credential.response.client_data_json)?;
        check_client_data(self, &data, "webauthn.get", challenge)?;
        let auth_data = decode(
            &credential.response.authenticator_data,
            "auth_data_not_base64url",
        )?;
        let parsed = auth_data::parse(&auth_data)?;
        check_auth_data(self, &parsed)?;
        if parsed.attested.is_some() {
            return refuse("assertion_with_attested_data");
        }
        // BE is fixed when the credential is created (§6.1.3).
        if parsed.flags.backup_eligible() != stored.backup_eligible {
            return refuse("backup_eligibility_changed");
        }
        let key = cose::parse(stored.public_key)?;
        if key.alg() != stored.alg {
            return refuse("algorithm_changed");
        }
        let signature = decode(&credential.response.signature, "signature_not_base64url")?;
        let mut signed = auth_data;
        signed.extend_from_slice(&sha256(&client_data_json));
        if !key.verifies(&signed, &signature) {
            return refuse("signature_invalid");
        }
        Ok(Verified {
            sign_count: parsed.sign_count,
            backup_state: parsed.flags.backed_up(),
        })
    }
}

/// A stored passkey, as verification needs it.
#[derive(Debug, Clone, Copy)]
pub struct Stored<'a> {
    /// Its credential id.
    pub credential_id: &'a [u8],
    /// Its COSE_Key.
    pub public_key: &'a [u8],
    /// Its COSE algorithm.
    pub alg: i64,
    /// Whether it was backup eligible at registration.
    pub backup_eligible: bool,
}

/// What a verified assertion reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Verified {
    /// The authenticator's signature counter (0 when it keeps none).
    pub sign_count: u32,
    /// BS: whether it is synced now.
    pub backup_state: bool,
}
