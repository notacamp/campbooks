//! End to end through the issuer: a browser signs up and in with a (fake)
//! Google, the web client redeems codes and refreshes, the held mailbox goes
//! to a workspace; and the protocol's refusals, one by one.

use axum::http::{Method, StatusCode};
use campbooks_kernel::{config::SignupPolicy, testing::TestUser};
use sqlx::PgPool;

use crate::{
    domain::clients::{NATIVE_CLIENT_ID, WEB_CLIENT_ID},
    testkit::{Kit, MAIL_SCOPE, ORIGIN, query},
};

#[sqlx::test(migrations = "../../../migrations")]
async fn a_new_person_signs_up_with_google_and_their_mailbox_waits_for_a_workspace(db: PgPool) {
    let kit = Kit::new(db).await;

    // The web client asks; nobody is signed in, so the sign-in screen opens.
    let (url, verifier) = Kit::web_authorize();
    let to_sign_in = kit.get(&url).await;
    assert_eq!(to_sign_in.status, StatusCode::SEE_OTHER);
    assert!(
        to_sign_in.location().starts_with("/sign-in?request="),
        "{to_sign_in:?}"
    );
    let request = to_sign_in.location_query()["request"].clone();
    let shown = kit.get(&format!("/auth/requests/{request}")).await;
    assert_eq!(shown.json["client_id"], WEB_CLIENT_ID);
    assert_eq!(
        shown.json["providers"],
        serde_json::json!(["google", "microsoft", "zoho"])
    );

    // One consent: identity, the mailbox the instance can sync, offline.
    let to_google = kit
        .get(&format!("/auth/start/google?request={request}"))
        .await;
    let asked = query(&to_google.location());
    assert_eq!(asked["scope"], format!("openid email profile {MAIL_SCOPE}"));
    assert_eq!(asked["access_type"], "offline");
    assert_eq!(asked["code_challenge_method"], "S256");

    // Unknown at Campbooks: nothing is created before the person says so.
    let to_new = kit.via_google(&to_google.location()).await;
    assert_eq!(to_new.location(), format!("/sign-in/new?request={request}"));
    let accounts: i64 = sqlx::query_scalar("SELECT count(*) FROM identity.accounts")
        .fetch_one(&kit.state.db)
        .await
        .unwrap();
    assert_eq!(accounts, 0);
    let pending = kit.get("/auth/new-account").await;
    assert_eq!(pending.json["provider"], "google");
    assert_eq!(pending.json["email"], "test-user@example.com");

    // Create: signed in, and the code goes back to the web client.
    let created = kit.create_account().await;
    assert_eq!(created.status, StatusCode::CREATED, "creating the account");
    let back = query(created.json["redirect"].as_str().unwrap());
    assert_eq!(back["state"], "client-state");
    assert_eq!(back["iss"], ORIGIN);
    let tokens = kit.redeem(&back["code"], &verifier).await;
    assert_eq!(tokens.status, StatusCode::OK, "{tokens:?}");
    assert_eq!(tokens.headers["cache-control"], "no-store");
    let access = tokens.json["access_token"].as_str().unwrap().to_owned();

    // The access token is an RFC 9068 JWT the API accepts.
    let (header, claims) = Kit::decode(&access);
    assert_eq!(header["typ"], "at+jwt");
    assert_eq!(header["alg"], "EdDSA");
    assert_eq!(claims["iss"], ORIGIN);
    assert_eq!(claims["client_id"], WEB_CLIENT_ID);
    assert_eq!(claims["email"], "test-user@example.com");
    assert_eq!(claims["email_verified"], true);
    let (_, id) = Kit::decode(tokens.json["id_token"].as_str().unwrap());
    assert_eq!(id["nonce"], "client-nonce");
    assert_eq!(id["aud"], WEB_CLIENT_ID);
    assert_eq!(id["sub"], claims["sub"]);

    // The mailbox waits; adopting it hands the grant to the connectors.
    let held = kit
        .api(Method::GET, "/api/v1/identity/held-grants", &access)
        .await;
    assert_eq!(held.status, StatusCode::OK, "{held:?}");
    let grant = held.json["items"][0].clone();
    assert_eq!(grant["address"], "test-user@example.com");
    let workspace = uuid::Uuid::now_v7();
    let adopted = kit
        .api(
            Method::POST,
            &format!(
                "/api/v1/w/{workspace}/identity/held-grants/{}/adopt",
                grant["id"].as_str().unwrap()
            ),
            &access,
        )
        .await;
    assert_eq!(adopted.status, StatusCode::OK, "{adopted:?}");
    let handed = kit.intake.adopted();
    assert_eq!(handed.len(), 1);
    assert_eq!(handed[0].0.to_string(), workspace.to_string());
    assert_eq!(handed[0].1, claims["sub"].as_str().unwrap());
    assert!(handed[0].3.is_some(), "the refresh token went along");
    assert!(handed[0].4.contains(&MAIL_SCOPE.to_owned()));
    assert_eq!(kit.intake.issuers(), [kit.google.issuer()]);
    let after = kit
        .api(Method::GET, "/api/v1/identity/held-grants", &access)
        .await;
    assert_eq!(after.json["items"], serde_json::json!([]));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_known_identity_signs_in_without_a_new_account_or_a_new_grant(db: PgPool) {
    let kit = Kit::new(db).await;
    let (code, verifier) = kit.sign_up().await;
    let first = kit.redeem(&code, &verifier).await;
    let (_, first) = Kit::decode(first.json["access_token"].as_str().unwrap());

    kit.new_browser();
    let (url, verifier) = Kit::web_authorize();
    let request = kit.get(&url).await.location_query()["request"].clone();
    let back = kit.round_trip(Some(&request)).await;
    assert!(
        back.location()
            .starts_with(&format!("{ORIGIN}/bff/callback?code=")),
        "{back:?}"
    );
    let tokens = kit.redeem(&back.location_query()["code"], &verifier).await;
    let (_, again) = Kit::decode(tokens.json["access_token"].as_str().unwrap());
    assert_eq!(again["sub"], first["sub"]);
    let held: i64 = sqlx::query_scalar("SELECT count(*) FROM identity.held_grants")
        .fetch_one(&kit.state.db)
        .await
        .unwrap();
    assert_eq!(held, 1, "the sign-up's grant only");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_signed_in_browser_gets_a_code_at_once_unless_the_client_asks_otherwise(db: PgPool) {
    let kit = Kit::new(db).await;
    kit.sign_up().await;

    let (url, _) = Kit::web_authorize();
    let at_once = kit.get(&url).await;
    assert!(
        at_once.location().contains("/bff/callback?code="),
        "{at_once:?}"
    );

    let login = kit.get(&format!("{url}&prompt=login")).await;
    assert!(
        login.location().starts_with("/sign-in?request="),
        "{login:?}"
    );

    kit.new_browser();
    let none = kit.get(&format!("{url}&prompt=none")).await;
    let none = none.location_query();
    assert_eq!(none["error"], "login_required");
    assert_eq!(none["state"], "client-state");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_bad_client_or_redirect_is_shown_never_redirected(db: PgPool) {
    let kit = Kit::new(db).await;
    let (url, _) = Kit::authorize_url("someone-else", &format!("{ORIGIN}/bff/callback"), "openid");
    let response = kit.get(&url).await;
    assert_eq!(response.status, StatusCode::BAD_REQUEST);
    assert_eq!(response.json["code"], "invalid_client");
    assert!(response.headers.get("location").is_none());

    let (url, _) = Kit::authorize_url(WEB_CLIENT_ID, "https://evil.example.com/cb", "openid");
    let response = kit.get(&url).await;
    assert_eq!(response.status, StatusCode::BAD_REQUEST);
    assert_eq!(response.json["code"], "invalid_redirect_uri");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn other_authorization_errors_go_back_to_the_client(db: PgPool) {
    let kit = Kit::new(db).await;
    let (url, _) = Kit::web_authorize();
    for (edit, error) in [
        (
            url.replace("code_challenge_method=S256", "code_challenge_method=plain"),
            "invalid_request",
        ),
        (
            url.replace("response_type=code", "response_type=token"),
            "unsupported_response_type",
        ),
        (
            url.replace("scope=openid", "scope=openid+admin%3Aeverything"),
            "invalid_scope",
        ),
    ] {
        let response = kit.get(&edit).await;
        let back = response.location_query();
        assert_eq!(back["error"], error, "{edit}");
        assert_eq!(back["iss"], ORIGIN);
        assert!(
            response
                .location()
                .starts_with(&format!("{ORIGIN}/bff/callback?"))
        );
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn codes_are_single_use_and_bound_to_the_pkce_verifier(db: PgPool) {
    let kit = Kit::new(db).await;
    let (code, verifier) = kit.sign_up().await;

    let wrong = kit.redeem(&code, &"x".repeat(43)).await;
    assert_eq!(wrong.json["error"], "invalid_grant");
    let tokens = kit.redeem(&code, &verifier).await;
    assert_eq!(
        tokens.status,
        StatusCode::OK,
        "a failed attempt does not spend the code"
    );
    let refresh = tokens.json["refresh_token"].as_str().unwrap().to_owned();

    let replay = kit.redeem(&code, &verifier).await;
    assert_eq!(replay.status, StatusCode::BAD_REQUEST);
    assert_eq!(replay.json["error"], "invalid_grant");
    // What the first redemption issued is revoked.
    assert_eq!(kit.refresh(&refresh).await.json["error"], "invalid_grant");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn refresh_tokens_rotate_and_reuse_revokes_the_family(db: PgPool) {
    let kit = Kit::new(db).await;
    let (code, verifier) = kit.sign_up().await;
    let r1 = kit.redeem(&code, &verifier).await.json["refresh_token"]
        .as_str()
        .unwrap()
        .to_owned();

    let second = kit.refresh(&r1).await;
    assert_eq!(second.status, StatusCode::OK, "{second:?}");
    let r2 = second.json["refresh_token"].as_str().unwrap().to_owned();
    assert_ne!(r1, r2);

    let reuse = kit.refresh(&r1).await;
    assert_eq!(reuse.json["error"], "invalid_grant");
    assert_eq!(
        kit.refresh(&r2).await.json["error"],
        "invalid_grant",
        "the family is revoked"
    );
    let audited: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM identity.audit_events WHERE kind = 'refresh_reuse_detected'",
    )
    .fetch_one(&kit.state.db)
    .await
    .unwrap();
    assert_eq!(audited, 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_token_endpoint_authenticates_clients(db: PgPool) {
    let kit = Kit::new(db).await;
    let (code, verifier) = kit.sign_up().await;
    let bad = kit
        .post_form(
            "/oauth2/token",
            &[
                ("grant_type", "authorization_code"),
                ("code", &code),
                ("redirect_uri", &format!("{ORIGIN}/bff/callback")),
                ("code_verifier", &verifier),
                ("client_id", WEB_CLIENT_ID),
                ("client_secret", "guess"),
            ],
        )
        .await;
    assert_eq!(bad.status, StatusCode::UNAUTHORIZED);
    assert_eq!(bad.json["error"], "invalid_client");

    // HTTP Basic works too.
    let basic = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        format!("{WEB_CLIENT_ID}:{}", Kit::web_secret()),
    );
    let body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("grant_type", "authorization_code")
        .append_pair("code", &code)
        .append_pair("redirect_uri", &format!("{ORIGIN}/bff/callback"))
        .append_pair("code_verifier", &verifier)
        .finish();
    let ok = kit
        .request(
            Method::POST,
            "/oauth2/token",
            &[
                ("content-type", "application/x-www-form-urlencoded"),
                ("authorization", &format!("Basic {basic}")),
            ],
            Some(body),
        )
        .await;
    assert_eq!(ok.status, StatusCode::OK, "{ok:?}");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_native_client_is_refused_until_native_sign_in_ships(db: PgPool) {
    let kit = Kit::new(db).await;
    kit.sign_up().await;
    // A signed-in browser must not hand a code to whatever listens on a
    // loopback port (RFC 8252 §8.6).
    let (url, verifier) = Kit::authorize_url(
        NATIVE_CLIENT_ID,
        "http://127.0.0.1:53124/callback",
        "openid offline_access",
    );
    let refused = kit.get(&url).await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST);
    assert_eq!(refused.json["code"], "invalid_client");
    assert!(refused.headers.get("location").is_none());
    let token = kit
        .post_form(
            "/oauth2/token",
            &[
                ("grant_type", "authorization_code"),
                ("code", "anything"),
                ("redirect_uri", "http://127.0.0.1:53124/callback"),
                ("code_verifier", &verifier),
                ("client_id", NATIVE_CLIENT_ID),
            ],
        )
        .await;
    assert_eq!(token.status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn round_trips_are_single_use_and_bound_to_their_browser(db: PgPool) {
    let kit = Kit::new(db).await;
    let to_google = kit.get("/auth/start/google").await;
    let callback = kit.google.authorize(&to_google.location());

    kit.new_browser();
    let foreign = kit.get(&callback).await;
    assert_eq!(foreign.location(), "/sign-in?error=invalid_state");

    let to_google = kit.get("/auth/start/google").await;
    let callback = kit.google.authorize(&to_google.location());
    let first = kit.get(&callback).await;
    assert!(first.location().starts_with("/sign-in/new"), "{first:?}");
    let replay = kit.get(&callback).await;
    assert_eq!(replay.location(), "/sign-in?error=invalid_state");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn google_is_sent_the_route_campbooks_serves_unless_a_redirect_path_is_set(db: PgPool) {
    let kit = Kit::new(db).await;
    let to_google = kit.get("/auth/start/google").await;
    assert_eq!(
        query(&to_google.location())["redirect_uri"],
        format!("{ORIGIN}/auth/callback/google")
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_configured_redirect_path_is_sent_in_both_the_authorization_and_the_exchange(db: PgPool) {
    let kit = Kit::with_google_redirect_path(db, "/oauth/gmail/callback").await;
    let to_google = kit.get("/auth/start/google").await;
    assert_eq!(
        query(&to_google.location())["redirect_uri"],
        format!("{ORIGIN}/oauth/gmail/callback")
    );

    println!(
        "authorize  redirect_uri = {}",
        query(&to_google.location())["redirect_uri"]
    );
    // Google redirects the browser to the registered path; the proxy rewrites
    // it to the route Campbooks serves.
    let registered = kit.google.authorize(&to_google.location());
    assert!(
        registered.starts_with(&format!("{ORIGIN}/oauth/gmail/callback?")),
        "{registered}"
    );
    let proxied = registered.replace("/oauth/gmail/callback", "/auth/callback/google");
    // The fake Google redeems the code only when the exchange repeats the
    // authorization's redirect_uri, as the real one does.
    let back = kit.get(&proxied).await;
    assert!(back.location().starts_with("/sign-in/new"), "{back:?}");
    println!(
        "exchange   redirect_uri = same, fake Google redeemed the code -> {}",
        back.location()
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn cancelling_at_the_provider_returns_to_the_sign_in_screen(db: PgPool) {
    let kit = Kit::new(db).await;
    let to_google = kit.get("/auth/start/google?request=r1").await;
    let state = query(&to_google.location())["state"].clone();
    let back = kit
        .get(&format!(
            "/auth/callback/google?error=access_denied&state={state}"
        ))
        .await;
    assert_eq!(
        back.location(),
        "/sign-in?error=cancelled_at_provider&request=r1"
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_new_account_without_a_refresh_token_asks_google_for_consent_once(db: PgPool) {
    let kit = Kit::new(db).await;
    kit.google.set_offline(false);
    let back = kit.round_trip(None).await;
    let again = query(&back.location());
    assert_eq!(again["prompt"], "consent", "{back:?}");
    assert_eq!(again["login_hint"], "test-user");

    // Still none: the account is offered anyway, with no mailbox held.
    let to_new = kit.via_google(&back.location()).await;
    assert!(to_new.location().starts_with("/sign-in/new"), "{to_new:?}");
    assert_eq!(kit.create_account().await.status, StatusCode::CREATED);
    let held: i64 = sqlx::query_scalar("SELECT count(*) FROM identity.held_grants")
        .fetch_one(&kit.state.db)
        .await
        .unwrap();
    assert_eq!(held, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_declined_mail_permission_signs_up_without_a_mailbox(db: PgPool) {
    let kit = Kit::new(db).await;
    let to_google = kit.get("/auth/start/google").await;
    let without_mail = to_google.location().replace(
        &format!(
            "+{}",
            url::form_urlencoded::byte_serialize(MAIL_SCOPE.as_bytes()).collect::<String>()
        ),
        "",
    );
    let to_new = kit.via_google(&without_mail).await;
    assert!(to_new.location().starts_with("/sign-in/new"), "{to_new:?}");
    assert_eq!(kit.create_account().await.status, StatusCode::CREATED);
    let held: i64 = sqlx::query_scalar("SELECT count(*) FROM identity.held_grants")
        .fetch_one(&kit.state.db)
        .await
        .unwrap();
    assert_eq!(held, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn creating_an_account_needs_the_csrf_header_and_the_pending_browser(db: PgPool) {
    let kit = Kit::new(db).await;
    kit.round_trip(None).await;
    let no_header = kit
        .request(
            Method::POST,
            "/auth/new-account",
            &[("sec-fetch-site", "same-origin")],
            None,
        )
        .await;
    assert_eq!(no_header.status, StatusCode::FORBIDDEN);
    assert_eq!(no_header.json["code"], "csrf_check_failed");
    kit.new_browser();
    assert_eq!(kit.create_account().await.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn two_people_with_the_same_email_get_two_accounts(db: PgPool) {
    let kit = Kit::new(db).await;
    let (code, verifier) = kit.sign_up().await;
    let (_, ana) = Kit::decode(
        kit.redeem(&code, &verifier).await.json["access_token"]
            .as_str()
            .unwrap(),
    );

    // Another Google account claiming the same address is never linked by it.
    kit.new_browser();
    kit.google.set_user(TestUser {
        subject: "someone-else".to_owned(),
        ..TestUser::default()
    });
    let (code, verifier) = kit.sign_up().await;
    let (_, other) = Kit::decode(
        kit.redeem(&code, &verifier).await.json["access_token"]
            .as_str()
            .unwrap(),
    );
    assert_ne!(ana["sub"], other["sub"]);

    // The operator's CLI finds both by the address, so it refuses to guess.
    let mut conn = kit.state.db.acquire().await.unwrap();
    let found = crate::domain::accounts::operator_lookup(&mut conn, " Test-User@Example.com")
        .await
        .unwrap();
    let mut expected = [ana["sub"].as_str().unwrap(), other["sub"].as_str().unwrap()];
    expected.sort_unstable();
    let found: Vec<String> = found.iter().map(ToString::to_string).collect();
    assert_eq!(found, expected);
    let (name, email) = crate::domain::accounts::describe(&mut conn, found[0].parse().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (name.as_deref(), email.as_deref()),
        (Some("Test User"), Some("test-user@example.com"))
    );
    assert!(
        crate::domain::accounts::operator_lookup(&mut conn, "nobody@example.com")
            .await
            .unwrap()
            .is_empty()
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn logout_ends_the_session_and_its_refresh_tokens(db: PgPool) {
    let kit = Kit::new(db).await;
    let (code, verifier) = kit.sign_up().await;
    let tokens = kit.redeem(&code, &verifier).await;
    let refresh = tokens.json["refresh_token"].as_str().unwrap().to_owned();
    let hint = tokens.json["id_token"].as_str().unwrap().to_owned();
    let out = kit
        .get(&format!(
            "/oauth2/end_session?id_token_hint={hint}&client_id={WEB_CLIENT_ID}&post_logout_redirect_uri={ORIGIN}/"
        ))
        .await;
    assert_eq!(out.location(), format!("{ORIGIN}/"));
    assert_eq!(kit.refresh(&refresh).await.json["error"], "invalid_grant");
    let (url, _) = Kit::web_authorize();
    assert!(kit.get(&url).await.location().starts_with("/sign-in?"));

    // Somewhere it did not register: signed out, not redirected.
    let elsewhere = kit
        .get(&format!(
            "/oauth2/end_session?id_token_hint={hint}&post_logout_redirect_uri=https://evil.example.com/"
        ))
        .await;
    assert_eq!(elsewhere.status, StatusCode::NO_CONTENT);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn revocation_ends_a_refresh_family_and_ignores_unknown_tokens(db: PgPool) {
    let kit = Kit::new(db).await;
    let (code, verifier) = kit.sign_up().await;
    let refresh = kit.redeem(&code, &verifier).await.json["refresh_token"]
        .as_str()
        .unwrap()
        .to_owned();
    let secret = Kit::web_secret();
    for token in ["not-a-token", refresh.as_str()] {
        let revoked = kit
            .post_form(
                "/oauth2/revoke",
                &[
                    ("token", token),
                    ("client_id", WEB_CLIENT_ID),
                    ("client_secret", &secret),
                ],
            )
            .await;
        assert_eq!(revoked.status, StatusCode::OK);
    }
    assert_eq!(kit.refresh(&refresh).await.json["error"], "invalid_grant");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn discovery_and_keys_describe_the_issuer(db: PgPool) {
    let kit = Kit::new(db).await;
    for path in [
        "/.well-known/openid-configuration",
        "/.well-known/oauth-authorization-server",
    ] {
        let metadata = kit.get(path).await.json;
        assert_eq!(metadata["issuer"], ORIGIN);
        assert_eq!(
            metadata["code_challenge_methods_supported"],
            serde_json::json!(["S256"])
        );
        assert_eq!(metadata["token_endpoint"], format!("{ORIGIN}/oauth2/token"));
        assert_eq!(
            metadata["authorization_response_iss_parameter_supported"],
            true
        );
    }
    let (code, verifier) = kit.sign_up().await;
    let access = kit.redeem(&code, &verifier).await.json["access_token"]
        .as_str()
        .unwrap()
        .to_owned();
    let jwks = kit.get("/oauth2/jwks").await.json;
    let (header, _) = Kit::decode(&access);
    let key = &jwks["keys"][0];
    assert_eq!(key["kid"], header["kid"]);
    assert_eq!(
        (key["kty"].as_str(), key["crv"].as_str()),
        (Some("OKP"), Some("Ed25519"))
    );
    assert!(key.get("d").is_none(), "no private part");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn nothing_answers_while_another_identity_provider_serves_the_instance(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    let app = crate::protocol::router(kit.state.clone(), crate::Deps::default());
    for path in [
        "/.well-known/openid-configuration",
        "/oauth2/jwks",
        "/auth/start/google",
    ] {
        let response = campbooks_kernel::testing::get(&app, path).await;
        assert_eq!(response.status, StatusCode::NOT_FOUND, "{path}");
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn open_code_and_approval_instances_let_new_accounts_in(db: PgPool) {
    // Their gate is creating a workspace (a code, or an administrator).
    for policy in [
        SignupPolicy::Open,
        SignupPolicy::Code,
        SignupPolicy::Approval,
    ] {
        let kit = Kit::with_signup(db.clone(), policy, &[]).await;
        kit.new_browser();
        kit.google.set_user(TestUser {
            subject: format!("{policy:?}"),
            ..TestUser::default()
        });
        let to_new = kit.round_trip(None).await;
        assert!(to_new.location().starts_with("/sign-in/new"), "{policy:?}");
        assert_eq!(
            kit.create_account().await.status,
            StatusCode::CREATED,
            "{policy:?}"
        );
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_invite_only_instance_keeps_nothing_of_an_uninvited_person(db: PgPool) {
    let kit = Kit::with_signup(db, SignupPolicy::InviteOnly, &["someone-else@example.com"]).await;
    let to_google = kit.get("/auth/start/google?request=r%201").await;
    let refused = kit.via_google(&to_google.location()).await;
    assert_eq!(
        refused.location(),
        "/sign-in?error=signup_invite_only&request=r+1"
    );
    for count in [
        "SELECT count(*) FROM identity.accounts",
        "SELECT count(*) FROM identity.pending_sign_ups",
        "SELECT count(*) FROM identity.held_grants",
        "SELECT count(*) FROM identity.sessions",
    ] {
        let rows: i64 = sqlx::query_scalar(count)
            .fetch_one(&kit.state.db)
            .await
            .unwrap();
        assert_eq!(rows, 0, "{count}");
    }
    assert_eq!(kit.create_account().await.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_invite_only_instance_lets_an_invited_verified_address_in(db: PgPool) {
    let kit = Kit::with_signup(db, SignupPolicy::InviteOnly, &["test-user@example.com"]).await;
    let to_new = kit.round_trip(None).await;
    assert!(to_new.location().starts_with("/sign-in/new"), "{to_new:?}");
    assert_eq!(kit.create_account().await.status, StatusCode::CREATED);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_pending_sign_up_is_refused_if_the_invitation_went_meanwhile(db: PgPool) {
    // The pending sign-up was made while invited; the policy is checked
    // again when the account would be created.
    let open = Kit::with_signup(db.clone(), SignupPolicy::Open, &[]).await;
    open.round_trip(None).await;
    let strict = Kit::with_signup(db, SignupPolicy::InviteOnly, &[]).await;
    strict.share_cookies_with(&open);
    let refused = strict.create_account().await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN);
    assert_eq!(refused.json["code"], "signup_invite_only");
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM identity.pending_sign_ups")
        .fetch_one(&strict.state.db)
        .await
        .unwrap();
    assert_eq!(left, 0, "the refused sign-up and its tokens are gone");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn sign_out_needs_an_id_token_of_this_issuer(db: PgPool) {
    let kit = Kit::new(db).await;
    let (code, verifier) = kit.sign_up().await;
    let tokens = kit.redeem(&code, &verifier).await;
    let refresh = tokens.json["refresh_token"].as_str().unwrap().to_owned();
    let id_token = tokens.json["id_token"].as_str().unwrap().to_owned();

    // Another site linking or posting here signs nobody out.
    for forged in [
        format!("/oauth2/end_session?client_id={WEB_CLIENT_ID}&post_logout_redirect_uri={ORIGIN}/"),
        "/oauth2/end_session?id_token_hint=not.a.token".to_owned(),
        format!(
            "/oauth2/end_session?id_token_hint={}",
            kit.google
                .id_token(&kit.google.id_token_claims("x", "n", "s"))
        ),
    ] {
        let refused = kit.get(&forged).await;
        assert_eq!(refused.status, StatusCode::BAD_REQUEST, "{forged}");
        assert_eq!(refused.json["code"], "invalid_id_token_hint");
    }
    let still = kit.refresh(&refresh).await;
    assert_eq!(still.status, StatusCode::OK, "the session survived");
    let refresh = still.json["refresh_token"].as_str().unwrap().to_owned();

    // The client's own hint (also as a form POST) ends it.
    let out = kit
        .post_form(
            "/oauth2/end_session",
            &[("id_token_hint", &id_token), ("client_id", WEB_CLIENT_ID)],
        )
        .await;
    assert_eq!(out.status, StatusCode::NO_CONTENT);
    assert_eq!(kit.refresh(&refresh).await.json["error"], "invalid_grant");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn adopting_a_mailbox_needs_the_connectors_write_scope_too(db: PgPool) {
    let kit = Kit::new(db).await;
    let (url, verifier) = Kit::authorize_url(
        WEB_CLIENT_ID,
        &format!("{ORIGIN}/bff/callback"),
        "openid email identity:read identity:write",
    );
    let request = kit.get(&url).await.location_query()["request"].clone();
    kit.round_trip(Some(&request)).await;
    let created = kit.create_account().await;
    let code = query(created.json["redirect"].as_str().unwrap())["code"].clone();
    let access = kit.redeem(&code, &verifier).await.json["access_token"]
        .as_str()
        .unwrap()
        .to_owned();
    let held = kit
        .api(Method::GET, "/api/v1/identity/held-grants", &access)
        .await;
    let grant = held.json["items"][0]["id"].as_str().unwrap().to_owned();
    let refused = kit
        .api(
            Method::POST,
            &format!(
                "/api/v1/w/{}/identity/held-grants/{grant}/adopt",
                uuid::Uuid::now_v7()
            ),
            &access,
        )
        .await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN);
    assert!(kit.intake.adopted().is_empty());
}
