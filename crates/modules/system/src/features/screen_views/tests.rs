use std::collections::BTreeMap;

use campbooks_kernel::config::AnalyticsConfig;
use secrecy::SecretString;

use super::*;

const ADDRESS: &str = "maya@northwind-studio.example";
const SUBJECT: &str = "Q4 restructuring plan";
const WORKSPACE: &str = "0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b";
const KEY: &str = "a-visitor-key-for-tests-only-0123456789";

fn day(text: &str) -> NaiveDate {
    text.parse().unwrap()
}

fn analytics() -> AnalyticsConfig {
    AnalyticsConfig {
        matomo_url: Some("https://analytics.example.com/".to_owned()),
        matomo_site_id: Some("2".to_owned()),
        matomo_visitor_key: Some(SecretString::from(KEY)),
    }
}

fn params(url: &Url) -> BTreeMap<String, String> {
    url.query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect()
}

#[test]
fn route_templates_pass() {
    for template in [
        "/",
        "/inbox",
        "/sign-in/new",
        "/w/:workspaceId",
        "/w/:workspaceId/inbox/:stream/:conversationId",
        "/w/:workspaceId/settings/members",
        "/snake_case",
    ] {
        assert_eq!(route(template), Some(template), "{template}");
    }
}

#[test]
fn concrete_paths_and_anything_else_are_refused() {
    let long = format!("/{}", ["inbox"; 30].join("/"));
    let segment = format!("/{}", "a".repeat(MAX_SEGMENT + 1));
    for value in [
        // Concrete ids: a UUID, a number-only segment, letters and digits.
        format!("/w/{WORKSPACE}/inbox"),
        "/w/123/inbox".to_owned(),
        "/people/42".to_owned(),
        "/inbox/primary/k3Jx8vQ2mN7".to_owned(),
        // Query strings and fragments.
        "/search?q=boiler".to_owned(),
        "/inbox#message-3".to_owned(),
        "/inbox?".to_owned(),
        // Titles, addresses and other text.
        format!("/{SUBJECT}"),
        "Inbox – Campbooks".to_owned(),
        format!("/people/{ADDRESS}"),
        "/Inbox".to_owned(),
        "/inbox/thread.html".to_owned(),
        "/inbox%20x".to_owned(),
        // Not a template: the router's `$param`, empty or odd parameters,
        // no leading slash, empty segments, a trailing slash.
        "/w/$workspaceId/inbox".to_owned(),
        "/w/:/inbox".to_owned(),
        "/w/:WorkspaceId".to_owned(),
        "/w/:workspace1".to_owned(),
        "/w/:workspace-id".to_owned(),
        "inbox".to_owned(),
        String::new(),
        "//inbox".to_owned(),
        "/inbox/".to_owned(),
        "/-inbox".to_owned(),
        "https://evil.example.com/inbox".to_owned(),
        // Too long, too deep, too wide.
        long,
        "/a/b/c/d/e/f/g/h/i".to_owned(),
        segment,
    ] {
        assert_eq!(route(&value), None, "{value:?}");
    }
}

#[test]
fn the_visitor_id_is_daily_and_per_account() {
    let today = visitor_id(KEY.as_bytes(), "account-1", day("2026-10-18"));
    assert_eq!(today.len(), 16);
    assert!(
        today
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
        "{today}"
    );
    // Stable within the day.
    assert_eq!(
        visitor_id(KEY.as_bytes(), "account-1", day("2026-10-18")),
        today
    );
    // Another day, another account or another key: another id.
    assert_ne!(
        visitor_id(KEY.as_bytes(), "account-1", day("2026-10-19")),
        today
    );
    assert_ne!(
        visitor_id(KEY.as_bytes(), "account-2", day("2026-10-18")),
        today
    );
    assert_ne!(
        visitor_id(
            b"another-key-another-key-another-key",
            "account-1",
            day("2026-10-18")
        ),
        today
    );
    // Never the account itself.
    assert!(!today.contains("account"));
}

#[test]
fn the_visitor_id_is_the_head_of_the_hmac() {
    let tag = crypto::hmac_sha256(KEY.as_bytes(), b"account-1:2026-10-18");
    let head: String = tag.iter().take(8).fold(String::new(), |mut hex, byte| {
        use std::fmt::Write as _;
        let _ = write!(hex, "{byte:02x}");
        hex
    });
    assert_eq!(
        visitor_id(KEY.as_bytes(), "account-1", day("2026-10-18")),
        head
    );
}

#[test]
fn the_tracking_request_holds_the_route_and_nothing_identifying() {
    let config = analytics();
    let matomo = config.matomo().unwrap();
    let hit = Hit {
        route: "/w/:workspaceId/inbox",
        visitor: "0123456789abcdef",
        user_agent: Some("Mozilla/5.0 (Macintosh) Chrome/141.0"),
        lang: Some(Locale::PtPt),
    };
    let url = tracking_url(&matomo, "https://app.example.com/", &hit).unwrap();
    assert_eq!(
        url.origin().ascii_serialization(),
        "https://analytics.example.com"
    );
    assert_eq!(url.path(), "/matomo.php");
    let params = params(&url);
    let expected: BTreeMap<String, String> = [
        ("idsite", "2"),
        ("rec", "1"),
        ("apiv", "1"),
        ("url", "https://app.example.com/w/:workspaceId/inbox"),
        ("action_name", "/w/:workspaceId/inbox"),
        ("cid", "0123456789abcdef"),
        ("ua", "Mozilla/5.0 (Macintosh) Chrome/141.0"),
        ("lang", "pt-PT"),
        ("send_image", "0"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v.to_owned()))
    .collect();
    assert_eq!(params, expected);
    for never in [
        "cip",
        "uid",
        "token_auth",
        "_id",
        "_cvar",
        "cvar",
        "dimension1",
    ] {
        assert!(!params.contains_key(never), "{never}");
    }
    assert!(!url.as_str().contains(KEY));
}

#[test]
fn without_a_user_agent_or_a_shipped_language_they_are_left_out() {
    let config = analytics();
    let matomo = config.matomo().unwrap();
    let hit = Hit {
        route: "/",
        visitor: "0123456789abcdef",
        user_agent: None,
        lang: None,
    };
    let params = params(&tracking_url(&matomo, "http://localhost:8080", &hit).unwrap());
    assert!(!params.contains_key("ua"));
    assert!(!params.contains_key("lang"));
    assert_eq!(params["url"], "http://localhost:8080/");
}

#[tokio::test]
async fn a_full_queue_drops_without_waiting() {
    let (relay, mut queue) = ScreenViewRelay::detached(2);
    let url = Url::parse("https://analytics.example.com/matomo.php?rec=1").unwrap();
    let outcomes = tokio::time::timeout(Duration::from_millis(500), async {
        (0..5)
            .map(|_| relay.enqueue(url.clone()))
            .collect::<Vec<_>>()
    })
    .await
    .expect("enqueue never waits");
    assert_eq!(outcomes, [true, true, false, false, false]);
    assert!(queue.try_recv().is_ok());
    assert!(queue.try_recv().is_ok());
    assert!(queue.try_recv().is_err());
}

mod handler {
    use std::{sync::Arc, time::Duration};

    use axum::{
        body::Body,
        http::{Method, Request, StatusCode, header},
    };
    use campbooks_kernel::{
        config::AnalyticsConfig,
        testing::{self, TestIdp},
    };
    use chrono::Utc;
    use sqlx::PgPool;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{self, method},
    };

    use super::{KEY, WORKSPACE, analytics};

    const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) Firefox/131.0";

    fn post(token: Option<&str>, route: &str) -> Request<Body> {
        let mut request = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/system/screen-views")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::USER_AGENT, USER_AGENT)
            .header(header::ACCEPT_LANGUAGE, "pt-BR,pt;q=0.9,en;q=0.5");
        if let Some(token) = token {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        request
            .body(Body::from(
                serde_json::json!({ "route": route }).to_string(),
            ))
            .unwrap()
    }

    async fn app(db: &PgPool, idp: &TestIdp, analytics: AnalyticsConfig) -> axum::Router {
        let mut state = idp.state(testing::app_role_pool(db).await);
        let config = Arc::make_mut(&mut state.config);
        config.analytics = analytics;
        testing::app(crate::module(), state)
    }

    fn pointing_at(matomo: &MockServer) -> AnalyticsConfig {
        AnalyticsConfig {
            matomo_url: Some(matomo.uri()),
            ..analytics()
        }
    }

    /// The requests Matomo received, waiting a little for the worker.
    async fn received(matomo: &MockServer, at_least: usize) -> Vec<wiremock::Request> {
        let mut received = Vec::new();
        for _ in 0..50 {
            received = matomo.received_requests().await.unwrap();
            if received.len() >= at_least {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        received
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn views_are_relayed_to_matomo_in_the_background(db: PgPool) {
        let idp = TestIdp::start().await;
        let matomo = MockServer::start().await;
        Mock::given(method("GET"))
            .and(matchers::path("/matomo.php"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&matomo)
            .await;
        let app = app(&db, &idp, pointing_at(&matomo)).await;
        let token = idp.token(&idp.claims("system:read"));
        let before = Utc::now().date_naive();

        let response = testing::send(&app, post(Some(&token), "/w/:workspaceId/inbox")).await;
        assert_eq!(response.status, StatusCode::NO_CONTENT);

        let received = received(&matomo, 1).await;
        assert_eq!(received.len(), 1);
        let params: std::collections::BTreeMap<String, String> = received[0]
            .url
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        assert_eq!(params["idsite"], "2");
        assert_eq!(params["rec"], "1");
        assert_eq!(params["apiv"], "1");
        assert_eq!(params["send_image"], "0");
        assert_eq!(params["action_name"], "/w/:workspaceId/inbox");
        assert_eq!(params["url"], "http://localhost:8080/w/:workspaceId/inbox");
        assert_eq!(params["ua"], USER_AGENT);
        assert_eq!(params["lang"], "pt-PT");
        let days = [before, Utc::now().date_naive()];
        assert!(
            days.iter()
                .any(|day| params["cid"] == super::visitor_id(KEY.as_bytes(), "test-user", *day)),
            "{params:?}"
        );
        for never in ["cip", "uid", "token_auth", "_id"] {
            assert!(!params.contains_key(never), "{never}");
        }
        let url = received[0].url.as_str();
        assert!(!url.contains("test-user"), "{url}");
        assert!(!url.contains(KEY), "{url}");
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn when_analytics_is_off_nothing_is_sent(db: PgPool) {
        let idp = TestIdp::start().await;
        let matomo = MockServer::start().await;
        let token = idp.token(&idp.claims("system:read"));
        // Each setting missing in turn, then all of them.
        for analytics in [
            AnalyticsConfig {
                matomo_site_id: None,
                ..pointing_at(&matomo)
            },
            AnalyticsConfig {
                matomo_visitor_key: None,
                ..pointing_at(&matomo)
            },
            AnalyticsConfig {
                matomo_url: Some(String::new()),
                ..pointing_at(&matomo)
            },
            AnalyticsConfig::default(),
        ] {
            let app = app(&db, &idp, analytics).await;
            let response = testing::send(&app, post(Some(&token), "/inbox")).await;
            assert_eq!(response.status, StatusCode::NO_CONTENT);
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(matomo.received_requests().await.unwrap().is_empty());
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn anything_but_a_route_template_is_refused_and_never_sent(db: PgPool) {
        let idp = TestIdp::start().await;
        let matomo = MockServer::start().await;
        let app = app(&db, &idp, pointing_at(&matomo)).await;
        let token = idp.token(&idp.claims("system:read"));
        for route in [
            format!("/w/{WORKSPACE}/inbox"),
            "/people/42".to_owned(),
            "/search?q=boiler".to_owned(),
            "/inbox#message-3".to_owned(),
            "Inbox – Campbooks".to_owned(),
        ] {
            let response = testing::send(&app, post(Some(&token), &route)).await;
            assert_eq!(response.status, StatusCode::BAD_REQUEST, "{route}");
            assert_eq!(response.json["code"], "invalid_route", "{route}");
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(matomo.received_requests().await.unwrap().is_empty());
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn anonymous_views_are_refused(db: PgPool) {
        let idp = TestIdp::start().await;
        let matomo = MockServer::start().await;
        let app = app(&db, &idp, pointing_at(&matomo)).await;
        let response = testing::send(&app, post(None, "/inbox")).await;
        assert_eq!(response.status, StatusCode::UNAUTHORIZED);
        assert!(matomo.received_requests().await.unwrap().is_empty());
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn matomo_failing_is_never_the_persons_problem(db: PgPool) {
        let idp = TestIdp::start().await;
        let matomo = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&matomo)
            .await;
        let app = app(&db, &idp, pointing_at(&matomo)).await;
        let token = idp.token(&idp.claims("system:read"));
        for _ in 0..2 {
            let response = testing::send(&app, post(Some(&token), "/inbox")).await;
            assert_eq!(response.status, StatusCode::NO_CONTENT);
        }
        // Each view is tried once and dropped: never retried.
        let received = received(&matomo, 2).await;
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(received.len(), 2);
        assert_eq!(matomo.received_requests().await.unwrap().len(), 2);
    }
}
