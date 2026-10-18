//! Access tokens already issued stop at a revocation, through every door of
//! the composed server (campbooks-cloud#122): scoped routes, the
//! idempotency layer (a replayed key included), the admin API and the web
//! BFF's proxied calls, with the in-process issuer and the identity
//! module's revocation port wired as `with_composed_ports` wires them.

use axum::http::{Method, StatusCode};
use campbooks_identity::domain::{accounts, suspension, tokens};
use campbooks_kernel::admin::Permission;
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::identity_tests::{
    Browser, ORIGIN, Reply, create_workspace, sign_up_through_bff, step, world,
};

/// An access token of the web client for `account`, with `scope`, for a
/// sign-in made at `auth_time`, minted now.
async fn mint(
    db: &PgPool,
    issuer: &campbooks_identity::deps::Issuer,
    account: Uuid,
    scope: &str,
    auth_time: DateTime<Utc>,
) -> String {
    let grant = tokens::Grant {
        client_id: campbooks_identity::domain::clients::WEB_CLIENT_ID.to_owned(),
        account_id: account,
        session_id: None,
        scope: scope.to_owned(),
        auth_time,
        resource: format!("{ORIGIN}/api"),
        workspace: None,
    };
    let profile = accounts::profile(db, account).await.unwrap().unwrap();
    let signer = issuer.keys.signer(db).await.unwrap();
    tokens::Mint {
        issuer: &issuer.url,
        signer: &signer,
        grant: &grant,
        profile: &profile,
        amr: &[],
        family: None,
        consent: None,
        now: Utc::now(),
    }
    .access_token()
    .unwrap()
}

/// An administrator's account (created here, as the CLI would grant one).
async fn administrator(db: &PgPool) -> Uuid {
    let account = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO identity.accounts (id, display_name, created_at) VALUES ($1, 'Ada', now())",
    )
    .bind(account)
    .execute(db)
    .await
    .unwrap();
    let mut conn = db.acquire().await.unwrap();
    campbooks_admin::cli::grant(
        &mut conn,
        &account.to_string(),
        &[Permission::Super],
        None,
        Utc::now(),
    )
    .await
    .unwrap();
    account
}

/// The only person who signed up through the browser.
async fn the_person(db: &PgPool, admins: &[Uuid]) -> Uuid {
    let ids: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM identity.accounts")
        .fetch_all(db)
        .await
        .unwrap();
    let people: Vec<Uuid> = ids.into_iter().filter(|id| !admins.contains(id)).collect();
    assert_eq!(people.len(), 1, "{people:?}");
    people[0]
}

async fn call(
    browser: &Browser,
    method: Method,
    url: &str,
    token: &str,
    idempotency_key: Option<&str>,
    body: Option<Value>,
) -> Reply {
    let bearer = format!("Bearer {token}");
    let mut headers = vec![("authorization", bearer.as_str())];
    if let Some(key) = idempotency_key {
        headers.push(("idempotency-key", key));
    }
    if body.is_some() {
        headers.push(("content-type", "application/json"));
    }
    browser
        .raw(method, url, &headers, body.map(|b| b.to_string()))
        .await
}

fn assert_invalid(reply: &Reply, like: &Reply) {
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED, "{}", reply.body);
    assert_eq!(reply.json()["code"], "invalid_token", "{}", reply.body);
    assert_eq!(
        reply.json(),
        like.json(),
        "the same answer as a forged token"
    );
    assert_eq!(
        reply.headers.get("www-authenticate"),
        like.headers.get("www-authenticate")
    );
}

/// Sign in again with the (fake) Google in a fresh browser, through the web
/// app.
async fn sign_in_again(browser: &mut Browser, google: &campbooks_kernel::testing::TestIdp) {
    browser.forget_cookies();
    let to_issuer = browser.get("/bff/login?return_to=/").await;
    let screen = browser.get(&to_issuer.location()).await.location();
    let request = screen.trim_start_matches("/sign-in?request=").to_owned();
    let to_google = browser
        .get(&format!("/auth/start/google?request={request}"))
        .await;
    let to_bff = browser.get(&google.authorize(&to_google.location())).await;
    assert!(
        to_bff
            .location()
            .starts_with(&format!("{ORIGIN}/bff/callback")),
        "{}",
        to_bff.location()
    );
    let landed = browser.get(&to_bff.location()).await;
    assert!(
        landed.status.is_success() || landed.status.is_redirection(),
        "{}: {}",
        landed.status,
        landed.body
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_suspension_stops_issued_tokens_on_every_door(db: PgPool) {
    println!("\nAna's access token stops the moment an administrator suspends her:");
    let (mut browser, google, issuer) = world(db.clone()).await;
    sign_up_through_bff(&mut browser, &google).await;
    create_workspace(&mut browser, "Home").await;
    let ada = administrator(&db).await;
    let ana = the_person(&db, &[ada]).await;
    let token = mint(
        &db,
        &issuer,
        ana,
        "workspaces:read workspaces:write",
        Utc::now() - TimeDelta::minutes(1),
    )
    .await;
    let admin = mint(&db, &issuer, ada, "admin:access", Utc::now()).await;
    let forged = call(
        &browser,
        Method::GET,
        "/api/v1/workspaces",
        "a.b.c",
        None,
        None,
    )
    .await;

    // Before: the API, an idempotent create and the web app all work.
    let listed = call(
        &browser,
        Method::GET,
        "/api/v1/workspaces",
        &token,
        None,
        None,
    )
    .await;
    assert_eq!(listed.status, StatusCode::OK, "{}", listed.body);
    step("Ana signed up; her access token (10 min) -> GET /api/v1/workspaces: 200");
    let created = call(
        &browser,
        Method::POST,
        "/api/v1/workspaces",
        &token,
        Some("create-shop"),
        Some(json!({ "name": "Shop", "kind": "business" })),
    )
    .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.body);
    step("POST /api/v1/workspaces, Idempotency-Key create-shop: 201");
    assert_eq!(
        browser.get("/bff/api/v1/workspaces").await.status,
        StatusCode::OK
    );
    step("the web app (BFF session): 200");

    // An administrator suspends Ana through the admin API.
    let suspended = call(
        &browser,
        Method::POST,
        &format!("/admin/v1/identity/accounts/{ana}/suspend"),
        &admin,
        None,
        None,
    )
    .await;
    assert_eq!(suspended.status, StatusCode::OK, "{}", suspended.body);
    step(&format!(
        "admin POST /admin/v1/identity/accounts/{{ana}}/suspend: {}",
        suspended.status.as_u16()
    ));

    // At once (no job has run): every door refuses the token, as forged.
    assert_invalid(
        &call(
            &browser,
            Method::GET,
            "/api/v1/workspaces",
            &token,
            None,
            None,
        )
        .await,
        &forged,
    );
    step("same token, at once -> GET /api/v1/workspaces: 401 invalid_token (as a forged token)");
    // The idempotency layer replays nothing for it, not even its own key.
    let body = json!({ "name": "Shop", "kind": "business" });
    assert_invalid(
        &call(
            &browser,
            Method::POST,
            "/api/v1/workspaces",
            &token,
            Some("create-shop"),
            Some(body.clone()),
        )
        .await,
        &forged,
    );
    assert_invalid(
        &call(
            &browser,
            Method::POST,
            "/api/v1/workspaces",
            &token,
            Some("create-another"),
            Some(body),
        )
        .await,
        &forged,
    );
    // The web BFF's stored token too: the API refuses it before the BFF's
    // session is dropped by `identity.account_suspended`.
    let through_bff = browser.get("/bff/api/v1/workspaces").await;
    assert_eq!(
        through_bff.status,
        StatusCode::UNAUTHORIZED,
        "{}",
        through_bff.body
    );
    assert_eq!(through_bff.json()["code"], "invalid_token");
    step("replaying Idempotency-Key create-shop: 401; the web app's stored token: 401");

    // Unsuspended: the old token stays refused; a new sign-in works.
    let lifted = call(
        &browser,
        Method::POST,
        &format!("/admin/v1/identity/accounts/{ana}/unsuspend"),
        &admin,
        None,
        None,
    )
    .await;
    assert_eq!(lifted.status, StatusCode::OK, "{}", lifted.body);
    step(&format!(
        "admin POST /admin/v1/identity/accounts/{{ana}}/unsuspend: {}",
        lifted.status.as_u16()
    ));
    assert_invalid(
        &call(
            &browser,
            Method::GET,
            "/api/v1/workspaces",
            &token,
            None,
            None,
        )
        .await,
        &forged,
    );
    step("the old token after unsuspending: still 401 (nothing comes back)");
    sign_in_again(&mut browser, &google).await;
    let after = browser.get("/bff/api/v1/workspaces").await;
    assert_eq!(after.status, StatusCode::OK, "{}", after.body);
    step("Ana signs in again: her new token -> 200");
}

#[sqlx::test(migrations = "../../migrations")]
async fn the_admin_api_refuses_a_revoked_administrators_token(db: PgPool) {
    println!("\nAda, an administrator, signs out everywhere:");
    let (browser, _google, issuer) = world(db.clone()).await;
    let ada = administrator(&db).await;
    let scope = "admin:access identity:write";
    let token = mint(&db, &issuer, ada, scope, Utc::now()).await;
    let overview = "/admin/v1/system/overview";
    let forged = call(&browser, Method::GET, overview, "a.b.c", None, None).await;
    assert_eq!(forged.status, StatusCode::NOT_FOUND);
    let before = call(&browser, Method::GET, overview, &token, None, None).await;
    assert_eq!(before.status, StatusCode::OK, "{}", before.body);
    step("Ada's token -> GET /admin/v1/system/overview: 200");

    // Ada signs out everywhere: her token gets what a forged one gets (the
    // admin API answers 404 to every token it cannot accept).
    let ended = call(
        &browser,
        Method::POST,
        "/api/v1/identity/sign-out-everywhere",
        &token,
        None,
        None,
    )
    .await;
    assert_eq!(ended.status, StatusCode::NO_CONTENT, "{}", ended.body);
    let refused = call(&browser, Method::GET, overview, &token, None, None).await;
    assert_eq!(refused.status, StatusCode::NOT_FOUND);
    assert_eq!(refused.json(), forged.json());
    step(
        "POST /api/v1/identity/sign-out-everywhere: 204; the same token -> admin API: 404 (as forged)",
    );
    // A sign-in after it: works.
    let fresh = mint(&db, &issuer, ada, scope, Utc::now()).await;
    let again = call(&browser, Method::GET, overview, &fresh, None, None).await;
    assert_eq!(again.status, StatusCode::OK, "{}", again.body);
    step("a token from a sign-in right after, same second or not: 200");

    // Suspended (by an operator, below the admin API's own refusal to
    // suspend an administrator): refused the same way.
    let mut tx = db.begin().await.unwrap();
    suspension::suspend(
        &mut tx,
        &campbooks_kernel::EventBus::default(),
        ada,
        "operator",
        Utc::now(),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let refused = call(&browser, Method::GET, overview, &fresh, None, None).await;
    assert_eq!(refused.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn only_the_in_process_issuer_gets_the_revocation_port() {
    // An outside identity provider: its subjects are not identity accounts.
    let idp_config = campbooks_kernel::config::AuthConfig {
        issuer: Some("https://idp.example.com".to_owned()),
        ..campbooks_kernel::config::AuthConfig::default()
    };
    let mut config = campbooks_kernel::testing::config("postgres://test@localhost/test");
    config.auth = idp_config;
    let pool = campbooks_kernel::testing::unreachable_pool();
    let state = crate::with_composed_ports(
        campbooks_kernel::AppState::new(pool, config),
        &crate::Composition::default(),
    );
    let debug = format!("{:?}", state.auth);
    assert!(debug.contains("idp.example.com"), "{debug}");
    assert!(debug.contains("revocation: false"), "{debug}");
}
