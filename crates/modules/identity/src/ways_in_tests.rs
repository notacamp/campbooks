//! End to end through the issuer, slice S2: adding a way in, connecting and
//! reconnecting a mailbox through the front door, removing a way in, and
//! signing in with Microsoft and Zoho; with every refusal, one by one.

use axum::http::{Method, StatusCode};
use campbooks_kernel::{config::SignupPolicy, connections::ConsentRequest, testing::TestUser};
use serde_json::json;
use sqlx::PgPool;

use crate::{
    testkit::{
        HEALTHY, Kit, MAIL_SCOPE, ORIGIN, RECONNECTABLE, RECONNECTABLE_ADDRESS, TENANT,
        ZOHO_MAIL_SCOPES, query,
    },
    upstream::{
        Provider,
        client::{MICROSOFT_ISSUER_PREFIX, MICROSOFT_ISSUER_SUFFIX},
    },
};

const SETTINGS: &str = "/w/ws/settings/sign-in";

pub(crate) fn user(subject: &str) -> TestUser {
    TestUser {
        subject: subject.to_owned(),
        name: "Sam Rivera".to_owned(),
        email: format!("{subject}@example.com"),
    }
}

async fn count(kit: &Kit, sql: &'static str) -> i64 {
    sqlx::query_scalar(sql)
        .fetch_one(&kit.kernel)
        .await
        .unwrap()
}

/// Start a `link` to `provider` and follow it to the provider: the
/// authorization request the browser is sent to.
pub(crate) async fn link_to(kit: &Kit, access: &str, provider: Provider) -> String {
    let started = kit
        .start_link(
            access,
            &json!({ "purpose": "link", "provider": provider.as_str(), "return_to": SETTINGS }),
        )
        .await;
    assert_eq!(started.status, StatusCode::CREATED, "{started:?}");
    let url = started.json["url"].as_str().unwrap().to_owned();
    assert!(
        url.starts_with(&format!("/auth/start/{}?link=", provider.as_str())),
        "{url}"
    );
    let to_provider = kit.get(&url).await;
    assert_eq!(to_provider.status, StatusCode::SEE_OTHER, "{to_provider:?}");
    to_provider.location()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_person_adds_zoho_as_a_way_in_then_signs_in_with_it(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let (_, me) = Kit::decode(&access);

    kit.zoho.set_user(user("zoho-sam"));
    let at_zoho = link_to(&kit, &access, Provider::Zoho).await;
    let asked = query(&at_zoho);
    assert_eq!(asked["prompt"], "consent");
    assert_eq!(asked["scope"], "openid email profile", "identity only");
    assert!(!asked.contains_key("access_type"), "no offline access");

    let back = kit.via(Provider::Zoho, &at_zoho).await;
    assert_eq!(back.location(), format!("{SETTINGS}?linked=zoho"));
    let ways = kit.ways_in(&access).await;
    let providers: Vec<&str> = ways["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["provider"].as_str().unwrap())
        .collect();
    assert_eq!(providers, ["google", "zoho"]);
    assert_eq!(ways["items"][0]["linked_via"], "sign_up");
    assert_eq!(ways["items"][1]["linked_via"], "link");
    assert_eq!(ways["items"][1]["email"], "zoho-sam@example.com");
    assert_eq!(ways["providers"], json!(["google", "microsoft", "zoho"]));
    // Mail only where there is an adapter: not Microsoft, Zoho once synced.
    assert_eq!(ways["mailbox_providers"], json!(["google"]));
    kit.intake.sync_zoho();
    assert_eq!(
        kit.ways_in(&access).await["mailbox_providers"],
        json!(["google", "zoho"])
    );
    let added = count(
        &kit,
        "SELECT count(*) FROM kernel.events WHERE name = 'identity.way_in_added'",
    )
    .await;
    assert_eq!(added, 1);

    // Another browser: Zoho now opens the same account.
    kit.new_browser();
    let (url, verifier) = Kit::web_authorize();
    let request = kit.get(&url).await.location_query()["request"].clone();
    let shown = kit.get(&format!("/auth/requests/{request}")).await;
    assert_eq!(
        shown.json["providers"],
        json!(["google", "microsoft", "zoho", "passkey"])
    );
    let signed_in = kit.round_trip_with(Provider::Zoho, Some(&request)).await;
    assert!(
        signed_in
            .location()
            .starts_with(&format!("{ORIGIN}/bff/callback?code=")),
        "{signed_in:?}"
    );
    let tokens = kit
        .redeem(&signed_in.location_query()["code"], &verifier)
        .await;
    let (_, again) = Kit::decode(tokens.json["access_token"].as_str().unwrap());
    assert_eq!(again["sub"], me["sub"]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn adding_a_way_in_needs_a_recent_sign_in(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let stale = kit.stale(&access).await;
    let refused = kit
        .start_link(&stale, &json!({ "purpose": "link", "provider": "zoho" }))
        .await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN, "{refused:?}");
    assert_eq!(refused.json["code"], "step_up_required");
    assert_eq!(
        count(&kit, "SELECT count(*) FROM identity.link_tickets").await,
        0
    );
    let connect = kit
        .start_link(
            &stale,
            &json!({ "purpose": "connect", "provider": "google", "workspace_id": uuid::Uuid::now_v7() }),
        )
        .await;
    assert_eq!(connect.json["code"], "step_up_required");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_link_ticket_works_once_and_only_in_its_accounts_browser(db: PgPool) {
    let kit = Kit::new(db).await;
    let ana = kit.signed_up().await;
    let started = kit
        .start_link(
            &ana,
            &json!({ "purpose": "link", "provider": "zoho", "return_to": SETTINGS }),
        )
        .await;
    let url = started.json["url"].as_str().unwrap().to_owned();

    // Another browser, signed in as someone else: nothing starts.
    kit.new_browser();
    kit.google.set_user(user("bea"));
    kit.signed_up().await;
    let elsewhere = kit.get(&url).await;
    assert_eq!(
        elsewhere.location(),
        format!("{SETTINGS}?link_error=sign_in_again")
    );
    // And the ticket is spent.
    let again = kit.get(&url).await;
    assert_eq!(again.location(), "/?link_error=link_expired");

    // Not signed in at all: nothing starts either.
    let fresh = kit
        .start_link(&ana, &json!({ "purpose": "link", "provider": "zoho" }))
        .await;
    kit.new_browser();
    let anonymous = kit.get(fresh.json["url"].as_str().unwrap()).await;
    assert_eq!(anonymous.location(), "/?link_error=sign_in_again");

    // A ticket for one provider does not start another.
    let other = kit
        .start_link(&ana, &json!({ "purpose": "link", "provider": "zoho" }))
        .await;
    let swapped = other.json["url"]
        .as_str()
        .unwrap()
        .replace("/auth/start/zoho", "/auth/start/google");
    assert_eq!(
        kit.get(&swapped).await.location(),
        "/?link_error=link_expired"
    );
    assert_eq!(
        count(
            &kit,
            "SELECT count(*) FROM identity.upstream_flows WHERE purpose = 'link'"
        )
        .await,
        0
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn another_accounts_way_in_is_refused_and_nothing_is_revoked(db: PgPool) {
    let kit = Kit::new(db).await;
    // Ana signs up with Google account `ana`.
    kit.google.set_user(user("ana"));
    kit.signed_up().await;
    // Bea, in another browser, signs up with Google account `bea`, then
    // tries to add Ana's Google account.
    kit.new_browser();
    kit.google.set_user(user("bea"));
    let bea = kit.signed_up().await;
    kit.google.set_user(user("ana"));
    let at_google = link_to(&kit, &bea, Provider::Google).await;
    let back = kit.via(Provider::Google, &at_google).await;
    assert_eq!(
        back.location(),
        format!("{SETTINGS}?link_error=way_in_belongs_to_another_account")
    );
    assert_eq!(
        kit.ways_in(&bea).await["items"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        count(&kit, "SELECT count(*) FROM identity.provider_identities").await,
        2
    );
    assert!(kit.google.revoked_tokens().is_empty(), "nothing revoked");
    assert!(kit.intake.adopted().is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn linking_your_own_way_in_again_changes_nothing(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let at_google = link_to(&kit, &access, Provider::Google).await;
    let back = kit.via(Provider::Google, &at_google).await;
    assert_eq!(back.location(), format!("{SETTINGS}?linked=google"));
    assert_eq!(
        kit.ways_in(&access).await["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_last_way_in_cannot_be_removed_and_others_can(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let google = kit.ways_in(&access).await["items"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let path = |id: &str| format!("/api/v1/identity/ways-in/{id}");

    let last = kit.api(Method::DELETE, &path(&google), &access).await;
    assert_eq!(last.status, StatusCode::CONFLICT, "{last:?}");
    assert_eq!(last.json["code"], "last_way_in");

    kit.zoho.set_user(user("zoho-sam"));
    let at_zoho = link_to(&kit, &access, Provider::Zoho).await;
    kit.via(Provider::Zoho, &at_zoho).await;
    let zoho = kit.ways_in(&access).await["items"][1]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    // A stale sign-in can't remove it; nor can anyone else.
    let stale = kit.stale(&access).await;
    let refused = kit.api(Method::DELETE, &path(&zoho), &stale).await;
    assert_eq!(refused.json["code"], "step_up_required");
    // The browser's session is the stale one: sign in again for a fresh one.
    kit.new_browser();
    let fresh = {
        let (url, verifier) = Kit::web_authorize();
        let request = kit.get(&url).await.location_query()["request"].clone();
        let signed_in = kit.round_trip(Some(&request)).await;
        let tokens = kit
            .redeem(&signed_in.location_query()["code"], &verifier)
            .await;
        tokens.json["access_token"].as_str().unwrap().to_owned()
    };
    kit.new_browser();
    kit.google.set_user(user("bea"));
    let bea = kit.signed_up().await;
    let not_hers = kit.api(Method::DELETE, &path(&zoho), &bea).await;
    assert_eq!(not_hers.status, StatusCode::NOT_FOUND);

    let removed = kit.api(Method::DELETE, &path(&zoho), &fresh).await;
    assert_eq!(removed.status, StatusCode::NO_CONTENT, "{removed:?}");
    assert_eq!(
        kit.ways_in(&fresh).await["items"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        count(
            &kit,
            "SELECT count(*) FROM kernel.events WHERE name = 'identity.way_in_removed'"
        )
        .await,
        1
    );
    // Zoho no longer opens the account: it is a stranger again.
    kit.new_browser();
    let stranger = kit.round_trip_with(Provider::Zoho, None).await;
    assert_eq!(stranger.location(), "/sign-in/new");
    assert!(kit.intake.adopted().is_empty(), "no mailbox was touched");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn connecting_a_mailbox_links_it_and_hands_its_grant_over(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let workspace = uuid::Uuid::now_v7();
    kit.google.set_user(user("work-sam"));
    let started = kit
        .start_link(
            &access,
            &json!({ "purpose": "connect", "provider": "google", "workspace_id": workspace,
                     "return_to": "/w/ws" }),
        )
        .await;
    assert_eq!(started.status, StatusCode::CREATED, "{started:?}");
    let to_google = kit.get(started.json["url"].as_str().unwrap()).await;
    let asked = query(&to_google.location());
    assert_eq!(asked["scope"], format!("openid email profile {MAIL_SCOPE}"));
    assert_eq!(asked["access_type"], "offline");
    assert_eq!(asked["prompt"], "consent");

    let back = kit.via(Provider::Google, &to_google.location()).await;
    assert_eq!(back.location(), "/w/ws?connected=connection-1");
    let handed = kit.intake.adopted();
    assert_eq!(handed.len(), 1);
    assert_eq!(handed[0].0.to_string(), workspace.to_string());
    assert_eq!(handed[0].2, "work-sam@example.com");
    assert!(handed[0].3.is_some(), "with its refresh token");
    assert!(handed[0].4.contains(&MAIL_SCOPE.to_owned()));
    assert_eq!(handed[0].5, None);
    let ways = kit.ways_in(&access).await;
    assert_eq!(ways["items"][1]["linked_via"], "connect");

    // Zoho's mailbox can't be connected here (no adapter).
    let zoho = kit
        .start_link(
            &access,
            &json!({ "purpose": "connect", "provider": "zoho", "workspace_id": workspace }),
        )
        .await;
    assert_eq!(zoho.status, StatusCode::CONFLICT);
    assert_eq!(zoho.json["code"], "mailbox_unsupported");
    // A missing workspace is a bad request; an unknown provider is 404.
    let incomplete = kit
        .start_link(
            &access,
            &json!({ "purpose": "connect", "provider": "google" }),
        )
        .await;
    assert_eq!(incomplete.json["code"], "link_incomplete");
    let unknown = kit
        .start_link(&access, &json!({ "purpose": "link", "provider": "yahoo" }))
        .await;
    assert_eq!(unknown.status, StatusCode::NOT_FOUND);
    let off_site = kit
        .start_link(
            &access,
            &json!({ "purpose": "link", "provider": "zoho", "return_to": "https://evil.example.com" }),
        )
        .await;
    assert_eq!(off_site.json["code"], "invalid_return_to");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_shared_mailbox_is_connected_without_becoming_a_way_in(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let workspace = uuid::Uuid::now_v7();
    let connect = async |kit: &Kit| {
        let started = kit
            .start_link(
                &access,
                &json!({ "purpose": "connect", "provider": "google", "workspace_id": workspace,
                         "sign_in": false, "return_to": "/w/ws" }),
            )
            .await;
        assert_eq!(started.status, StatusCode::CREATED, "{started:?}");
        let to_google = kit.get(started.json["url"].as_str().unwrap()).await;
        kit.via(Provider::Google, &to_google.location()).await
    };

    // Unticked for the account you signed up with: still connected.
    let back = connect(&kit).await;
    assert_eq!(back.location(), "/w/ws?connected=connection-1");

    // A shared mailbox: connected, never a way in, nothing announced.
    kit.google.set_user(user("support"));
    let back = connect(&kit).await;
    assert_eq!(back.location(), "/w/ws?connected=connection-2");
    assert_eq!(kit.intake.adopted()[1].2, "support@example.com");
    let ways = kit.ways_in(&access).await;
    assert_eq!(ways["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        count(
            &kit,
            "SELECT count(*) FROM kernel.events WHERE name = 'identity.way_in_added'"
        )
        .await,
        0
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_mailbox_declined_at_the_provider_adds_no_way_in(db: PgPool) {
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
    assert!(kit.intake.adopted().is_empty());
    // Nothing was connected, so the way in this connect made goes again.
    assert_eq!(
        kit.ways_in(&access).await["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn reconnecting_needs_no_step_up_and_names_the_connection(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let stale = kit.stale(&access).await;
    let workspace = uuid::Uuid::now_v7();
    let started = kit
        .start_link(
            &stale,
            &json!({ "purpose": "reconnect", "workspace_id": workspace,
                     "connection_id": RECONNECTABLE, "return_to": "/w/ws" }),
        )
        .await;
    assert_eq!(started.status, StatusCode::CREATED, "{started:?}");
    let to_google = kit.get(started.json["url"].as_str().unwrap()).await;
    let asked = query(&to_google.location());
    assert_eq!(asked["login_hint"], RECONNECTABLE_ADDRESS);
    assert_eq!(asked["prompt"], "consent");
    let back = kit.via(Provider::Google, &to_google.location()).await;
    assert_eq!(back.location(), "/w/ws?connected=connection-1");
    assert_eq!(
        kit.intake.adopted()[0].5.as_deref(),
        Some(RECONNECTABLE),
        "the grant extends the connection"
    );

    let not_yours = kit
        .start_link(
            &stale,
            &json!({ "purpose": "reconnect", "workspace_id": workspace, "connection_id": "c-2" }),
        )
        .await;
    assert_eq!(not_yours.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_consent_door_starts_a_connect_for_other_modules(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let principal = kit.state.auth.verify(&access).await.unwrap();
    let door = crate::consent_door(kit.deps.clone());
    let started = door
        .start(
            &kit.state,
            ConsentRequest {
                principal,
                workspace: campbooks_kernel::tenancy::WorkspaceId::new(),
                provider: "google".to_owned(),
                connection: None,
                capabilities: Vec::new(),
            },
        )
        .await
        .unwrap();
    assert!(
        started.url.starts_with("/auth/start/google?link="),
        "{}",
        started.url
    );
    let to_google = kit.get(&started.url).await;
    assert_eq!(query(&to_google.location())["access_type"], "offline");
    // A mailbox consent asks for nothing besides the mailbox.
    let scope = query(&to_google.location())["scope"].clone();
    assert!(scope.contains(crate::testkit::MAIL_SCOPE), "{scope}");
    assert!(!scope.contains(crate::testkit::CALENDAR_SCOPE), "{scope}");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_consent_that_names_calendar_asks_for_its_scopes_too(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let principal = kit.state.auth.verify(&access).await.unwrap();
    let door = crate::consent_door(kit.deps.clone());
    let started = door
        .start(
            &kit.state,
            ConsentRequest {
                principal,
                workspace: campbooks_kernel::tenancy::WorkspaceId::new(),
                provider: "google".to_owned(),
                connection: None,
                capabilities: vec!["messages".to_owned(), "calendar".to_owned()],
            },
        )
        .await
        .unwrap();
    let to_google = kit.get(&started.url).await;
    let scope = query(&to_google.location())["scope"].clone();
    assert!(scope.contains(crate::testkit::MAIL_SCOPE), "{scope}");
    assert!(scope.contains(crate::testkit::CALENDAR_SCOPE), "{scope}");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn microsoft_binds_the_issuer_to_the_tokens_own_tenant(db: PgPool) {
    let kit = Kit::new(db).await;
    let to_new = kit.round_trip_with(Provider::Microsoft, None).await;
    assert_eq!(to_new.location(), "/sign-in/new", "{to_new:?}");
    let pending = kit.get("/auth/new-account").await;
    assert_eq!(pending.json["provider"], "microsoft");
    kit.create_account().await;
    let issuer: String = sqlx::query_scalar(
        "SELECT issuer FROM identity.provider_identities WHERE provider = 'microsoft'",
    )
    .fetch_one(&kit.state.db)
    .await
    .unwrap();
    assert_eq!(
        issuer,
        format!("{MICROSOFT_ISSUER_PREFIX}{TENANT}{MICROSOFT_ISSUER_SUFFIX}")
    );
    let verified: bool = sqlx::query_scalar(
        "SELECT email_verified_at_link FROM identity.provider_identities WHERE provider = 'microsoft'",
    )
    .fetch_one(&kit.state.db)
    .await
    .unwrap();
    assert!(!verified, "Microsoft's address is not verified");

    // A token whose `iss` names another tenant than its `tid`.
    kit.new_browser();
    kit.microsoft.set_id_token_claims(json!({
        "iss": format!("{MICROSOFT_ISSUER_PREFIX}00000000-0000-0000-0000-000000000000{MICROSOFT_ISSUER_SUFFIX}"),
        "tid": TENANT,
        "email_verified": null,
    }));
    let forged = kit.round_trip_with(Provider::Microsoft, None).await;
    assert_eq!(forged.location(), "/sign-in?error=login_failed");
    // No tenant at all.
    kit.microsoft.set_id_token_claims(json!({
        "iss": format!("{MICROSOFT_ISSUER_PREFIX}{TENANT}{MICROSOFT_ISSUER_SUFFIX}"),
        "tid": null,
    }));
    let tenantless = kit.round_trip_with(Provider::Microsoft, None).await;
    assert_eq!(tenantless.location(), "/sign-in?error=login_failed");
    // A callback naming another issuer (RFC 9207) is refused before the
    // code is redeemed.
    let start = kit.get("/auth/start/microsoft").await;
    let callback = kit.microsoft.authorize(&start.location());
    let before = kit.microsoft.token_requests();
    let mixed_up = kit.get(&callback).await;
    assert_eq!(mixed_up.location(), "/sign-in?error=login_failed");
    assert_eq!(kit.microsoft.token_requests(), before);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn microsofts_address_counts_as_verified_only_with_xms_edov(db: PgPool) {
    let kit = Kit::with_signup(db, SignupPolicy::InviteOnly, &["test-user@example.com"]).await;
    let unverified = kit.round_trip_with(Provider::Microsoft, None).await;
    assert_eq!(unverified.location(), "/sign-in?error=signup_invite_only");

    kit.microsoft.set_id_token_claims(json!({
        "iss": format!("{MICROSOFT_ISSUER_PREFIX}{TENANT}{MICROSOFT_ISSUER_SUFFIX}"),
        "tid": TENANT,
        "xms_edov": true,
        // An `email_verified` claim is never believed from Microsoft.
        "email_verified": null,
    }));
    let verified = kit.round_trip_with(Provider::Microsoft, None).await;
    assert_eq!(verified.location(), "/sign-in/new");

    kit.microsoft.set_id_token_claims(json!({
        "iss": format!("{MICROSOFT_ISSUER_PREFIX}{TENANT}{MICROSOFT_ISSUER_SUFFIX}"),
        "tid": TENANT,
        "email_verified": true,
    }));
    kit.new_browser();
    let claimed = kit.round_trip_with(Provider::Microsoft, None).await;
    assert_eq!(claimed.location(), "/sign-in?error=signup_invite_only");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn zoho_redeems_only_at_a_zoho_accounts_server(db: PgPool) {
    let kit = Kit::new(db).await;
    let start = kit.get("/auth/start/zoho").await;
    let callback = kit.zoho.authorize(&start.location());
    let mut evil = url::Url::parse(&callback).unwrap();
    evil.query_pairs_mut()
        .append_pair("accounts-server", "https://evil.example.com");
    let refused = kit.get(evil.as_str()).await;
    assert_eq!(refused.location(), "/sign-in?error=login_failed");
    assert_eq!(kit.zoho.token_requests(), 0, "no request went anywhere");

    // Without the parameter at all, the same.
    let start = kit.get("/auth/start/zoho").await;
    let bare = kit.get(&kit.zoho.authorize(&start.location())).await;
    assert_eq!(bare.location(), "/sign-in?error=login_failed");
    assert_eq!(kit.zoho.token_requests(), 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_european_zoho_account_signs_in_at_its_data_centre(db: PgPool) {
    let kit = Kit::new(db).await;
    kit.zoho_eu.set_user(user("eu-sam"));
    let start = kit.get("/auth/start/zoho").await;
    let to_new = kit.via_zoho_eu(&start.location()).await;
    assert_eq!(to_new.location(), "/sign-in/new", "{to_new:?}");
    assert_eq!(kit.zoho.token_requests(), 0);
    assert_eq!(kit.zoho_eu.token_requests(), 1);
    kit.create_account().await;
    let issuer: String = sqlx::query_scalar(
        "SELECT issuer FROM identity.provider_identities WHERE provider = 'zoho'",
    )
    .fetch_one(&kit.state.db)
    .await
    .unwrap();
    assert_eq!(issuer, kit.zoho_eu.issuer());
    // Zoho has no mailbox adapter here: no offline access, nothing held.
    assert!(!start.location().contains("access_type"));
    assert_eq!(
        count(&kit, "SELECT count(*) FROM identity.held_grants").await,
        0
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_zoho_sign_up_brings_its_mailbox_and_its_data_centre(db: PgPool) {
    let kit = Kit::new(db).await;
    kit.intake.sync_zoho();
    kit.zoho_eu.set_user(user("eu-sam"));
    let (url, verifier) = Kit::web_authorize();
    let request = kit.get(&url).await.location_query()["request"].clone();
    let start = kit
        .get(&format!("/auth/start/zoho?request={request}"))
        .await;
    // A Zoho sign-in asks who you are only: the mailbox (and a refresh
    // token, which Zoho caps per person) only once the account is new.
    let asked = query(&start.location());
    assert!(!asked.contains_key("access_type"), "{asked:?}");
    assert_eq!(asked["scope"], "openid email profile");
    let again = kit.via_zoho_eu(&start.location()).await;
    let asked = query(&again.location());
    assert_eq!(asked["access_type"], "offline");
    assert_eq!(asked["prompt"], "consent");
    assert_eq!(
        asked["scope"],
        format!("openid email profile {}", ZOHO_MAIL_SCOPES.join(" "))
    );
    kit.via_zoho_eu(&again.location()).await;
    let created = kit.create_account().await;
    assert_eq!(created.status, StatusCode::CREATED, "{created:?}");
    let back = query(created.json["redirect"].as_str().unwrap());
    let tokens = kit.redeem(&back["code"], &verifier).await;
    let access = tokens.json["access_token"].as_str().unwrap().to_owned();
    let held = kit
        .api(Method::GET, "/api/v1/identity/held-grants", &access)
        .await;
    let grant = held.json["items"][0]["id"].as_str().unwrap().to_owned();
    assert_eq!(held.json["items"][0]["provider"], "zoho");
    let workspace = uuid::Uuid::now_v7();
    let adopted = kit
        .api(
            Method::POST,
            &format!("/api/v1/w/{workspace}/identity/held-grants/{grant}/adopt"),
            &access,
        )
        .await;
    assert_eq!(adopted.status, StatusCode::OK, "{adopted:?}");
    // The grant names the data centre that vouched for the account.
    assert_eq!(kit.intake.issuers(), [kit.zoho_eu.issuer()]);
    let handed = kit.intake.adopted();
    assert!(handed[0].3.is_some(), "the refresh token went along");
    for scope in ZOHO_MAIL_SCOPES {
        assert!(handed[0].4.contains(&scope.to_owned()), "{scope}");
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_known_zoho_account_signs_in_without_a_new_refresh_token(db: PgPool) {
    let kit = Kit::new(db).await;
    kit.intake.sync_zoho();
    let access = kit.signed_up().await;
    kit.zoho.set_user(user("zoho-ana"));
    let at_zoho = link_to(&kit, &access, Provider::Zoho).await;
    kit.via(Provider::Zoho, &at_zoho).await;
    // Signing in with that Zoho account again: no mailbox, no offline
    // access, and nothing new is held.
    kit.new_browser();
    let start = kit.get("/auth/start/zoho").await;
    let asked = query(&start.location());
    assert!(!asked.contains_key("access_type"), "{asked:?}");
    assert_eq!(asked["scope"], "openid email profile");
    let signed_in = kit.via(Provider::Zoho, &start.location()).await;
    assert_eq!(signed_in.status, StatusCode::SEE_OTHER, "{signed_in:?}");
    assert!(
        !signed_in.location().starts_with("/sign-in"),
        "{signed_in:?}"
    );
    assert_eq!(
        count(
            &kit,
            "SELECT count(*) FROM identity.held_grants WHERE provider = 'zoho'"
        )
        .await,
        0
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_zoho_refresh_token_campbooks_does_not_keep_is_revoked(db: PgPool) {
    let kit = Kit::new(db).await;
    kit.intake.sync_zoho();
    // Ana's Zoho account is her way in.
    kit.google.set_user(user("ana"));
    let ana = kit.signed_up().await;
    kit.zoho.set_user(user("zoho-ana"));
    let at_zoho = link_to(&kit, &ana, Provider::Zoho).await;
    kit.via(Provider::Zoho, &at_zoho).await;
    // Bea, elsewhere, tries to connect Ana's Zoho mailbox: refused, and the
    // refresh token Zoho issued for it is revoked, not left behind.
    kit.new_browser();
    kit.google.set_user(user("bea"));
    let bea = kit.signed_up().await;
    let started = kit
        .start_link(
            &bea,
            &json!({ "purpose": "connect", "provider": "zoho",
                     "workspace_id": uuid::Uuid::now_v7(), "return_to": SETTINGS }),
        )
        .await;
    assert_eq!(started.status, StatusCode::CREATED, "{started:?}");
    let to_zoho = kit.get(started.json["url"].as_str().unwrap()).await;
    assert!(query(&to_zoho.location()).contains_key("access_type"));
    let before = kit.zoho.revoked_tokens().len();
    let back = kit.via(Provider::Zoho, &to_zoho.location()).await;
    assert_eq!(
        back.location(),
        format!("{SETTINGS}?link_error=way_in_belongs_to_another_account")
    );
    assert_eq!(kit.zoho.revoked_tokens().len(), before + 1);
    assert!(kit.intake.adopted().is_empty());
}

/// Start a `connect` of `provider` without a way in (`sign_in: false`) as
/// `access`, and follow it through the provider: where the browser lands.
async fn connect_shared(kit: &Kit, access: &str, provider: Provider) -> String {
    let started = kit
        .start_link(
            access,
            &json!({ "purpose": "connect", "provider": provider.as_str(), "sign_in": false,
                     "workspace_id": uuid::Uuid::now_v7(), "return_to": SETTINGS }),
        )
        .await;
    assert_eq!(started.status, StatusCode::CREATED, "{started:?}");
    let to_provider = kit.get(started.json["url"].as_str().unwrap()).await;
    kit.via(provider, &to_provider.location()).await.location()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_shared_connect_of_another_accounts_way_in_is_refused(db: PgPool) {
    let kit = Kit::new(db).await;
    // Bea signs up with Google account `x`: it is her way in.
    kit.google.set_user(user("x"));
    let bea = kit.signed_up().await;
    let bea_ways = kit.ways_in(&bea).await;
    // Ana, in another browser, connects `x` as a shared mailbox.
    kit.new_browser();
    kit.google.set_user(user("ana"));
    let ana = kit.signed_up().await;
    kit.google.set_user(user("x"));
    let back = connect_shared(&kit, &ana, Provider::Google).await;

    assert_eq!(
        back,
        format!("{SETTINGS}?link_error=way_in_belongs_to_another_account")
    );
    assert!(kit.intake.adopted().is_empty(), "nothing adopted");
    // A Google token is dropped, never revoked: revoking it would revoke
    // Bea's whole grant to the app (see `revoke_unkept`).
    assert!(kit.google.revoked_tokens().is_empty());
    assert_eq!(kit.ways_in(&bea).await, bea_ways, "Bea's way in untouched");
    assert_eq!(
        kit.ways_in(&ana).await["items"].as_array().unwrap().len(),
        1
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_shared_zoho_connect_of_another_accounts_way_in_revokes_its_token(db: PgPool) {
    let kit = Kit::new(db).await;
    kit.intake.sync_zoho();
    // Bea's Zoho account `zoho-x` is her way in.
    kit.google.set_user(user("bea"));
    let bea = kit.signed_up().await;
    kit.zoho.set_user(user("zoho-x"));
    let at_zoho = link_to(&kit, &bea, Provider::Zoho).await;
    kit.via(Provider::Zoho, &at_zoho).await;
    let bea_ways = kit.ways_in(&bea).await;
    // Ana connects it as a shared mailbox.
    kit.new_browser();
    kit.google.set_user(user("ana"));
    let ana = kit.signed_up().await;
    let before = kit.zoho.revoked_tokens().len();
    let back = connect_shared(&kit, &ana, Provider::Zoho).await;

    assert_eq!(
        back,
        format!("{SETTINGS}?link_error=way_in_belongs_to_another_account")
    );
    assert!(kit.intake.adopted().is_empty(), "nothing adopted");
    assert_eq!(
        kit.zoho.revoked_tokens().len(),
        before + 1,
        "its token revoked"
    );
    assert_eq!(kit.ways_in(&bea).await, bea_ways, "Bea's way in untouched");
}

/// Ways in, live sessions and connections handed over, as counts.
async fn footprint(kit: &Kit) -> (i64, i64, usize) {
    (
        count(kit, "SELECT count(*) FROM identity.provider_identities").await,
        count(
            kit,
            "SELECT count(*) FROM identity.sessions WHERE ended_at IS NULL",
        )
        .await,
        kit.intake.adopted().len(),
    )
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_reconnect_by_another_provider_account_changes_nothing(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let stale = kit.stale(&access).await;
    let before = footprint(&kit).await;
    let started = kit
        .start_link(
            &stale,
            &json!({ "purpose": "reconnect", "workspace_id": uuid::Uuid::now_v7(),
                     "connection_id": RECONNECTABLE, "return_to": "/w/ws" }),
        )
        .await;
    assert_eq!(started.status, StatusCode::CREATED, "{started:?}");
    let to_google = kit.get(started.json["url"].as_str().unwrap()).await;
    // At the provider, someone signs in with another account.
    kit.google.set_user(user("attacker"));
    let back = kit.via(Provider::Google, &to_google.location()).await;
    assert_eq!(back.location(), "/w/ws?link_error=account_mismatch");
    assert_eq!(
        footprint(&kit).await,
        before,
        "no way in, session or connection"
    );
    assert_eq!(
        count(
            &kit,
            "SELECT count(*) FROM kernel.events WHERE name = 'identity.way_in_added'"
        )
        .await,
        0
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn renewing_a_working_grant_needs_a_recent_sign_in(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let stale = kit.stale(&access).await;
    let body = json!({ "purpose": "reconnect", "workspace_id": uuid::Uuid::now_v7(),
                       "connection_id": HEALTHY });
    let refused = kit.start_link(&stale, &body).await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN, "{refused:?}");
    assert_eq!(refused.json["code"], "step_up_required");
    // Signing in again (the same account) makes it recent.
    kit.new_browser();
    let (url, verifier) = Kit::web_authorize();
    let request = kit.get(&url).await.location_query()["request"].clone();
    let signed_in = kit.round_trip(Some(&request)).await;
    let tokens = kit
        .redeem(&signed_in.location_query()["code"], &verifier)
        .await;
    let fresh = tokens.json["access_token"].as_str().unwrap().to_owned();
    let allowed = kit.start_link(&fresh, &body).await;
    assert_eq!(allowed.status, StatusCode::CREATED, "{allowed:?}");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn removing_a_way_in_signs_out_whoever_used_it(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    kit.zoho.set_user(user("zoho-sam"));
    let at_zoho = link_to(&kit, &access, Provider::Zoho).await;
    kit.via(Provider::Zoho, &at_zoho).await;
    let zoho = kit.ways_in(&access).await["items"][1]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    // Another browser signs in with Zoho and holds a refresh token.
    kit.new_browser();
    let (url, verifier) = Kit::web_authorize();
    let request = kit.get(&url).await.location_query()["request"].clone();
    let signed_in = kit.round_trip_with(Provider::Zoho, Some(&request)).await;
    let tokens = kit
        .redeem(&signed_in.location_query()["code"], &verifier)
        .await;
    let refresh = tokens.json["refresh_token"].as_str().unwrap().to_owned();
    let zoho_access = tokens.json["access_token"].as_str().unwrap().to_owned();
    assert_eq!(
        kit.api(Method::GET, "/api/v1/identity/ways-in", &zoho_access)
            .await
            .status,
        StatusCode::OK
    );

    // The owner removes Zoho from the first sign-in (still recent).
    let removed = kit
        .api(
            Method::DELETE,
            &format!("/api/v1/identity/ways-in/{zoho}"),
            &access,
        )
        .await;
    assert_eq!(removed.status, StatusCode::NO_CONTENT, "{removed:?}");

    // That browser's access token stops at once (campbooks-cloud#143),
    // while the owner's own sign-in goes on.
    let stopped = kit
        .api(Method::GET, "/api/v1/identity/ways-in", &zoho_access)
        .await;
    assert_eq!(stopped.status, StatusCode::UNAUTHORIZED, "{stopped:?}");
    assert_eq!(stopped.json["code"], "invalid_token");
    assert_eq!(
        kit.api(Method::GET, "/api/v1/identity/ways-in", &access)
            .await
            .status,
        StatusCode::OK
    );
    // That browser's session and refresh tokens are gone.
    let refreshed = kit.refresh(&refresh).await;
    assert_eq!(refreshed.status, StatusCode::BAD_REQUEST, "{refreshed:?}");
    assert_eq!(refreshed.json["error"], "invalid_grant");
    let (url, _) = Kit::web_authorize();
    assert!(
        kit.get(&url)
            .await
            .location()
            .starts_with("/sign-in?request="),
        "signed out of the issuer too"
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_link_finishes_only_in_the_same_signed_in_browser(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    kit.zoho.set_user(user("zoho-sam"));
    let at_zoho = link_to(&kit, &access, Provider::Zoho).await;
    // While at Zoho, the browser's issuer session ends (signed out
    // elsewhere, or expired).
    sqlx::query("UPDATE identity.sessions SET ended_at = now()")
        .execute(&kit.state.db)
        .await
        .unwrap();
    let back = kit.via(Provider::Zoho, &at_zoho).await;
    assert_eq!(
        back.location(),
        format!("{SETTINGS}?link_error=sign_in_again")
    );
    assert_eq!(
        count(&kit, "SELECT count(*) FROM identity.provider_identities").await,
        1,
        "nothing linked"
    );
}
