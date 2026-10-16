//! Signing out everywhere: every session and refresh token of the account
//! ends, the event tells the web BFF; a bound token cannot do it.

use axum::http::{Method, StatusCode};
use sqlx::PgPool;

use crate::testkit::{HOME, Kit};

const PATH: &str = "/api/v1/identity/sign-out-everywhere";

#[sqlx::test(migrations = "../../../migrations")]
async fn every_sign_in_of_the_account_ends(db: PgPool) {
    let kit = Kit::new(db).await;
    let (code, verifier) = kit.sign_up().await;
    let here = kit.redeem(&code, &verifier).await;
    let access = here.json["access_token"].as_str().unwrap().to_owned();
    let refresh_here = here.json["refresh_token"].as_str().unwrap().to_owned();

    // Another browser (a stolen session, say) holds its own refresh token.
    kit.new_browser();
    let (url, verifier) = Kit::web_authorize();
    let request = kit.get(&url).await.location_query()["request"].clone();
    let signed_in = kit.round_trip(Some(&request)).await;
    let there = kit
        .redeem(&signed_in.location_query()["code"], &verifier)
        .await;
    let refresh_there = there.json["refresh_token"].as_str().unwrap().to_owned();

    // A bound token (an app in a workspace) cannot: 404, nothing ends.
    let bound = kit.bound(&access, HOME).await;
    let refused = kit.api(Method::POST, PATH, &bound).await;
    assert_eq!(refused.status, StatusCode::NOT_FOUND, "{refused:?}");
    let live: i64 =
        sqlx::query_scalar("SELECT count(*) FROM identity.sessions WHERE ended_at IS NULL")
            .fetch_one(&kit.state.db)
            .await
            .unwrap();
    assert_eq!(live, 2);

    let ended = kit.api(Method::POST, PATH, &access).await;
    assert_eq!(ended.status, StatusCode::NO_CONTENT, "{ended:?}");
    for refresh in [&refresh_here, &refresh_there] {
        let refused = kit.refresh(refresh).await;
        assert_eq!(refused.json["error"], "invalid_grant", "{refused:?}");
    }
    let (url, _) = Kit::web_authorize();
    assert!(
        kit.get(&url)
            .await
            .location()
            .starts_with("/sign-in?request="),
        "the other browser is signed out of the issuer"
    );
    let (events, audits): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM kernel.events
                  WHERE name = 'identity.signed_out_everywhere'),
                (SELECT count(*) FROM identity.audit_events
                  WHERE kind = 'signed_out_everywhere')",
    )
    .fetch_one(&kit.kernel)
    .await
    .unwrap();
    assert_eq!((events, audits), (1, 1));
}
