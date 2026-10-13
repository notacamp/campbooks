//! Clients other than the web app, end to end (slice S4): the native app's
//! consent on every authorization, third parties' consent (remembered only
//! for `https` redirects) bound to one workspace, a workspace's own clients
//! and `client_credentials`, dynamic registration, metadata documents,
//! resource indicators, and every refusal.

use axum::http::{Method, StatusCode};
use sqlx::PgPool;

use crate::{
    domain::clients::NATIVE_CLIENT_ID,
    testkit::{FOREIGN, HOME, Kit, MEMBER_IN_HOME, MEMBER_ONLY, ORIGIN, Response, WORK, query},
};

const LOOPBACK: &str = "http://127.0.0.1:53124/callback";
const AGENT_CALLBACK: &str = "https://agent.example.com/callback";
const RESOURCE: &str = "http://localhost:8080/api";

/// Follow `url` (an authorization) to the consent screen; the request id.
async fn to_consent(kit: &Kit, url: &str) -> String {
    let response = kit.get(url).await;
    assert_eq!(response.status, StatusCode::SEE_OTHER, "{response:?}");
    assert!(
        response.location().starts_with("/consent?request="),
        "{response:?}"
    );
    response.location_query()["request"].clone()
}

/// The consent screen's answer for request `id`.
async fn answer(kit: &Kit, id: &str, body: &serde_json::Value) -> Response {
    kit.request(
        Method::POST,
        &format!("/auth/consent/{id}"),
        &[
            ("x-campbooks-csrf", "1"),
            ("sec-fetch-site", "same-origin"),
            ("content-type", "application/json"),
        ],
        Some(body.to_string()),
    )
    .await
}

/// Allow request `id` (in `workspace`, when given); the code.
async fn allow(kit: &Kit, id: &str, workspace: Option<uuid::Uuid>) -> String {
    let answered = answer(
        kit,
        id,
        &serde_json::json!({ "decision": "allow", "workspace_id": workspace }),
    )
    .await;
    assert_eq!(answered.status, StatusCode::OK, "{answered:?}");
    query(answered.json["redirect"].as_str().unwrap_or_default())["code"].clone()
}

/// Redeem `code` as public client `client` at `redirect`.
async fn redeem_public(
    kit: &Kit,
    client: &str,
    code: &str,
    redirect: &str,
    verifier: &str,
) -> Response {
    kit.post_form(
        "/oauth2/token",
        &[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect),
            ("code_verifier", verifier),
            ("client_id", client),
        ],
    )
    .await
}

/// How many apps the person behind `web` has let in.
async fn live(kit: &Kit, web: &str) -> usize {
    let apps = kit.api(Method::GET, "/api/v1/identity/consents", web).await;
    apps.json["items"].as_array().map_or(0, Vec::len)
}

/// Register a client dynamically.
async fn register(kit: &Kit, body: &serde_json::Value) -> Response {
    kit.request(
        Method::POST,
        "/oauth2/register",
        &[("content-type", "application/json")],
        Some(body.to_string()),
    )
    .await
}

/// A dynamically registered agent with an `https` callback; its id.
async fn agent(kit: &Kit, scope: &str) -> String {
    let registered = register(
        kit,
        &serde_json::json!({
            "client_name": "Helpful Agent",
            "redirect_uris": [AGENT_CALLBACK, "http://localhost:3334/callback"],
            "grant_types": ["authorization_code", "refresh_token"],
            "scope": scope,
        }),
    )
    .await;
    assert_eq!(registered.status, StatusCode::CREATED, "{registered:?}");
    registered.json["client_id"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_native_app_is_asked_for_every_time_and_works_in_every_workspace(db: PgPool) {
    let kit = Kit::new(db).await;
    kit.sign_up().await;

    let (url, verifier) = Kit::authorize_url(
        NATIVE_CLIENT_ID,
        LOOPBACK,
        "openid offline_access identity:read",
    );
    let id = to_consent(&kit, &url).await;
    let shown = kit.get(&format!("/auth/consent/{id}")).await;
    assert_eq!(shown.status, StatusCode::OK, "{shown:?}");
    assert_eq!(
        shown.json["client"]["name"],
        "Campbooks for desktop and mobile"
    );
    assert_eq!(shown.json["client"]["verified"], true);
    assert_eq!(shown.json["client"]["device"], true);
    assert_eq!(shown.json["client"]["redirect_host"], "127.0.0.1");
    assert_eq!(shown.json["binds_workspace"], false);
    assert_eq!(shown.json["remembered"], false);
    assert_eq!(shown.json["workspaces"], serde_json::json!([]));
    assert_eq!(shown.json["account_email"], "test-user@example.com");

    // Another site cannot answer for the person.
    let forged = kit
        .request(
            Method::POST,
            &format!("/auth/consent/{id}"),
            &[("content-type", "application/json")],
            Some(serde_json::json!({ "decision": "allow" }).to_string()),
        )
        .await;
    assert_eq!(forged.status, StatusCode::FORBIDDEN);
    assert_eq!(forged.json["code"], "csrf_check_failed");

    let code = allow(&kit, &id, None).await;
    let tokens = redeem_public(&kit, NATIVE_CLIENT_ID, &code, LOOPBACK, &verifier).await;
    assert_eq!(tokens.status, StatusCode::OK, "{tokens:?}");
    assert!(tokens.json["refresh_token"].is_string());
    let (_, claims) = Kit::decode(tokens.json["access_token"].as_str().unwrap_or_default());
    assert_eq!(claims["aud"], RESOURCE);
    assert_eq!(claims["client_id"], NATIVE_CLIENT_ID);
    assert!(claims.get("workspace_id").is_none(), "{claims}");
    // It works: the API accepts the token.
    let held = kit
        .api(
            Method::GET,
            "/api/v1/identity/held-grants",
            tokens.json["access_token"].as_str().unwrap_or_default(),
        )
        .await;
    assert_eq!(held.status, StatusCode::OK, "{held:?}");

    // A loopback redirect is never remembered: a local process could claim
    // it (RFC 8252 §8.6). The next authorization asks again; denying sends
    // `access_denied`.
    let (again, _) = Kit::authorize_url(NATIVE_CLIENT_ID, "http://[::1]:9999/callback", "openid");
    let id = to_consent(&kit, &again).await;
    let denied = answer(&kit, &id, &serde_json::json!({ "decision": "deny" })).await;
    let back = query(denied.json["redirect"].as_str().unwrap_or_default());
    assert_eq!(back["error"], "access_denied");
    assert_eq!(back["state"], "client-state");
    assert!(!back.contains_key("code"));
    // The request is spent either way.
    let replayed = answer(&kit, &id, &serde_json::json!({ "decision": "allow" })).await;
    assert_eq!(replayed.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_consent_screen_follows_a_recent_sign_in(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    kit.stale(&access).await;
    let (url, _) = Kit::authorize_url(NATIVE_CLIENT_ID, LOOPBACK, "openid");
    let to_sign_in = kit.get(&url).await;
    assert!(
        to_sign_in.location().starts_with("/sign-in?request="),
        "{to_sign_in:?}"
    );
    let request = to_sign_in.location_query()["request"].clone();
    // Signing in again leads to the screen.
    let back = kit.round_trip(Some(&request)).await;
    assert!(back.location().starts_with("/consent?request="), "{back:?}");
    let id = back.location_query()["request"].clone();

    // The screen sat open for an hour: Allow asks for a fresh sign-in.
    let stale = chrono::Utc::now() - chrono::TimeDelta::hours(1);
    sqlx::query("UPDATE identity.sessions SET auth_time = $1")
        .bind(stale)
        .execute(&kit.state.db)
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let refused = answer(&kit, &id, &serde_json::json!({ "decision": "allow" })).await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN);
    assert_eq!(refused.json["code"], "step_up_required");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_consent_request_is_only_its_persons(db: PgPool) {
    let kit = Kit::new(db).await;
    kit.sign_up().await;
    let (url, _) = Kit::authorize_url(NATIVE_CLIENT_ID, LOOPBACK, "openid");
    let id = to_consent(&kit, &url).await;
    // Another browser (signed in as someone else, or not at all) sees nothing.
    kit.new_browser();
    assert_eq!(
        kit.get(&format!("/auth/consent/{id}")).await.status,
        StatusCode::NOT_FOUND
    );
    kit.google.set_user(campbooks_kernel::testing::TestUser {
        subject: "someone-else".to_owned(),
        email: "bea@example.com".to_owned(),
        ..campbooks_kernel::testing::TestUser::default()
    });
    kit.sign_up().await;
    assert_eq!(
        kit.get(&format!("/auth/consent/{id}")).await.status,
        StatusCode::NOT_FOUND
    );
    let answered = answer(&kit, &id, &serde_json::json!({ "decision": "allow" })).await;
    assert_eq!(answered.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_third_party_is_bound_to_the_workspace_the_person_picks_and_remembered(db: PgPool) {
    let kit = Kit::new(db).await;
    let web = kit.signed_up().await;
    let client = agent(&kit, "conversations:read").await;
    let (url, verifier) = Kit::authorize_url(&client, AGENT_CALLBACK, "conversations:read");
    let id = to_consent(&kit, &url).await;
    let shown = kit.get(&format!("/auth/consent/{id}")).await;
    assert_eq!(shown.json["client"]["name"], "Helpful Agent");
    assert_eq!(shown.json["client"]["verified"], false);
    assert_eq!(shown.json["client"]["kind"], "dynamic");
    assert_eq!(shown.json["client"]["device"], false);
    assert_eq!(shown.json["client"]["redirect_host"], "agent.example.com");
    assert_eq!(shown.json["binds_workspace"], true);
    assert_eq!(shown.json["remembered"], true);
    assert_eq!(shown.json["workspaces"][0]["name"], "Home");
    assert_eq!(shown.json["workspaces"][1]["name"], "Work");
    assert_eq!(shown.json["scopes"][0]["name"], "conversations:read");
    assert_eq!(
        shown.json["scopes"][0]["description"],
        "Read your conversations."
    );

    // It must name a workspace the person belongs to.
    let none = answer(&kit, &id, &serde_json::json!({ "decision": "allow" })).await;
    assert_eq!(none.status, StatusCode::BAD_REQUEST);
    assert_eq!(none.json["code"], "consent_needs_workspace");
    let foreign = answer(
        &kit,
        &id,
        &serde_json::json!({ "decision": "allow", "workspace_id": FOREIGN }),
    )
    .await;
    assert_eq!(foreign.status, StatusCode::NOT_FOUND);

    let code = allow(&kit, &id, Some(HOME)).await;
    let tokens = redeem_public(&kit, &client, &code, AGENT_CALLBACK, &verifier).await;
    assert_eq!(tokens.status, StatusCode::OK, "{tokens:?}");
    let (_, claims) = Kit::decode(tokens.json["access_token"].as_str().unwrap_or_default());
    assert_eq!(claims["workspace_id"], HOME.to_string());
    assert_eq!(claims["aud"], RESOURCE);
    assert_eq!(claims["scope"], "conversations:read");
    assert!(claims.get("email").is_none());
    assert!(
        tokens.json.get("refresh_token").is_none(),
        "no offline_access"
    );

    // Remembered for an https redirect: the next one needs no screen and
    // stays in that workspace.
    let (again, _) = Kit::authorize_url(&client, AGENT_CALLBACK, "conversations:read");
    let at_once = kit.get(&again).await;
    assert!(
        at_once.location().starts_with(AGENT_CALLBACK),
        "{at_once:?}"
    );
    // …unless the client asks for the screen, or for more.
    let (prompted, _) = Kit::authorize_url(&client, AGENT_CALLBACK, "conversations:read");
    to_consent(&kit, &format!("{prompted}&prompt=consent")).await;
    let (more, _) = Kit::authorize_url(&client, AGENT_CALLBACK, "conversations:read openid");
    to_consent(&kit, &more).await;
    // The same agent through its loopback callback is asked every time.
    let (local, _) = Kit::authorize_url(
        &client,
        "http://localhost:4000/callback",
        "conversations:read",
    );
    to_consent(&kit, &local).await;
    // `prompt=none` never shows a screen.
    let (silent, _) = Kit::authorize_url(
        &client,
        "http://localhost:4000/callback",
        "conversations:read",
    );
    let refused = kit.get(&format!("{silent}&prompt=none")).await;
    assert_eq!(refused.location_query()["error"], "consent_required");

    // Never an `identity:*` scope, registered or asked.
    let (asks, _) = Kit::authorize_url(&client, AGENT_CALLBACK, "identity:write");
    assert_eq!(
        kit.get(&asks).await.location_query()["error"],
        "invalid_scope"
    );

    // Connected apps lists it; the person picked Home.
    let apps = kit
        .api(Method::GET, "/api/v1/identity/consents", &web)
        .await;
    assert_eq!(apps.status, StatusCode::OK, "{apps:?}");
    let item = &apps.json["items"][0];
    assert_eq!(item["client_name"], "Helpful Agent");
    assert_eq!(item["workspace_id"], HOME.to_string());
    assert_eq!(item["verified"], false);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn revoking_a_consent_ends_its_tokens_and_asks_again(db: PgPool) {
    let kit = Kit::new(db).await;
    let web = kit.signed_up().await;
    let client = agent(&kit, "conversations:read").await;
    let (url, verifier) =
        Kit::authorize_url(&client, AGENT_CALLBACK, "conversations:read offline_access");
    let id = to_consent(&kit, &url).await;
    let code = allow(&kit, &id, Some(WORK)).await;
    let tokens = redeem_public(&kit, &client, &code, AGENT_CALLBACK, &verifier).await;
    let refresh = tokens.json["refresh_token"]
        .as_str()
        .unwrap_or_else(|| panic!("{tokens:?}"))
        .to_owned();

    let apps = kit
        .api(Method::GET, "/api/v1/identity/consents", &web)
        .await;
    let consent = apps.json["items"][0]["id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let revoked = kit
        .api(
            Method::DELETE,
            &format!("/api/v1/identity/consents/{consent}"),
            &web,
        )
        .await;
    assert_eq!(revoked.status, StatusCode::NO_CONTENT, "{revoked:?}");
    let again = kit
        .api(
            Method::DELETE,
            &format!("/api/v1/identity/consents/{consent}"),
            &web,
        )
        .await;
    assert_eq!(again.status, StatusCode::NOT_FOUND);

    let refreshed = kit
        .post_form(
            "/oauth2/token",
            &[
                ("grant_type", "refresh_token"),
                ("refresh_token", &refresh),
                ("client_id", &client),
            ],
        )
        .await;
    assert_eq!(refreshed.json["error"], "invalid_grant");
    let (next, _) = Kit::authorize_url(&client, AGENT_CALLBACK, "conversations:read");
    to_consent(&kit, &next).await;
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_member_leaving_or_a_workspace_going_ends_the_apps_allowed_there(db: PgPool) {
    use campbooks_kernel::db::BeginFor as _;

    let kit = Kit::new(db).await;
    let web = kit.signed_up().await;
    let client = agent(&kit, "conversations:read").await;
    for workspace in [HOME, WORK] {
        let (url, _) =
            Kit::authorize_url(&client, "http://localhost:1/callback", "conversations:read");
        let id = to_consent(&kit, &url).await;
        allow(&kit, &id, Some(workspace)).await;
    }
    // Only the last consent (Work) is live; give one in Home too.
    let other = agent(&kit, "conversations:read").await;
    let (url, _) = Kit::authorize_url(&other, AGENT_CALLBACK, "conversations:read");
    let id = to_consent(&kit, &url).await;
    allow(&kit, &id, Some(HOME)).await;
    assert_eq!(live(&kit, &web).await, 2);

    let home = campbooks_kernel::tenancy::WorkspaceId::from_uuid(HOME);
    let mut tx = kit
        .state
        .db
        .begin_for(home)
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    kit.state
        .events
        .publish(
            &mut tx,
            &crate::events::MemberLeft {
                workspace: HOME.to_string(),
                user: MEMBER_IN_HOME,
            },
        )
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    tx.commit().await.unwrap_or_else(|e| panic!("{e}"));
    kit.run_jobs().await;
    assert_eq!(live(&kit, &web).await, 1, "the Home consent ended");

    let work = campbooks_kernel::tenancy::WorkspaceId::from_uuid(WORK);
    let mut tx = kit
        .state
        .db
        .begin_for(work)
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    kit.state
        .events
        .publish(
            &mut tx,
            &crate::events::WorkspaceDeleting {
                workspace: WORK.to_string(),
            },
        )
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    tx.commit().await.unwrap_or_else(|e| panic!("{e}"));
    kit.run_jobs().await;
    assert_eq!(live(&kit, &web).await, 0, "the Work consent ended");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn dynamic_registration_takes_public_code_clients_only(db: PgPool) {
    let kit = Kit::new(db).await;
    let registered = register(
        &kit,
        &serde_json::json!({
            "redirect_uris": ["http://127.0.0.1:3334/callback"],
            "scope": "conversations:read identity:write",
        }),
    )
    .await;
    assert_eq!(registered.status, StatusCode::CREATED, "{registered:?}");
    assert_eq!(registered.headers["cache-control"], "no-store");
    assert!(
        registered.json["client_id"]
            .as_str()
            .unwrap_or_default()
            .starts_with("dcr_")
    );
    assert_eq!(registered.json["token_endpoint_auth_method"], "none");
    assert_eq!(registered.json["client_name"], "127.0.0.1");
    assert_eq!(
        registered.json["scope"], "conversations:read",
        "never identity:*"
    );
    assert!(registered.json.get("client_secret").is_none());

    for (body, error) in [
        (
            serde_json::json!({ "redirect_uris": ["http://agent.example.com/cb"] }),
            "invalid_redirect_uri",
        ),
        (
            serde_json::json!({ "redirect_uris": [] }),
            "invalid_redirect_uri",
        ),
        (
            serde_json::json!({ "redirect_uris": ["https://agent.example.com/cb#x"] }),
            "invalid_redirect_uri",
        ),
        (
            serde_json::json!({ "redirect_uris": ["com.example.agent:/cb"] }),
            "invalid_redirect_uri",
        ),
        (
            serde_json::json!({ "redirect_uris": [AGENT_CALLBACK], "token_endpoint_auth_method": "client_secret_basic" }),
            "invalid_client_metadata",
        ),
        (
            serde_json::json!({ "redirect_uris": [AGENT_CALLBACK], "grant_types": ["client_credentials"] }),
            "invalid_client_metadata",
        ),
        (
            serde_json::json!(["not", "metadata"]),
            "invalid_client_metadata",
        ),
    ] {
        let refused = register(&kit, &body).await;
        assert_eq!(refused.status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(refused.json["error"], error, "{body}");
    }

    // A dynamic client can't use client credentials.
    let id = registered.json["client_id"].as_str().unwrap_or_default();
    let refused = kit
        .post_form(
            "/oauth2/token",
            &[("grant_type", "client_credentials"), ("client_id", id)],
        )
        .await;
    assert_eq!(refused.json["error"], "unauthorized_client");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn registration_and_tokens_are_rate_limited_per_address(db: PgPool) {
    let kit = Kit::new(db).await;
    kit.send_from(Some("203.0.113.7:4000"));
    let body = serde_json::json!({ "redirect_uris": [AGENT_CALLBACK] });
    for _ in 0..20 {
        assert_eq!(register(&kit, &body).await.status, StatusCode::CREATED);
    }
    let limited = register(&kit, &body).await;
    assert_eq!(limited.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(limited.json["code"], "rate_limited");
    assert!(limited.headers.contains_key("retry-after"));
    // Another address is not affected; the server's own calls never are.
    kit.send_from(Some("198.51.100.9:4000"));
    assert_eq!(register(&kit, &body).await.status, StatusCode::CREATED);
    kit.send_from(Some("198.51.100.9:4000"));
    for _ in 0..120 {
        kit.post_form("/oauth2/token", &[("grant_type", "client_credentials")])
            .await;
    }
    let limited = kit
        .post_form("/oauth2/token", &[("grant_type", "client_credentials")])
        .await;
    assert_eq!(limited.status, StatusCode::TOO_MANY_REQUESTS);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_workspace_registers_rotates_and_removes_its_own_clients(db: PgPool) {
    let kit = Kit::new(db).await;
    let web = kit.signed_up().await;
    let clients = format!("/api/v1/w/{HOME}/identity/clients");
    let created = kit
        .api_json(
            Method::POST,
            &clients,
            &web,
            &serde_json::json!({ "name": "Nightly sync", "type": "confidential",
                "scopes": ["conversations:read"] }),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{created:?}");
    let client = created.json["client"]["client_id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let secret = created.json["client_secret"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert!(secret.len() >= 43, "256 bits");

    let listed = kit.api(Method::GET, &clients, &web).await;
    assert_eq!(listed.json["items"][0]["client_id"], client.as_str());
    assert_eq!(listed.json["items"][0]["type"], "confidential");
    assert!(
        !listed.json.to_string().contains(&secret),
        "never shown again"
    );
    // Row-level security: outside the workspace and the authorization
    // server, the row is invisible.
    let hidden: Option<Vec<u8>> =
        sqlx::query_scalar("SELECT secret_hash FROM identity.clients WHERE client_id = $1")
            .bind(&client)
            .fetch_optional(&kit.state.db)
            .await
            .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(hidden, None);
    let mut tx = crate::domain::clients::begin_as_server(&kit.state.db)
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let stored: Vec<u8> =
        sqlx::query_scalar("SELECT secret_hash FROM identity.clients WHERE client_id = $1")
            .bind(&client)
            .fetch_one(&mut *tx)
            .await
            .unwrap_or_else(|e| panic!("{e}"));
    tx.commit().await.unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        stored,
        campbooks_kernel::crypto::hash(&secret),
        "only its hash"
    );

    let credentials = |secret: String| {
        let client = client.clone();
        let kit = &kit;
        async move {
            kit.post_form(
                "/oauth2/token",
                &[
                    ("grant_type", "client_credentials"),
                    ("client_id", &client),
                    ("client_secret", &secret),
                ],
            )
            .await
        }
    };
    let token = credentials(secret.clone()).await;
    assert_eq!(token.status, StatusCode::OK, "{token:?}");
    assert!(token.json.get("refresh_token").is_none());
    assert!(token.json.get("id_token").is_none());
    let (header, claims) = Kit::decode(token.json["access_token"].as_str().unwrap_or_default());
    assert_eq!(header["typ"], "at+jwt");
    assert_eq!(claims["sub"], client.as_str());
    assert_eq!(claims["azp"], client.as_str());
    assert_eq!(claims["workspace_id"], HOME.to_string());
    assert_eq!(claims["aud"], RESOURCE);
    assert_eq!(claims["scope"], "conversations:read");
    // The API sees a client bound to Home.
    let principal = kit
        .state
        .auth
        .verify(token.json["access_token"].as_str().unwrap_or_default())
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        principal.kind,
        campbooks_kernel::auth::PrincipalKind::Client
    );
    assert_eq!(
        principal.workspace.as_deref(),
        Some(HOME.to_string().as_str())
    );

    let wrong = credentials(format!("{secret}x")).await;
    assert_eq!(wrong.status, StatusCode::UNAUTHORIZED);
    let more = kit
        .post_form(
            "/oauth2/token",
            &[
                ("grant_type", "client_credentials"),
                ("client_id", &client),
                ("client_secret", &secret),
                ("scope", "connectors:write"),
            ],
        )
        .await;
    assert_eq!(more.json["error"], "invalid_scope");
    let elsewhere = kit
        .post_form(
            "/oauth2/token",
            &[
                ("grant_type", "client_credentials"),
                ("client_id", &client),
                ("client_secret", &secret),
                ("resource", "https://other.example.com/api"),
            ],
        )
        .await;
    assert_eq!(elsewhere.json["error"], "invalid_target");
    // It can't use the code flow.
    let (url, _) = Kit::authorize_url(&client, AGENT_CALLBACK, "conversations:read");
    let refused = kit.get(&url).await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST, "no redirect URIs");

    // Rotation: the old secret stops at once.
    let rotated = kit
        .api(Method::POST, &format!("{clients}/{client}/secret"), &web)
        .await;
    assert_eq!(rotated.status, StatusCode::OK, "{rotated:?}");
    let fresh = rotated.json["client_secret"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert_eq!(credentials(secret).await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(credentials(fresh.clone()).await.status, StatusCode::OK);

    // Removal: no more tokens.
    let removed = kit
        .api(Method::DELETE, &format!("{clients}/{client}"), &web)
        .await;
    assert_eq!(removed.status, StatusCode::NO_CONTENT);
    assert_eq!(credentials(fresh).await.status, StatusCode::UNAUTHORIZED);
    let gone = kit
        .api(Method::DELETE, &format!("{clients}/{client}"), &web)
        .await;
    assert_eq!(gone.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn workspace_clients_are_for_admins_with_a_recent_sign_in(db: PgPool) {
    let kit = Kit::new(db).await;
    let web = kit.signed_up().await;
    let body = serde_json::json!({ "name": "Sync", "type": "confidential", "scopes": [] });
    let member = kit
        .api_json(
            Method::POST,
            &format!("/api/v1/w/{MEMBER_ONLY}/identity/clients"),
            &web,
            &body,
        )
        .await;
    assert_eq!(member.status, StatusCode::FORBIDDEN);
    assert_eq!(member.json["code"], "not_workspace_admin");
    let foreign = kit
        .api(
            Method::GET,
            &format!("/api/v1/w/{FOREIGN}/identity/clients"),
            &web,
        )
        .await;
    assert_eq!(foreign.status, StatusCode::NOT_FOUND);

    let clients = format!("/api/v1/w/{HOME}/identity/clients");
    for (bad, code) in [
        (
            serde_json::json!({ "name": "Sync", "type": "confidential", "scopes": ["identity:read"] }),
            "client_scope_not_allowed",
        ),
        (
            serde_json::json!({ "name": "Sync", "type": "confidential", "scopes": ["mail:send"] }),
            "client_scope_not_allowed",
        ),
        (
            serde_json::json!({ "name": "App", "type": "public", "scopes": [] }),
            "client_needs_redirect",
        ),
        (
            serde_json::json!({ "name": "App", "type": "public", "scopes": [], "redirect_uris": ["http://app.example.com/cb"] }),
            "client_needs_redirect",
        ),
        (
            serde_json::json!({ "name": "Sync", "type": "confidential", "scopes": [], "redirect_uris": [AGENT_CALLBACK] }),
            "client_needs_redirect",
        ),
        (
            serde_json::json!({ "name": " ", "type": "confidential", "scopes": [] }),
            "invalid_client_metadata",
        ),
    ] {
        let refused = kit.api_json(Method::POST, &clients, &web, &bad).await;
        assert_eq!(refused.status, StatusCode::BAD_REQUEST, "{bad}");
        assert_eq!(refused.json["code"], code, "{bad}");
    }
    let stale = kit.stale(&web).await;
    let refused = kit.api_json(Method::POST, &clients, &stale, &body).await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN);
    assert_eq!(refused.json["code"], "step_up_required");

    // A token bound to this very workspace (an app let in here) manages
    // none of its apps, nor adopts a mailbox into it: 404.
    let created = kit.api_json(Method::POST, &clients, &web, &body).await;
    assert_eq!(created.status, StatusCode::CREATED, "{created:?}");
    let client = created.json["client"]["client_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let bound = kit.bound(&web, HOME).await;
    for (method, path) in [
        (Method::GET, clients.clone()),
        (Method::POST, clients.clone()),
        (Method::POST, format!("{clients}/{client}/secret")),
        (Method::DELETE, format!("{clients}/{client}")),
        (
            Method::POST,
            format!(
                "/api/v1/w/{HOME}/identity/held-grants/{}/adopt",
                uuid::Uuid::now_v7()
            ),
        ),
    ] {
        let refused = kit.api_json(method.clone(), &path, &bound, &body).await;
        assert_eq!(
            refused.status,
            StatusCode::NOT_FOUND,
            "{method} {path}: {refused:?}"
        );
    }
    let listed = kit.api(Method::GET, &clients, &web).await;
    assert_eq!(
        listed.json["items"].as_array().map(Vec::len),
        Some(1),
        "nothing registered or removed: {listed:?}"
    );
    assert_eq!(listed.json["items"][0]["client_id"], client.as_str());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_workspaces_own_sign_in_app_is_allowed_in_that_workspace_only(db: PgPool) {
    let kit = Kit::new(db).await;
    let web = kit.signed_up().await;
    let created = kit
        .api_json(
            Method::POST,
            &format!("/api/v1/w/{HOME}/identity/clients"),
            &web,
            &serde_json::json!({ "name": "Team dashboard", "type": "public",
                "scopes": ["conversations:read"], "redirect_uris": [AGENT_CALLBACK] }),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{created:?}");
    assert!(created.json["client_secret"].is_null());
    let client = created.json["client"]["client_id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let (url, verifier) = Kit::authorize_url(&client, AGENT_CALLBACK, "conversations:read");
    let id = to_consent(&kit, &url).await;
    let shown = kit.get(&format!("/auth/consent/{id}")).await;
    // Named by its workspace, not "verified": an admin chose its name.
    assert_eq!(shown.json["client"]["verified"], false);
    assert_eq!(shown.json["client"]["workspace_name"], "Home");
    let listed: Vec<&str> = shown.json["workspaces"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|w| w["name"].as_str())
        .collect();
    assert_eq!(listed, ["Home"]);
    let elsewhere = answer(
        &kit,
        &id,
        &serde_json::json!({ "decision": "allow", "workspace_id": WORK }),
    )
    .await;
    assert_eq!(elsewhere.status, StatusCode::FORBIDDEN);
    assert_eq!(elsewhere.json["code"], "consent_wrong_workspace");
    let code = allow(&kit, &id, Some(HOME)).await;
    let tokens = redeem_public(&kit, &client, &code, AGENT_CALLBACK, &verifier).await;
    assert_eq!(tokens.status, StatusCode::OK, "{tokens:?}");
    // No secret to rotate, and no client credentials.
    let rotate = kit
        .api(
            Method::POST,
            &format!("/api/v1/w/{HOME}/identity/clients/{client}/secret"),
            &web,
        )
        .await;
    assert_eq!(rotate.status, StatusCode::CONFLICT);
    let refused = kit
        .post_form(
            "/oauth2/token",
            &[("grant_type", "client_credentials"), ("client_id", &client)],
        )
        .await;
    assert_eq!(refused.json["error"], "unauthorized_client");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_client_known_by_its_metadata_document(db: PgPool) {
    let kit = Kit::new(db).await;
    kit.sign_up().await;
    let id = "https://agent.example.com/oauth/client.json";
    kit.documents.serve(
        id,
        &serde_json::json!({
            "client_id": id,
            "client_name": "Doc Agent",
            "redirect_uris": ["http://localhost:3334/callback"],
            "grant_types": ["authorization_code"],
        }),
        Some(600),
    );
    let (url, verifier) =
        Kit::authorize_url(id, "http://localhost:9000/callback", "conversations:read");
    let request = to_consent(&kit, &url).await;
    let shown = kit.get(&format!("/auth/consent/{request}")).await;
    assert_eq!(shown.json["client"]["name"], "Doc Agent");
    assert_eq!(shown.json["client"]["kind"], "metadata_document");
    assert_eq!(shown.json["client"]["document_host"], "agent.example.com");
    assert_eq!(shown.json["client"]["device"], true);
    assert_eq!(shown.json["client"]["verified"], false);
    let code = allow(&kit, &request, Some(HOME)).await;
    let tokens = redeem_public(&kit, id, &code, "http://localhost:9000/callback", &verifier).await;
    assert_eq!(tokens.status, StatusCode::OK, "{tokens:?}");
    // Cached: the next authorization does not fetch it again.
    let (again, _) = Kit::authorize_url(id, "http://localhost:9000/callback", "conversations:read");
    to_consent(&kit, &again).await;
    assert_eq!(kit.documents.fetched(), [id]);

    // A document that lies about its id, or is not there, is no client.
    let liar = "https://agent.example.com/oauth/liar.json";
    kit.documents.serve(
        liar,
        &serde_json::json!({ "client_id": id, "redirect_uris": [AGENT_CALLBACK] }),
        None,
    );
    for missing in [liar, "https://agent.example.com/oauth/none.json"] {
        let (url, _) = Kit::authorize_url(missing, AGENT_CALLBACK, "openid");
        let refused = kit.get(&url).await;
        assert_eq!(refused.status, StatusCode::BAD_REQUEST, "{missing}");
        assert_eq!(refused.json["code"], "invalid_client");
        assert!(refused.headers.get("location").is_none());
    }
    // Looking documents up is limited per address: the server can't be
    // made to fetch without end.
    kit.send_from(Some("203.0.113.8:4000"));
    let mut limited = None;
    for n in 0..31 {
        let (url, _) = Kit::authorize_url(
            &format!("https://agent.example.com/oauth/{n}.json"),
            AGENT_CALLBACK,
            "openid",
        );
        let response = kit.get(&url).await;
        if response.status == StatusCode::TOO_MANY_REQUESTS {
            limited = Some(n);
            break;
        }
    }
    assert_eq!(limited, Some(30));
    kit.send_from(None);
    // An http client id is not a document to fetch.
    let (url, _) = Kit::authorize_url("http://agent.example.com/c.json", AGENT_CALLBACK, "openid");
    assert_eq!(kit.get(&url).await.status, StatusCode::BAD_REQUEST);
    assert_eq!(kit.documents.fetched().len(), 33);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn resource_indicators_name_the_api_only(db: PgPool) {
    let kit = Kit::new(db).await;
    kit.sign_up().await;
    let (url, _) = Kit::web_authorize();
    let other = kit
        .get(&format!(
            "{url}&resource=https%3A%2F%2Fother.example.com%2Fapi"
        ))
        .await;
    assert_eq!(other.location_query()["error"], "invalid_target");
    let (url, verifier) = Kit::web_authorize();
    let ok = kit
        .get(&format!(
            "{url}&resource=http%3A%2F%2Flocalhost%3A8080%2Fapi"
        ))
        .await;
    let tokens = kit.redeem(&ok.location_query()["code"], &verifier).await;
    let (_, claims) = Kit::decode(tokens.json["access_token"].as_str().unwrap_or_default());
    assert_eq!(claims["aud"], RESOURCE);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn discovery_names_registration_and_the_protected_resource(db: PgPool) {
    let kit = Kit::new(db).await;
    let metadata = kit.get("/.well-known/oauth-authorization-server").await;
    assert_eq!(
        metadata.json["registration_endpoint"],
        format!("{ORIGIN}/oauth2/register")
    );
    assert_eq!(metadata.json["client_id_metadata_document_supported"], true);
    assert!(
        metadata.json["grant_types_supported"]
            .as_array()
            .is_some_and(|g| g.iter().any(|g| g == "client_credentials"))
    );
    for path in [
        "/.well-known/oauth-protected-resource/api",
        "/.well-known/oauth-protected-resource",
    ] {
        let resource = kit.get(path).await;
        assert_eq!(resource.status, StatusCode::OK, "{path}");
        assert_eq!(resource.json["resource"], RESOURCE);
        assert_eq!(
            resource.json["authorization_servers"],
            serde_json::json!([ORIGIN])
        );
        let scopes = resource.json["scopes_supported"].to_string();
        assert!(scopes.contains("conversations:read"), "{scopes}");
        assert!(!scopes.contains("identity:"), "{scopes}");
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_consent_stands_behind_every_token_of_its_app(db: PgPool) {
    let kit = Kit::new(db).await;
    let web = kit.signed_up().await;
    let client = agent(&kit, "conversations:read").await;
    let refresh_in = |workspace| {
        let kit = &kit;
        let client = client.clone();
        async move {
            let (url, verifier) = Kit::authorize_url(
                &client,
                "http://localhost:7000/callback",
                "conversations:read offline_access",
            );
            let id = to_consent(kit, &url).await;
            let code = allow(kit, &id, Some(workspace)).await;
            let tokens = redeem_public(
                kit,
                &client,
                &code,
                "http://localhost:7000/callback",
                &verifier,
            )
            .await;
            tokens.json["refresh_token"]
                .as_str()
                .unwrap_or_else(|| panic!("{tokens:?}"))
                .to_owned()
        }
    };
    let refresh = |token: String| {
        let kit = &kit;
        let client = client.clone();
        async move {
            kit.post_form(
                "/oauth2/token",
                &[
                    ("grant_type", "refresh_token"),
                    ("refresh_token", &token),
                    ("client_id", &client),
                ],
            )
            .await
        }
    };
    let home = refresh_in(HOME).await;
    // Allowing it again in Work ends the Home tokens.
    let work = refresh_in(WORK).await;
    assert_eq!(refresh(home).await.json["error"], "invalid_grant");
    let rotated = refresh(work).await;
    assert_eq!(rotated.status, StatusCode::OK, "{rotated:?}");

    // A code allowed, then the consent revoked before redemption: nothing.
    let (url, verifier) = Kit::authorize_url(
        &client,
        "http://localhost:7000/callback",
        "conversations:read",
    );
    let id = to_consent(&kit, &url).await;
    let code = allow(&kit, &id, Some(HOME)).await;
    let apps = kit
        .api(Method::GET, "/api/v1/identity/consents", &web)
        .await;
    let consent = apps.json["items"][0]["id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    kit.api(
        Method::DELETE,
        &format!("/api/v1/identity/consents/{consent}"),
        &web,
    )
    .await;
    let late = redeem_public(
        &kit,
        &client,
        &code,
        "http://localhost:7000/callback",
        &verifier,
    )
    .await;
    assert_eq!(late.json["error"], "invalid_grant");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_remembered_consent_covers_its_own_redirect_only(db: PgPool) {
    let kit = Kit::new(db).await;
    kit.sign_up().await;
    let registered = register(
        &kit,
        &serde_json::json!({
            "client_name": "Two Doors",
            "redirect_uris": [AGENT_CALLBACK, "https://other.example.com/callback"],
            "scope": "conversations:read",
        }),
    )
    .await;
    let client = registered.json["client_id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let (url, _) = Kit::authorize_url(&client, AGENT_CALLBACK, "conversations:read");
    let id = to_consent(&kit, &url).await;
    allow(&kit, &id, Some(HOME)).await;
    let (same, _) = Kit::authorize_url(&client, AGENT_CALLBACK, "conversations:read");
    assert!(kit.get(&same).await.location().starts_with(AGENT_CALLBACK));
    let (other, _) = Kit::authorize_url(
        &client,
        "https://other.example.com/callback",
        "conversations:read",
    );
    to_consent(&kit, &other).await;

    // After 90 days the screen comes back.
    let (again, _) = Kit::authorize_url(&client, AGENT_CALLBACK, "conversations:read");
    let id = to_consent(&kit, &format!("{again}&prompt=consent")).await;
    allow(&kit, &id, Some(HOME)).await;
    sqlx::query("UPDATE identity.consents SET remember_until = now() - interval '1 second'")
        .execute(&kit.state.db)
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let (expired, _) = Kit::authorize_url(&client, AGENT_CALLBACK, "conversations:read");
    to_consent(&kit, &expired).await;
}

#[sqlx::test(migrations = "../../../migrations")]
async fn nobody_else_may_be_called_campbooks(db: PgPool) {
    let kit = Kit::new(db).await;
    let web = kit.signed_up().await;
    let fake = register(
        &kit,
        &serde_json::json!({
            "client_name": "C\u{0430}mpbooks Desktop\u{202E}",
            "redirect_uris": [AGENT_CALLBACK],
        }),
    )
    .await;
    assert_eq!(fake.status, StatusCode::CREATED);
    assert_eq!(
        fake.json["client_name"], "agent.example.com",
        "its host instead"
    );
    assert_eq!(
        fake.json["scope"], "system:read conversations:read",
        "reading only by default"
    );
    let hidden = register(
        &kit,
        &serde_json::json!({
            "client_name": "Desk\u{202E}Agent\u{200B}",
            "redirect_uris": [AGENT_CALLBACK],
        }),
    )
    .await;
    assert_eq!(hidden.json["client_name"], "DeskAgent");
    let reserved = kit
        .api_json(
            Method::POST,
            &format!("/api/v1/w/{HOME}/identity/clients"),
            &web,
            &serde_json::json!({ "name": "Not A Camp sync", "type": "confidential", "scopes": [] }),
        )
        .await;
    assert_eq!(reserved.status, StatusCode::BAD_REQUEST);
    assert_eq!(reserved.json["code"], "client_name_reserved");
    let write = kit
        .api_json(
            Method::POST,
            &format!("/api/v1/w/{HOME}/identity/clients"),
            &web,
            &serde_json::json!({ "name": "Sync", "type": "confidential",
                "scopes": ["workspaces:write"] }),
        )
        .await;
    assert_eq!(
        write.json["code"], "client_scope_not_allowed",
        "Campbooks' own only"
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_document_that_failed_is_not_fetched_again_at_once(db: PgPool) {
    let kit = Kit::new(db).await;
    kit.sign_up().await;
    let missing = "https://agent.example.com/oauth/gone.json";
    for _ in 0..3 {
        let (url, _) = Kit::authorize_url(missing, AGENT_CALLBACK, "openid");
        assert_eq!(kit.get(&url).await.status, StatusCode::BAD_REQUEST);
    }
    assert_eq!(kit.documents.fetched(), [missing]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn self_registration_is_off_unless_the_instance_turns_it_on(db: PgPool) {
    let kit = Kit::without_self_registration(db).await;
    kit.sign_up().await;
    let refused = register(
        &kit,
        &serde_json::json!({ "redirect_uris": [AGENT_CALLBACK] }),
    )
    .await;
    assert_eq!(refused.status, StatusCode::NOT_FOUND);
    let metadata = kit.get("/.well-known/oauth-authorization-server").await;
    assert!(
        metadata.json.get("registration_endpoint").is_none(),
        "{metadata:?}"
    );
    assert_eq!(
        metadata.json["client_id_metadata_document_supported"],
        false
    );

    // An https client id is no document to fetch: an unknown client.
    let id = "https://agent.example.com/oauth/client.json";
    kit.documents.serve(
        id,
        &serde_json::json!({ "client_id": id, "redirect_uris": [AGENT_CALLBACK] }),
        None,
    );
    let (url, _) = Kit::authorize_url(id, AGENT_CALLBACK, "openid");
    let unknown = kit.get(&url).await;
    assert_eq!(unknown.status, StatusCode::BAD_REQUEST);
    assert_eq!(unknown.json["code"], "invalid_client");
    assert!(kit.documents.fetched().is_empty(), "nothing fetched");

    // A client registered while it was on stops with it.
    let mut tx = crate::domain::clients::begin_as_server(&kit.state.db)
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    sqlx::query(
        "INSERT INTO identity.clients
             (client_id, kind, name, redirect_uris, grant_types, auth_method, allowed_scopes,
              created_at)
         VALUES ('dcr_old', 'dynamic', 'Old Agent', $1, '{authorization_code}', 'none',
                 '{conversations:read}', now())",
    )
    .bind(vec![AGENT_CALLBACK.to_owned()])
    .execute(&mut *tx)
    .await
    .unwrap_or_else(|e| panic!("{e}"));
    tx.commit().await.unwrap_or_else(|e| panic!("{e}"));
    let (url, _) = Kit::authorize_url("dcr_old", AGENT_CALLBACK, "conversations:read");
    assert_eq!(kit.get(&url).await.json["code"], "invalid_client");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn turned_on_both_are_advertised(db: PgPool) {
    let kit = Kit::new(db).await;
    let metadata = kit.get("/.well-known/oauth-authorization-server").await;
    assert_eq!(
        metadata.json["registration_endpoint"],
        format!("{ORIGIN}/oauth2/register")
    );
    assert_eq!(metadata.json["client_id_metadata_document_supported"], true);
}
