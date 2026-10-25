//! The Microsoft client against a fake (wiremock): the token refresh, Graph
//! `/me`, and what each refusal means. No test talks to Microsoft.

use std::time::Duration;

use secrecy::SecretString;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_string_contains, header, method, path, query_param},
};

use super::*;

const TENANT: &str = "common";

async fn fake() -> (MockServer, Microsoft) {
    let server = MockServer::start().await;
    let base = Url::parse(&server.uri()).unwrap();
    let microsoft = Microsoft::new(
        "microsoft-client-1".into(),
        ClientSecret::Inline(SecretString::from("test-microsoft-secret")),
        Endpoints::under(&base, TENANT).unwrap(),
    )
    .unwrap();
    (server, microsoft)
}

fn json(status: u16, body: &serde_json::Value) -> ResponseTemplate {
    ResponseTemplate::new(status).set_body_json(body)
}

#[test]
fn production_endpoints_are_microsofts_own_hosts() {
    let endpoints = Endpoints::microsoft("common").unwrap();
    assert_eq!(
        endpoints.token.as_str(),
        "https://login.microsoftonline.com/common/oauth2/v2.0/token"
    );
    assert_eq!(
        endpoints.graph.as_str(),
        "https://graph.microsoft.com/v1.0/"
    );
    let tenant = Endpoints::microsoft("72f988bf-86f1-41af-91ab-2d7cd011db47").unwrap();
    assert_eq!(tenant.token.host_str(), Some("login.microsoftonline.com"));
    for bad in [
        "",
        "not a tenant",
        "../evil",
        "common/../x",
        "evil.example.com",
        "72F988BF-86F1-41AF-91AB-2D7CD011DB47",
    ] {
        let refused = Endpoints::microsoft(bad).unwrap_err();
        assert!(
            refused.contains("CAMPBOOKS_CONNECTORS__MICROSOFT__TENANT"),
            "{refused}"
        );
    }
    for good in ["common", "organizations", "consumers"] {
        assert!(is_tenant(good));
    }
}

#[test]
fn scopes_compare_without_the_graph_prefix_and_case() {
    assert!(same_scope(
        "https://graph.microsoft.com/Mail.ReadWrite",
        "Mail.ReadWrite"
    ));
    assert!(same_scope("mail.send", "Mail.Send"));
    assert!(same_scope(
        "HTTPS://GRAPH.MICROSOFT.COM/user.read",
        "User.Read"
    ));
    assert!(!same_scope("Mail.Read", "Mail.ReadWrite"));
    assert!(!same_scope(
        "https://graph.example.com/Mail.ReadWrite",
        "Mail.ReadWrite"
    ));
    let prefixed: Vec<String> = [
        "openid",
        "https://graph.microsoft.com/Mail.ReadWrite",
        "https://graph.microsoft.com/Mail.Send",
        "https://graph.microsoft.com/User.Read",
    ]
    .map(str::to_owned)
    .to_vec();
    assert!(mail_granted(&prefixed));
    let short: Vec<String> = ["Mail.ReadWrite", "Mail.Send", "User.Read", "offline_access"]
        .map(str::to_owned)
        .to_vec();
    assert!(mail_granted(&short));
    let without_send: Vec<String> = ["Mail.ReadWrite", "User.Read"].map(str::to_owned).to_vec();
    assert!(!mail_granted(&without_send));
    assert!(!mail_granted(&[]));
}

#[tokio::test]
async fn a_refresh_asks_the_mail_tenant_for_the_mail_scopes_only() {
    let (server, microsoft) = fake().await;
    Mock::given(method("POST"))
        .and(path("/common/oauth2/v2.0/token"))
        .and(body_string_contains("grant_type=refresh_token"))
        .and(body_string_contains("refresh_token=old-refresh"))
        .and(body_string_contains("client_id=microsoft-client-1"))
        .and(body_string_contains(
            "scope=offline_access+Mail.ReadWrite+Mail.Send+User.Read",
        ))
        .respond_with(json(
            200,
            &serde_json::json!({
                "token_type": "Bearer",
                "access_token": "new-access",
                "refresh_token": "new-refresh",
                "expires_in": 3599,
                "ext_expires_in": 3599,
                "scope": "https://graph.microsoft.com/Mail.ReadWrite https://graph.microsoft.com/Mail.Send https://graph.microsoft.com/User.Read"
            }),
        ))
        .expect(1)
        .mount(&server)
        .await;
    let grant = microsoft.refresh("old-refresh").await.unwrap();
    assert_eq!(grant.access_token, "new-access");
    assert_eq!(grant.refresh_token.as_deref(), Some("new-refresh"));
    assert_eq!(grant.expires_in, Some(3599));
}

#[tokio::test]
async fn a_grant_that_is_gone_is_invalid_grant() {
    for answer in [
        serde_json::json!({"error": "invalid_grant", "error_codes": [70000],
                           "error_description": "AADSTS70000: The provided grant is invalid."}),
        serde_json::json!({"error": "invalid_grant", "error_codes": [700_082]}),
        serde_json::json!({"error": "invalid_grant", "error_codes": [50173]}),
        serde_json::json!({"error": "invalid_grant"}),
        serde_json::json!({"error": "interaction_required", "error_codes": [65001]}),
        serde_json::json!({"error": "invalid_request",
                           "error_description": "AADSTS65001: The user has not consented."}),
        serde_json::json!({"error": "invalid_request", "error_codes": [50076]}),
    ] {
        let (server, microsoft) = fake().await;
        Mock::given(method("POST"))
            .respond_with(json(400, &answer))
            .mount(&server)
            .await;
        assert_eq!(
            microsoft.refresh("old-refresh").await.unwrap_err(),
            ProviderError::InvalidGrant,
            "{answer}"
        );
    }
}

#[tokio::test]
async fn throttling_and_outages_are_not_a_lost_grant() {
    let (server, microsoft) = fake().await;
    Mock::given(method("POST"))
        .respond_with(
            json(
                429,
                &serde_json::json!({"error": "temporarily_unavailable"}),
            )
            .insert_header("retry-after", "120"),
        )
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(503).insert_header("retry-after", "30"))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(502))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .respond_with(json(
            401,
            &serde_json::json!({"error": "invalid_client", "error_codes": [7_000_215],
                                "error_description": "AADSTS7000215: Invalid client secret. <secret>"}),
        ))
        .mount(&server)
        .await;
    assert_eq!(
        microsoft.refresh("r").await.unwrap_err(),
        ProviderError::RateLimited {
            retry_after: Some(Duration::from_mins(2))
        }
    );
    assert_eq!(
        microsoft.refresh("r").await.unwrap_err(),
        ProviderError::RateLimited {
            retry_after: Some(Duration::from_secs(30))
        }
    );
    assert!(matches!(
        microsoft.refresh("r").await.unwrap_err(),
        ProviderError::Unavailable(_)
    ));
    // Our own client's trouble: never the member's to fix, and nothing of
    // the answer's description in the error.
    let misconfigured = microsoft.refresh("r").await.unwrap_err();
    assert_eq!(
        misconfigured,
        ProviderError::Unexpected("HTTP 401 (invalid_client)".to_owned())
    );
}

#[tokio::test]
async fn me_names_the_mailbox() {
    let (server, microsoft) = fake().await;
    Mock::given(method("GET"))
        .and(path("/v1.0/me"))
        .and(query_param(
            "$select",
            "id,mail,userPrincipalName,displayName",
        ))
        .and(header("authorization", "Bearer an-access-token"))
        .respond_with(json(
            200,
            &serde_json::json!({
                "@odata.context": "https://graph.microsoft.com/v1.0/$metadata#users/$entity",
                "id": "0d9ed3e4-1c2b-4f6a-9e8d-7c6b5a4f3e2d",
                "mail": "  Ana.Rivera@Example.com ",
                "userPrincipalName": "ana@example.onmicrosoft.com",
                "displayName": " Ana\u{0007} Rivera "
            }),
        ))
        .mount(&server)
        .await;
    let user = microsoft.me("an-access-token").await.unwrap();
    assert_eq!(
        user,
        GraphUser {
            id: "0d9ed3e4-1c2b-4f6a-9e8d-7c6b5a4f3e2d".to_owned(),
            address: "ana.rivera@example.com".to_owned(),
            display_name: Some("Ana Rivera".to_owned()),
        }
    );
}

#[tokio::test]
async fn a_personal_account_is_named_by_its_principal_name() {
    let (server, microsoft) = fake().await;
    Mock::given(method("GET"))
        .and(path("/v1.0/me"))
        .respond_with(json(
            200,
            &serde_json::json!({
                "id": "a1b2c3d4e5f60718",
                "mail": null,
                "userPrincipalName": "ana@outlook.example",
                "displayName": null
            }),
        ))
        .mount(&server)
        .await;
    let user = microsoft.me("t").await.unwrap();
    assert_eq!(user.id, "a1b2c3d4e5f60718");
    assert_eq!(user.address, "ana@outlook.example");
    assert_eq!(user.display_name, None);
}

#[tokio::test]
async fn me_refuses_what_is_not_a_mailbox() {
    let long_id = "a".repeat(65);
    for answer in [
        serde_json::json!({"mail": "ana@example.com"}),
        serde_json::json!({"id": "", "mail": "ana@example.com"}),
        serde_json::json!({"id": "../../users/someone-else", "mail": "ana@example.com"}),
        serde_json::json!({"id": long_id, "mail": "ana@example.com"}),
        serde_json::json!({"id": "a1b2c3d4e5f60718"}),
        serde_json::json!({"id": "a1b2c3d4e5f60718", "mail": "not an address",
                           "userPrincipalName": ""}),
        serde_json::json!({"id": "a1b2c3d4e5f60718", "mail": "ana@localhost"}),
    ] {
        let (server, microsoft) = fake().await;
        Mock::given(method("GET"))
            .respond_with(json(200, &answer))
            .mount(&server)
            .await;
        assert!(
            matches!(
                microsoft.me("t").await.unwrap_err(),
                ProviderError::Unexpected(_)
            ),
            "{answer}"
        );
    }
}

#[tokio::test]
async fn answers_are_bounded() {
    let (server, microsoft) = fake().await;
    let huge = "x".repeat(MAX_ANSWER_BYTES + 1);
    Mock::given(method("GET"))
        .respond_with(json(
            200,
            &serde_json::json!({"id": "a1b2c3d4e5f60718", "mail": "ana@example.com",
                                "displayName": huge}),
        ))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_string("y".repeat(MAX_ANSWER_BYTES * 2)))
        .mount(&server)
        .await;
    assert_eq!(
        microsoft.me("t").await.unwrap_err(),
        ProviderError::Unexpected("the answer is too large".to_owned())
    );
    assert_eq!(
        microsoft.refresh("r").await.unwrap_err(),
        ProviderError::Unexpected("the answer is too large".to_owned())
    );
}

#[tokio::test]
async fn graph_refusals_are_classified() {
    let (server, microsoft) = fake().await;
    Mock::given(method("GET"))
        .respond_with(json(
            401,
            &serde_json::json!({"error": {"code": "InvalidAuthenticationToken",
                                          "message": "Access token has expired."}}),
        ))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "7"))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .respond_with(json(
            403,
            &serde_json::json!({"error": {"code": "Authorization_RequestDenied",
                                          "message": "Insufficient privileges <ana@example.com>"}}),
        ))
        .mount(&server)
        .await;
    assert_eq!(
        microsoft.me("t").await.unwrap_err(),
        ProviderError::Unauthorized
    );
    assert_eq!(
        microsoft.me("t").await.unwrap_err(),
        ProviderError::RateLimited {
            retry_after: Some(Duration::from_secs(7))
        }
    );
    assert_eq!(
        microsoft.me("t").await.unwrap_err(),
        ProviderError::Unexpected("HTTP 403 (Authorization_RequestDenied)".to_owned())
    );
}

#[test]
fn aadsts_codes_are_read_from_a_description_only_at_its_start() {
    assert_eq!(
        aadsts_code("AADSTS70000: The grant is invalid."),
        Some(70000)
    );
    assert_eq!(aadsts_code("  AADSTS700082: expired"), Some(700_082));
    assert_eq!(aadsts_code("Something AADSTS70000: later"), None);
    assert_eq!(aadsts_code("AADSTS: none"), None);
    assert_eq!(aadsts_code("AADSTS12345678901234: too long"), None);
}
