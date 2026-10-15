use axum::http::{Method, StatusCode};
use campbooks_ai::{AiConfig, ExternalContent, Gateway, ProviderConfig, Role};
use campbooks_kernel::testing::TestResponse;
use serde_json::{Value, json};
use sqlx::PgPool;
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method};

use crate::{Deps, Facts, IdentityProvider};

use crate::testkit::{World, world};

impl World {
    async fn overview(&self, sub: &str, auth_age_seconds: i64, language: &str) -> TestResponse {
        self.send(
            Method::GET,
            "/admin/v1/system/overview",
            sub,
            auth_age_seconds,
            language,
            None,
        )
        .await
    }
}

fn gateway(roles: impl IntoIterator<Item = (Role, ProviderConfig)>) -> Gateway {
    Gateway::new(AiConfig {
        roles: roles.into_iter().collect(),
        ..AiConfig::default()
    })
    .unwrap()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_super_admin_sees_the_system_without_secrets(db: PgPool) {
    let provider = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&provider)
        .await;
    let base = provider
        .uri()
        .replace("http://", "http://user:url-password@");
    let deps = Deps {
        ai: Some(gateway([
            (
                Role::Decide,
                ProviderConfig::new(format!("{base}/v1").parse().unwrap(), "qwen3:8b")
                    .with_api_key("sk-ai-secret"),
            ),
            // Off the instance's network (a documentation address): opted in…
            (
                Role::Embed,
                ProviderConfig::new("http://203.0.113.7/v1".parse().unwrap(), "e")
                    .with_external_content(ExternalContent::Allow),
            ),
            // …and not.
            (
                Role::Complete,
                ProviderConfig::new("http://203.0.113.7/v1".parse().unwrap(), "c"),
            ),
        ])),
        facts: Facts {
            identity_provider: IdentityProvider::BuiltIn,
            sign_in_providers: vec!["google".into()],
            mail_providers: vec!["google".into()],
        },
        ..Deps::default()
    };
    let w = world(db, deps).await;
    let response = w.overview("ada", 60, "en").await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.json);
    let body = &response.json;
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
    assert!(body["uptime_seconds"].as_i64().unwrap() >= 0);
    assert_eq!(body["database"]["reachable"], true);
    assert!(body["database"]["migrations_applied"].as_i64().unwrap() > 10);
    assert!(body["database"]["latest_migration"].as_i64().is_some());
    assert_eq!(body["configuration"]["signup_policy"], "open");
    assert_eq!(body["configuration"]["blob_store"], "fs");
    assert_eq!(body["services"]["identity_provider"], "built_in");
    assert_eq!(body["services"]["sign_in_providers"], json!(["google"]));
    let ai = body["ai"].as_array().unwrap();
    assert_eq!(ai.len(), 4);
    assert_eq!(ai[0]["role"], "decide");
    assert_eq!(ai[0]["model"], "qwen3:8b");
    assert_eq!(ai[0]["endpoint"], provider.uri());
    assert_eq!(ai[0]["protocol"], "openai");
    assert_eq!(ai[0]["reachable"], true);
    assert_eq!(ai[0]["location"], "instance");
    assert_eq!(ai[0]["content_allowed"], true);
    assert_eq!(
        (&ai[1]["location"], &ai[1]["content_allowed"]),
        (&json!("external"), &json!(true))
    );
    assert_eq!(
        (&ai[2]["location"], &ai[2]["content_allowed"]),
        (&json!("external"), &json!(false))
    );
    assert_eq!(ai[3]["configured"], false);
    assert_eq!(ai[3]["location"], Value::Null);

    let text = body.to_string();
    for secret in [
        "db-password",
        "signup-code-1",
        "sk-ai-secret",
        "url-password",
        "postgres://",
    ] {
        assert!(!text.contains(secret), "{secret} leaked: {text}");
    }

    let recorded = w.trail.0.lock().unwrap().clone();
    assert_eq!(recorded.len(), 1);
    assert_eq!(
        (
            recorded[0].actor.as_str(),
            recorded[0].operation.as_str(),
            recorded[0].status
        ),
        ("ada", "system.admin_overview", 200)
    );
    assert_eq!(recorded[0].target, json!({}));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn anyone_else_is_told_there_is_nothing_here(db: PgPool) {
    let w = world(db, Deps::default()).await;
    for sub in ["member", "ops"] {
        let response = w.overview(sub, 60, "en").await;
        assert_eq!(response.status, StatusCode::NOT_FOUND, "{sub}");
        assert_eq!(response.json["code"], "not_found");
    }
    assert!(w.trail.0.lock().unwrap().is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_stale_sign_in_is_asked_to_sign_in_again_in_its_language(db: PgPool) {
    let w = world(db, Deps::default()).await;
    let response = w.overview("ada", 13 * 3600, "pt-PT").await;
    assert_eq!(response.status, StatusCode::UNAUTHORIZED);
    assert_eq!(response.json["code"], "admin_reauth_required");
    assert!(
        response.json["detail"]
            .as_str()
            .unwrap()
            .contains("Inicie sessão novamente")
    );
    let recorded = w.trail.0.lock().unwrap().clone();
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].status, 401);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn without_ai_every_role_is_unconfigured(db: PgPool) {
    let w = world(db, Deps::default()).await;
    let response = w.overview("ada", 0, "en").await;
    assert_eq!(response.status, StatusCode::OK);
    let ai = response.json["ai"].as_array().unwrap();
    assert!(
        ai.iter()
            .all(|role| role["configured"] == false && role["reachable"].is_null())
    );
    assert_eq!(response.json["services"]["identity_provider"], "none");
}
