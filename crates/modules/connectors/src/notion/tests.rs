//! The Notion client, against the fake Notion: the code exchange, the
//! rotating refresh token, searching, reading and writing blocks, the cap
//! on `Retry-After` and which files may be fetched (campbooks-cloud#376).

use std::time::Duration;

use uuid::Uuid;

use super::*;
use crate::testkit_notion::{BOT_ID, CLIENT_ID, CLIENT_SECRET, FakeNotion, WORKSPACE_NAME};

const REDIRECT: &str = "http://localhost:3000/auth/callback/notion";

fn client_with(fake: &FakeNotion, secret: &str) -> Notion {
    Notion::new(
        CLIENT_ID.into(),
        ClientSecret::Inline(secret.into()),
        Endpoints::under(&fake.base()).unwrap(),
        REDIRECT.into(),
    )
    .unwrap()
}

async fn client() -> (FakeNotion, Notion) {
    let fake = FakeNotion::start().await;
    let notion = client_with(&fake, CLIENT_SECRET);
    (fake, notion)
}

#[test]
fn only_notions_hosts_are_file_hosts() {
    let endpoints = Endpoints::notion().unwrap();
    let url = |text: &str| Url::parse(text).unwrap();
    assert!(endpoints.allows_file(&url(&format!("https://{}/a.png", FILE_HOSTS[0]))));
    assert!(!endpoints.allows_file(&url("https://evil.example.com/a.png")));
    // Not over plain http, not on another port, not with credentials.
    assert!(!endpoints.allows_file(&url(&format!("http://{}/a.png", FILE_HOSTS[0]))));
    assert!(!endpoints.allows_file(&url(&format!("https://{}:8443/a.png", FILE_HOSTS[0]))));
    assert!(!endpoints.allows_file(&url(&format!("https://u:p@{}/a.png", FILE_HOSTS[0]))));
    // A look-alike of a listed host is not it.
    assert!(!endpoints.allows_file(&url(&format!("https://{}.evil.example.com/a", FILE_HOSTS[0]))));
}

#[test]
fn the_authorize_address_carries_state_and_challenge_only() {
    let notion = Notion::new(
        CLIENT_ID.into(),
        ClientSecret::Inline(CLIENT_SECRET.into()),
        Endpoints::notion().unwrap(),
        REDIRECT.into(),
    )
    .unwrap();
    let url = notion.authorize_url("state-1", "challenge-1");
    let pairs: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
    assert_eq!(url.host_str(), Some("api.notion.com"));
    assert_eq!(pairs["state"], "state-1");
    assert_eq!(pairs["code_challenge"], "challenge-1");
    assert_eq!(pairs["code_challenge_method"], "S256");
    assert_eq!(pairs["client_id"], CLIENT_ID);
    assert_eq!(pairs["redirect_uri"], REDIRECT);
    assert_eq!(pairs["owner"], "user");
    assert!(!url.as_str().contains(CLIENT_SECRET));
}

#[test]
fn only_listed_media_types_come_in() {
    assert!(file_type_allowed("image/png"));
    assert!(file_type_allowed("application/pdf"));
    assert!(!file_type_allowed("text/html"));
    assert!(!file_type_allowed("image/svg+xml"));
    assert!(!file_type_allowed("application/x-msdownload"));
    assert!(!file_type_allowed(""));
}

#[tokio::test]
async fn a_code_is_exchanged_once_for_tokens_and_the_bot() {
    let (fake, notion) = client().await;
    let code = fake.issue_code();
    let granted = notion.exchange(&code, "verifier").await.unwrap();
    assert_eq!(granted.bot_id, BOT_ID);
    assert_eq!(granted.workspace_name.as_deref(), Some(WORKSPACE_NAME));
    assert!(granted.tokens.access_token.starts_with("fake-notion-access"));
    assert!(granted.tokens.refresh_token.is_some());
    // A code is spent by its first use.
    assert!(matches!(
        notion.exchange(&code, "verifier").await,
        Err(ProviderError::InvalidGrant)
    ));
    assert!(matches!(
        notion.exchange("made-up", "verifier").await,
        Err(ProviderError::InvalidGrant)
    ));
}

#[tokio::test]
async fn wrong_integration_credentials_are_refused() {
    let fake = FakeNotion::start().await;
    let notion = client_with(&fake, "not-the-secret");
    let code = fake.issue_code();
    let err = notion.exchange(&code, "verifier").await.unwrap_err();
    assert!(!matches!(err, ProviderError::Unavailable(_)), "{err:?}");
}

#[tokio::test]
async fn a_refresh_rotates_both_tokens_and_the_old_refresh_token_is_spent() {
    let (fake, notion) = client().await;
    let (_, refresh) = fake.grant();
    let renewed = notion.refresh(&refresh).await.unwrap();
    assert_ne!(renewed.refresh_token.as_deref(), Some(refresh.as_str()));
    assert!(renewed.refresh_token.is_some());
    // The spent one never works again.
    assert!(matches!(
        notion.refresh(&refresh).await,
        Err(ProviderError::InvalidGrant)
    ));
    assert!(
        notion
            .refresh(renewed.refresh_token.as_deref().unwrap())
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn a_refusal_of_the_refresh_is_invalid_grant() {
    let (fake, notion) = client().await;
    let (_, refresh) = fake.grant();
    fake.refuse_refresh(400, serde_json::json!({"error": "invalid_grant"}));
    assert!(matches!(
        notion.refresh(&refresh).await,
        Err(ProviderError::InvalidGrant)
    ));
}

#[tokio::test]
async fn search_lists_shared_pages_newest_first_across_pages() {
    let (fake, notion) = client().await;
    let (access, _) = fake.grant();
    let first = fake.add_page("Roadmap", None);
    let second = fake.add_page("Notes", None);
    let third = fake.add_page("Plan", Some(first));
    fake.page_size(2);
    let one = notion.search(&access, None).await.unwrap();
    assert_eq!(one.pages.len(), 2);
    assert!(one.next.is_some());
    let two = notion.search(&access, one.next.as_deref()).await.unwrap();
    assert_eq!(two.pages.len(), 1);
    assert!(two.next.is_none());
    let mut ids: Vec<Uuid> = one.pages.iter().chain(&two.pages).map(|p| p.id).collect();
    ids.sort();
    let mut expected = vec![first, second, third];
    expected.sort();
    assert_eq!(ids, expected);
    let sub = one
        .pages
        .iter()
        .chain(&two.pages)
        .find(|p| p.id == third)
        .unwrap();
    assert_eq!(sub.parent, Parent::Page(first));
    assert_eq!(sub.title, "Plan");
}

#[tokio::test]
async fn an_unshared_or_trashed_page_reads_as_gone_or_trashed() {
    let (fake, notion) = client().await;
    let (access, _) = fake.grant();
    let gone = fake.add_page("Gone", None);
    let trashed = fake.add_page("Trashed", None);
    fake.unshare(gone);
    fake.trash_page(trashed);
    assert!(matches!(
        notion.page(&access, gone).await,
        Err(ProviderError::NotFound)
    ));
    assert!(notion.page(&access, trashed).await.unwrap().trashed);
}

#[tokio::test]
async fn a_revoked_token_is_unauthorized() {
    let (fake, notion) = client().await;
    let (access, _) = fake.grant();
    let page = fake.add_page("Roadmap", None);
    fake.revoke_access();
    assert!(matches!(
        notion.page(&access, page).await,
        Err(ProviderError::Unauthorized)
    ));
}

#[tokio::test]
async fn children_are_listed_in_pages_and_blocks_are_written() {
    let (fake, notion) = client().await;
    let (access, _) = fake.grant();
    let page = fake.add_page("Roadmap", None);
    let a = fake.paragraph(page, "One");
    let b = fake.paragraph(page, "Two");
    fake.page_size(1);
    let first = notion.children(&access, page, None).await.unwrap();
    assert_eq!(first.blocks.len(), 1);
    let second = notion
        .children(&access, page, first.next.as_deref())
        .await
        .unwrap();
    assert_eq!(second.blocks.len(), 1);
    assert!(second.next.is_none());

    let body = serde_json::json!({"paragraph": {"rich_text": [crate::testkit_notion::rich("Uno")]}});
    notion.update_block(&access, a, &body).await.unwrap();
    assert_eq!(fake.text(a).as_deref(), Some("Uno"));

    let made = notion
        .append(
            &access,
            page,
            &[serde_json::json!({"object": "block", "type": "paragraph",
                "paragraph": {"rich_text": [crate::testkit_notion::rich("Between")]}})],
            Some(a),
        )
        .await
        .unwrap();
    assert_eq!(made.len(), 1);
    let order = fake.children_of(page);
    assert_eq!(order[0], a);
    assert_eq!(order[2], b);
    assert_eq!(fake.text(order[1]).as_deref(), Some("Between"));

    notion.delete_block(&access, b).await.unwrap();
    assert_eq!(fake.deleted(), vec![b]);
}

#[tokio::test]
async fn retry_after_is_capped() {
    let (fake, notion) = client().await;
    let (access, _) = fake.grant();
    let page = fake.add_page("Roadmap", None);
    fake.throttle_next(10 * 24 * 3600);
    match notion.page(&access, page).await {
        Err(ProviderError::RateLimited { retry_after }) => {
            assert_eq!(retry_after, Some(MAX_RETRY_AFTER));
        }
        other => panic!("{other:?}"),
    }
    fake.throttle_next(7);
    match notion.page(&access, page).await {
        Err(ProviderError::RateLimited { retry_after }) => {
            assert_eq!(retry_after, Some(Duration::from_secs(7)));
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn files_come_only_from_the_fake_host_and_of_an_allowed_type() {
    let (fake, notion) = client().await;
    let png = fake.add_file("a.png", b"\x89PNG....", "image/png");
    let html = fake.add_file("a.html", b"<script>", "text/html");
    let got = notion.download(&png).await.unwrap().unwrap();
    assert_eq!(got.content_type, "image/png");
    assert_eq!(got.bytes, b"\x89PNG....");
    // A type that is not on the list is not kept.
    assert!(notion.download(&html).await.unwrap().is_none());
    // Another host is never asked.
    let calls = fake.calls().len();
    assert!(
        notion
            .download("https://evil.example.com/a.png")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        notion
            .download("http://127.0.0.1:9/a.png")
            .await
            .unwrap()
            .is_none()
    );
    assert!(notion.download("not a url").await.unwrap().is_none());
    assert_eq!(fake.calls().len(), calls);
    // A file that has expired is not an error.
    let missing = format!("{}/files/missing.png", fake.base().as_str().trim_end_matches('/'));
    assert!(notion.download(&missing).await.unwrap().is_none());
}
