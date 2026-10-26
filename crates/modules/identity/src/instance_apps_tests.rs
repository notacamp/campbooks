//! Instance apps end to end (campbooks-cloud#369): registered, rotated and
//! removed from the server's CLI only (on the migration login, here the test
//! database's owner), then signing people in with the code, PKCE and their
//! secret; the consent screen once per person, remembered and revocable; the
//! scope ceiling; exact redirect URIs; RP-initiated logout back to the app;
//! and the runtime role unable to change one.

use axum::http::{Method, StatusCode};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use sqlx::PgPool;

use crate::{
    domain::{
        clients,
        instance_apps::{self, InstanceApp, NewInstanceApp},
    },
    testkit::{HOME, Kit, ORIGIN, Response, WORK, query},
    tracking_tests::{account_of, no_address, tracked},
};

const CALLBACK: &str = "https://applicamp.example.com/auth/callback";
const AFTER_LOGOUT: &str = "https://applicamp.example.com/";
/// What Applicamp may ever hold (of the test catalogue).
const CEILING: [&str; 5] = [
    "openid",
    "profile",
    "email",
    "offline_access",
    "conversations:read",
];

/// Register Applicamp as `admin apps add` does, on the owner's connection:
/// the app and its secret.
async fn applicamp(kit: &Kit) -> (InstanceApp, String) {
    let asked = NewInstanceApp {
        name: "Applicamp".to_owned(),
        redirect_uris: vec![CALLBACK.to_owned()],
        post_logout_redirect_uris: vec![AFTER_LOGOUT.to_owned()],
        scopes: CEILING.iter().map(|s| (*s).to_owned()).collect(),
    };
    let issuer = kit.deps.issuer.as_ref().unwrap();
    let app = instance_apps::validate(&asked, &issuer.api_scopes, false).unwrap();
    let mut conn = kit.owner.acquire().await.unwrap();
    instance_apps::register(&mut conn, &kit.state.tracker, &app, chrono::Utc::now())
        .await
        .unwrap()
}

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

/// Allow request `id` in `workspace`; the code.
async fn allow(kit: &Kit, id: &str, workspace: uuid::Uuid) -> String {
    let answered = kit
        .request(
            Method::POST,
            &format!("/auth/consent/{id}"),
            &[
                ("x-campbooks-csrf", "1"),
                ("sec-fetch-site", "same-origin"),
                ("content-type", "application/json"),
            ],
            Some(serde_json::json!({ "decision": "allow", "workspace_id": workspace }).to_string()),
        )
        .await;
    assert_eq!(answered.status, StatusCode::OK, "{answered:?}");
    query(answered.json["redirect"].as_str().unwrap_or_default())["code"].clone()
}

/// A token request with `client_secret_basic`.
async fn token_basic(kit: &Kit, id: &str, secret: &str, form: &[(&str, &str)]) -> Response {
    let basic = format!("Basic {}", STANDARD.encode(format!("{id}:{secret}")));
    let body = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(form.iter().copied())
        .finish();
    kit.request(
        Method::POST,
        "/oauth2/token",
        &[
            ("content-type", "application/x-www-form-urlencoded"),
            ("authorization", &basic),
        ],
        Some(body),
    )
    .await
}

/// Redeem `code` with `client_secret_basic`.
async fn redeem(kit: &Kit, id: &str, secret: &str, code: &str, verifier: &str) -> Response {
    token_basic(
        kit,
        id,
        secret,
        &[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", CALLBACK),
            ("code_verifier", verifier),
        ],
    )
    .await
}

/// Refresh with `client_secret_post`.
async fn refresh_post(kit: &Kit, id: &str, secret: &str, refresh: &str) -> Response {
    kit.post_form(
        "/oauth2/token",
        &[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh),
            ("client_id", id),
            ("client_secret", secret),
        ],
    )
    .await
}

fn text(response: &Response, field: &str) -> String {
    response.json[field]
        .as_str()
        .unwrap_or_else(|| panic!("no {field}: {response:?}"))
        .to_owned()
}

/// Sign a person in to Applicamp in `workspace` (consent included): the
/// token response.
async fn signed_in(kit: &Kit, app: &InstanceApp, secret: &str, workspace: uuid::Uuid) -> Response {
    let (url, verifier) = Kit::authorize_url(&app.client_id, CALLBACK, &CEILING.join(" "));
    let id = to_consent(kit, &url).await;
    let code = allow(kit, &id, workspace).await;
    let tokens = redeem(kit, &app.client_id, secret, &code, &verifier).await;
    assert_eq!(tokens.status, StatusCode::OK, "{tokens:?}");
    tokens
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_instance_app_signs_people_in_with_its_secret_and_pkce(db: PgPool) {
    let kit = Kit::new(db).await;
    let web = kit.signed_up().await;
    let (app, secret) = applicamp(&kit).await;
    assert!(app.client_id.starts_with("app_"), "{app:?}");

    // Asking for more than the ceiling: `system:read` is not granted.
    let (url, verifier) = Kit::authorize_url(
        &app.client_id,
        CALLBACK,
        "openid profile email offline_access conversations:read system:read",
    );
    let id = to_consent(&kit, &url).await;
    let shown = kit.get(&format!("/auth/consent/{id}")).await;
    assert_eq!(shown.status, StatusCode::OK, "{shown:?}");
    assert_eq!(shown.json["client"]["name"], "Applicamp");
    assert_eq!(shown.json["client"]["kind"], "instance");
    assert_eq!(shown.json["client"]["verified"], false);
    assert_eq!(shown.json["client"]["device"], false);
    assert_eq!(
        shown.json["client"]["redirect_host"],
        "applicamp.example.com"
    );
    assert_eq!(shown.json["binds_workspace"], true);
    assert_eq!(shown.json["remembered"], true);
    let scopes: Vec<&str> = shown.json["scopes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| s["name"].as_str())
        .collect();
    assert_eq!(scopes, CEILING);

    let code = allow(&kit, &id, HOME).await;
    // PKCE is required even of a confidential client.
    let no_verifier = token_basic(
        &kit,
        &app.client_id,
        &secret,
        &[
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("redirect_uri", CALLBACK),
        ],
    )
    .await;
    assert_eq!(no_verifier.json["error"], "invalid_request");
    // Without the secret, or with another, it is not the app.
    let public = kit
        .post_form(
            "/oauth2/token",
            &[
                ("grant_type", "authorization_code"),
                ("code", &code),
                ("redirect_uri", CALLBACK),
                ("code_verifier", &verifier),
                ("client_id", &app.client_id),
            ],
        )
        .await;
    assert_eq!(public.status, StatusCode::UNAUTHORIZED);
    assert_eq!(public.json["error"], "invalid_client");
    let wrong = redeem(&kit, &app.client_id, "not-the-secret", &code, &verifier).await;
    assert_eq!(wrong.json["error"], "invalid_client");

    let tokens = redeem(&kit, &app.client_id, &secret, &code, &verifier).await;
    assert_eq!(tokens.status, StatusCode::OK, "{tokens:?}");
    assert_eq!(tokens.json["scope"], CEILING.join(" "));
    let (_, claims) = Kit::decode(&text(&tokens, "access_token"));
    assert_eq!(claims["client_id"], app.client_id);
    assert_eq!(claims["workspace_id"], HOME.to_string());
    assert_eq!(claims["sub"], account_of(&web));
    assert!(!claims["scope"].as_str().unwrap().contains("system:read"));
    let (_, id_token) = Kit::decode(&text(&tokens, "id_token"));
    assert_eq!(id_token["iss"], ORIGIN);
    assert_eq!(id_token["aud"], app.client_id);
    assert_eq!(id_token["sub"], account_of(&web));
    assert_eq!(id_token["email"], "test-user@example.com");
    assert_eq!(id_token["email_verified"], true);
    assert_eq!(id_token["name"], "Test User");
    assert_eq!(id_token["nonce"], "client-nonce");

    // The refresh token rotates; the spent one is reuse.
    let first = text(&tokens, "refresh_token");
    let renewed = refresh_post(&kit, &app.client_id, &secret, &first).await;
    assert_eq!(renewed.status, StatusCode::OK, "{renewed:?}");
    let second = text(&renewed, "refresh_token");
    assert_ne!(first, second);
    let reused = refresh_post(&kit, &app.client_id, &secret, &first).await;
    assert_eq!(reused.json["error"], "invalid_grant");

    // Connected apps lists it as an instance app the person can revoke.
    let apps = kit
        .api(Method::GET, "/api/v1/identity/consents", &web)
        .await;
    let item = &apps.json["items"][0];
    assert_eq!(item["client_id"], app.client_id);
    assert_eq!(item["client_name"], "Applicamp");
    assert_eq!(item["client_kind"], "instance");
    assert_eq!(item["workspace_id"], HOME.to_string());
    assert_eq!(item["scopes"], serde_json::json!(CEILING));

    let issued = tracked(&kit, "identity.tokens_issued").await;
    let last = issued.last().unwrap();
    assert_eq!(last.body["client_kind"], "instance");
    assert_eq!(last.body["client_id"], app.client_id);
    assert_eq!(last.body["workspace_bound"], true);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn its_redirect_uris_match_exactly_and_nothing_above_the_ceiling(db: PgPool) {
    let kit = Kit::new(db).await;
    kit.signed_up().await;
    let (app, _) = applicamp(&kit).await;
    for bad in [
        "https://applicamp.example.com/auth/callback/",
        "https://applicamp.example.com/auth/callback?next=x",
        "https://applicamp.example.com:8443/auth/callback",
        "https://evil.example.com/auth/callback",
        "http://127.0.0.1:4000/auth/callback",
    ] {
        let (url, _) = Kit::authorize_url(&app.client_id, bad, "openid");
        let refused = kit.get(&url).await;
        assert_eq!(refused.status, StatusCode::BAD_REQUEST, "{bad}");
        assert_eq!(refused.json["code"], "invalid_redirect_uri", "{bad}");
    }
    // Only scopes above the ceiling: nothing to grant.
    for scope in ["system:read", "identity:read", "admin:access"] {
        let (url, _) = Kit::authorize_url(&app.client_id, CALLBACK, scope);
        let refused = kit.get(&url).await;
        assert_eq!(
            refused.location_query()["error"],
            "invalid_scope",
            "{scope}"
        );
    }
    // `identity:*` and `admin:access` are dropped like anything else above it.
    let (url, _) = Kit::authorize_url(
        &app.client_id,
        CALLBACK,
        "openid identity:write admin:access workspaces:admin",
    );
    let id = to_consent(&kit, &url).await;
    let shown = kit.get(&format!("/auth/consent/{id}")).await;
    assert_eq!(shown.json["scopes"].as_array().map(Vec::len), Some(1));
    assert_eq!(shown.json["scopes"][0]["name"], "openid");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn consent_is_asked_once_remembered_and_asked_again_once_revoked(db: PgPool) {
    let kit = Kit::new(db).await;
    let web = kit.signed_up().await;
    let (app, secret) = applicamp(&kit).await;
    let tokens = signed_in(&kit, &app, &secret, WORK).await;
    let access = text(&tokens, "access_token");
    let refresh = text(&tokens, "refresh_token");

    // Asked once: the next sign-in goes straight back with a code, in Work.
    let (again, verifier) = Kit::authorize_url(&app.client_id, CALLBACK, &CEILING.join(" "));
    let back = kit.get(&again).await;
    assert!(back.location().starts_with(CALLBACK), "{back:?}");
    let code = back.location_query()["code"].clone();
    let next = redeem(&kit, &app.client_id, &secret, &code, &verifier).await;
    let (_, claims) = Kit::decode(&text(&next, "access_token"));
    assert_eq!(claims["workspace_id"], WORK.to_string());

    // Revoked by the person: its tokens stop, and the screen comes back.
    let apps = kit
        .api(Method::GET, "/api/v1/identity/consents", &web)
        .await;
    let consent = apps.json["items"][0]["id"].as_str().unwrap().to_owned();
    let revoked = kit
        .api(
            Method::DELETE,
            &format!("/api/v1/identity/consents/{consent}"),
            &web,
        )
        .await;
    assert_eq!(revoked.status, StatusCode::NO_CONTENT, "{revoked:?}");
    let probe = kit
        .api(Method::GET, "/api/v1/identity/ways-in", &access)
        .await;
    assert_eq!(probe.status, StatusCode::UNAUTHORIZED, "{probe:?}");
    let refreshed = refresh_post(&kit, &app.client_id, &secret, &refresh).await;
    assert_eq!(refreshed.json["error"], "invalid_grant");
    let (url, _) = Kit::authorize_url(&app.client_id, CALLBACK, &CEILING.join(" "));
    to_consent(&kit, &url).await;
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_cli_lists_rotates_and_removes_and_the_secret_is_kept_hashed(db: PgPool) {
    campbooks_kernel::tracking::set_command("admin");
    let kit = Kit::new(db).await;
    kit.signed_up().await;
    let (app, secret) = applicamp(&kit).await;
    let mut conn = kit.owner.acquire().await.unwrap();

    // Only the secret's SHA-256 is stored.
    let stored: Vec<u8> =
        sqlx::query_scalar("SELECT secret_hash FROM identity.clients WHERE client_id = $1")
            .bind(&app.client_id)
            .fetch_one(&mut *conn)
            .await
            .unwrap();
    assert_eq!(stored, campbooks_kernel::crypto::hash(&secret));
    assert_ne!(stored, secret.as_bytes());
    assert_eq!(secret.len(), 43, "256 random bits");

    let listed = instance_apps::list(&mut conn).await.unwrap();
    assert_eq!(listed, [app.clone()]);
    assert_eq!(listed[0].allowed_scopes, CEILING);
    assert_eq!(listed[0].post_logout_redirect_uris, [AFTER_LOGOUT]);

    // Rotated: the old secret stops at once, the new one works.
    let tokens = signed_in(&kit, &app, &secret, HOME).await;
    let refresh = text(&tokens, "refresh_token");
    let (rotated, fresh) = instance_apps::rotate_secret(
        &mut conn,
        &kit.state.tracker,
        &app.client_id,
        chrono::Utc::now(),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(rotated.secret_rotated_at.is_some());
    assert_ne!(fresh, secret);
    let old = refresh_post(&kit, &app.client_id, &secret, &refresh).await;
    assert_eq!(old.json["error"], "invalid_client");
    let renewed = refresh_post(&kit, &app.client_id, &fresh, &refresh).await;
    assert_eq!(renewed.status, StatusCode::OK, "{renewed:?}");
    let refresh = text(&renewed, "refresh_token");
    let access = text(&renewed, "access_token");
    assert!(
        instance_apps::rotate_secret(
            &mut conn,
            &kit.state.tracker,
            "app_nope",
            chrono::Utc::now()
        )
        .await
        .unwrap()
        .is_none()
    );

    // Removed: its consents and refresh tokens end, and the token endpoint
    // no longer knows it.
    let removed = instance_apps::remove(
        &mut conn,
        &kit.state.tracker,
        &app.client_id,
        chrono::Utc::now(),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(removed.consents_revoked, 1);
    assert_eq!(removed.refresh_tokens_revoked, 1);
    let gone = refresh_post(&kit, &app.client_id, &fresh, &refresh).await;
    assert_eq!(gone.status, StatusCode::UNAUTHORIZED, "{gone:?}");
    assert_eq!(gone.json["error"], "invalid_client");
    let probe = kit
        .api(Method::GET, "/api/v1/identity/ways-in", &access)
        .await;
    assert_eq!(probe.status, StatusCode::UNAUTHORIZED, "{probe:?}");
    let (url, _) = Kit::authorize_url(&app.client_id, CALLBACK, "openid");
    assert_eq!(kit.get(&url).await.json["code"], "invalid_client");
    assert!(instance_apps::list(&mut conn).await.unwrap().is_empty());
    assert!(
        instance_apps::remove(
            &mut conn,
            &kit.state.tracker,
            &app.client_id,
            chrono::Utc::now()
        )
        .await
        .unwrap()
        .is_none()
    );

    // Each change is a security event and a tracked event, as the command.
    let kinds: Vec<String> = sqlx::query_scalar(
        "SELECT kind FROM identity.audit_events
          WHERE kind LIKE 'instance_app_%' AND metadata->>'client' = $1 ORDER BY at, id",
    )
    .bind(&app.client_id)
    .fetch_all(&mut *conn)
    .await
    .unwrap();
    assert_eq!(
        kinds,
        [
            "instance_app_registered",
            "instance_app_secret_rotated",
            "instance_app_removed"
        ]
    );
    let command = serde_json::json!({"type": "system", "id": "admin"});
    let registered = tracked(&kit, "identity.instance_app_registered").await;
    assert_eq!(registered.len(), 1);
    assert_eq!(registered[0].workspace_id, None);
    assert_eq!(registered[0].envelope["actor"], command);
    assert_eq!(
        registered[0].body["client"],
        serde_json::json!({
            "client_id": app.client_id, "kind": "instance", "confidential": true,
            "scope_count": 5, "redirect_uri_count": 1,
            "created_at": registered[0].body["client"]["created_at"],
        })
    );
    assert!(!registered[0].body.to_string().contains("Applicamp"));
    let rotations = tracked(&kit, "identity.instance_app_secret_rotated").await;
    assert_eq!(rotations.len(), 1, "an unknown id tracks nothing");
    assert!(rotations[0].body["client"]["secret_rotated_at"].is_string());
    let removals = tracked(&kit, "identity.instance_app_removed").await;
    assert_eq!(removals.len(), 1, "removing it again tracks nothing");
    assert_eq!(
        removals[0].body,
        serde_json::json!({
            "client_id": app.client_id, "consents_revoked": 1, "refresh_tokens_revoked": 1,
        })
    );
    for event in registered.iter().chain(&rotations).chain(&removals) {
        no_address(event);
        assert!(!event.body.to_string().contains(&secret));
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn signing_out_of_the_app_returns_to_its_registered_page_only(db: PgPool) {
    let kit = Kit::new(db).await;
    kit.signed_up().await;
    let (app, secret) = applicamp(&kit).await;
    let tokens = signed_in(&kit, &app, &secret, HOME).await;
    let hint = text(&tokens, "id_token");
    let refresh = text(&tokens, "refresh_token");
    let out = kit
        .get(&format!(
            "/oauth2/end_session?id_token_hint={hint}&client_id={}&post_logout_redirect_uri={AFTER_LOGOUT}&state=bye",
            app.client_id
        ))
        .await;
    assert_eq!(out.status, StatusCode::SEE_OTHER, "{out:?}");
    assert_eq!(out.location(), format!("{AFTER_LOGOUT}?state=bye"));
    let after = refresh_post(&kit, &app.client_id, &secret, &refresh).await;
    assert_eq!(after.json["error"], "invalid_grant", "the sign-in ended");
    // Somewhere it did not register: signed out, not redirected.
    let elsewhere = kit
        .get(&format!(
            "/oauth2/end_session?id_token_hint={hint}&post_logout_redirect_uri=https://evil.example.com/"
        ))
        .await;
    assert_eq!(elsewhere.status, StatusCode::NO_CONTENT);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_runtime_role_cannot_register_change_or_remove_one(db: PgPool) {
    let kit = Kit::new(db).await;
    let (app, _) = applicamp(&kit).await;
    let refused = |result: Result<sqlx::postgres::PgQueryResult, sqlx::Error>, what: &str| {
        let error = result.expect_err(what);
        assert!(
            error
                .to_string()
                .contains("only with campbooks-server admin apps"),
            "{what}: {error}"
        );
    };
    for (what, sql) in [
        (
            "insert",
            "INSERT INTO identity.clients
                 (client_id, kind, name, redirect_uris, grant_types, auth_method, secret_hash,
                  allowed_scopes, created_at)
             VALUES ('app_AAAAAAAAAAAAAAAAAAAAAA', 'instance', 'Sneaky',
                     '{https://evil.example.com/cb}', '{authorization_code}',
                     'client_secret_basic', '\\x00'::bytea, '{openid}', now())",
        ),
        (
            "widen its ceiling",
            "UPDATE identity.clients SET allowed_scopes = allowed_scopes || '{system:read}'
              WHERE kind = 'instance'",
        ),
        (
            "add a redirect",
            "UPDATE identity.clients
                SET redirect_uris = redirect_uris || '{https://evil.example.com/cb}'
              WHERE kind = 'instance'",
        ),
        (
            "set its secret",
            "UPDATE identity.clients SET secret_hash = '\\x00'::bytea WHERE kind = 'instance'",
        ),
        (
            "make it another kind",
            "UPDATE identity.clients SET kind = 'dynamic', auth_method = 'none', secret_hash = NULL
              WHERE kind = 'instance'",
        ),
        (
            "delete",
            "DELETE FROM identity.clients WHERE kind = 'instance'",
        ),
    ] {
        let mut tx = clients::begin_as_server(&kit.state.db).await.unwrap();
        refused(sqlx::query(sql).execute(&mut *tx).await, what);
    }
    // Recording that it got a token is the runtime's.
    clients::touch(&kit.state.db, &app.client_id, chrono::Utc::now())
        .await
        .unwrap();
    let mut conn = kit.owner.acquire().await.unwrap();
    let listed = instance_apps::list(&mut conn).await.unwrap();
    assert!(listed[0].last_used_at.is_some());
    assert_eq!(listed[0].allowed_scopes, CEILING);
    assert_eq!(listed[0].redirect_uris, [CALLBACK]);
}
