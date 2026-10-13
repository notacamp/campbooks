//! The relying party against a software authenticator: every algorithm
//! registers and signs in, and every check of §7.1 and §7.2 refuses what
//! it should.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use uuid::Uuid;

use super::{
    AuthenticationCredential, RegistrationCredential, RelyingParty, Stored,
    authenticator::{Alg, Authenticator, BE, BS, UP, UV},
    refuse,
};

const ORIGIN: &str = "https://app.example.com";
const CHALLENGE: &str = "c2luZ2xlLXVzZSBjaGFsbGVuZ2UgZm9yIGEgdGVzdCBjZXJlbW9ueQ";

fn rp() -> RelyingParty {
    RelyingParty::for_origin(ORIGIN).unwrap()
}

fn creation(user: Uuid) -> serde_json::Value {
    serde_json::to_value(rp().creation_options(CHALLENGE, user, "ana@example.com", "Ana", vec![]))
        .unwrap()
}

fn request() -> serde_json::Value {
    serde_json::to_value(rp().request_options(CHALLENGE)).unwrap()
}

fn registration(json: serde_json::Value) -> RegistrationCredential {
    serde_json::from_value(json).unwrap()
}

fn assertion(json: serde_json::Value) -> AuthenticationCredential {
    serde_json::from_value(json).unwrap()
}

/// Register a passkey with `alg`, then sign in with it.
fn round_trip(alg: Alg, synced: bool) {
    let user = Uuid::now_v7();
    let mut device = Authenticator::new(ORIGIN, "app.example.com");
    let ceremony = device.ceremony("webauthn.create", CHALLENGE);
    let made = device.create_with(&creation(user), alg, synced, &ceremony, "none");
    let new = rp()
        .verify_registration(CHALLENGE, &registration(made))
        .unwrap_or_else(|e| panic!("{alg:?}: {e}"));
    assert_eq!(new.credential_id, device.passkeys[0].id);
    assert_eq!(new.backup_eligible, synced);
    assert_eq!(new.backup_state, synced);
    assert_eq!(new.aaguid, Uuid::nil());
    assert_eq!(new.transports, vec!["hybrid", "internal"]);
    assert_eq!(new.attestation_format, "none");

    let signed = assertion(device.get(&request(), 0));
    assert_eq!(signed.user_handle().unwrap(), user);
    let stored = Stored {
        credential_id: &new.credential_id,
        public_key: &new.public_key,
        alg: new.alg,
        backup_eligible: new.backup_eligible,
    };
    let verified = rp()
        .verify_assertion(CHALLENGE, &signed, &stored)
        .unwrap_or_else(|e| panic!("{alg:?}: {e}"));
    assert_eq!(verified.backup_state, synced);
}

#[test]
fn eddsa_es256_and_rs256_passkeys_register_and_sign_in() {
    round_trip(Alg::Ed25519, false);
    round_trip(Alg::P256, true);
    round_trip(Alg::Rsa, false);
}

#[test]
fn the_relying_party_is_the_public_hosts_and_needs_a_secure_context() {
    let rp = RelyingParty::for_origin("https://App.Example.com:8443").unwrap();
    assert_eq!(rp.id, "app.example.com");
    assert_eq!(rp.origin, "https://app.example.com:8443");
    assert!(RelyingParty::for_origin("http://localhost:8080").is_some());
    assert!(RelyingParty::for_origin("http://app.example.com").is_none());
    assert!(RelyingParty::for_origin("https://192.0.2.1").is_none());
    assert!(RelyingParty::for_origin("https://[2001:db8::1]").is_none());
}

#[test]
fn options_ask_for_a_discoverable_verified_unattested_passkey() {
    let user = Uuid::now_v7();
    let options = creation(user);
    assert_eq!(options["rp"]["id"], "app.example.com");
    assert_eq!(
        options["user"]["id"],
        URL_SAFE_NO_PAD.encode(user.as_bytes())
    );
    assert_eq!(options["attestation"], "none");
    assert_eq!(options["authenticatorSelection"]["residentKey"], "required");
    assert_eq!(
        options["authenticatorSelection"]["userVerification"],
        "required"
    );
    let algs: Vec<i64> = options["pubKeyCredParams"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["alg"].as_i64().unwrap())
        .collect();
    assert_eq!(algs, vec![-8, -7, -257]);
    assert_eq!(options["timeout"], 300_000);
    let request = request();
    assert_eq!(request["rpId"], "app.example.com");
    assert_eq!(request["allowCredentials"], serde_json::json!([]));
    assert_eq!(request["userVerification"], "required");
}

/// A registration whose ceremony `bend` changed.
fn register_bent(
    bend: impl FnOnce(&mut super::authenticator::Ceremony),
) -> Result<(), super::Refusal> {
    let mut device = Authenticator::new(ORIGIN, "app.example.com");
    let mut ceremony = device.ceremony("webauthn.create", CHALLENGE);
    bend(&mut ceremony);
    let made = device.create_with(
        &creation(Uuid::now_v7()),
        Alg::P256,
        false,
        &ceremony,
        "none",
    );
    rp().verify_registration(CHALLENGE, &registration(made))
        .map(|_| ())
}

#[test]
fn registrations_are_refused_check_by_check() {
    assert_eq!(
        register_bent(|c| c.kind = "webauthn.get".into()),
        refuse("client_data_type")
    );
    assert_eq!(
        register_bent(|c| c.challenge = "another".into()),
        refuse("challenge_mismatch")
    );
    assert_eq!(
        register_bent(|c| c.origin = "https://evil.example.com".into()),
        refuse("origin_mismatch")
    );
    assert_eq!(
        register_bent(|c| c.origin = "http://app.example.com".into()),
        refuse("origin_mismatch")
    );
    assert_eq!(
        register_bent(|c| c.cross_origin = Some(true)),
        refuse("cross_origin")
    );
    assert_eq!(
        register_bent(|c| c.rp_id = "example.com".into()),
        refuse("rp_id_mismatch")
    );
    assert_eq!(register_bent(|c| c.flags = UV), refuse("user_not_present"));
    assert_eq!(register_bent(|c| c.flags = UP), refuse("user_not_verified"));
    assert_eq!(
        register_bent(|c| c.flags = UP | UV | BS),
        refuse("backup_state_without_eligibility")
    );
    // A well-behaved one passes, so each refusal above is its own check.
    assert_eq!(register_bent(|_| {}), Ok(()));
}

#[test]
fn attestation_formats_and_ids_are_checked() {
    let options = creation(Uuid::now_v7());
    let mut device = Authenticator::new(ORIGIN, "app.example.com");
    let ceremony = device.ceremony("webauthn.create", CHALLENGE);

    let packed = device.create_with(&options, Alg::Ed25519, false, &ceremony, "packed");
    let new = rp()
        .verify_registration(CHALLENGE, &registration(packed))
        .unwrap();
    assert_eq!(new.attestation_format, "packed", "accepted, not trusted");

    let odd = device.create_with(&options, Alg::Ed25519, false, &ceremony, "made-up");
    assert_eq!(
        rp().verify_registration(CHALLENGE, &registration(odd))
            .map(|_| ()),
        refuse("attestation_format_unknown")
    );

    let mut swapped = device.create_with(&options, Alg::Ed25519, false, &ceremony, "none");
    swapped["rawId"] = serde_json::json!(URL_SAFE_NO_PAD.encode([1_u8; 32]));
    swapped["id"] = swapped["rawId"].clone();
    assert_eq!(
        rp().verify_registration(CHALLENGE, &registration(swapped))
            .map(|_| ()),
        refuse("credential_id_mismatch")
    );

    let mut not_public_key = device.create_with(&options, Alg::Ed25519, false, &ceremony, "none");
    not_public_key["type"] = serde_json::json!("password");
    assert_eq!(
        rp().verify_registration(CHALLENGE, &registration(not_public_key))
            .map(|_| ()),
        refuse("credential_type")
    );

    let mut garbage = device.create_with(&options, Alg::Ed25519, false, &ceremony, "none");
    garbage["response"]["attestationObject"] = serde_json::json!("AAAA");
    assert!(
        rp().verify_registration(CHALLENGE, &registration(garbage))
            .is_err()
    );
}

/// Register a P-256 passkey (synced when `synced`); the device and what was
/// stored.
fn registered(synced: bool) -> (Authenticator, super::NewCredential) {
    let mut device = Authenticator::new(ORIGIN, "app.example.com");
    let ceremony = device.ceremony("webauthn.create", CHALLENGE);
    let made = device.create_with(
        &creation(Uuid::now_v7()),
        Alg::P256,
        synced,
        &ceremony,
        "none",
    );
    let new = rp()
        .verify_registration(CHALLENGE, &registration(made))
        .unwrap();
    (device, new)
}

fn stored(new: &super::NewCredential) -> Stored<'_> {
    Stored {
        credential_id: &new.credential_id,
        public_key: &new.public_key,
        alg: new.alg,
        backup_eligible: new.backup_eligible,
    }
}

#[test]
fn assertions_are_refused_check_by_check() {
    let (mut device, new) = registered(false);
    let mut bent = |bend: &dyn Fn(&mut super::authenticator::Ceremony)| {
        let mut ceremony = device.ceremony("webauthn.get", CHALLENGE);
        bend(&mut ceremony);
        let signed = assertion(device.get_with(0, &ceremony));
        rp().verify_assertion(CHALLENGE, &signed, &stored(&new))
            .map(|_| ())
    };
    assert_eq!(bent(&|_| {}), Ok(()));
    assert_eq!(
        bent(&|c| c.kind = "webauthn.create".into()),
        refuse("client_data_type")
    );
    assert_eq!(
        bent(&|c| c.challenge = "replayed".into()),
        refuse("challenge_mismatch")
    );
    assert_eq!(
        bent(&|c| c.origin = "https://app.example.com.evil.example".into()),
        refuse("origin_mismatch")
    );
    assert_eq!(
        bent(&|c| c.rp_id = "evil.example".into()),
        refuse("rp_id_mismatch")
    );
    assert_eq!(bent(&|c| c.flags = UP), refuse("user_not_verified"));
    assert_eq!(bent(&|c| c.flags = UV), refuse("user_not_present"));
    // BE is fixed at registration: a non-synced passkey that now claims to
    // be eligible is not the one registered.
    assert_eq!(
        bent(&|c| c.flags = UP | UV | BE),
        refuse("backup_eligibility_changed")
    );
}

#[test]
fn a_forged_or_tampered_signature_is_refused() {
    let (mut device, new) = registered(true);
    let mut signed = device.get(&request(), 0);
    let mut sig = URL_SAFE_NO_PAD
        .decode(signed["response"]["signature"].as_str().unwrap())
        .unwrap();
    let last = sig.len() - 1;
    sig[last] ^= 1;
    signed["response"]["signature"] = serde_json::json!(URL_SAFE_NO_PAD.encode(&sig));
    assert_eq!(
        rp().verify_assertion(CHALLENGE, &assertion(signed), &stored(&new))
            .map(|_| ()),
        refuse("signature_invalid")
    );

    // Signed by another passkey, presented as this one.
    let (mut other, _) = registered(true);
    let mut impostor = other.get(&request(), 0);
    impostor["id"] = serde_json::json!(URL_SAFE_NO_PAD.encode(&new.credential_id));
    impostor["rawId"] = impostor["id"].clone();
    assert_eq!(
        rp().verify_assertion(CHALLENGE, &assertion(impostor), &stored(&new))
            .map(|_| ()),
        refuse("signature_invalid")
    );

    // An assertion for another credential than the one looked up.
    let mut elsewhere = device.get(&request(), 0);
    elsewhere["id"] = serde_json::json!(URL_SAFE_NO_PAD.encode([3_u8; 32]));
    elsewhere["rawId"] = elsewhere["id"].clone();
    assert_eq!(
        rp().verify_assertion(CHALLENGE, &assertion(elsewhere), &stored(&new))
            .map(|_| ()),
        refuse("credential_id_mismatch")
    );

    // A synced passkey that stopped claiming eligibility.
    let mut ceremony = device.ceremony("webauthn.get", CHALLENGE);
    ceremony.flags = UP | UV;
    device.passkeys[0].synced = false;
    let downgraded = device.get_with(0, &ceremony);
    assert_eq!(
        rp().verify_assertion(CHALLENGE, &assertion(downgraded), &stored(&new))
            .map(|_| ()),
        refuse("backup_eligibility_changed")
    );
}

#[test]
fn counters_and_user_handles_are_reported() {
    let (mut device, new) = registered(false);
    device.passkeys[0].count = 41;
    let signed = assertion(device.get(&request(), 0));
    let verified = rp()
        .verify_assertion(CHALLENGE, &signed, &stored(&new))
        .unwrap();
    assert_eq!(verified.sign_count, 42);
    assert!(!verified.backup_state);

    let mut no_handle = device.get(&request(), 0);
    no_handle["response"]["userHandle"] = serde_json::Value::Null;
    assert_eq!(
        assertion(no_handle).user_handle(),
        refuse("user_handle_missing")
    );
    let mut short_handle = device.get(&request(), 0);
    short_handle["response"]["userHandle"] = serde_json::json!("AAEC");
    assert_eq!(
        assertion(short_handle).user_handle(),
        refuse("user_handle_not_an_account")
    );
}
