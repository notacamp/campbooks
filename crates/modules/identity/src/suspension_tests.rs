//! End to end through the issuer: a suspended account gets in by no door
//! (provider sign-in, the issuer session, a code, a refresh token), and an
//! unsuspended one signs in afresh.

use axum::http::StatusCode;
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    domain::suspension::{self, Outcome},
    testkit::{Kit, ORIGIN},
};

async fn count(kit: &Kit, sql: &'static str, account: Uuid) -> i64 {
    sqlx::query_scalar(sql)
        .bind(account)
        .fetch_one(&kit.kernel)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_suspended_account_gets_in_by_no_door_until_it_is_unsuspended(db: PgPool) {
    let kit = Kit::new(db).await;
    let (code, verifier) = kit.sign_up().await;
    let tokens = kit.redeem(&code, &verifier).await;
    let refresh = tokens.json["refresh_token"].as_str().unwrap().to_owned();
    let (_, claims) = Kit::decode(tokens.json["access_token"].as_str().unwrap());
    let account: Uuid = claims["sub"].as_str().unwrap().parse().unwrap();
    // A code handed out before the suspension, not redeemed yet.
    let (url, early_verifier) = Kit::web_authorize();
    let early = kit.get(&url).await.location_query()["code"].clone();

    let mut tx = kit.state.db.begin().await.unwrap();
    let outcome = suspension::suspend(&mut tx, &kit.state.events, account, "root", Utc::now())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(outcome, Outcome::Changed);

    // Refresh tokens and codes are refused.
    assert_eq!(kit.refresh(&refresh).await.json["error"], "invalid_grant");
    assert_eq!(
        kit.redeem(&early, &early_verifier).await.json["error"],
        "invalid_grant"
    );
    // The issuer session is over: no code at once, the sign-in screen opens.
    let (url, _) = Kit::web_authorize();
    let to_sign_in = kit.get(&url).await;
    assert!(
        to_sign_in.location().starts_with("/sign-in?request="),
        "{to_sign_in:?}"
    );
    // And signing in again with the provider says why.
    let request = to_sign_in.location_query()["request"].clone();
    let refused = kit.round_trip(Some(&request)).await;
    assert_eq!(
        refused.location_query()["error"],
        "account_suspended",
        "{refused:?}"
    );

    assert_eq!(
        count(
            &kit,
            "SELECT count(*) FROM identity.sessions WHERE account_id = $1 AND ended_at IS NULL",
            account
        )
        .await,
        0
    );
    assert_eq!(
        count(
            &kit,
            "SELECT count(*) FROM identity.refresh_families
              WHERE account_id = $1 AND revoke_reason = 'suspended'",
            account
        )
        .await,
        1
    );
    assert_eq!(
        count(
            &kit,
            "SELECT count(*) FROM kernel.events
              WHERE name = 'identity.account_suspended' AND (payload->>'account')::uuid = $1",
            account
        )
        .await,
        1
    );

    // Again: nothing changes, nothing is recorded twice.
    let mut tx = kit.state.db.begin().await.unwrap();
    let again = suspension::suspend(&mut tx, &kit.state.events, account, "root", Utc::now())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(again, Outcome::Unchanged);
    assert_eq!(
        count(
            &kit,
            "SELECT count(*) FROM identity.audit_events
              WHERE account_id = $1 AND kind = 'account_suspended'",
            account
        )
        .await,
        1
    );

    // Unsuspended: the same way in signs in afresh.
    let mut tx = kit.state.db.begin().await.unwrap();
    let lifted = suspension::unsuspend(&mut tx, &kit.state.events, account, "root", Utc::now())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(lifted, Outcome::Changed);
    let (url, verifier) = Kit::web_authorize();
    let request = kit.get(&url).await.location_query()["request"].clone();
    let back = kit.round_trip(Some(&request)).await;
    assert!(
        back.location()
            .starts_with(&format!("{ORIGIN}/bff/callback?code=")),
        "{back:?}"
    );
    let tokens = kit.redeem(&back.location_query()["code"], &verifier).await;
    assert_eq!(tokens.status, StatusCode::OK, "{tokens:?}");

    let mut tx = kit.state.db.begin().await.unwrap();
    assert_eq!(
        suspension::unsuspend(&mut tx, &kit.state.events, account, "root", Utc::now())
            .await
            .unwrap(),
        Outcome::Unchanged
    );
    assert_eq!(
        suspension::suspend(
            &mut tx,
            &kit.state.events,
            Uuid::now_v7(),
            "root",
            Utc::now()
        )
        .await
        .unwrap(),
        Outcome::Missing
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn suspending_and_lifting_publish_events_ordered_by_at(db: PgPool) {
    let kit = Kit::new(db).await;
    let (code, verifier) = kit.sign_up().await;
    let tokens = kit.redeem(&code, &verifier).await;
    let (_, claims) = Kit::decode(tokens.json["access_token"].as_str().unwrap());
    let account: Uuid = claims["sub"].as_str().unwrap().parse().unwrap();
    // One clock for both: `at` orders them, not the caller's time.
    let now = Utc::now();
    for lift in [false, true, true] {
        let mut tx = kit.state.db.begin().await.unwrap();
        if lift {
            suspension::unsuspend(&mut tx, &kit.state.events, account, "root", now)
                .await
                .unwrap();
        } else {
            suspension::suspend(&mut tx, &kit.state.events, account, "root", now)
                .await
                .unwrap();
        }
        tx.commit().await.unwrap();
    }
    let events: Vec<(String, serde_json::Value)> = sqlx::query_as(
        "SELECT name, payload FROM kernel.events
          WHERE name IN ('identity.account_suspended', 'identity.account_unsuspended')
            AND payload ->> 'account' = $1::text
          ORDER BY id",
    )
    .bind(account)
    .fetch_all(&kit.kernel)
    .await
    .unwrap();
    let names: Vec<&str> = events.iter().map(|(n, _)| n.as_str()).collect();
    // Lifting twice publishes once.
    assert_eq!(
        names,
        ["identity.account_suspended", "identity.account_unsuspended"]
    );
    let at = |i: usize| {
        events[i].1["at"]
            .as_str()
            .unwrap()
            .parse::<chrono::DateTime<Utc>>()
            .unwrap()
    };
    assert!(at(1) > at(0), "{events:?}");
}
