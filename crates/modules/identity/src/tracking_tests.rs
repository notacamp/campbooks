//! The module's tracked events (ADR 0020, campbooks-cloud#326), read back
//! from the outbox: one per change, with the actor, the operation and a
//! snapshot of ids, enums and counts, nothing for a change that changes
//! nothing, and never an address. Apps, consents and the handlers are
//! checked next to their own tests (they reuse those helpers).

use axum::http::{Method, StatusCode};
use campbooks_kernel::{
    config::SignupPolicy,
    testing::{self, TrackedRow},
};
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{
    admin_testkit::AdminKit,
    passkey_tests::{
        add_passkey, device, redeem, screen, sign_in_with_passkey, sign_up_with_passkey,
        waiting_request,
    },
    testkit::{Kit, MAIL_SCOPE, query},
    upstream::Provider,
    ways_in_tests::{link_to, user},
    webauthn::authenticator::{Alg, Authenticator},
};

/// The tracked events named `name`, oldest first.
pub(crate) async fn tracked(kit: &Kit, name: &str) -> Vec<TrackedRow> {
    testing::tracked_events(&kit.owner, name).await
}

/// The account an access token is for (its `sub`).
pub(crate) fn account_of(access: &str) -> String {
    Kit::decode(access).1["sub"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

/// Ids, enums, counts and times only: no address anywhere in the event.
pub(crate) fn no_address(event: &TrackedRow) {
    let body = event.body.to_string();
    assert!(
        !body.contains('@'),
        "{} holds an address: {body}",
        event.name
    );
    let envelope = event.envelope.to_string();
    assert!(
        !envelope.contains('@'),
        "{} holds an address: {envelope}",
        event.name
    );
}

/// `event` is version 1 of `name`, instance-wide, by `actor`, in
/// `operation`, and holds no address.
fn instance(event: &TrackedRow, name: &str, actor: &Value, operation: &str) {
    assert_eq!(event.name, name);
    assert_eq!(event.version, 1, "{name}");
    assert_eq!(event.workspace_id, None, "{name}");
    assert_eq!(event.envelope["name"], name);
    assert_eq!(event.envelope["actor"], *actor, "{name}");
    assert_eq!(event.envelope["operation"], operation, "{name}");
    assert_eq!(
        event.envelope["plan_version"],
        campbooks_kernel::tracking::PLAN_VERSION
    );
    no_address(event);
}

/// Nobody signed in yet: a sign-in step.
fn anonymous() -> Value {
    json!({"type": "anonymous"})
}

/// A person, by their account id.
fn person(account: &str) -> Value {
    json!({"type": "user", "id": account})
}

/// Sign in again in another browser as the fake Google's current user, and
/// redeem the code as the web client: the token response.
async fn sign_in_again(kit: &Kit) -> Value {
    kit.new_browser();
    let (url, verifier) = Kit::web_authorize();
    let request = kit.get(&url).await.location_query()["request"].clone();
    let back = kit.round_trip(Some(&request)).await;
    let tokens = kit.redeem(&back.location_query()["code"], &verifier).await;
    assert_eq!(tokens.status, StatusCode::OK, "{tokens:?}");
    tokens.json
}

#[sqlx::test(migrations = "../../../migrations")]
async fn signing_up_and_in_with_a_provider_is_tracked(db: PgPool) {
    let kit = Kit::new(db).await;
    let (code, verifier) = kit.sign_up().await;
    let tokens = kit.redeem(&code, &verifier).await;
    assert_eq!(tokens.status, StatusCode::OK, "{tokens:?}");
    let account = account_of(tokens.json["access_token"].as_str().unwrap());

    let created = tracked(&kit, "identity.account_created").await;
    assert_eq!(created.len(), 1);
    instance(
        &created[0],
        "identity.account_created",
        &anonymous(),
        "identity.create_account",
    );
    let body = &created[0].body;
    assert_eq!(body["account"]["id"], account.as_str());
    assert_eq!(body["account"]["provider_ways_in"], 1);
    assert_eq!(body["account"]["passkeys"], 0);
    assert_eq!(body["account"]["suspended"], false);
    assert_eq!(body["method"], "provider");
    assert_eq!(body["provider"], "google");
    assert_eq!(body["mailbox_held"], true);

    let signed_in = tracked(&kit, "identity.sign_in_succeeded").await;
    assert_eq!(signed_in.len(), 1);
    instance(
        &signed_in[0],
        "identity.sign_in_succeeded",
        &anonymous(),
        "identity.create_account",
    );
    let session = &signed_in[0].body["session"];
    assert_eq!(session["account_id"], account.as_str());
    assert_eq!(session["method"], "provider");
    assert_eq!(session["provider"], "google");
    assert!(session.get("ended_at").is_none());
    assert_eq!(signed_in[0].body["new_account"], true);

    let issued = tracked(&kit, "identity.tokens_issued").await;
    assert_eq!(issued.len(), 1);
    instance(
        &issued[0],
        "identity.tokens_issued",
        &anonymous(),
        "identity.token",
    );
    let body = &issued[0].body;
    assert_eq!(body["account_id"], account.as_str());
    assert_eq!(body["session_id"], session["id"]);
    assert_eq!(body["client_kind"], "first_party");
    assert_eq!(body["client_id"], "campbooks-web");
    assert_eq!(body["refresh_token_issued"], true);
    assert_eq!(body["workspace_bound"], false);
    assert!(body["scope_count"].as_i64().unwrap() > 0);
    assert!(!body.to_string().contains(MAIL_SCOPE), "no scope names");

    // The same person in another browser: a sign-in, no new account.
    let again = sign_in_again(&kit).await;
    let signed_in = tracked(&kit, "identity.sign_in_succeeded").await;
    assert_eq!(signed_in.len(), 2);
    instance(
        &signed_in[1],
        "identity.sign_in_succeeded",
        &anonymous(),
        "identity.finish_sign_in",
    );
    assert_eq!(signed_in[1].body["new_account"], false);
    assert_eq!(signed_in[1].body["session"]["account_id"], account.as_str());
    assert_eq!(tracked(&kit, "identity.account_created").await.len(), 1);
    assert_eq!(tracked(&kit, "identity.tokens_issued").await.len(), 2);

    // A refresh rotation is routine: nothing more is tracked.
    let rotated = kit.refresh(again["refresh_token"].as_str().unwrap()).await;
    assert_eq!(rotated.status, StatusCode::OK, "{rotated:?}");
    assert_eq!(tracked(&kit, "identity.tokens_issued").await.len(), 2);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_refused_sign_in_is_tracked_by_its_kind_and_names_nobody(db: PgPool) {
    let kit = Kit::new(db).await;
    let to_google = kit.get("/auth/start/google?request=r1").await;
    let state = query(&to_google.location())["state"].clone();
    let cancelled = kit
        .get(&format!(
            "/auth/callback/google?error=access_denied&state={state}"
        ))
        .await;
    assert_eq!(
        cancelled.location(),
        "/sign-in?error=cancelled_at_provider&request=r1"
    );
    let forged = kit.get("/auth/callback/google?code=x&state=forged").await;
    assert_eq!(forged.location(), "/sign-in?error=invalid_state");

    let failed = tracked(&kit, "identity.sign_in_failed").await;
    assert_eq!(failed.len(), 2);
    for event in &failed {
        instance(
            event,
            "identity.sign_in_failed",
            &anonymous(),
            "identity.finish_sign_in",
        );
    }
    assert_eq!(
        failed[0].body,
        json!({"method": "provider", "provider": "google", "failure": "cancelled_at_provider"})
    );
    assert_eq!(
        failed[1].body,
        json!({"method": "provider", "provider": "google", "failure": "invalid_state"})
    );
    assert!(tracked(&kit, "identity.sign_in_succeeded").await.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_uninvited_sign_up_is_tracked_as_refused(db: PgPool) {
    let kit = Kit::with_signup(db, SignupPolicy::InviteOnly, &[]).await;
    let back = kit.round_trip(None).await;
    assert_eq!(back.location(), "/sign-in?error=signup_invite_only");
    let failed = tracked(&kit, "identity.sign_in_failed").await;
    assert_eq!(failed.len(), 1);
    instance(
        &failed[0],
        "identity.sign_in_failed",
        &anonymous(),
        "identity.finish_sign_in",
    );
    assert_eq!(failed[0].body["failure"], "signup_invite_only");
    assert!(tracked(&kit, "identity.account_created").await.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn passkeys_are_tracked_from_sign_up_to_removal(db: PgPool) {
    let kit = Kit::new(db).await;
    let (request, verifier) = waiting_request(&kit).await;
    let mut device = device();
    let redirect = sign_up_with_passkey(&kit, &mut device, &request).await;
    let tokens = redeem(&kit, &redirect, &verifier).await;
    let access = tokens["access_token"].as_str().unwrap().to_owned();
    let account = account_of(&access);

    let created = tracked(&kit, "identity.account_created").await;
    assert_eq!(created.len(), 1);
    instance(
        &created[0],
        "identity.account_created",
        &anonymous(),
        "identity.finish_passkey_sign_up",
    );
    let body = &created[0].body;
    assert_eq!(body["method"], "passkey");
    assert!(body.get("provider").is_none());
    assert_eq!(body["mailbox_held"], false);
    assert_eq!(body["account"]["passkeys"], 1);
    assert_eq!(body["account"]["provider_ways_in"], 0);
    let signed_in = tracked(&kit, "identity.sign_in_succeeded").await;
    assert_eq!(signed_in.len(), 1);
    assert_eq!(signed_in[0].body["session"]["method"], "passkey");
    assert!(signed_in[0].body["session"].get("provider").is_none());
    assert_eq!(signed_in[0].body["new_account"], true);
    // The account's name is never kept.
    assert!(!created[0].body.to_string().contains("Ana"));

    // Added while signed in.
    let mut laptop = Authenticator::new(crate::testkit::ORIGIN, "localhost");
    let added = add_passkey(&kit, &access, &mut laptop, Alg::P256).await;
    assert_eq!(added.status, StatusCode::CREATED, "{added:?}");
    let laptop_id = added.json["id"].as_str().unwrap().to_owned();
    let registered = tracked(&kit, "identity.passkey_registered").await;
    assert_eq!(registered.len(), 1);
    instance(
        &registered[0],
        "identity.passkey_registered",
        &person(&account),
        "identity.register_passkey",
    );
    let passkey = &registered[0].body["passkey"];
    assert_eq!(passkey["id"], laptop_id.as_str());
    assert_eq!(passkey["account_id"], account.as_str());
    assert_eq!(passkey["added_via"], "passkey");
    assert!(passkey["backup_eligible"].is_boolean());
    assert!(passkey.get("name").is_none());

    // Renamed; the same name again changes nothing.
    let rename = format!("/api/v1/identity/passkeys/{laptop_id}");
    for _ in 0..2 {
        let renamed = kit
            .api_json(
                Method::PATCH,
                &rename,
                &access,
                &json!({ "name": "Work laptop" }),
            )
            .await;
        assert_eq!(renamed.status, StatusCode::OK, "{renamed:?}");
    }
    let renamed = tracked(&kit, "identity.passkey_renamed").await;
    assert_eq!(renamed.len(), 1);
    instance(
        &renamed[0],
        "identity.passkey_renamed",
        &person(&account),
        "identity.rename_passkey",
    );
    assert!(!renamed[0].body.to_string().contains("laptop"));

    // Signing in with it, and a counter that goes backwards.
    device.passkeys[0].count = 9;
    let (request, _) = waiting_request(&kit).await;
    let first = sign_in_with_passkey(&kit, &mut device, 0, &request).await;
    assert_eq!(first.status, StatusCode::OK, "{first:?}");
    device.passkeys[0].count = 9;
    let (request, _) = waiting_request(&kit).await;
    let clone = sign_in_with_passkey(&kit, &mut device, 0, &request).await;
    assert_eq!(clone.status, StatusCode::OK, "{clone:?}");
    let signed_in = tracked(&kit, "identity.sign_in_succeeded").await;
    assert_eq!(signed_in.len(), 3);
    instance(
        &signed_in[2],
        "identity.sign_in_succeeded",
        &anonymous(),
        "identity.finish_passkey_sign_in",
    );
    assert_eq!(signed_in[2].body["new_account"], false);
    let regressed = tracked(&kit, "identity.passkey_counter_regressed").await;
    assert_eq!(regressed.len(), 1);
    instance(
        &regressed[0],
        "identity.passkey_counter_regressed",
        &anonymous(),
        "identity.finish_passkey_sign_in",
    );
    assert_eq!(regressed[0].body["passkey"]["added_via"], "sign_up");
    let first_id = regressed[0].body["passkey"]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    // A passkey nobody registered here is refused, and tracked as such; a
    // request that fails the CSRF check is no sign-in attempt.
    let mut stranger = Authenticator::new(crate::testkit::ORIGIN, "localhost");
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
    let forged = kit
        .request(
            Method::POST,
            "/auth/passkey/sign-in",
            &[("content-type", "application/json")],
            Some("{}".to_owned()),
        )
        .await;
    assert!(forged.status.is_client_error(), "{forged:?}");
    let failed = tracked(&kit, "identity.sign_in_failed").await;
    assert_eq!(failed.len(), 1);
    instance(
        &failed[0],
        "identity.sign_in_failed",
        &anonymous(),
        "identity.finish_passkey_sign_in",
    );
    assert_eq!(
        failed[0].body,
        json!({"method": "passkey", "failure": "passkey_not_accepted"})
    );

    // Removed, with the sessions signed in with it.
    let removed = kit
        .api(
            Method::DELETE,
            &format!("/api/v1/identity/passkeys/{first_id}"),
            &access,
        )
        .await;
    assert_eq!(removed.status, StatusCode::NO_CONTENT, "{removed:?}");
    let removed = tracked(&kit, "identity.passkey_removed").await;
    assert_eq!(removed.len(), 1);
    instance(
        &removed[0],
        "identity.passkey_removed",
        &person(&account),
        "identity.remove_passkey",
    );
    assert_eq!(removed[0].body["passkey"]["id"], first_id.as_str());
    assert!(removed[0].body["sessions_ended"].as_i64().unwrap() >= 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn logging_out_tracks_the_session_once(db: PgPool) {
    let kit = Kit::new(db).await;
    let (code, verifier) = kit.sign_up().await;
    let tokens = kit.redeem(&code, &verifier).await;
    let hint = tokens.json["id_token"].as_str().unwrap().to_owned();
    let account = account_of(tokens.json["access_token"].as_str().unwrap());
    for _ in 0..2 {
        let out = kit
            .post_form("/oauth2/end_session", &[("id_token_hint", &hint)])
            .await;
        assert_eq!(out.status, StatusCode::NO_CONTENT, "{out:?}");
    }
    let ended = tracked(&kit, "identity.session_ended").await;
    assert_eq!(ended.len(), 1, "a session already ended tracks nothing");
    instance(
        &ended[0],
        "identity.session_ended",
        &anonymous(),
        "identity.end_session_post",
    );
    let session = &ended[0].body["session"];
    assert_eq!(session["account_id"], account.as_str());
    assert!(session["ended_at"].is_string());
    let started = tracked(&kit, "identity.sign_in_succeeded").await;
    assert_eq!(session["id"], started[0].body["session"]["id"]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn signing_out_everywhere_counts_what_ended(db: PgPool) {
    let kit = Kit::new(db).await;
    let (code, verifier) = kit.sign_up().await;
    kit.redeem(&code, &verifier).await;
    let there = sign_in_again(&kit).await;
    let access = there["access_token"].as_str().unwrap().to_owned();
    let ended = kit
        .api(
            Method::POST,
            "/api/v1/identity/sign-out-everywhere",
            &access,
        )
        .await;
    assert_eq!(ended.status, StatusCode::NO_CONTENT, "{ended:?}");
    let events = tracked(&kit, "identity.sessions_ended").await;
    assert_eq!(events.len(), 1);
    let account = account_of(&access);
    instance(
        &events[0],
        "identity.sessions_ended",
        &person(&account),
        "identity.sign_out_everywhere",
    );
    assert_eq!(
        events[0].body,
        json!({"account_id": account, "sessions_ended": 2, "refresh_families_revoked": 2})
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_way_in_added_and_removed_is_tracked(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let account = account_of(&access);
    kit.zoho.set_user(user("zoho-sam"));
    let at_zoho = link_to(&kit, &access, Provider::Zoho).await;
    kit.via(Provider::Zoho, &at_zoho).await;
    // Linking it again changes nothing.
    let at_zoho = link_to(&kit, &access, Provider::Zoho).await;
    kit.via(Provider::Zoho, &at_zoho).await;

    let added = tracked(&kit, "identity.way_in_added").await;
    assert_eq!(added.len(), 1);
    instance(
        &added[0],
        "identity.way_in_added",
        &anonymous(),
        "identity.finish_sign_in",
    );
    let way_in = &added[0].body["way_in"];
    assert_eq!(way_in["account_id"], account.as_str());
    assert_eq!(way_in["provider"], "zoho");
    assert_eq!(way_in["linked_via"], "link");
    assert!(way_in["address_verified"].is_boolean());
    let zoho = way_in["id"].as_str().unwrap().to_owned();

    let removed = kit
        .api(
            Method::DELETE,
            &format!("/api/v1/identity/ways-in/{zoho}"),
            &access,
        )
        .await;
    assert_eq!(removed.status, StatusCode::NO_CONTENT, "{removed:?}");
    let removed = tracked(&kit, "identity.way_in_removed").await;
    assert_eq!(removed.len(), 1);
    instance(
        &removed[0],
        "identity.way_in_removed",
        &person(&account),
        "identity.remove_way_in",
    );
    assert_eq!(
        removed[0].body,
        json!({
            "way_in_id": zoho, "account_id": account, "provider": "zoho",
            "cause": "member", "sessions_ended": 0,
        })
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_failed_connect_takes_its_way_in_back_tracked(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    kit.google.set_user(user("work-sam"));
    let started = kit
        .start_link(
            &access,
            &json!({ "purpose": "connect", "provider": "google",
                     "workspace_id": uuid::Uuid::now_v7(), "return_to": "/w/ws" }),
        )
        .await;
    let to_google = kit.get(started.json["url"].as_str().unwrap()).await;
    // The person unticks the mail permission.
    let declined = to_google.location().replace(
        &url::form_urlencoded::byte_serialize(MAIL_SCOPE.as_bytes()).collect::<String>(),
        "",
    );
    let back = kit.via(Provider::Google, &declined).await;
    assert_eq!(back.location(), "/w/ws?link_error=consent_incomplete");

    let added = tracked(&kit, "identity.way_in_added").await;
    assert_eq!(added.len(), 1);
    assert_eq!(added[0].body["way_in"]["linked_via"], "connect");
    let removed = tracked(&kit, "identity.way_in_removed").await;
    assert_eq!(removed.len(), 1);
    // The hand-over runs off the request, in its tracking context.
    instance(
        &removed[0],
        "identity.way_in_removed",
        &anonymous(),
        "identity.finish_sign_in",
    );
    assert_eq!(removed[0].body["cause"], "connect_failed");
    assert_eq!(removed[0].body["way_in_id"], added[0].body["way_in"]["id"]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn reuse_and_revocation_of_refresh_tokens_are_tracked(db: PgPool) {
    let kit = Kit::new(db).await;
    let (code, verifier) = kit.sign_up().await;
    let first = kit.redeem(&code, &verifier).await;
    let account = account_of(first.json["access_token"].as_str().unwrap());
    let spent = first.json["refresh_token"].as_str().unwrap().to_owned();
    let replay = kit.redeem(&code, &verifier).await;
    assert_eq!(replay.json["error"], "invalid_grant");
    let reused = tracked(&kit, "identity.refresh_reuse_detected").await;
    assert_eq!(reused.len(), 1);
    instance(
        &reused[0],
        "identity.refresh_reuse_detected",
        &anonymous(),
        "identity.token",
    );
    assert_eq!(
        reused[0].body,
        json!({"account_id": account, "client_kind": "first_party", "client_id": "campbooks-web"})
    );

    let live = sign_in_again(&kit).await;
    let refresh = live["refresh_token"].as_str().unwrap().to_owned();
    let secret = Kit::web_secret();
    // The live token twice, and the one reuse revoked already.
    for token in [refresh.as_str(), refresh.as_str(), spent.as_str()] {
        let revoked = kit
            .post_form(
                "/oauth2/revoke",
                &[
                    ("token", token),
                    ("client_id", "campbooks-web"),
                    ("client_secret", &secret),
                ],
            )
            .await;
        assert_eq!(revoked.status, StatusCode::OK);
    }
    let revoked = tracked(&kit, "identity.refresh_token_revoked").await;
    assert_eq!(revoked.len(), 1, "only a live family is revoked");
    instance(
        &revoked[0],
        "identity.refresh_token_revoked",
        &anonymous(),
        "identity.revoke",
    );
    assert_eq!(revoked[0].body["account_id"], account.as_str());
    assert_eq!(revoked[0].body["client_id"], "campbooks-web");
    assert!(revoked[0].body["family_id"].is_string());
}

/// The held mailbox grant of the account `access` signed up: its id.
async fn held_grant(kit: &Kit, access: &str) -> String {
    let held = kit
        .api(Method::GET, "/api/v1/identity/held-grants", access)
        .await;
    assert_eq!(held.status, StatusCode::OK, "{held:?}");
    held.json["items"][0]["id"].as_str().unwrap().to_owned()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_held_grant_discarded_is_tracked(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let account = account_of(&access);
    let grant = held_grant(&kit, &access).await;
    let discarded = kit
        .api(
            Method::DELETE,
            &format!("/api/v1/identity/held-grants/{grant}"),
            &access,
        )
        .await;
    assert_eq!(discarded.status, StatusCode::NO_CONTENT, "{discarded:?}");
    let events = tracked(&kit, "identity.held_grant_discarded").await;
    assert_eq!(events.len(), 1);
    instance(
        &events[0],
        "identity.held_grant_discarded",
        &person(&account),
        "identity.discard_held_grant",
    );
    assert_eq!(
        events[0].body,
        json!({"held_grant_id": grant, "account_id": account, "provider": "google"})
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_held_grant_adopted_is_tracked_with_its_workspace(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let account = account_of(&access);
    let grant = held_grant(&kit, &access).await;
    let workspace = uuid::Uuid::now_v7();
    let adopted = kit
        .api(
            Method::POST,
            &format!("/api/v1/w/{workspace}/identity/held-grants/{grant}/adopt"),
            &access,
        )
        .await;
    assert_eq!(adopted.status, StatusCode::OK, "{adopted:?}");
    let events = tracked(&kit, "identity.held_grant_adopted").await;
    assert_eq!(events.len(), 1);
    instance(
        &events[0],
        "identity.held_grant_adopted",
        &person(&account),
        "identity.adopt_held_grant",
    );
    assert_eq!(
        events[0].body,
        json!({
            "held_grant_id": grant, "account_id": account, "provider": "google",
            "in_workspace_id": workspace.to_string(),
        })
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_administrator_suspending_an_account_is_tracked_once(db: PgPool) {
    let kit = AdminKit::new(db, &[]).await;
    let ana = kit.account("ana").await;
    for (action, name, operation, suspended) in [
        (
            "suspend",
            "identity.account_suspended",
            "identity.admin_suspend_account",
            true,
        ),
        (
            "unsuspend",
            "identity.account_unsuspended",
            "identity.admin_unsuspend_account",
            false,
        ),
    ] {
        let uri = format!("/admin/v1/identity/accounts/{ana}/{action}");
        for _ in 0..2 {
            let done = kit.call("root", Method::POST, &uri, "en").await;
            assert_eq!(done.status, StatusCode::OK, "{}", done.json);
        }
        let events = testing::tracked_events(&kit.db, name).await;
        assert_eq!(events.len(), 1, "a retry tracks nothing: {name}");
        instance(
            &events[0],
            name,
            &json!({"type": "admin", "id": "root"}),
            operation,
        );
        let account = &events[0].body["account"];
        assert_eq!(account["id"], ana.to_string());
        assert_eq!(account["suspended"], suspended);
        assert_eq!(account["provider_ways_in"], 1);
        assert_eq!(account["passkeys"], 1);
    }
}
