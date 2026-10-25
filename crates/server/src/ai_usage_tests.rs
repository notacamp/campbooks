//! The daily AI limit end to end (campbooks-cloud#152): the server's
//! composition (the conversations module's `decide` pass and the
//! assistant's compiling, through a gateway metered by the system module's
//! ledger), fictional mail, fake model servers, a clock moved by hand, and
//! the admin AI usage read.
//!
//! `cargo test -p campbooks-server ai_usage -- --nocapture` tells the story
//! as it runs.

use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
};
use campbooks_ai::{
    AiConfig, Gateway, Protocol, ProviderConfig, Role,
    decide::check::{CheckOutcome, CheckReport},
};
use campbooks_conversations::{ConversationsConfig, ConversationsDeps, dev};
use campbooks_kernel::{
    AppState,
    admin::{ACCESS_SCOPE, Permission},
    blobs::BlobStore,
    clock::FixedClock,
    config::BlobsConfig,
    jobs::Worker,
    tenancy::WorkspaceId,
    testing::{self, TestIdp, TestResponse},
    tracking::Tracker,
};
use campbooks_system::AiUsageMeter;
use chrono::{TimeDelta, Utc};
use serde_json::{Value, json};
use sqlx::PgPool;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

use crate::{Composition, ai_gateway, registry_with, router_with, with_ports};

const EN: &str = include_str!("../../modules/conversations/fixtures/en.json");
const FR: &str = include_str!("../../modules/conversations/fixtures/fr.json");
const COMMENT: &str = include_str!("../../modules/conversations/fixtures/github-comment.json");

struct World {
    idp: TestIdp,
    db: PgPool,
    state: AppState,
    app: Router,
    clock: Arc<FixedClock>,
    laya: MockServer,
}

/// laya-serve answering every triage question (`decide`), and an instruct
/// model finding nothing to teach in a sentence (`complete`).
async fn laya() -> MockServer {
    let noul = json!({"type": "noul", "noul": 0.1});
    let server = MockServer::start().await;
    let nothing = json!({"outcome": "unclear", "unsupported": "none", "rules": []});
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": "demo-instruct",
            "choices": [{"message": {"role": "assistant", "content": nothing.to_string()},
                         "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 400, "completion_tokens": 20}
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": "laya-demo",
            "answers": {
                "stream": {"type": "choice", "choice": "primary",
                    "probabilities": {"primary": 0.9, "updates": 0.04, "receipts": 0.03,
                                      "newsletters": 0.03}},
                "needs_reply": noul, "asks_action": noul, "has_deadline": noul,
                "urgency": {"type": "score", "score": 0.2,
                    "probabilities": {"0": 0.8, "1": 0.2, "2": 0.0}},
                "suspicious": noul,
                "money": {"type": "choice", "choice": "neither", "probabilities": {"owed": 0.1, "paid": 0.1, "neither": 0.8}},
                "has_dated_thing": noul
            },
            "usage": {"input_tokens": 180, "output_tokens": 0}
        })))
        .mount(&server)
        .await;
    server
}

/// The server composed as `Composition::from_env` does, with a `decide`
/// role on the laya fake and a daily limit of `cap` calls per workspace.
async fn world(db: PgPool, cap: u32) -> World {
    let laya = laya().await;
    let clock = Arc::new(FixedClock::new("2026-10-16T09:00:00Z".parse().unwrap()));
    let (gateway, meter) = composed(&laya, cap);
    let meter = meter.with_clock(clock.clone());
    // The same meter, on the hand-moved clock.
    let gateway = gateway.unwrap().with_meter(Arc::new(meter.clone()));
    gateway.record_answers_check(CheckReport {
        outcome: CheckOutcome::Passed,
        right: 33,
        total: 33,
        model: Some("laya-demo".into()),
        checked_at: Utc::now(),
    });
    let composition = Composition {
        conversations: ConversationsDeps::new(
            Some(gateway.clone()),
            ConversationsConfig {
                // The fixtures' mail is old: the model reads it anyway.
                decide_backfill_days: 100_000,
                ..ConversationsConfig::default()
            },
        ),
        assistant: campbooks_assistant::AssistantDeps {
            ai: Some(gateway.clone()),
            ..campbooks_assistant::AssistantDeps::default()
        },
        system: campbooks_system::Deps {
            ai: Some(gateway),
            ai_usage: meter,
            ..campbooks_system::Deps::default()
        },
        ..Composition::default()
    };
    let idp = TestIdp::start().await;
    let pool = testing::app_role_pool(&db).await;
    let state = with_ports(
        idp.state(pool.clone())
            .with_blobs(BlobStore::memory(&BlobsConfig::default(), pool))
            .with_registry(registry_with(&composition).unwrap()),
    );
    let app = router_with(state.clone(), None, &composition);
    World {
        idp,
        db,
        state,
        app,
        clock,
        laya,
    }
}

fn composed(laya: &MockServer, cap: u32) -> (Option<Gateway>, AiUsageMeter) {
    let url = || format!("{}/v1", laya.uri()).parse().unwrap();
    let decide = ProviderConfig::new(url(), "convaiinnovations/laya-multilingual")
        .with_protocol(Protocol::Laya);
    ai_gateway(AiConfig {
        roles: [
            (Role::Decide, decide),
            (Role::Complete, ProviderConfig::new(url(), "demo-instruct")),
        ]
        .into(),
        daily_calls_per_workspace: Some(cap),
        ..AiConfig::default()
    }, Tracker::default())
    .unwrap()
}

impl World {
    /// A new inbox in `workspace` receives the recorded `batch`; every job
    /// runs. Returns how many model calls went out for it.
    async fn receive(&self, workspace: WorkspaceId, address: &str, batch: &str) -> usize {
        let before = self.model_calls().await;
        let inbox = dev::create_inbox(&self.state, workspace, address, &[])
            .await
            .unwrap();
        let recorded: Value = serde_json::from_str(batch).unwrap();
        let batch = dev::record(&self.state, workspace, inbox, &recorded)
            .await
            .unwrap();
        dev::submit(&self.state, workspace, &batch).await.unwrap();
        let stats = Worker::new(self.state.clone()).run_until_idle().await;
        assert_eq!((stats.failed, stats.retried), (0, 0), "{stats:?}");
        self.model_calls().await - before
    }

    /// `decide` calls sent so far.
    async fn model_calls(&self) -> usize {
        let requests = self.laya.received_requests().await.unwrap();
        requests
            .iter()
            .filter(|r| r.url.path() == "/v1/systemone")
            .count()
    }

    /// A member's token for `sub`.
    fn member(&self, sub: &str) -> String {
        let mut claims = self.idp.claims(
            "workspaces:read workspaces:write conversations:read assistant:read assistant:write",
        );
        claims["sub"] = json!(sub);
        self.idp.token(&claims)
    }

    async fn post(&self, token: &str, uri: &str, body: &Value) -> TestResponse {
        let request = Request::post(uri)
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        testing::send(&self.app, request).await
    }

    /// Every inbound message: subject, stream, who placed it, whether the
    /// model answered.
    async fn sorted(&self) -> Vec<(String, String, String, bool)> {
        sqlx::query_as(
            "SELECT m.subject, c.stream, c.stream_source,
                    EXISTS (SELECT 1 FROM conversations.decisions d
                             WHERE d.message_id = m.id AND d.source = 'model')
               FROM conversations.messages m
               JOIN conversations.conversations c ON c.id = m.conversation_id
              WHERE m.direction = 'inbound'
              ORDER BY m.sent_at",
        )
        .fetch_all(&self.db)
        .await
        .unwrap()
    }

    async fn usage(&self, query: &str) -> TestResponse {
        let mut claims = self.idp.claims(ACCESS_SCOPE);
        claims["sub"] = json!("ada");
        claims["auth_time"] = json!(Utc::now().timestamp());
        let request = Request::get(format!("/admin/v1/system/ai-usage{query}"))
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", self.idp.token(&claims)),
            )
            .body(Body::empty())
            .unwrap();
        testing::send(&self.app, request).await
    }

    async fn grant_ada(&self) {
        let mut conn = self.db.acquire().await.unwrap();
        campbooks_admin::cli::grant(&mut conn, "ada", &[Permission::Super], None, Utc::now())
            .await
            .unwrap();
    }
}

fn story(line: &str) {
    println!("\n▸ {line}");
}

fn show(rows: &[(String, String, String, bool)]) {
    for (subject, stream, source, model) in rows {
        let by = if *model {
            "read by the AI"
        } else {
            "over the limit: rules only"
        };
        println!("    {subject:<45} {stream:<9} placed by {source:<6} {by}");
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn ai_usage_stops_at_the_daily_limit_and_resumes_the_next_day(db: PgPool) {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("campbooks_system=warn")
        .with_target(false)
        .without_time()
        .with_test_writer()
        .try_init();
    let w = world(db, 3).await;
    w.grant_ada().await;
    let sam = w.member("sam");
    let created = w
        .post(
            &sam,
            "/api/v1/workspaces",
            &json!({"name": "Acme", "kind": "business"}),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.json);
    let acme: WorkspaceId = created.json["id"].as_str().unwrap().parse().unwrap();
    let studio = WorkspaceId::new();

    story(
        "Daily AI limit: 3 model calls per workspace (CAMPBOOKS_AI__DAILY_CALLS_PER_WORKSPACE=3); \
         a tenth, 1 call, is kept for what people ask",
    );
    story("Thursday 16 October: three emails arrive for the demo workspace \"acme\"");
    assert_eq!(w.receive(acme, "sam@acme.example.com", EN).await, 2);
    let sorted = w.sorted().await;
    show(&sorted);
    assert_eq!(
        sorted.iter().filter(|(.., model)| *model).count(),
        2,
        "{sorted:?}"
    );
    // The one past the limit is left as it was stored: no model placed it.
    assert!(
        sorted
            .iter()
            .all(|(_, _, source, model)| *model || source != "model")
    );

    story("Sam asks Scout to remember something: the reserve answers");
    let compile = format!("/api/v1/w/{acme}/assistant/compile");
    let asked = json!({"text": "invoices from my accountant always matter"});
    let answered = w.post(&sam, &compile, &asked).await;
    assert_eq!(answered.status, StatusCode::OK, "{}", answered.json);
    println!("    POST {compile} -> {}", answered.status);
    story("Sam asks again: the whole day's limit is used");
    let refused = w.post(&sam, &compile, &asked).await;
    assert_eq!(refused.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(refused.json["code"], "ai_daily_limit_reached");
    println!("    -> {} {}", refused.status, refused.json["detail"]);

    story("The same day, one email arrives for another workspace, \"studio\"");
    assert_eq!(
        w.receive(studio, "kim@studio.example.com", COMMENT).await,
        1
    );

    story("Administrators read the day's usage (GET /admin/v1/system/ai-usage)");
    let today = w.usage("").await;
    assert_eq!(today.status, StatusCode::OK, "{}", today.json);
    println!(
        "    day {}, cap {}, reserve {}, resets {}",
        today.json["day"], today.json["cap"], today.json["reserve"], today.json["resets_at"]
    );
    for usage in today.json["workspaces"].as_array().unwrap() {
        let name = if usage["workspace_id"] == acme.to_string() {
            "acme"
        } else {
            "studio"
        };
        println!(
            "    {name:<7} calls {} refused {} capped {} remaining {}",
            usage["calls"], usage["refused"], usage["capped"], usage["remaining"],
        );
        for used in usage["uses"].as_array().unwrap() {
            println!(
                "              {:<9} {:<22} calls {}, tokens {} in / {} out",
                used["role"].as_str().unwrap(),
                used["feature"].as_str().unwrap(),
                used["calls"],
                used["prompt_tokens"],
                used["completion_tokens"],
            );
        }
    }
    let workspaces = today.json["workspaces"].as_array().unwrap();
    assert_eq!(
        (today.json["cap"].clone(), today.json["reserve"].clone()),
        (json!(3), json!(1))
    );
    assert_eq!(workspaces[0]["workspace_id"], acme.to_string());
    assert_eq!(
        (
            &workspaces[0]["calls"],
            &workspaces[0]["refused"],
            &workspaces[0]["capped"],
            &workspaces[0]["remaining"]
        ),
        (&json!(3), &json!(2), &json!(true), &json!(0))
    );
    let uses = workspaces[0]["uses"].as_array().unwrap();
    assert_eq!(
        uses.iter()
            .map(|u| (u["feature"].as_str().unwrap(), u["calls"].as_i64().unwrap()))
            .collect::<Vec<_>>(),
        [("conversations.triage", 2), ("assistant.compile", 1)]
    );
    assert_eq!(uses[0]["prompt_tokens"], 360);
    assert_eq!(workspaces[1]["workspace_id"], studio.to_string());
    assert_eq!(workspaces[1]["remaining"], 2);

    story(
        "Friday 17 October, 00:00 UTC: the count starts again; nothing from yesterday is replayed",
    );
    w.clock.advance(TimeDelta::days(1));
    let before = w.model_calls().await;
    Worker::new(w.state.clone()).run_until_idle().await;
    assert_eq!(w.model_calls().await, before, "no burst at midnight");

    story("Three more emails arrive for \"acme\"");
    assert_eq!(w.receive(acme, "compta@acme.example.com", FR).await, 2);
    let friday = w.usage("").await;
    assert_eq!(friday.json["day"], "2026-10-17");
    assert_eq!(friday.json["workspaces"][0]["calls"], 2);
    assert_eq!(friday.json["workspaces"][0]["refused"], 1);
    assert_eq!(friday.json["workspaces"][0]["remaining"], 1);
    let thursday = w.usage("?day=2026-10-16").await;
    assert_eq!(thursday.json["workspaces"][0]["calls"], 3);
    assert_eq!(thursday.json["workspaces"][0]["remaining"], Value::Null);
    println!(
        "    acme: Thursday {} calls ({} refused), Friday {} calls ({} refused, {} left for people)",
        thursday.json["workspaces"][0]["calls"],
        thursday.json["workspaces"][0]["refused"],
        friday.json["workspaces"][0]["calls"],
        friday.json["workspaces"][0]["refused"],
        friday.json["workspaces"][0]["remaining"],
    );
}

#[test]
fn the_composed_gateway_is_metered_with_the_default_cap() {
    let provider = ProviderConfig::new("http://laya:8000/v1".parse().unwrap(), "laya");
    let (gateway, meter) = ai_gateway(AiConfig {
        roles: [(Role::Decide, provider.clone())].into(),
        ..AiConfig::default()
    }, Tracker::default())
    .unwrap();
    assert!(gateway.unwrap().is_metered());
    assert_eq!(
        meter.cap(),
        Some(campbooks_ai::usage::DEFAULT_DAILY_CALLS_PER_WORKSPACE)
    );
    let (_, unlimited) = ai_gateway(AiConfig {
        roles: [(Role::Decide, provider)].into(),
        daily_calls_per_workspace: Some(0),
        ..AiConfig::default()
    }, Tracker::default())
    .unwrap();
    assert_eq!(unlimited.cap(), None);
    let (none, _) = ai_gateway(AiConfig::default(), Tracker::default()).unwrap();
    assert!(none.is_none(), "no role, no gateway");
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_member_cannot_read_ai_usage(db: PgPool) {
    let w = world(db, 2).await;
    let response = w.usage("").await;
    assert_eq!(response.status, StatusCode::NOT_FOUND);
}
