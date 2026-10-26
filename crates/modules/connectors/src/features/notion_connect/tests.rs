//! The browser's round trip to Notion and back: the state is single use and
//! bound to the browser, a consent expires, and the tokens are stored
//! sealed, bound to their connection and column.

use axum::http::{StatusCode, header};
use campbooks_kernel::{
    crypto::{Cipher, SecretKey},
    tenancy::{Role, WorkspaceId},
};
use chrono::TimeDelta;
use serde_json::json;
use sqlx::PgPool;

use super::safe_return_to;
use crate::{deps::CREDENTIALS_PURPOSE, notion::CALLBACK_PATH, testkit::Kit};

fn location(response: &campbooks_kernel::testing::TestResponse) -> String {
    response.headers[header::LOCATION]
        .to_str()
        .unwrap()
        .to_owned()
}

fn cipher() -> Cipher {
    Cipher::new(&SecretKey::from_bytes([3; 32]), CREDENTIALS_PURPOSE)
}

async fn started(kit: &Kit, who: &crate::testkit::Caller, ws: WorkspaceId) -> String {
    let started = kit.notion_start(who, ws, json!({})).await;
    assert_eq!(started.status, StatusCode::CREATED, "{:?}", started.json);
    started.json["authorize_url"].as_str().unwrap().to_owned()
}

async fn connections(kit: &Kit) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM connectors.connections WHERE provider = 'notion'")
        .fetch_one(&kit.db)
        .await
        .unwrap()
}

#[test]
fn only_paths_of_this_app_are_safe_to_return_to() {
    assert!(safe_return_to("/"));
    assert!(safe_return_to("/w/1/settings/mailboxes?tab=x"));
    assert!(!safe_return_to(""));
    assert!(!safe_return_to("//evil.example.com"));
    assert!(!safe_return_to("https://evil.example.com"));
    assert!(!safe_return_to("/a\\b"));
    assert!(!safe_return_to("/a\nb"));
    assert!(!safe_return_to(&format!("/{}", "a".repeat(300))));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_round_trip_connects_notion_and_returns_to_the_page_that_started_it(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    let to = kit
        .connect_notion_browser(&ana, ws, json!({"return_to": "/w/acme/settings/mailboxes"}))
        .await;
    assert!(
        to.starts_with("/w/acme/settings/mailboxes?connected="),
        "{to}"
    );
    assert_eq!(connections(&kit).await, 1);
    let connection = kit.connect_notion(&ana, ws).await;
    // Reconnecting the same Notion account refreshes the connection.
    assert_eq!(connections(&kit).await, 1);
    assert_eq!(connection["provider"], "notion");
    assert_eq!(connection["display_name"], "Acme");
    assert_eq!(connection["capabilities"], json!(["pages"]));
    assert_eq!(connection["health"], "ok");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_ticket_sends_the_browser_to_notion_with_a_state_and_a_cookie_bound_to_it(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    let ticket = started(&kit, &ana, ws).await;
    let sent = kit.notion_get(&ticket, None).await;
    assert_eq!(sent.status, StatusCode::SEE_OTHER);
    let to = url::Url::parse(&location(&sent)).unwrap();
    assert_eq!(to.path(), "/v1/oauth/authorize");
    let pairs: std::collections::HashMap<_, _> = to.query_pairs().into_owned().collect();
    assert_eq!(pairs["response_type"], "code");
    assert_eq!(pairs["code_challenge_method"], "S256");
    assert!(pairs["state"].len() > 40);
    let cookie = sent.headers[header::SET_COOKIE].to_str().unwrap();
    assert!(cookie.starts_with("cb_notion_flow="));
    assert!(cookie.contains("HttpOnly") && cookie.contains("SameSite=Lax"));
    assert!(cookie.contains("Path=/auth"));
    // Only hashes are kept: neither the state nor the cookie value is in
    // the row, and the verifier is sealed.
    let (state_hash, browser_hash, verifier): (Vec<u8>, Vec<u8>, Vec<u8>) = sqlx::query_as(
        "SELECT state_hash, browser_hash, pkce_verifier FROM connectors.notion_authorizations",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    let (state, cookie) = Kit::consent_of(&sent);
    let value = cookie.trim_start_matches("cb_notion_flow=");
    for kept in [&state_hash, &browser_hash, &verifier] {
        let text = String::from_utf8_lossy(kept).into_owned();
        assert!(!text.contains(&state) && !text.contains(value));
    }
    assert_eq!(state_hash.len(), 32);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_ticket_works_once(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    let ticket = started(&kit, &ana, ws).await;
    assert_eq!(
        kit.notion_get(&ticket, None).await.status,
        StatusCode::SEE_OTHER
    );
    let again = kit.notion_get(&ticket, None).await;
    assert_eq!(again.status, StatusCode::SEE_OTHER);
    assert!(
        location(&again).contains("link_error=link_expired"),
        "{}",
        location(&again)
    );
    assert!(
        again
            .headers
            .get_all(header::SET_COOKIE)
            .iter()
            .any(|c| { c.to_str().unwrap().contains("Max-Age=0") })
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_forged_or_swapped_ticket_or_state_is_refused(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    let forged = kit.notion_get("/auth/start/notion?ticket=AAAA", None).await;
    assert!(location(&forged).contains("link_error=link_expired"));
    let none = kit.notion_get("/auth/start/notion", None).await;
    assert!(location(&none).contains("link_error=link_expired"));

    // A ticket is not a state: the two are sealed for different purposes.
    let ticket = started(&kit, &ana, ws).await;
    let sealed = ticket.split("ticket=").nth(1).unwrap().to_owned();
    let sent = kit.notion_get(&ticket, None).await;
    let (_, cookie) = Kit::consent_of(&sent);
    let code = kit.notion.issue_code();
    let back = kit
        .notion_get(
            &format!("{CALLBACK_PATH}?code={code}&state={sealed}"),
            Some(&cookie),
        )
        .await;
    assert!(
        location(&back).contains("link_error=link_expired"),
        "{}",
        location(&back)
    );
    assert_eq!(connections(&kit).await, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_replayed_state_is_refused(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    let ticket = started(&kit, &ana, ws).await;
    let sent = kit.notion_get(&ticket, None).await;
    let (state, cookie) = Kit::consent_of(&sent);
    let uri = |code: &str| format!("{CALLBACK_PATH}?code={code}&state={state}");
    let first = kit
        .notion_get(&uri(&kit.notion.issue_code()), Some(&cookie))
        .await;
    assert!(
        location(&first).contains("connected="),
        "{}",
        location(&first)
    );
    let replay = kit
        .notion_get(&uri(&kit.notion.issue_code()), Some(&cookie))
        .await;
    assert!(
        location(&replay).contains("link_error=link_expired"),
        "{}",
        location(&replay)
    );
    assert_eq!(connections(&kit).await, 1);
    // The consent was spent by the first.
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM connectors.notion_authorizations")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert_eq!(left, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn another_browser_cannot_finish_a_consent(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    let sent = kit.notion_get(&started(&kit, &ana, ws).await, None).await;
    let (state, cookie) = Kit::consent_of(&sent);
    let uri = format!(
        "{CALLBACK_PATH}?code={}&state={state}",
        kit.notion.issue_code()
    );
    // No cookie at all, and the cookie of another browser's round trip.
    let bare = kit.notion_get(&uri, None).await;
    assert!(location(&bare).contains("link_error=link_expired"));
    let other = kit
        .notion_get(&uri, Some("cb_notion_flow=some-other-browsers-value"))
        .await;
    assert!(location(&other).contains("link_error=link_expired"));
    assert_eq!(connections(&kit).await, 0);
    // The consent was left as it was: the right browser can still finish.
    let right = kit.notion_get(&uri, Some(&cookie)).await;
    assert!(
        location(&right).contains("connected="),
        "{}",
        location(&right)
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_expired_consent_is_refused(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    let ticket = started(&kit, &ana, ws).await;
    kit.clock.advance(TimeDelta::minutes(11));
    let late = kit.notion_get(&ticket, None).await;
    assert!(location(&late).contains("link_error=link_expired"));

    // Armed in time, finished too late.
    let ticket = started(&kit, &ana, ws).await;
    let sent = kit.notion_get(&ticket, None).await;
    let (state, cookie) = Kit::consent_of(&sent);
    kit.clock.advance(TimeDelta::minutes(11));
    let back = kit
        .notion_get(
            &format!(
                "{CALLBACK_PATH}?code={}&state={state}",
                kit.notion.issue_code()
            ),
            Some(&cookie),
        )
        .await;
    assert!(location(&back).contains("link_error=link_expired"));
    assert_eq!(connections(&kit).await, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_member_who_says_no_at_notion_comes_back_with_a_short_code(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    let sent = kit.notion_get(&started(&kit, &ana, ws).await, None).await;
    let (state, cookie) = Kit::consent_of(&sent);
    let back = kit
        .notion_get(
            &format!("{CALLBACK_PATH}?error=access_denied&state={state}"),
            Some(&cookie),
        )
        .await;
    assert!(
        location(&back).contains("link_error=cancelled_at_provider"),
        "{}",
        location(&back)
    );
    assert_eq!(connections(&kit).await, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_code_notion_refuses_is_a_failed_login_and_leaks_nothing(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    let sent = kit.notion_get(&started(&kit, &ana, ws).await, None).await;
    let (state, cookie) = Kit::consent_of(&sent);
    let back = kit
        .notion_get(
            &format!("{CALLBACK_PATH}?code=forged&state={state}"),
            Some(&cookie),
        )
        .await;
    let to = location(&back);
    assert!(to.contains("link_error=login_failed"), "{to}");
    assert!(!to.contains("forged") && !to.contains(&state));
    assert_eq!(connections(&kit).await, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn tokens_are_stored_sealed_and_bound_to_their_connection_and_column(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    let bea = kit.user("bea");
    kit.join(&bea, ws, Role::Member);
    let mine = kit.connect_notion(&ana, ws).await;
    let id: uuid::Uuid = mine["id"].as_str().unwrap().parse().unwrap();
    let (refresh, access): (Vec<u8>, Option<Vec<u8>>) = sqlx::query_as(
        "SELECT refresh_token, access_token FROM connectors.credentials WHERE connection_id = $1",
    )
    .bind(id)
    .fetch_one(&kit.db)
    .await
    .unwrap();
    let access = access.unwrap();
    for sealed in [&refresh, &access] {
        let text = String::from_utf8_lossy(sealed).into_owned();
        assert!(
            !text.contains("fake-notion"),
            "a token is stored in the clear"
        );
    }
    let cipher = cipher();
    let context =
        |column: &str, id: uuid::Uuid| format!("connectors.credentials:{column}:{id}").into_bytes();
    let opened = cipher
        .open(&refresh, &context("refresh_token", id))
        .unwrap();
    assert!(opened.starts_with("fake-notion-refresh"));
    // Not under another connection's context, nor the other column's.
    assert!(
        cipher
            .open(&refresh, &context("refresh_token", uuid::Uuid::now_v7()))
            .is_err()
    );
    assert!(cipher.open(&refresh, &context("access_token", id)).is_err());
    assert!(cipher.open(&access, &context("refresh_token", id)).is_err());
    // The API never shows them.
    let shown = serde_json::to_string(&mine).unwrap();
    assert!(!shown.contains("fake-notion"));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_second_member_cannot_take_over_a_notion_account_already_connected(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    let bea = kit.user("bea");
    kit.join(&bea, ws, Role::Member);
    kit.connect_notion(&ana, ws).await;
    let to = kit.connect_notion_browser(&bea, ws, json!({})).await;
    assert!(
        to.contains("link_error=account_connected_by_another_member"),
        "{to}"
    );
    assert_eq!(connections(&kit).await, 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn without_a_refresh_token_the_connection_is_refused(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    kit.notion.withhold_refresh_token();
    let to = kit.connect_notion_browser(&ana, ws, json!({})).await;
    assert!(to.contains("link_error=no_offline_access"), "{to}");
    assert_eq!(connections(&kit).await, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_routes_are_absent_unless_notion_is_offered(db: PgPool) {
    let kit = Kit::new(db).await;
    let gone = kit.notion_get("/auth/start/notion?ticket=x", None).await;
    assert_eq!(gone.status, StatusCode::NOT_FOUND);
    let gone = kit.notion_get(CALLBACK_PATH, None).await;
    assert_eq!(gone.status, StatusCode::NOT_FOUND);
}
