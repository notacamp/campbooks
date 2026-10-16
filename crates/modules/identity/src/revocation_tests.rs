//! Access tokens already issued stop at a revocation (campbooks-cloud#122):
//! suspension and sign out everywhere move the account's epoch, the API
//! (the kit's verifier is wired as the server wires it) refuses older
//! tokens at once, unsuspending revives none, and the issuer mints nothing
//! more for a sign-in from before while a new sign-in works at once.

use axum::http::{Method, StatusCode};
use chrono::{DateTime, TimeDelta, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    domain::{accounts, suspension, tokens},
    testkit::{Kit, ORIGIN},
};

const WAYS_IN: &str = "/api/v1/identity/ways-in";
const SIGN_OUT_EVERYWHERE: &str = "/api/v1/identity/sign-out-everywhere";

/// The account `access` was issued to.
fn account_of(access: &str) -> Uuid {
    let (_, claims) = Kit::decode(access);
    claims["sub"].as_str().unwrap().parse().unwrap()
}

/// An access token like `access` (its account, client, scopes), for a
/// sign-in made at `auth_time`, minted at `now`: the issuer's clock seam.
async fn mint(kit: &Kit, access: &str, auth_time: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let (_, claims) = Kit::decode(access);
    let text = |name: &str| claims[name].as_str().unwrap_or_default().to_owned();
    let grant = tokens::Grant {
        client_id: text("client_id"),
        account_id: account_of(access),
        session_id: None,
        scope: text("scope"),
        auth_time,
        resource: text("aud"),
        workspace: None,
    };
    let issuer = kit.deps.issuer.clone().unwrap();
    let profile = accounts::profile(&kit.state.db, grant.account_id)
        .await
        .unwrap()
        .unwrap();
    let signer = issuer.keys.signer(&kit.state.db).await.unwrap();
    tokens::Mint {
        issuer: &issuer.url,
        signer: &signer,
        grant: &grant,
        profile: &profile,
        amr: &[],
        now,
    }
    .access_token()
    .unwrap()
}

async fn suspend(kit: &Kit, account: Uuid, now: DateTime<Utc>) {
    let mut tx = kit.state.db.begin().await.unwrap();
    suspension::suspend(&mut tx, &kit.state.events, account, "root", now)
        .await
        .unwrap();
    tx.commit().await.unwrap();
}

async fn unsuspend(kit: &Kit, account: Uuid) {
    let mut tx = kit.state.db.begin().await.unwrap();
    suspension::unsuspend(&mut tx, account, "root", Utc::now())
        .await
        .unwrap();
    tx.commit().await.unwrap();
}

/// Sign in again with Google in this browser and redeem the code: the
/// response of the token endpoint.
async fn sign_in_again(kit: &Kit) -> crate::testkit::Response {
    let (url, verifier) = Kit::web_authorize();
    let request = kit.get(&url).await.location_query()["request"].clone();
    let back = kit.round_trip(Some(&request)).await;
    assert!(
        back.location()
            .starts_with(&format!("{ORIGIN}/bff/callback?code=")),
        "{back:?}"
    );
    kit.redeem(&back.location_query()["code"], &verifier).await
}

fn assert_refused(reply: &crate::testkit::Response) {
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED, "{reply:?}");
    assert_eq!(reply.json["code"], "invalid_token", "{reply:?}");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn suspension_stops_issued_tokens_and_unsuspending_revives_none(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let account = account_of(&access);
    assert_eq!(
        kit.api(Method::GET, WAYS_IN, &access).await.status,
        StatusCode::OK
    );

    suspend(&kit, account, Utc::now()).await;
    // At once, before any job runs, as an invalid token.
    let refused = kit.api(Method::GET, WAYS_IN, &access).await;
    assert_refused(&refused);
    let forged = kit.api(Method::GET, WAYS_IN, "a.b.c").await;
    assert_eq!(
        refused.json, forged.json,
        "nothing says the account is suspended"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM identity.accounts WHERE id = $1 AND tokens_not_before IS NOT NULL"
        )
        .bind(account)
        .fetch_one(&kit.state.db)
        .await
        .unwrap(),
        1
    );

    unsuspend(&kit, account).await;
    assert_refused(&kit.api(Method::GET, WAYS_IN, &access).await);
    // A new sign-in, perhaps in the same second as the suspension: works.
    let tokens = sign_in_again(&kit).await;
    assert_eq!(tokens.status, StatusCode::OK, "{tokens:?}");
    let fresh = tokens.json["access_token"].as_str().unwrap();
    assert_eq!(
        kit.api(Method::GET, WAYS_IN, fresh).await.status,
        StatusCode::OK
    );
    let (_, claims) = Kit::decode(fresh);
    assert_eq!(
        claims["exp"].as_i64().unwrap() - claims["iat"].as_i64().unwrap(),
        600,
        "ten minutes after iat"
    );
    // And the old one stays refused.
    assert_refused(&kit.api(Method::GET, WAYS_IN, &access).await);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn signing_out_everywhere_stops_issued_tokens_and_unredeemed_codes(db: PgPool) {
    let kit = Kit::new(db).await;
    let (code, verifier) = kit.sign_up().await;
    let tokens = kit.redeem(&code, &verifier).await;
    let access = tokens.json["access_token"].as_str().unwrap().to_owned();
    let refresh = tokens.json["refresh_token"].as_str().unwrap().to_owned();
    // A code handed out before, not redeemed yet.
    let (url, early_verifier) = Kit::web_authorize();
    let early = kit.get(&url).await.location_query()["code"].clone();

    let ended = kit.api(Method::POST, SIGN_OUT_EVERYWHERE, &access).await;
    assert_eq!(ended.status, StatusCode::NO_CONTENT, "{ended:?}");

    assert_refused(&kit.api(Method::GET, WAYS_IN, &access).await);
    assert_eq!(kit.refresh(&refresh).await.json["error"], "invalid_grant");
    assert_eq!(
        kit.redeem(&early, &early_verifier).await.json["error"],
        "invalid_grant",
        "a sign-in from before mints nothing more"
    );
    // A sign-in after it works at once.
    let tokens = sign_in_again(&kit).await;
    assert_eq!(tokens.status, StatusCode::OK, "{tokens:?}");
    let fresh = tokens.json["access_token"].as_str().unwrap();
    assert_eq!(
        kit.api(Method::GET, WAYS_IN, fresh).await.status,
        StatusCode::OK
    );
    assert_refused(&kit.api(Method::GET, WAYS_IN, &access).await);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_revocations_own_second(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let account = account_of(&access);
    // A whole second a little ahead of the clock (minted tokens may be up
    // to the verifier's leeway ahead), the revocation half-way through it.
    let second = DateTime::from_timestamp(Utc::now().timestamp() + 5, 0).unwrap();
    let ms = |n: i64| second + TimeDelta::milliseconds(n);
    let old_sign_in = Utc::now() - TimeDelta::minutes(5);
    // Minted before the revocation, in its second.
    let before = mint(&kit, &access, old_sign_in, ms(200)).await;
    assert_eq!(
        kit.api(Method::GET, WAYS_IN, &before).await.status,
        StatusCode::OK
    );

    suspend(&kit, account, ms(500)).await;
    unsuspend(&kit, account).await;
    let epoch: DateTime<Utc> =
        sqlx::query_scalar("SELECT tokens_not_before FROM identity.accounts WHERE id = $1")
            .bind(account)
            .fetch_one(&kit.state.db)
            .await
            .unwrap();
    assert_eq!(epoch, ms(500), "the clock seam set the epoch");

    // Earlier in that second: refused.
    assert_refused(&kit.api(Method::GET, WAYS_IN, &before).await);
    // An older sign-in minted later in that second carries the same second:
    // refused too (the token endpoint would mint nothing for it anyway).
    let late_old = mint(&kit, &access, old_sign_in, ms(800)).await;
    assert_eq!(Kit::decode(&late_old).1["iat"], second.timestamp());
    assert_refused(&kit.api(Method::GET, WAYS_IN, &late_old).await);
    // A sign-in after the revocation, minted in that same second: stamped
    // with the next second, and accepted.
    let fresh = mint(&kit, &access, ms(700), ms(800)).await;
    let (_, claims) = Kit::decode(&fresh);
    assert_eq!(claims["iat"], second.timestamp() + 1);
    assert_eq!(claims["nbf"], second.timestamp(), "nbf is the real time");
    assert_eq!(
        kit.api(Method::GET, WAYS_IN, &fresh).await.status,
        StatusCode::OK
    );
    // And in a later second, as it comes.
    let later = mint(&kit, &access, ms(700), ms(1_200)).await;
    assert_eq!(Kit::decode(&later).1["iat"], second.timestamp() + 1);
    assert_eq!(
        kit.api(Method::GET, WAYS_IN, &later).await.status,
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_epoch_never_moves_back(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let account = account_of(&access);
    // Whole microseconds, as Postgres keeps them (Linux clocks have more).
    let ahead =
        DateTime::from_timestamp_micros((Utc::now() + TimeDelta::seconds(30)).timestamp_micros())
            .unwrap();
    suspend(&kit, account, ahead).await;
    unsuspend(&kit, account).await;
    // A revocation stamped earlier (another process's clock) keeps the later epoch.
    let mut tx = kit.state.db.begin().await.unwrap();
    accounts::lock(&mut tx, account).await.unwrap();
    let kept = accounts::revoke_tokens(&mut tx, account, Utc::now() - TimeDelta::hours(1))
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(kept, ahead);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_workspace_clients_tokens_are_not_the_persons(db: PgPool) {
    let kit = Kit::new(db).await;
    let web = kit.signed_up().await;
    let home = crate::testkit::HOME;
    let created = kit
        .api_json(
            Method::POST,
            &format!("/api/v1/w/{home}/identity/clients"),
            &web,
            &serde_json::json!({ "name": "Nightly sync", "type": "confidential",
                "scopes": ["conversations:read"] }),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{created:?}");
    let client = created.json["client"]["client_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let secret = created.json["client_secret"].as_str().unwrap().to_owned();
    let issued = kit
        .post_form(
            "/oauth2/token",
            &[
                ("grant_type", "client_credentials"),
                ("client_id", &client),
                ("client_secret", &secret),
            ],
        )
        .await;
    let token = issued.json["access_token"].as_str().unwrap().to_owned();

    // The member who registered it signs out everywhere: their own tokens
    // stop, the workspace client's do not (it is no session of theirs).
    let ended = kit.api(Method::POST, SIGN_OUT_EVERYWHERE, &web).await;
    assert_eq!(ended.status, StatusCode::NO_CONTENT);
    assert_refused(&kit.api(Method::GET, WAYS_IN, &web).await);
    let principal = kit.state.auth.verify(&token).await.unwrap();
    assert_eq!(
        principal.kind,
        campbooks_kernel::auth::PrincipalKind::Client
    );
}
