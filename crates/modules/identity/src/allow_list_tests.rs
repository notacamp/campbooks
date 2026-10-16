//! `CAMPBOOKS_SIGNUP__ALLOWED`, end to end through the issuer: it is asked at
//! every door that creates an account or issues a session or tokens, for
//! known accounts and new ones alike, and invitations do not bypass it.

use axum::http::StatusCode;
use campbooks_kernel::config::SignupPolicy;
use serde_json::json;
use sqlx::PgPool;

use crate::{
    passkey_tests::{
        add_passkey, count, device, redeem, screen, sign_in_with_passkey, sign_up_with_passkey,
        waiting_request,
    },
    testkit::Kit,
    upstream::Provider,
    ways_in_tests::{link_to, user},
};

/// The fake Google's default person.
const ME: &str = "test-user@example.com";

async fn nothing_is_kept(kit: &Kit) {
    for sql in [
        "SELECT count(*) FROM identity.accounts",
        "SELECT count(*) FROM identity.pending_sign_ups",
        "SELECT count(*) FROM identity.held_grants",
        "SELECT count(*) FROM identity.sessions",
    ] {
        assert_eq!(count(kit, sql).await, 0, "{sql}");
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_allowed_domain_signs_up_and_in(db: PgPool) {
    let kit = Kit::with_allowed(db, SignupPolicy::Open, &[], " Example.com ").await;
    let access = kit.signed_up().await;
    assert!(!access.is_empty());
    // And back in, from another browser.
    kit.new_browser();
    let again = kit.round_trip(None).await;
    assert_eq!(again.location(), "/");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_allowed_address_signs_up_and_a_neighbour_does_not(db: PgPool) {
    let kit = Kit::with_allowed(db, SignupPolicy::Open, &[], "TEST-USER@example.com").await;
    kit.signed_up().await;

    kit.new_browser();
    kit.google.set_user(user("neighbour"));
    let to_google = kit.get("/auth/start/google?request=r%201").await;
    let refused = kit.via_google(&to_google.location()).await;
    assert_eq!(
        refused.location(),
        "/sign-in?error=sign_in_not_allowed&request=r+1"
    );
    assert_eq!(
        count(&kit, "SELECT count(*) FROM identity.accounts").await,
        1
    );
    assert_eq!(
        count(&kit, "SELECT count(*) FROM identity.pending_sign_ups").await,
        0
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_subdomain_is_not_the_domain(db: PgPool) {
    let kit = Kit::with_allowed(db, SignupPolicy::Open, &[], "example.com").await;
    kit.google.set_user(campbooks_kernel::testing::TestUser {
        subject: "sub".to_owned(),
        name: "Sub".to_owned(),
        email: "someone@mail.example.com".to_owned(),
    });
    let refused = kit.round_trip(None).await;
    assert_eq!(refused.location(), "/sign-in?error=sign_in_not_allowed");
    nothing_is_kept(&kit).await;
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_invitation_does_not_bypass_the_allow_list(db: PgPool) {
    let kit = Kit::with_allowed(
        db,
        SignupPolicy::InviteOnly,
        &[ME],
        "someone-else@example.com",
    )
    .await;
    let refused = kit.round_trip(None).await;
    assert_eq!(refused.location(), "/sign-in?error=sign_in_not_allowed");
    nothing_is_kept(&kit).await;
    assert_eq!(kit.create_account().await.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_address_the_provider_did_not_verify_is_refused(db: PgPool) {
    let kit = Kit::with_allowed(db, SignupPolicy::Open, &[], "example.com").await;
    // Microsoft sends no `email_verified`, and no `xms_edov` here.
    let refused = kit.round_trip_with(Provider::Microsoft, None).await;
    assert_eq!(refused.location(), "/sign-in?error=sign_in_not_allowed");
    nothing_is_kept(&kit).await;
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_existing_account_off_the_list_cannot_sign_in(db: PgPool) {
    let open = Kit::new(db.clone()).await;
    open.signed_up().await;
    let accounts = count(&open, "SELECT count(*) FROM identity.accounts").await;
    let sessions = count(&open, "SELECT count(*) FROM identity.sessions").await;

    let strict = Kit::with_allowed(db, SignupPolicy::Open, &[], "someone-else@example.com").await;
    strict.new_browser();
    let refused = strict.round_trip(None).await;
    assert_eq!(refused.location(), "/sign-in?error=sign_in_not_allowed");
    assert_eq!(
        count(&strict, "SELECT count(*) FROM identity.accounts").await,
        accounts
    );
    assert_eq!(
        count(&strict, "SELECT count(*) FROM identity.sessions").await,
        sessions,
        "no session started"
    );

    // Said the same to someone who has no account: nothing tells them apart.
    strict.new_browser();
    strict.google.set_user(user("nobody"));
    let stranger = strict.round_trip(None).await;
    assert_eq!(stranger.location(), refused.location());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_pending_sign_up_is_refused_if_the_list_tightened_meanwhile(db: PgPool) {
    let open = Kit::new(db.clone()).await;
    open.round_trip(None).await;
    let strict = Kit::with_allowed(db, SignupPolicy::Open, &[], "someone-else@example.com").await;
    strict.share_cookies_with(&open);
    let refused = strict.create_account().await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN);
    assert_eq!(refused.json["code"], "sign_in_not_allowed");
    nothing_is_kept(&strict).await;
}

#[sqlx::test(migrations = "../../../migrations")]
async fn refreshing_stops_when_the_list_no_longer_has_the_account(db: PgPool) {
    let open = Kit::new(db.clone()).await;
    let (code, verifier) = open.sign_up().await;
    let tokens = open.redeem(&code, &verifier).await;
    assert_eq!(tokens.status, StatusCode::OK, "{tokens:?}");
    let refresh = tokens.json["refresh_token"].as_str().unwrap().to_owned();

    let strict = Kit::with_allowed(
        db.clone(),
        SignupPolicy::Open,
        &[],
        "someone-else@example.com",
    )
    .await;
    let refused = strict.refresh(&refresh).await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST, "{refused:?}");
    assert_eq!(refused.json["error"], "invalid_grant");
    assert!(refused.json.get("access_token").is_none());

    // The refusal changed nothing: widen the list again and it still works.
    let widened = Kit::with_allowed(db, SignupPolicy::Open, &[], "example.com").await;
    let refreshed = widened.refresh(&refresh).await;
    assert_eq!(refreshed.status, StatusCode::OK, "{refreshed:?}");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_unset_or_empty_list_changes_nothing(db: PgPool) {
    for allowed in [None, Some(""), Some(" , ")] {
        let kit = match allowed {
            None => Kit::new(db.clone()).await,
            Some(value) => Kit::with_allowed(db.clone(), SignupPolicy::Open, &[], value).await,
        };
        kit.new_browser();
        kit.google.set_user(user(&format!("anyone-{allowed:?}")));
        kit.signed_up().await;
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_passkey_proves_no_address_so_it_neither_creates_nor_opens_an_account(db: PgPool) {
    // An account made with a passkey alone, before the list existed.
    let open = Kit::new(db.clone()).await;
    let (request, verifier) = waiting_request(&open).await;
    let mut device = device();
    let redirect = sign_up_with_passkey(&open, &mut device, &request).await;
    redeem(&open, &redirect, &verifier).await;

    let strict = Kit::with_allowed(db, SignupPolicy::Open, &[], "example.com").await;
    strict.new_browser();
    let (request, _) = waiting_request(&strict).await;
    let shown = strict.get(&format!("/auth/requests/{request}")).await;
    assert_eq!(shown.json["passkey_sign_up"], false);

    let refused = sign_in_with_passkey(&strict, &mut device, 0, &request).await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN, "{refused:?}");
    assert_eq!(refused.json["code"], "sign_in_not_allowed");
    assert_eq!(
        count(
            &strict,
            "SELECT count(*) FROM identity.sessions WHERE ended_at IS NULL"
        )
        .await,
        1,
        "only the first sign-up's session"
    );

    let sign_up = screen(
        &strict,
        "/auth/passkey/sign-up/options",
        &json!({ "display_name": "Bo", "request": request }),
    )
    .await;
    assert_eq!(sign_up.status, StatusCode::FORBIDDEN, "{sign_up:?}");
    assert_eq!(sign_up.json["code"], "sign_in_not_allowed");
    assert_eq!(
        count(&strict, "SELECT count(*) FROM identity.accounts").await,
        1
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_passkey_opens_an_account_with_a_listed_address_only(db: PgPool) {
    let open = Kit::new(db.clone()).await;
    let access = open.signed_up().await;
    let mut device = device();
    let added = add_passkey(
        &open,
        &access,
        &mut device,
        crate::webauthn::authenticator::Alg::Ed25519,
    )
    .await;
    assert_eq!(added.status, StatusCode::CREATED, "{added:?}");

    let allowed = Kit::with_allowed(db.clone(), SignupPolicy::Open, &[], ME).await;
    allowed.new_browser();
    let (request, _) = waiting_request(&allowed).await;
    let signed = sign_in_with_passkey(&allowed, &mut device, 0, &request).await;
    assert_eq!(signed.status, StatusCode::OK, "{signed:?}");

    let strict = Kit::with_allowed(db, SignupPolicy::Open, &[], "someone-else@example.com").await;
    strict.new_browser();
    let (request, _) = waiting_request(&strict).await;
    let refused = sign_in_with_passkey(&strict, &mut device, 0, &request).await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN, "{refused:?}");
    assert_eq!(refused.json["code"], "sign_in_not_allowed");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_way_in_to_link_must_be_on_the_list_too(db: PgPool) {
    let kit = Kit::with_allowed(
        db,
        SignupPolicy::Open,
        &[],
        &format!("{ME}, zoho-ok@example.com"),
    )
    .await;
    let access = kit.signed_up().await;

    kit.zoho.set_user(user("zoho-stranger"));
    let at_zoho = link_to(&kit, &access, Provider::Zoho).await;
    let back = kit.via(Provider::Zoho, &at_zoho).await;
    assert!(
        back.location().ends_with("?link_error=sign_in_not_allowed"),
        "{back:?}"
    );
    assert_eq!(
        count(&kit, "SELECT count(*) FROM identity.provider_identities").await,
        1,
        "the way in was not linked"
    );

    kit.zoho.set_user(user("zoho-ok"));
    let at_zoho = link_to(&kit, &access, Provider::Zoho).await;
    let back = kit.via(Provider::Zoho, &at_zoho).await;
    assert!(back.location().ends_with("?linked=zoho"), "{back:?}");
}

/// Connect Google account `subject` as `access` (`sign_in`: also as a way
/// in); where the browser lands.
async fn connect_as(kit: &Kit, access: &str, subject: &str, sign_in: bool) -> String {
    kit.google.set_user(user(subject));
    let started = kit
        .start_link(
            access,
            &json!({ "purpose": "connect", "provider": "google", "sign_in": sign_in,
                     "workspace_id": uuid::Uuid::now_v7(), "return_to": "/w/ws" }),
        )
        .await;
    assert_eq!(started.status, StatusCode::CREATED, "{started:?}");
    let to_google = kit.get(started.json["url"].as_str().unwrap()).await;
    kit.via(Provider::Google, &to_google.location())
        .await
        .location()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_connect_that_would_add_a_way_in_must_be_on_the_list(db: PgPool) {
    let kit = Kit::with_allowed(db, SignupPolicy::Open, &[], ME).await;
    let access = kit.signed_up().await;

    // Also as a way in: refused before anything is written or handed over.
    let back = connect_as(&kit, &access, "stranger", true).await;
    assert_eq!(back, "/w/ws?link_error=sign_in_not_allowed");
    assert_eq!(
        count(&kit, "SELECT count(*) FROM identity.provider_identities").await,
        1,
        "no way in was added"
    );
    assert_eq!(
        count(&kit, "SELECT count(*) FROM identity.mailbox_links").await,
        0,
        "nothing recorded"
    );
    assert!(kit.intake.adopted().is_empty(), "no grant handed over");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_shared_mailbox_off_the_list_connects_but_never_signs_in(db: PgPool) {
    let kit = Kit::with_allowed(db, SignupPolicy::Open, &[], ME).await;
    let access = kit.signed_up().await;

    // Not a way in: connected, with nothing written on the identity side.
    let back = connect_as(&kit, &access, "stranger", false).await;
    assert_eq!(back, "/w/ws?connected=connection-1");
    assert_eq!(
        count(&kit, "SELECT count(*) FROM identity.provider_identities").await,
        1
    );
    assert_eq!(
        count(&kit, "SELECT count(*) FROM identity.mailbox_links").await,
        0
    );
    // And it is no road around the list: signing in with it is still refused.
    kit.new_browser();
    let refused = kit.round_trip(None).await;
    assert_eq!(refused.location(), "/sign-in?error=sign_in_not_allowed");
}
