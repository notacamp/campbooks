use std::collections::BTreeMap;

use chrono::Utc;

use super::*;

const ADDRESS: &str = "maya@northwind-studio.example";
const SUBJECT: &str = "Q4 restructuring plan";
const TOKEN: &str = "ya29.a0AfH6SMBx3examplelongaccesstokenvalue1234567890abcdef";
const WORKSPACE: &str = "0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b";

/// A report as a careless or hostile app could send it.
fn loaded_report() -> ClientReport {
    let now = Utc::now();
    ClientReport {
        session_id: "k3Jx8vQ2mN".to_owned(),
        platform: Some(ClientPlatform::Web),
        app_version: Some("0.42.0".to_owned()),
        view: Some(format!("/w/{WORKSPACE}/search?q={SUBJECT}")),
        browser: Some(ClientBrowser {
            name: Some("Chrome".to_owned()),
            version: Some("141.0".to_owned()),
            os: Some(format!("macOS for {ADDRESS}")),
            mobile: Some(false),
        }),
        exceptions: vec![ClientException {
            kind: "TypeError".to_owned(),
            message: format!("Cannot render \"{SUBJECT}\": from {ADDRESS} with {TOKEN}"),
            timestamp: now,
            frames: vec![
                ClientFrame {
                    file: Some(format!(
                        "https://app.example.com/assets/index-3f2c1e0.js?token={TOKEN}"
                    )),
                    function: Some("renderThread".to_owned()),
                    line: Some(12),
                    column: Some(4),
                },
                ClientFrame {
                    file: Some(format!("https://app.example.com/w/{WORKSPACE}/{ADDRESS}")),
                    function: Some(SUBJECT.to_owned()),
                    line: None,
                    column: None,
                },
            ],
        }],
        measurements: vec![
            ClientMeasurement {
                kind: "web-vitals".to_owned(),
                values: BTreeMap::from([
                    ("lcp".to_owned(), 1234.5),
                    ("inp".to_owned(), f64::NAN),
                    (ADDRESS.to_owned(), 1.0),
                ]),
                timestamp: now,
            },
            ClientMeasurement {
                kind: SUBJECT.to_owned(),
                values: BTreeMap::from([("lcp".to_owned(), 1.0)]),
                timestamp: now,
            },
        ],
        events: vec![
            ClientEvent {
                name: "session_start".to_owned(),
                timestamp: now,
            },
            ClientEvent {
                name: format!("opened {SUBJECT}"),
                timestamp: now,
            },
        ],
    }
}

#[test]
fn the_forwarded_payload_holds_no_personal_data() {
    let (payload, counts) = faro_payload(&loaded_report(), "staging");
    let json = payload.to_string();
    for leak in [
        ADDRESS,
        "northwind",
        SUBJECT,
        TOKEN,
        WORKSPACE,
        "?q=",
        "token=",
    ] {
        assert!(!json.contains(leak), "{leak} leaked into {json}");
    }
    assert_eq!(payload["meta"]["app"]["environment"], "staging");
    assert_eq!(payload["meta"]["session"]["id"], "k3Jx8vQ2mN");
    assert_eq!(payload["meta"]["view"]["name"], "unknown");
    assert_eq!(payload["meta"]["browser"]["name"], "Chrome");
    assert_eq!(payload["meta"]["browser"]["os"], Value::Null);
    let exception = &payload["exceptions"][0];
    assert_eq!(exception["type"], "TypeError");
    assert_eq!(exception["value"], "Cannot render \"…\"");
    let frames = exception["stacktrace"]["frames"].as_array().unwrap();
    assert_eq!(frames[0]["filename"], "index-3f2c1e0.js");
    assert_eq!(frames[0]["function"], "renderThread");
    assert_eq!(frames[1]["filename"], "");
    assert_eq!(frames[1]["function"], "");
    let measurements = payload["measurements"].as_array().unwrap();
    assert_eq!(measurements.len(), 1);
    assert_eq!(measurements[0]["values"], json!({ "lcp": 1234.5 }));
    assert_eq!(payload["events"].as_array().unwrap().len(), 1);
    assert_eq!(
        counts,
        vec![("exception", 1, 0), ("measurement", 1, 1), ("event", 1, 1)]
    );
}

#[test]
fn views_are_route_templates() {
    assert_eq!(
        view(Some("/w/$workspaceId/inbox")).as_deref(),
        Some("/w/$workspaceId/inbox")
    );
    assert_eq!(
        view(Some("/settings/_layout")).as_deref(),
        Some("/settings/_layout")
    );
    assert_eq!(view(Some(&format!("/w/{WORKSPACE}/inbox"))), None);
    assert_eq!(view(Some("/w/123/inbox")), None);
    assert_eq!(view(Some("/search?q=boiler")), None);
    assert_eq!(view(Some("inbox")), None);
}

#[test]
fn lists_are_capped() {
    let mut report = loaded_report();
    report.exceptions = vec![report.exceptions[0].clone(); MAX_ITEMS + 5];
    report.exceptions[0].frames = vec![report.exceptions[0].frames[0].clone(); MAX_FRAMES + 5];
    let (payload, counts) = faro_payload(&report, "production");
    assert_eq!(payload["exceptions"].as_array().unwrap().len(), MAX_ITEMS);
    assert_eq!(
        payload["exceptions"][0]["stacktrace"]["frames"]
            .as_array()
            .unwrap()
            .len(),
        MAX_FRAMES
    );
    assert_eq!(counts[0], ("exception", MAX_ITEMS as u64, 5));
}

mod handler {
    use std::{sync::Arc, time::Duration};

    use axum::{
        body::Body,
        http::{Method, Request, StatusCode, header},
    };
    use campbooks_kernel::testing::{self, TestIdp};
    use sqlx::PgPool;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{self, method},
    };

    use super::ADDRESS;

    fn report() -> String {
        serde_json::json!({
            "session_id": "k3Jx8vQ2mN",
            "platform": "web",
            "view": "/w/$workspaceId/inbox",
            "exceptions": [{
                "kind": "TypeError",
                "message": format!("Cannot read thread of {ADDRESS}: undefined"),
                "timestamp": "2026-10-17T10:00:00Z",
            }],
        })
        .to_string()
    }

    fn post(token: Option<&str>) -> Request<Body> {
        let mut request = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/system/client-telemetry")
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(token) = token {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        request.body(Body::from(report())).unwrap()
    }

    async fn app(db: &PgPool, idp: &TestIdp, collector: Option<String>) -> axum::Router {
        let mut state = idp.state(testing::app_role_pool(db).await);
        let config = Arc::make_mut(&mut state.config);
        config.telemetry.client_collector_url = collector;
        config.telemetry.client_collector_key = Some("collector-key".to_owned().into());
        testing::app(crate::module(), state)
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn without_a_collector_the_app_is_told_to_stop(db: PgPool) {
        let idp = TestIdp::start().await;
        let app = app(&db, &idp, None).await;
        let token = idp.token(&idp.claims("system:read"));
        let response = testing::send(&app, post(Some(&token))).await;
        assert_eq!(response.status, StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn anonymous_reports_are_refused(db: PgPool) {
        let idp = TestIdp::start().await;
        let collector = MockServer::start().await;
        let app = app(&db, &idp, Some(format!("{}/collect", collector.uri()))).await;
        let response = testing::send(&app, post(None)).await;
        assert_eq!(response.status, StatusCode::UNAUTHORIZED);
        assert!(collector.received_requests().await.unwrap().is_empty());
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn reports_are_forwarded_scrubbed_with_the_collector_key(db: PgPool) {
        let idp = TestIdp::start().await;
        let collector = MockServer::start().await;
        Mock::given(method("POST"))
            .and(matchers::path("/collect"))
            .and(matchers::header("x-api-key", "collector-key"))
            .respond_with(ResponseTemplate::new(202))
            .mount(&collector)
            .await;
        let app = app(&db, &idp, Some(format!("{}/collect", collector.uri()))).await;
        let token = idp.token(&idp.claims("system:read"));

        let response = testing::send(&app, post(Some(&token))).await;
        assert_eq!(response.status, StatusCode::ACCEPTED);

        // Forwarded in the background.
        let mut received = Vec::new();
        for _ in 0..50 {
            received = collector.received_requests().await.unwrap();
            if !received.is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(received.len(), 1);
        let body = String::from_utf8(received[0].body.clone()).unwrap();
        assert!(!body.contains(ADDRESS), "{body}");
        let body: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(
            body["exceptions"][0]["value"],
            "Cannot read thread of [address]"
        );
        assert_eq!(body["meta"]["view"]["name"], "/w/$workspaceId/inbox");
        assert_eq!(body["meta"]["app"]["environment"], "production");
    }
}
