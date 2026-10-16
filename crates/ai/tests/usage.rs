//! Usage metering at the gateway (campbooks-cloud#152): every model call of
//! a metered gateway is attributed, admitted or refused before anything is
//! sent, and settled afterwards. The meter here is in memory; the ledger
//! itself is the `system` module's.

// Helpers outside `#[test]` fns are not covered by clippy's test exemptions.
#![allow(clippy::unwrap_used)]

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use campbooks_ai::{
    AiConfig, ChatRequest, Error, Gateway, Message, Protocol, ProviderConfig, RetryPolicy, Role,
    decide::{
        DecisionContext, DecisionState, Question,
        check::{self, CheckOutcome},
        strategy::BoxFuture,
    },
    usage::{Admission, Meter, MeterError, MeteredCall, Outcome, Scope},
};
use campbooks_kernel::tenancy::WorkspaceId;
use chrono::NaiveDate;
use serde_json::{Value, json};
use sqlx::PgPool;
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method};

const FEATURE: &str = "test.feature";

fn day() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 16).unwrap()
}

/// What the meter saw.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Seen {
    Admit(WorkspaceId, Role, String),
    Settle(WorkspaceId, Role, String, NaiveDate, Outcome),
}

/// A meter in memory: admits `cap` calls, then refuses; or fails.
#[derive(Debug, Default)]
struct Fake {
    cap: Option<u32>,
    broken: bool,
    admitted: Mutex<u32>,
    seen: Mutex<Vec<Seen>>,
}

impl Fake {
    fn capped(cap: u32) -> Arc<Self> {
        Arc::new(Self {
            cap: Some(cap),
            ..Self::default()
        })
    }

    fn seen(&self) -> Vec<Seen> {
        self.seen.lock().unwrap().clone()
    }
}

impl Meter for Fake {
    fn admit<'a>(
        &'a self,
        _db: &'a PgPool,
        call: MeteredCall<'a>,
    ) -> BoxFuture<'a, Result<Admission, MeterError>> {
        Box::pin(async move {
            if self.broken {
                return Err("the ledger is down".into());
            }
            self.seen.lock().unwrap().push(Seen::Admit(
                call.workspace,
                call.role,
                call.feature.to_owned(),
            ));
            let mut admitted = self.admitted.lock().unwrap();
            match self.cap {
                Some(cap) if *admitted >= cap => Ok(Admission::Refused { cap }),
                _ => {
                    *admitted += 1;
                    Ok(Admission::Admitted { day: day() })
                }
            }
        })
    }

    fn settle<'a>(
        &'a self,
        _db: &'a PgPool,
        call: MeteredCall<'a>,
        day: NaiveDate,
        outcome: Outcome,
    ) -> BoxFuture<'a, Result<(), MeterError>> {
        Box::pin(async move {
            self.seen.lock().unwrap().push(Seen::Settle(
                call.workspace,
                call.role,
                call.feature.to_owned(),
                day,
                outcome,
            ));
            Ok(())
        })
    }
}

/// A pool that never connects: the fake meter never uses it.
fn pool() -> PgPool {
    sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://nobody@localhost:1/none")
        .unwrap()
}

fn scope(workspace: WorkspaceId) -> Scope {
    Scope::workspace(pool(), workspace, FEATURE)
}

/// A provider for every role, all answering from `server`: chat and
/// embeddings with token counts, `decide` as laya-serve does.
async fn server(status: u16) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(move |request: &wiremock::Request| {
            let body = match request.url.path() {
                "/v1/chat/completions" => json!({
                    "model": "m",
                    "choices": [{"message": {"role": "assistant", "content": "hi"},
                                 "finish_reason": "stop"}],
                    "usage": {"prompt_tokens": 12, "completion_tokens": 5}
                }),
                "/v1/embeddings" => json!({
                    "data": [{"index": 0, "embedding": [0.1, 0.2]}],
                    "usage": {"prompt_tokens": 3, "total_tokens": 3}
                }),
                _ => json!({
                    "model": "laya-rl-agent",
                    "answers": {"asks": {"type": "noul", "noul": 0.9}},
                    "usage": {"input_tokens": 120, "output_tokens": 0}
                }),
            };
            ResponseTemplate::new(status).set_body_json(body)
        })
        .mount(&server)
        .await;
    server
}

fn gateway(server: &MockServer, meter: Option<Arc<dyn Meter>>) -> Gateway {
    let url = || format!("{}/v1", server.uri()).parse().unwrap();
    let gateway = Gateway::new(AiConfig {
        roles: HashMap::from([
            (Role::Complete, ProviderConfig::new(url(), "m")),
            (Role::Embed, ProviderConfig::new(url(), "e")),
            (
                Role::Decide,
                ProviderConfig::new(url(), "laya").with_protocol(Protocol::Laya),
            ),
        ]),
        ..AiConfig::default()
    })
    .unwrap()
    .with_retry(RetryPolicy {
        attempts: 2,
        base_delay: Duration::from_millis(1),
        max_delay: Duration::from_millis(1),
    });
    match meter {
        Some(meter) => gateway.with_meter(meter),
        None => gateway,
    }
}

async fn chat(gateway: &Gateway) -> Result<(), Error> {
    gateway
        .chat(
            Role::Complete,
            ChatRequest::new(vec![Message::user("Hello")]),
        )
        .await
        .map(drop)
}

async fn embed(gateway: &Gateway) -> Result<(), Error> {
    gateway.embed(&["Hello".to_owned()]).await.map(drop)
}

async fn decide(gateway: &Gateway) -> Result<(), Error> {
    gateway
        .decide(
            &DecisionContext::new(FEATURE),
            &DecisionState::from("Subject: Hello"),
            &[Question::noul("asks", "The sender asks something.")],
        )
        .await
        .map(drop)
}

/// Every method that reaches a model, by name: a new one belongs here.
async fn every_call(gateway: &Gateway) -> Vec<(&'static str, Result<(), Error>)> {
    vec![
        ("chat", chat(gateway).await),
        ("embed", embed(gateway).await),
        ("decide", decide(gateway).await),
    ]
}

async fn requests(server: &MockServer) -> usize {
    server.received_requests().await.unwrap().len()
}

#[tokio::test]
async fn a_metered_call_outside_a_scope_is_refused_and_sends_nothing() {
    let server = server(200).await;
    let meter = Fake::capped(100);
    let gateway = gateway(&server, Some(meter.clone()));
    assert!(gateway.is_metered());
    for (name, result) in every_call(&gateway).await {
        assert!(
            matches!(result, Err(Error::Unattributed)),
            "{name}: {result:?}"
        );
    }
    assert_eq!(requests(&server).await, 0);
    assert!(meter.seen().is_empty());
}

#[tokio::test]
async fn every_call_in_a_scope_is_counted_and_settled_with_its_tokens() {
    let server = server(200).await;
    let meter = Fake::capped(100);
    let gateway = gateway(&server, Some(meter.clone()));
    let workspace = WorkspaceId::new();
    let results = scope(workspace).run(every_call(&gateway)).await;
    for (name, result) in results {
        assert!(result.is_ok(), "{name}: {result:?}");
    }
    assert_eq!(requests(&server).await, 3);
    let tokens = |prompt_tokens, completion_tokens| Outcome {
        failed: false,
        prompt_tokens,
        completion_tokens,
    };
    let feature = FEATURE.to_owned();
    assert_eq!(
        meter.seen(),
        [
            Seen::Admit(workspace, Role::Complete, feature.clone()),
            Seen::Settle(
                workspace,
                Role::Complete,
                feature.clone(),
                day(),
                tokens(12, 5)
            ),
            Seen::Admit(workspace, Role::Embed, feature.clone()),
            Seen::Settle(workspace, Role::Embed, feature.clone(), day(), tokens(3, 0)),
            Seen::Admit(workspace, Role::Decide, feature.clone()),
            Seen::Settle(workspace, Role::Decide, feature, day(), tokens(120, 0)),
        ]
    );
}

#[tokio::test]
async fn over_the_cap_nothing_is_sent() {
    let server = server(200).await;
    let meter = Fake::capped(1);
    let gateway = gateway(&server, Some(meter.clone()));
    let results = scope(WorkspaceId::new()).run(every_call(&gateway)).await;
    assert!(results[0].1.is_ok());
    for (name, result) in &results[1..] {
        assert!(
            matches!(result, Err(Error::OverDailyCap { cap: 1 })),
            "{name}: {result:?}"
        );
    }
    assert_eq!(
        requests(&server).await,
        1,
        "only the admitted call went out"
    );
}

#[tokio::test]
async fn a_meter_that_cannot_count_stops_the_call() {
    let server = server(200).await;
    let meter = Arc::new(Fake {
        broken: true,
        ..Fake::default()
    });
    let gateway = gateway(&server, Some(meter));
    let results = scope(WorkspaceId::new()).run(every_call(&gateway)).await;
    for (name, result) in results {
        assert!(matches!(result, Err(Error::Metering)), "{name}: {result:?}");
    }
    assert_eq!(requests(&server).await, 0, "a broken ledger never uncaps");
}

#[tokio::test]
async fn a_failed_call_counts_as_failed_once_whatever_its_retries() {
    let server = server(503).await;
    let meter = Fake::capped(100);
    let gateway = gateway(&server, Some(meter.clone()));
    let workspace = WorkspaceId::new();
    let result = scope(workspace).run(decide(&gateway)).await;
    assert!(matches!(result, Err(Error::Status { status: 503 })));
    assert_eq!(requests(&server).await, 2, "retried once");
    assert_eq!(
        meter.seen(),
        [
            Seen::Admit(workspace, Role::Decide, FEATURE.to_owned()),
            Seen::Settle(
                workspace,
                Role::Decide,
                FEATURE.to_owned(),
                day(),
                Outcome {
                    failed: true,
                    ..Outcome::default()
                }
            ),
        ]
    );
}

#[tokio::test]
async fn an_unconfigured_role_or_an_empty_call_costs_nothing() {
    let server = server(200).await;
    let meter = Fake::capped(100);
    let gateway = Gateway::new(AiConfig::default())
        .unwrap()
        .with_meter(meter.clone());
    let workspace = WorkspaceId::new();
    let configured = self::gateway(&server, Some(meter.clone()));
    scope(workspace)
        .run(async {
            assert!(matches!(
                chat(&gateway).await,
                Err(Error::RoleNotConfigured(Role::Complete))
            ));
            assert!(configured.embed(&[]).await.unwrap().is_empty());
        })
        .await;
    assert!(meter.seen().is_empty());
}

#[tokio::test]
async fn an_unmetered_gateway_needs_no_scope() {
    let server = server(200).await;
    let gateway = gateway(&server, None);
    assert!(!gateway.is_metered());
    for (name, result) in every_call(&gateway).await {
        assert!(result.is_ok(), "{name}: {result:?}");
    }
}

#[tokio::test]
async fn the_answers_check_is_the_instances_and_counts_against_no_workspace() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": "laya", "answers": answers_for_every_triage_question()
        })))
        .mount(&server)
        .await;
    let meter = Fake::capped(0);
    let gateway = gateway(&server, Some(meter.clone()));
    let report = gateway.run_answers_check().await;
    assert_ne!(
        report.outcome,
        CheckOutcome::Unavailable,
        "the check ran: {report:?}"
    );
    assert_eq!(requests(&server).await, check::cases().len());
    assert!(meter.seen().is_empty());
}

fn answers_for_every_triage_question() -> Value {
    let noul = json!({"type": "noul", "noul": 0.1});
    json!({
        "stream": {"type": "choice", "choice": "primary",
                   "probabilities": {"primary": 0.7, "updates": 0.1, "receipts": 0.1,
                                     "newsletters": 0.1}},
        "needs_reply": noul, "asks_action": noul, "has_deadline": noul,
        "urgency": {"type": "score", "score": 0.2,
                    "probabilities": {"0": 0.8, "1": 0.2, "2": 0.0}},
        "suspicious": noul
    })
}
