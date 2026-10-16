//! End to end through the issuer, slice S3: a passkey-only sign-up, signing
//! in with a passkey, adding and removing one while signed in; with every
//! refusal, one by one. The browser and its authenticator are the software
//! authenticator of `webauthn::authenticator`.

use axum::http::{Method, StatusCode};
use campbooks_kernel::config::SignupPolicy;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{
    testkit::{Kit, ORIGIN, Response, query},
    webauthn::authenticator::{Alg, Authenticator, UP},
};

/// What the sign-in screens send with every action.
const SCREEN: [(&str, &str); 3] = [
    ("x-campbooks-csrf", "1"),
    ("sec-fetch-site", "same-origin"),
    ("content-type", "application/json"),
];

pub(crate) fn device() -> Authenticator {
    Authenticator::new(ORIGIN, "localhost")
}

pub(crate) async fn count(kit: &Kit, sql: &'static str) -> i64 {
    sqlx::query_scalar(sql)
        .fetch_one(&kit.state.db)
        .await
        .unwrap()
}

/// A sign-in screen action: POST `body` as JSON with the CSRF header.
pub(crate) async fn screen(kit: &Kit, path: &str, body: &Value) -> Response {
    kit.request(Method::POST, path, &SCREEN, Some(body.to_string()))
        .await
}

/// The web client's authorization request: the sign-in screen's `request`
/// and the PKCE verifier.
pub(crate) async fn waiting_request(kit: &Kit) -> (String, String) {
    let (url, verifier) = Kit::web_authorize();
    let to_sign_in = kit.get(&url).await;
    (to_sign_in.location_query()["request"].clone(), verifier)
}

/// Create an account with a passkey alone, from the sign-in screen: the
/// `redirect` the screen is told to follow.
pub(crate) async fn sign_up_with_passkey(
    kit: &Kit,
    device: &mut Authenticator,
    request: &str,
) -> String {
    let options = screen(
        kit,
        "/auth/passkey/sign-up/options",
        &json!({ "display_name": "Ana Lima", "request": request }),
    )
    .await;
    assert_eq!(options.status, StatusCode::OK, "{options:?}");
    let made = device.create(&options.json, Alg::Ed25519);
    let created = screen(kit, "/auth/passkey/sign-up", &made).await;
    assert_eq!(created.status, StatusCode::CREATED, "{created:?}");
    created.json["redirect"].as_str().unwrap().to_owned()
}

/// Sign in with the device's passkey `index` from the sign-in screen.
pub(crate) async fn sign_in_with_passkey(
    kit: &Kit,
    device: &mut Authenticator,
    index: usize,
    request: &str,
) -> Response {
    let options = screen(
        kit,
        "/auth/passkey/sign-in/options",
        &json!({ "request": request }),
    )
    .await;
    assert_eq!(options.status, StatusCode::OK, "{options:?}");
    let signed = device.get(&options.json, index);
    screen(kit, "/auth/passkey/sign-in", &signed).await
}

/// Redeem the code in `redirect`: the token response.
pub(crate) async fn redeem(kit: &Kit, redirect: &str, verifier: &str) -> Value {
    let tokens = kit.redeem(&query(redirect)["code"], verifier).await;
    assert_eq!(tokens.status, StatusCode::OK, "{tokens:?}");
    tokens.json
}

/// Add a passkey to the account `access` belongs to: the API's answer.
pub(crate) async fn add_passkey(
    kit: &Kit,
    access: &str,
    device: &mut Authenticator,
    alg: Alg,
) -> Response {
    let options = kit
        .api(
            Method::POST,
            "/api/v1/identity/passkeys/registrations",
            access,
        )
        .await;
    assert_eq!(options.status, StatusCode::CREATED, "{options:?}");
    let made = device.create(&options.json, alg);
    kit.api_json(
        Method::POST,
        "/api/v1/identity/passkeys",
        access,
        &json!({ "credential": made, "name": "  Laptop\n" }),
    )
    .await
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_new_person_signs_up_with_a_passkey_alone_and_signs_in_with_it(db: PgPool) {
    let kit = Kit::new(db).await;
    let (request, verifier) = waiting_request(&kit).await;
    let shown = kit.get(&format!("/auth/requests/{request}")).await;
    assert_eq!(
        shown.json["providers"],
        json!(["google", "microsoft", "zoho", "passkey"])
    );
    assert_eq!(shown.json["passkey_sign_up"], true);

    let mut device = device();
    let redirect = sign_up_with_passkey(&kit, &mut device, &request).await;
    assert!(
        redirect.starts_with(&format!("{ORIGIN}/bff/callback?code=")),
        "{redirect}"
    );
    let tokens = redeem(&kit, &redirect, &verifier).await;
    let (_, id) = Kit::decode(tokens["id_token"].as_str().unwrap());
    assert_eq!(id["amr"], json!(["hwk", "user"]));
    assert_eq!(id["name"], "Ana Lima");
    assert!(id.get("email").is_none(), "a passkey proves no address");
    let (_, access) = Kit::decode(tokens["access_token"].as_str().unwrap());
    assert_eq!(access["email_verified"], false);
    // The sign-up's passkey is listed as the one the account was made with
    // (the app never calls it a new way in), and a later one is not.
    let token = tokens["access_token"].as_str().unwrap().to_owned();
    let listed = kit
        .api(Method::GET, "/api/v1/identity/passkeys", &token)
        .await;
    assert_eq!(
        listed.json["items"][0]["signed_up_with"], true,
        "{listed:?}"
    );
    // Its ways in are the passkeys alone: there is no provider way in to
    // flag (a provider sign-up's says `linked_via: sign_up`), so the
    // passkeys list is where the app reads it.
    let ways = kit.ways_in(&token).await;
    assert_eq!(ways["items"], json!([]), "{ways}");
    let mut laptop = Authenticator::new(ORIGIN, "localhost");
    let added = add_passkey(&kit, &token, &mut laptop, Alg::Ed25519).await;
    assert_eq!(added.status, StatusCode::CREATED, "{added:?}");
    assert_eq!(added.json["signed_up_with"], false);
    assert_eq!(
        count(
            &kit,
            "SELECT count(*) FROM kernel.events WHERE name = 'identity.account_created'"
        )
        .await,
        1
    );
    assert_eq!(
        count(
            &kit,
            "SELECT count(*) FROM kernel.events WHERE name = 'identity.way_in_added'"
        )
        .await,
        2,
        "the sign-up's passkey and the one added after"
    );

    // Another browser, the same passkey: the same account.
    kit.new_browser();
    let (request, verifier) = waiting_request(&kit).await;
    let signed_in = sign_in_with_passkey(&kit, &mut device, 0, &request).await;
    assert_eq!(signed_in.status, StatusCode::OK, "{signed_in:?}");
    let again = redeem(
        &kit,
        signed_in.json["redirect"].as_str().unwrap(),
        &verifier,
    )
    .await;
    let (_, again) = Kit::decode(again["id_token"].as_str().unwrap());
    assert_eq!(again["sub"], id["sub"]);
    assert!(
        signed_in
            .headers
            .get_all("set-cookie")
            .iter()
            .any(|c| c.to_str().unwrap().starts_with("campbooks_webauthn=;")),
        "the ceremony cookie is cleared"
    );
    let last_used: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
        "SELECT last_used_at FROM identity.passkeys WHERE added_via = 'sign_up'",
    )
    .fetch_one(&kit.state.db)
    .await
    .unwrap();
    assert!(last_used.is_some());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_signed_in_person_adds_a_passkey_and_signs_in_with_it(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let mut device = device();
    let added = add_passkey(&kit, &access, &mut device, Alg::P256).await;
    assert_eq!(added.status, StatusCode::CREATED, "{added:?}");
    assert_eq!(added.json["name"], "Laptop");
    assert_eq!(added.json["synced"], false);
    let listed = kit
        .api(Method::GET, "/api/v1/identity/passkeys", &access)
        .await;
    assert_eq!(listed.json["items"].as_array().unwrap().len(), 1);

    // The second registration excludes the first.
    let options = kit
        .api(
            Method::POST,
            "/api/v1/identity/passkeys/registrations",
            &access,
        )
        .await;
    assert_eq!(
        options.json["excludeCredentials"].as_array().unwrap().len(),
        1
    );
    assert_eq!(options.json["user"]["name"], "test-user@example.com");

    kit.new_browser();
    let (request, verifier) = waiting_request(&kit).await;
    let signed_in = sign_in_with_passkey(&kit, &mut device, 0, &request).await;
    assert_eq!(signed_in.status, StatusCode::OK, "{signed_in:?}");
    let tokens = redeem(
        &kit,
        signed_in.json["redirect"].as_str().unwrap(),
        &verifier,
    )
    .await;
    let (_, mine) = Kit::decode(&access);
    let (_, now) = Kit::decode(tokens["access_token"].as_str().unwrap());
    assert_eq!(now["sub"], mine["sub"]);
    assert_eq!(now["email"], "test-user@example.com");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn every_algorithm_and_a_synced_passkey_sign_in(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let mut device = device();
    for alg in [Alg::Ed25519, Alg::P256, Alg::Rsa] {
        let added = add_passkey(&kit, &access, &mut device, alg).await;
        assert_eq!(added.status, StatusCode::CREATED, "{alg:?}: {added:?}");
    }
    for index in 0..3 {
        kit.new_browser();
        let (request, verifier) = waiting_request(&kit).await;
        let signed_in = sign_in_with_passkey(&kit, &mut device, index, &request).await;
        assert_eq!(signed_in.status, StatusCode::OK, "{index}: {signed_in:?}");
        redeem(
            &kit,
            signed_in.json["redirect"].as_str().unwrap(),
            &verifier,
        )
        .await;
    }

    // A synced passkey reports `swk`.
    let options = kit
        .api(
            Method::POST,
            "/api/v1/identity/passkeys/registrations",
            &access,
        )
        .await;
    let ceremony = device.ceremony(
        "webauthn.create",
        options.json["challenge"].as_str().unwrap(),
    );
    let made = device.create_with(&options.json, Alg::P256, true, &ceremony, "none");
    let added = kit
        .api_json(
            Method::POST,
            "/api/v1/identity/passkeys",
            &access,
            &json!({ "credential": made }),
        )
        .await;
    assert_eq!(added.json["synced"], true);
    assert_eq!(added.json["name"], "Passkey");
    kit.new_browser();
    let (request, verifier) = waiting_request(&kit).await;
    let signed_in = sign_in_with_passkey(&kit, &mut device, 3, &request).await;
    let tokens = redeem(
        &kit,
        signed_in.json["redirect"].as_str().unwrap(),
        &verifier,
    )
    .await;
    let (_, id) = Kit::decode(tokens["id_token"].as_str().unwrap());
    assert_eq!(id["amr"], json!(["swk", "user"]));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn adding_or_removing_a_passkey_needs_a_recent_sign_in(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let mut device = device();
    add_passkey(&kit, &access, &mut device, Alg::Ed25519).await;
    let passkey = kit
        .api(Method::GET, "/api/v1/identity/passkeys", &access)
        .await
        .json["items"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let stale = kit.stale(&access).await;
    for (method, path, code) in [
        (
            Method::POST,
            "/api/v1/identity/passkeys/registrations".to_owned(),
            "fresh_sign_in_required",
        ),
        (
            Method::DELETE,
            format!("/api/v1/identity/passkeys/{passkey}"),
            "step_up_required",
        ),
    ] {
        let refused = kit.api(method, &path, &stale).await;
        assert_eq!(refused.status, StatusCode::FORBIDDEN, "{path}: {refused:?}");
        assert_eq!(refused.json["code"], code, "{path}");
    }
    // Adding a passkey asks for a sign-in from the last five minutes: one
    // seven minutes old still removes a passkey, but adds none.
    let recent = kit
        .signed_in_ago(&access, chrono::TimeDelta::minutes(7))
        .await;
    let refused = kit
        .api(
            Method::POST,
            "/api/v1/identity/passkeys/registrations",
            &recent,
        )
        .await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN, "{refused:?}");
    assert_eq!(refused.json["code"], "fresh_sign_in_required");
    let refused = kit
        .api_json(
            Method::POST,
            "/api/v1/identity/passkeys",
            &stale,
            &json!({ "credential": device.create(&json!({"challenge": "x", "user": {"id": "AA"}}), Alg::Ed25519) }),
        )
        .await;
    assert_eq!(refused.json["code"], "step_up_required");
    // Renaming changes no way in: no step-up.
    let renamed = kit
        .api_json(
            Method::PATCH,
            &format!("/api/v1/identity/passkeys/{passkey}"),
            &stale,
            &json!({ "name": "Phone" }),
        )
        .await;
    assert_eq!(renamed.status, StatusCode::OK, "{renamed:?}");
    assert_eq!(renamed.json["name"], "Phone");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_last_way_in_stays_and_removing_a_passkey_signs_out_whoever_used_it(db: PgPool) {
    let kit = Kit::new(db).await;
    let (request, verifier) = waiting_request(&kit).await;
    let mut device = device();
    let redirect = sign_up_with_passkey(&kit, &mut device, &request).await;
    let tokens = redeem(&kit, &redirect, &verifier).await;
    let access = tokens["access_token"].as_str().unwrap().to_owned();
    let refresh = tokens["refresh_token"].as_str().unwrap().to_owned();
    let first = kit
        .api(Method::GET, "/api/v1/identity/passkeys", &access)
        .await
        .json["items"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let refused = kit
        .api(
            Method::DELETE,
            &format!("/api/v1/identity/passkeys/{first}"),
            &access,
        )
        .await;
    assert_eq!(refused.status, StatusCode::CONFLICT, "{refused:?}");
    assert_eq!(refused.json["code"], "last_way_in");

    // With a second passkey, the first can go, and its sign-in with it.
    let added = add_passkey(&kit, &access, &mut device, Alg::P256).await;
    assert_eq!(added.status, StatusCode::CREATED, "{added:?}");
    let removed = kit
        .api(
            Method::DELETE,
            &format!("/api/v1/identity/passkeys/{first}"),
            &access,
        )
        .await;
    assert_eq!(removed.status, StatusCode::NO_CONTENT, "{removed:?}");
    let refreshed = kit.refresh(&refresh).await;
    assert_eq!(refreshed.json["error"], "invalid_grant", "{refreshed:?}");
    assert_eq!(
        count(
            &kit,
            "SELECT count(*) FROM kernel.events WHERE name = 'identity.way_in_removed'"
        )
        .await,
        1
    );

    // The authenticator still holds it, but it is no way in any more.
    kit.new_browser();
    let (request, _) = waiting_request(&kit).await;
    let gone = sign_in_with_passkey(&kit, &mut device, 0, &request).await;
    assert_eq!(gone.status, StatusCode::FORBIDDEN, "{gone:?}");
    assert_eq!(gone.json["code"], "passkey_not_accepted");
    let gone_again = kit
        .api(
            Method::DELETE,
            &format!("/api/v1/identity/passkeys/{first}"),
            &access,
        )
        .await;
    assert_eq!(gone_again.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_provider_way_in_can_go_when_a_passkey_remains(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let google = kit.ways_in(&access).await["items"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let refused = kit
        .api(
            Method::DELETE,
            &format!("/api/v1/identity/ways-in/{google}"),
            &access,
        )
        .await;
    assert_eq!(refused.json["code"], "last_way_in");
    add_passkey(&kit, &access, &mut device(), Alg::Ed25519).await;
    let removed = kit
        .api(
            Method::DELETE,
            &format!("/api/v1/identity/ways-in/{google}"),
            &access,
        )
        .await;
    assert_eq!(removed.status, StatusCode::NO_CONTENT, "{removed:?}");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn ceremonies_are_single_use_and_bound_to_their_browser(db: PgPool) {
    let kit = Kit::new(db).await;
    let (request, _) = waiting_request(&kit).await;
    let mut device = device();
    sign_up_with_passkey(&kit, &mut device, &request).await;
    kit.new_browser();

    // Options in one browser, the assertion from another: no ceremony there.
    let options = screen(&kit, "/auth/passkey/sign-in/options", &json!({})).await;
    let signed = device.get(&options.json, 0);
    kit.new_browser();
    let elsewhere = screen(&kit, "/auth/passkey/sign-in", &signed).await;
    assert_eq!(elsewhere.status, StatusCode::CONFLICT, "{elsewhere:?}");
    assert_eq!(elsewhere.json["code"], "passkey_expired");

    // In the right browser it works once; replayed, it is expired.
    let options = screen(&kit, "/auth/passkey/sign-in/options", &json!({})).await;
    let signed = device.get(&options.json, 0);
    let first = screen(&kit, "/auth/passkey/sign-in", &signed).await;
    assert_eq!(first.status, StatusCode::OK, "{first:?}");
    assert_eq!(first.json["redirect"], "/");
    let options = screen(&kit, "/auth/passkey/sign-in/options", &json!({})).await;
    let fresh = device.get(&options.json, 0);
    let replayed = screen(&kit, "/auth/passkey/sign-in", &signed).await;
    assert_eq!(replayed.json["code"], "passkey_expired", "{replayed:?}");
    // A failed attempt spends its challenge too.
    let mut tampered = fresh.clone();
    tampered["response"]["signature"] = json!("AAAA");
    let bad = screen(&kit, "/auth/passkey/sign-in", &tampered).await;
    assert_eq!(bad.json["code"], "passkey_not_accepted", "{bad:?}");
    let after = screen(&kit, "/auth/passkey/sign-in", &fresh).await;
    assert_eq!(after.json["code"], "passkey_expired", "{after:?}");

    // A sign-up challenge does not sign in.
    let options = screen(
        &kit,
        "/auth/passkey/sign-up/options",
        &json!({ "display_name": "Bo" }),
    )
    .await;
    let wrong = device.get(&json!({ "challenge": options.json["challenge"] }), 0);
    let refused = screen(&kit, "/auth/passkey/sign-in", &wrong).await;
    assert_eq!(refused.json["code"], "passkey_expired", "{refused:?}");

    // Expired ceremonies are refused.
    let options = screen(&kit, "/auth/passkey/sign-in/options", &json!({})).await;
    sqlx::query("UPDATE identity.webauthn_challenges SET expires_at = now() - interval '1 second'")
        .execute(&kit.state.db)
        .await
        .unwrap();
    let late = screen(&kit, "/auth/passkey/sign-in", &device.get(&options.json, 0)).await;
    assert_eq!(late.json["code"], "passkey_expired", "{late:?}");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn sign_in_refusals_one_by_one(db: PgPool) {
    let kit = Kit::new(db).await;
    let (request, _) = waiting_request(&kit).await;
    let mut device = device();
    sign_up_with_passkey(&kit, &mut device, &request).await;

    // Without the CSRF header, or from another site.
    for headers in [
        vec![("content-type", "application/json")],
        vec![
            ("x-campbooks-csrf", "1"),
            ("sec-fetch-site", "cross-site"),
            ("content-type", "application/json"),
        ],
    ] {
        let refused = kit
            .request(
                Method::POST,
                "/auth/passkey/sign-in/options",
                &headers,
                Some("{}".to_owned()),
            )
            .await;
        assert_eq!(refused.status, StatusCode::FORBIDDEN, "{refused:?}");
        assert_eq!(refused.json["code"], "csrf_check_failed");
    }
    let bad_return = screen(
        &kit,
        "/auth/passkey/sign-in/options",
        &json!({ "return_to": "https://evil.example.com" }),
    )
    .await;
    assert_eq!(bad_return.json["code"], "invalid_return_to");

    // A passkey nobody registered here.
    let mut stranger = Authenticator::new(ORIGIN, "localhost");
    let options = screen(
        &kit,
        "/auth/passkey/sign-up/options",
        &json!({ "display_name": "X" }),
    )
    .await;
    stranger.create(&options.json, Alg::P256);
    let options = screen(&kit, "/auth/passkey/sign-in/options", &json!({})).await;
    let unknown = screen(
        &kit,
        "/auth/passkey/sign-in",
        &stranger.get(&options.json, 0),
    )
    .await;
    assert_eq!(unknown.json["code"], "passkey_not_accepted", "{unknown:?}");

    // A user handle that is not the passkey's account.
    let options = screen(&kit, "/auth/passkey/sign-in/options", &json!({})).await;
    let mut other_user = device.get(&options.json, 0);
    other_user["response"]["userHandle"] = json!("AAAAAAAAAAAAAAAAAAAAAA");
    let refused = screen(&kit, "/auth/passkey/sign-in", &other_user).await;
    assert_eq!(refused.json["code"], "passkey_not_accepted", "{refused:?}");

    // Without user verification.
    let options = screen(&kit, "/auth/passkey/sign-in/options", &json!({})).await;
    let mut ceremony = device.ceremony("webauthn.get", options.json["challenge"].as_str().unwrap());
    ceremony.flags = UP;
    let unverified = screen(
        &kit,
        "/auth/passkey/sign-in",
        &device.get_with(0, &ceremony),
    )
    .await;
    assert_eq!(
        unverified.json["code"], "passkey_not_accepted",
        "{unverified:?}"
    );

    // From another origin (a phishing page relaying the challenge).
    let options = screen(&kit, "/auth/passkey/sign-in/options", &json!({})).await;
    let mut ceremony = device.ceremony("webauthn.get", options.json["challenge"].as_str().unwrap());
    ceremony.origin = "https://campbooks.example.net".to_owned();
    let phished = screen(
        &kit,
        "/auth/passkey/sign-in",
        &device.get_with(0, &ceremony),
    )
    .await;
    assert_eq!(phished.json["code"], "passkey_not_accepted", "{phished:?}");

    // An account an administrator suspended: its passkey opens nothing.
    sqlx::query("UPDATE identity.accounts SET disabled_at = now()")
        .execute(&kit.state.db)
        .await
        .unwrap();
    let options = screen(&kit, "/auth/passkey/sign-in/options", &json!({})).await;
    let disabled = screen(&kit, "/auth/passkey/sign-in", &device.get(&options.json, 0)).await;
    assert_eq!(disabled.status, StatusCode::FORBIDDEN, "{disabled:?}");
    assert_eq!(disabled.json["code"], "account_suspended", "{disabled:?}");

    // Nothing of it reached a session.
    assert_eq!(
        count(
            &kit,
            "SELECT count(*) FROM identity.sessions WHERE method = 'passkey'"
        )
        .await,
        1,
        "only the sign-up's"
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_counter_that_goes_backwards_is_recorded_not_refused(db: PgPool) {
    let kit = Kit::new(db).await;
    let (request, _) = waiting_request(&kit).await;
    let mut device = device();
    sign_up_with_passkey(&kit, &mut device, &request).await;
    device.passkeys[0].count = 9;
    let options = screen(&kit, "/auth/passkey/sign-in/options", &json!({})).await;
    let first = screen(&kit, "/auth/passkey/sign-in", &device.get(&options.json, 0)).await;
    assert_eq!(first.status, StatusCode::OK);
    // A clone that still says 10.
    device.passkeys[0].count = 9;
    let options = screen(&kit, "/auth/passkey/sign-in/options", &json!({})).await;
    let clone = screen(&kit, "/auth/passkey/sign-in", &device.get(&options.json, 0)).await;
    assert_eq!(clone.status, StatusCode::OK, "{clone:?}");
    assert_eq!(
        count(
            &kit,
            "SELECT count(*) FROM identity.audit_events WHERE kind = 'passkey_sign_count_regressed'"
        )
        .await,
        1
    );
    let stored: i64 = sqlx::query_scalar("SELECT sign_count FROM identity.passkeys")
        .fetch_one(&kit.state.db)
        .await
        .unwrap();
    assert_eq!(stored, 10, "the counter never moves back");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_invite_only_instance_takes_no_passkey_sign_ups(db: PgPool) {
    let kit = Kit::with_signup(db, SignupPolicy::InviteOnly, &["ana@example.com"]).await;
    let (request, _) = waiting_request(&kit).await;
    let shown = kit.get(&format!("/auth/requests/{request}")).await;
    assert_eq!(shown.json["passkey_sign_up"], false);
    let refused = screen(
        &kit,
        "/auth/passkey/sign-up/options",
        &json!({ "display_name": "Ana", "request": request }),
    )
    .await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN, "{refused:?}");
    assert_eq!(refused.json["code"], "signup_invite_only");
    assert_eq!(
        count(&kit, "SELECT count(*) FROM identity.webauthn_challenges").await,
        0
    );
    assert_eq!(
        count(&kit, "SELECT count(*) FROM identity.accounts").await,
        0
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn registrations_are_the_accounts_own_and_credentials_unique(db: PgPool) {
    let kit = Kit::new(db).await;
    let ana = kit.signed_up().await;
    kit.new_browser();
    kit.google.set_user(campbooks_kernel::testing::TestUser {
        subject: "bo".to_owned(),
        name: "Bo".to_owned(),
        email: "bo@example.com".to_owned(),
    });
    let bo = kit.signed_up().await;

    // Ana's challenge, finished by Bo: no registration waits for Bo.
    let options = kit
        .api(
            Method::POST,
            "/api/v1/identity/passkeys/registrations",
            &ana,
        )
        .await;
    let mut device = device();
    let made = device.create(&options.json, Alg::Ed25519);
    let stolen = kit
        .api_json(
            Method::POST,
            "/api/v1/identity/passkeys",
            &bo,
            &json!({ "credential": made }),
        )
        .await;
    assert_eq!(stolen.status, StatusCode::CONFLICT, "{stolen:?}");
    assert_eq!(stolen.json["code"], "passkey_expired");

    // Registered by Ana, then offered again by Bo: refused.
    let added = add_passkey(&kit, &ana, &mut device, Alg::Ed25519).await;
    assert_eq!(added.status, StatusCode::CREATED);
    let id = added.json["id"].as_str().unwrap().to_owned();
    let options = kit
        .api(Method::POST, "/api/v1/identity/passkeys/registrations", &bo)
        .await;
    let ceremony = device.ceremony(
        "webauthn.create",
        options.json["challenge"].as_str().unwrap(),
    );
    let mut again = device.create_with(&options.json, Alg::Ed25519, false, &ceremony, "none");
    // The same credential id as Ana's (an authenticator ignoring
    // excludeCredentials).
    let anas = base64::Engine::encode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        &device.passkeys[1].id,
    );
    let duplicate = rewrite_credential_id(&mut again, &anas);
    let refused = kit
        .api_json(
            Method::POST,
            "/api/v1/identity/passkeys",
            &bo,
            &json!({ "credential": duplicate }),
        )
        .await;
    assert_eq!(refused.status, StatusCode::CONFLICT, "{refused:?}");
    assert_eq!(refused.json["code"], "passkey_already_registered");

    // Ana's passkey is invisible to Bo.
    assert_eq!(
        kit.api(Method::GET, "/api/v1/identity/passkeys", &bo)
            .await
            .json["items"],
        json!([])
    );
    for (method, body) in [
        (Method::PATCH, Some(json!({ "name": "Mine" }))),
        (Method::DELETE, None),
    ] {
        let path = format!("/api/v1/identity/passkeys/{id}");
        let answer = match body {
            Some(body) => kit.api_json(method, &path, &bo, &body).await,
            None => kit.api(method, &path, &bo).await,
        };
        assert_eq!(answer.status, StatusCode::NOT_FOUND, "{answer:?}");
    }
    let blank = kit
        .api_json(
            Method::PATCH,
            &format!("/api/v1/identity/passkeys/{id}"),
            &ana,
            &json!({ "name": " \n" }),
        )
        .await;
    assert_eq!(blank.json["code"], "passkey_name_required");
}

/// `credential` (a registration from the software authenticator) with its
/// credential id replaced by `id` everywhere: `id`, `rawId` and the
/// attested credential data.
fn rewrite_credential_id(credential: &mut Value, id: &str) -> Value {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    let object = URL_SAFE_NO_PAD
        .decode(
            credential["response"]["attestationObject"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
    let mut value: ciborium::Value = ciborium::de::from_reader(object.as_slice()).unwrap();
    let new_id = URL_SAFE_NO_PAD.decode(id).unwrap();
    if let ciborium::Value::Map(entries) = &mut value {
        for (key, entry) in entries.iter_mut() {
            if key.as_text() == Some("authData")
                && let ciborium::Value::Bytes(data) = entry
            {
                let len = usize::from(u16::from_be_bytes([data[53], data[54]]));
                let mut rewritten = data[..53].to_vec();
                rewritten.extend_from_slice(&u16::try_from(new_id.len()).unwrap().to_be_bytes());
                rewritten.extend_from_slice(&new_id);
                rewritten.extend_from_slice(&data[55 + len..]);
                *data = rewritten;
            }
        }
    }
    let mut encoded = Vec::new();
    ciborium::ser::into_writer(&value, &mut encoded).unwrap();
    credential["response"]["attestationObject"] = json!(URL_SAFE_NO_PAD.encode(encoded));
    credential["id"] = json!(id);
    credential["rawId"] = json!(id);
    credential.clone()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_ceremony_names_only_a_waiting_request_and_reads_a_small_body(db: PgPool) {
    let kit = Kit::new(db).await;
    let (request, _) = waiting_request(&kit).await;
    for start in [
        "/auth/passkey/sign-in/options",
        "/auth/passkey/sign-up/options",
    ] {
        for bad in [
            "not-a-request".to_owned(),
            "x".repeat(65),
            "has spaces".to_owned(),
            String::new(),
        ] {
            let refused = screen(
                &kit,
                start,
                &json!({ "display_name": "Ana", "request": bad }),
            )
            .await;
            assert_eq!(
                refused.status,
                StatusCode::NOT_FOUND,
                "{start} {bad:?}: {refused:?}"
            );
        }
        // An expired request is refused too.
        sqlx::query(
            "UPDATE identity.authorize_requests SET expires_at = now() - interval '1 second'",
        )
        .execute(&kit.state.db)
        .await
        .unwrap();
        let expired = screen(
            &kit,
            start,
            &json!({ "display_name": "Ana", "request": request }),
        )
        .await;
        assert_eq!(
            expired.status,
            StatusCode::NOT_FOUND,
            "{start}: {expired:?}"
        );
        sqlx::query(
            "UPDATE identity.authorize_requests SET expires_at = now() + interval '10 minutes'",
        )
        .execute(&kit.state.db)
        .await
        .unwrap();
        // A body over 16 KiB is not read.
        let huge = screen(
            &kit,
            start,
            &json!({ "display_name": "Ana", "request": "a".repeat(20_000) }),
        )
        .await;
        assert_eq!(huge.status, StatusCode::BAD_REQUEST, "{start}: {huge:?}");
        assert_eq!(huge.json["code"], "invalid_body");
    }
    // Nothing of it was stored.
    assert_eq!(
        count(&kit, "SELECT count(*) FROM identity.webauthn_challenges").await,
        0
    );

    // The table refuses an oversized request id whatever the code does.
    let stored = sqlx::query(
        "INSERT INTO identity.webauthn_challenges
             (challenge_hash, purpose, browser_hash, request_id, return_to, created_at, expires_at)
         VALUES ('\\x01', 'sign_in', '\\x02', $1, '/', now(), now())",
    )
    .bind("r".repeat(65))
    .execute(&kit.state.db)
    .await;
    assert!(stored.is_err(), "the CHECK refuses it");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn autofill_and_the_dialog_keep_their_own_challenges(db: PgPool) {
    let kit = Kit::new(db).await;
    let (request, _) = waiting_request(&kit).await;
    let mut device = device();
    sign_up_with_passkey(&kit, &mut device, &request).await;
    kit.new_browser();

    // The page opens (autofill starts), then the person presses P.
    let (request, _) = waiting_request(&kit).await;
    let autofill = screen(
        &kit,
        "/auth/passkey/sign-in/options",
        &json!({ "request": request }),
    )
    .await;
    let dialog = screen(
        &kit,
        "/auth/passkey/sign-in/options",
        &json!({ "request": request }),
    )
    .await;
    assert_ne!(autofill.json["challenge"], dialog.json["challenge"]);

    // Either answers in this browser: here, the autofill pick that was
    // already under way.
    let picked = screen(
        &kit,
        "/auth/passkey/sign-in",
        &device.get(&autofill.json, 0),
    )
    .await;
    assert_eq!(picked.status, StatusCode::OK, "{picked:?}");
    assert_eq!(
        count(&kit, "SELECT count(*) FROM identity.webauthn_challenges").await,
        1
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn waiting_registrations_are_capped_per_account(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    for _ in 0..10 {
        let started = kit
            .api(
                Method::POST,
                "/api/v1/identity/passkeys/registrations",
                &access,
            )
            .await;
        assert_eq!(started.status, StatusCode::CREATED, "{started:?}");
    }
    let capped = kit
        .api(
            Method::POST,
            "/api/v1/identity/passkeys/registrations",
            &access,
        )
        .await;
    assert_eq!(capped.status, StatusCode::TOO_MANY_REQUESTS, "{capped:?}");
    assert_eq!(capped.json["code"], "rate_limited");
}

/// Create an account with a passkey alone in a new browser: the status.
async fn passkey_sign_up(kit: &Kit) -> StatusCode {
    kit.new_browser();
    let mut device = device();
    let options = screen(
        kit,
        "/auth/passkey/sign-up/options",
        &json!({ "display_name": "Ana" }),
    )
    .await;
    assert_eq!(options.status, StatusCode::OK, "{options:?}");
    let made = device.create(&options.json, Alg::Ed25519);
    screen(kit, "/auth/passkey/sign-up", &made).await.status
}

#[sqlx::test(migrations = "../../../migrations")]
async fn passkey_only_accounts_are_capped_per_address_and_instance_wide(db: PgPool) {
    use campbooks_kernel::rate_limit::RateKey;

    use crate::protocol::passkey::{INSTANCE, SIGN_UP_LIMIT};

    let kit = Kit::new(db).await;
    kit.send_from(Some("203.0.113.7:50000"));
    for _ in 0..5 {
        assert_eq!(passkey_sign_up(&kit).await, StatusCode::CREATED);
    }
    assert_eq!(
        passkey_sign_up(&kit).await,
        StatusCode::TOO_MANY_REQUESTS,
        "five per address an hour"
    );
    kit.send_from(Some("198.51.100.2:50000"));
    assert_eq!(
        passkey_sign_up(&kit).await,
        StatusCode::CREATED,
        "another address has its own budget"
    );
    // Many addresses together reach the instance's ceiling.
    kit.state
        .rate_limits
        .exhaust(&kit.state.db, &SIGN_UP_LIMIT, &RateKey::text(INSTANCE))
        .await;
    kit.send_from(Some("192.0.2.9:50000"));
    assert_eq!(passkey_sign_up(&kit).await, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        count(&kit, "SELECT count(*) FROM identity.accounts").await,
        6
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn waiting_challenges_are_capped_per_address(db: PgPool) {
    use axum::{body::Body, extract::ConnectInfo, http::Request};
    use tower::ServiceExt as _;

    let kit = Kit::new(db).await;
    let from = |address: [u8; 4]| {
        let mut request = Request::builder()
            .method(Method::POST)
            .uri("/auth/passkey/sign-in/options")
            .header("x-campbooks-csrf", "1")
            .header("sec-fetch-site", "same-origin")
            .header("content-type", "application/json")
            .body(Body::from("{}"))
            .unwrap();
        request
            .extensions_mut()
            .insert(ConnectInfo(std::net::SocketAddr::from((address, 50_000))));
        request
    };
    for _ in 0..10 {
        let answer = kit
            .app
            .clone()
            .oneshot(from([203, 0, 113, 7]))
            .await
            .unwrap();
        assert_eq!(answer.status(), StatusCode::OK);
    }
    let capped = kit
        .app
        .clone()
        .oneshot(from([203, 0, 113, 7]))
        .await
        .unwrap();
    assert_eq!(capped.status(), StatusCode::TOO_MANY_REQUESTS);
    let elsewhere = kit
        .app
        .clone()
        .oneshot(from([198, 51, 100, 2]))
        .await
        .unwrap();
    assert_eq!(
        elsewhere.status(),
        StatusCode::OK,
        "another address has its own budget"
    );
    assert_eq!(
        count(&kit, "SELECT count(*) FROM identity.webauthn_challenges").await,
        11
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_suspended_account_is_told_so_only_after_its_passkey_verifies(db: PgPool) {
    use crate::domain::suspension;

    let kit = Kit::new(db).await;
    let (request, verifier) = waiting_request(&kit).await;
    let mut device = device();
    let redirect = sign_up_with_passkey(&kit, &mut device, &request).await;
    let tokens = redeem(&kit, &redirect, &verifier).await;
    let (_, id) = Kit::decode(tokens["id_token"].as_str().unwrap());
    let account: uuid::Uuid = id["sub"].as_str().unwrap().parse().unwrap();
    let mut tx = kit.state.db.begin().await.unwrap();
    suspension::suspend(
        &mut tx,
        &kit.state.events,
        account,
        "root",
        chrono::Utc::now(),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    kit.new_browser();
    let (request, _) = waiting_request(&kit).await;
    let refused = sign_in_with_passkey(&kit, &mut device, 0, &request).await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN, "{refused:?}");
    assert_eq!(refused.json["code"], "account_suspended");
    assert_eq!(
        count(
            &kit,
            "SELECT count(*) FROM identity.sessions WHERE ended_at IS NULL"
        )
        .await,
        0,
        "no session starts"
    );

    // A signature that does not verify learns nothing about the account.
    kit.new_browser();
    let (request, _) = waiting_request(&kit).await;
    let options = screen(
        &kit,
        "/auth/passkey/sign-in/options",
        &json!({ "request": request }),
    )
    .await;
    let mut forged = device.get(&options.json, 0);
    let signature = forged["response"]["signature"].as_str().unwrap().to_owned();
    let tampered = if signature.starts_with('A') {
        signature.replacen('A', "B", 1)
    } else {
        format!("A{}", &signature[1..])
    };
    forged["response"]["signature"] = json!(tampered);
    let probe = screen(&kit, "/auth/passkey/sign-in", &forged).await;
    assert_eq!(probe.json["code"], "passkey_not_accepted", "{probe:?}");

    let mut tx = kit.state.db.begin().await.unwrap();
    suspension::unsuspend(&mut tx, account, "root", chrono::Utc::now())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    kit.new_browser();
    let (request, _) = waiting_request(&kit).await;
    let back = sign_in_with_passkey(&kit, &mut device, 0, &request).await;
    assert_eq!(back.status, StatusCode::OK, "{back:?}");
}
