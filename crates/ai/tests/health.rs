//! What the admin system overview shows of each AI role.

// Helpers outside `#[test]` fns are not covered by clippy's test exemptions.
#![allow(clippy::unwrap_used)]

use std::collections::HashMap;

use campbooks_ai::{AiConfig, Gateway, ProviderConfig, Role, unconfigured};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

fn gateway(roles: impl IntoIterator<Item = (Role, ProviderConfig)>) -> Gateway {
    Gateway::new(AiConfig {
        roles: roles.into_iter().collect::<HashMap<_, _>>(),
        ..AiConfig::default()
    })
    .unwrap()
}

#[tokio::test]
async fn configured_roles_are_probed_once_a_minute_and_shown_without_secrets() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    let base = server.uri().replace("http://", "http://user:url-password@");
    let decide = ProviderConfig::new(format!("{base}/v1?token=q").parse().unwrap(), "qwen3:8b")
        .with_api_key("sk-probe-secret");
    let gw = gateway([(Role::Decide, decide)]);

    let first = gw.health().await;
    let second = gw.health().await;
    assert_eq!(first, second, "the second answer comes from the cache");
    assert_eq!(first.len(), 4);
    let decide = first.iter().find(|r| r.role == Role::Decide).unwrap();
    assert!(decide.configured);
    assert_eq!(decide.reachable, Some(true));
    assert_eq!(decide.model.as_deref(), Some("qwen3:8b"));
    assert_eq!(decide.endpoint.as_deref(), Some(server.uri().as_str()));
    let json = serde_json::to_string(&first).unwrap();
    for secret in ["sk-probe-secret", "url-password", "user", "token=q", "/v1"] {
        assert!(!json.contains(secret), "{secret} leaked: {json}");
    }
    let embed = first.iter().find(|r| r.role == Role::Embed).unwrap();
    assert_eq!(
        (embed.configured, embed.reachable, embed.model.as_ref()),
        (false, None, None)
    );
}

#[tokio::test]
async fn a_failing_or_silent_provider_is_unreachable() {
    let failing = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&failing)
        .await;
    let answering = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&answering)
        .await;
    let gw = gateway([
        (
            Role::Decide,
            ProviderConfig::new(format!("{}/v1", failing.uri()).parse().unwrap(), "m"),
        ),
        (
            Role::Embed,
            ProviderConfig::new("http://127.0.0.1:1/v1".parse().unwrap(), "e"),
        ),
        (
            Role::Complete,
            ProviderConfig::new(format!("{}/v1", answering.uri()).parse().unwrap(), "c"),
        ),
    ]);
    let health = gw.health().await;
    let reachable = |role| health.iter().find(|r| r.role == role).unwrap().reachable;
    assert_eq!(reachable(Role::Decide), Some(false), "5xx");
    assert_eq!(reachable(Role::Embed), Some(false), "nothing listens");
    assert_eq!(reachable(Role::Complete), Some(true), "it answered");
    assert_eq!(reachable(Role::Vision), None);
}

#[test]
fn nothing_configured() {
    let roles = unconfigured();
    assert_eq!(roles.len(), 4);
    assert!(roles.iter().all(|r| !r.configured && r.reachable.is_none()));
}
