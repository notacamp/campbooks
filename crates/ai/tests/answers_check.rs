//! The `decide` role's answers check (campbooks-cloud#128), against
//! laya-serve answers recorded from a real server.
//!
//! `fixtures/answers-check/laya-multilingual-*.jsonl` hold, one per line in
//! the order of [`check::cases`], what laya-serve 0.3.21 with the
//! multilingual checkpoint (the image `deploy/laya` builds, CPU) answered
//! about the eight fictional check emails on 2026-10-14:
//!
//! - `ctx1`: the requests with triage context version 1 (yes/no questions;
//!   version 2 only rewords the hints line, from the facts);
//! - `before`: the requests before that change (no context, the questions
//!   as declarative statements).
//!
//! The recordings predate the money question and the check's last two
//! emails (the bill and the payment confirmation): the replay answers those
//! as a model that says "yes" to everything and picks each choice's first
//! option would ([`yes_sayer`]), so the counts below include them.
//!
//! Only the fictional check emails went into them (reserved domains, and
//! GitHub's notification address for the code-review case); they hold
//! probabilities and the score levels' legend, nothing else.

// Helpers outside `#[test]` fns are not covered by clippy's test exemptions.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use campbooks_ai::{
    AiConfig, Error, Gateway, Protocol, ProviderConfig, RetryPolicy, Role,
    decide::{
        DecisionContext,
        check::{self, AnswersCheck, CheckOutcome, Expected},
        signals::email::triage_questions,
        strategy::{Model, Strategy},
    },
};
use serde_json::{Value, json};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate, matchers::method};

fn recorded(name: &str) -> Vec<Value> {
    let path = format!(
        "{}/tests/fixtures/answers-check/{name}.jsonl",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path)
        .expect(&path)
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

/// Which check case a request is about, by its subject.
fn case_of(request: &Request) -> usize {
    let body: Value = serde_json::from_slice(&request.body).unwrap();
    let state = body["state"].as_str().unwrap();
    check::cases()
        .iter()
        .position(|case| state.contains(&format!("Subject: {}\n", case.envelope.subject)))
        .expect("a check case")
}

/// What a model that says "yes" to everything and picks each choice's
/// first option answers.
fn yes_sayer() -> Value {
    let noul = json!({"type": "noul", "noul": 0.95});
    json!({
        "model": "laya-rl-agent",
        "answers": {
            "stream": {"type": "choice", "choice": "primary",
                       "probabilities": {"primary": 0.7, "updates": 0.1, "receipts": 0.1,
                                         "newsletters": 0.1}},
            "needs_reply": noul, "asks_action": noul, "has_deadline": noul,
            "urgency": {"type": "score", "score": 2.0,
                        "probabilities": {"0": 0.0, "1": 0.1, "2": 0.9}},
            "suspicious": noul,
            "money": money_first(),
        }
    })
}

fn money_first() -> Value {
    json!({"type": "choice", "choice": "owed",
           "probabilities": {"owed": 0.8, "paid": 0.1, "neither": 0.1}})
}

/// Answers each case with the recorded response for it; the money question
/// and the cases recorded before it, as [`yes_sayer`].
struct Replay(Vec<Value>);

impl Respond for Replay {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let mut body = self
            .0
            .get(case_of(request))
            .cloned()
            .unwrap_or_else(yes_sayer);
        if body["answers"].get("money").is_none() {
            body["answers"]["money"] = money_first();
        }
        ResponseTemplate::new(200).set_body_json(body)
    }
}

/// A model that gets every expected answer right.
struct Right;

impl Respond for Right {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let case = &check::cases()[case_of(request)];
        let noul = |p: f64| json!({"type": "noul", "noul": p});
        let mut answers = json!({
            "stream": {"type": "choice", "choice": "primary",
                       "probabilities": {"primary": 0.7, "updates": 0.1, "receipts": 0.1,
                                         "newsletters": 0.1}},
            "needs_reply": noul(0.1), "asks_action": noul(0.1), "has_deadline": noul(0.1),
            "urgency": {"type": "score", "score": 0.2,
                        "probabilities": {"0": 0.8, "1": 0.2, "2": 0.0}},
            "suspicious": noul(0.05),
            "money": {"type": "choice", "choice": "neither",
                      "probabilities": {"owed": 0.1, "paid": 0.1, "neither": 0.8}}
        });
        for (question, expected) in &case.expect {
            answers[*question] = match expected {
                Expected::Yes => noul(0.95),
                Expected::No => noul(0.05),
                Expected::Stream(stream) => {
                    let mut p = json!({"primary": 0.0, "updates": 0.0, "receipts": 0.0,
                                       "newsletters": 0.0});
                    p[*stream] = json!(1.0);
                    json!({"type": "choice", "choice": stream, "probabilities": p})
                }
                Expected::Money(option) => {
                    let mut p = json!({"owed": 0.0, "paid": 0.0, "neither": 0.0});
                    p[*option] = json!(1.0);
                    json!({"type": "choice", "choice": option, "probabilities": p})
                }
            };
        }
        ResponseTemplate::new(200)
            .set_body_json(json!({"model": "laya-rl-agent", "answers": answers}))
    }
}

async fn laya(respond: impl Respond + 'static) -> (MockServer, Gateway) {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(respond)
        .mount(&server)
        .await;
    let provider = ProviderConfig::new(
        format!("{}/v1", server.uri()).parse().unwrap(),
        "convaiinnovations/laya-multilingual",
    )
    .with_protocol(Protocol::Laya);
    let gateway = Gateway::new(AiConfig {
        roles: [(Role::Decide, provider)].into(),
        ..AiConfig::default()
    })
    .unwrap()
    .with_retry(RetryPolicy {
        attempts: 1,
        base_delay: Duration::ZERO,
        max_delay: Duration::ZERO,
    });
    (server, gateway)
}

fn sample() -> campbooks_ai::decide::signals::email::Envelope {
    check::cases().remove(1).envelope
}

#[tokio::test]
async fn the_recorded_laya_answers_fail_the_check() {
    let (_server, gateway) = laya(Replay(recorded("laya-multilingual-ctx1"))).await;
    assert_eq!(gateway.answers_check(), AnswersCheck::Pending);

    let report = gateway.run_answers_check().await;
    assert_eq!(report.outcome, CheckOutcome::Failed);
    assert_eq!((report.right, report.total), (11, 50));
    assert_eq!(
        report.model.as_deref(),
        Some("convaiinnovations/laya/multilingual")
    );
    assert_eq!(gateway.answers_check(), AnswersCheck::Done(report));
    assert!(!gateway.answers_trusted());
}

#[tokio::test]
async fn the_answers_before_the_context_failed_it_too() {
    let (_server, gateway) = laya(Replay(recorded("laya-multilingual-before"))).await;
    let report = gateway.run_answers_check().await;
    assert_eq!(report.outcome, CheckOutcome::Failed);
    assert_eq!((report.right, report.total), (21, 50));
}

/// The "yes" is the model's own: each answer is what laya-serve said,
/// mapped without inversion (`noul` is P(true), choice probabilities by
/// label in option order).
#[tokio::test]
async fn recorded_answers_map_to_what_laya_said() {
    let recorded = recorded("laya-multilingual-ctx1");
    let (_server, gateway) = laya(Replay(recorded.clone())).await;
    let questions = triage_questions();
    for (case, wire) in check::cases().iter().zip(&recorded) {
        let answers = gateway
            .decide(
                &DecisionContext::new("test.check"),
                &case.envelope.decision_state(),
                &questions,
            )
            .await
            .unwrap();
        for answer in &answers {
            let said = &wire["answers"][&answer.question_id];
            // Not recorded: the money question came later.
            let Some(kind) = said["type"].as_str() else {
                continue;
            };
            match kind {
                "noul" => {
                    let p = said["noul"].as_f64().unwrap();
                    assert!((answer.probability_true().unwrap() - p).abs() < 1e-9);
                    assert_eq!(answer.noul(), Some(p >= 0.5));
                }
                "choice" => {
                    assert_eq!(answer.choice(), said["choice"].as_str());
                    let p = answer.probabilities.as_ref().unwrap();
                    for (i, label) in ["primary", "updates", "receipts", "newsletters"]
                        .iter()
                        .enumerate()
                    {
                        let want = said["probabilities"][*label].as_f64().unwrap();
                        assert!((p[i] - want).abs() < 1e-3, "{label}");
                    }
                }
                _ => {}
            }
        }
    }
}

#[tokio::test]
async fn a_checked_model_refuses_while_the_check_fails() {
    let (server, gateway) = laya(Replay(recorded("laya-multilingual-ctx1"))).await;
    gateway.run_answers_check().await;
    let asked = server.received_requests().await.unwrap().len();

    let model = Model::new("decide", gateway, "test.check").checked();
    let result = model.decide(&sample(), &triage_questions()).await;
    assert!(
        matches!(result, Err(Error::AnswersNotTrusted)),
        "{result:?}"
    );
    assert_eq!(
        server.received_requests().await.unwrap().len(),
        asked,
        "no request for the message"
    );
}

#[tokio::test]
async fn a_model_that_gets_them_right_passes_and_its_answers_are_stamped() {
    let (_server, gateway) = laya(Right).await;
    let report = gateway.run_answers_check().await;
    assert_eq!(report.outcome, CheckOutcome::Passed);
    assert_eq!((report.right, report.total), (50, 50));

    let model = Model::new("decide", gateway.clone(), "test.check").checked();
    let decided = model.decide(&sample(), &triage_questions()).await.unwrap();
    for answer in decided.answers.iter().flatten() {
        assert_eq!(answer.answer.model_version.as_deref(), Some("checked:ctx3"));
    }

    // An unchecked strategy (the eval harness) leaves the version alone.
    let plain = Model::new("decide", gateway, "test.check");
    let decided = plain.decide(&sample(), &triage_questions()).await.unwrap();
    assert!(
        decided
            .answers
            .iter()
            .flatten()
            .all(|d| d.answer.model_version.is_none())
    );
}

#[tokio::test]
async fn an_unanswered_check_is_unavailable_and_not_trusted() {
    let (_server, gateway) = laya(ResponseTemplate::new(503)).await;
    let report = gateway.run_answers_check().await;
    assert_eq!(report.outcome, CheckOutcome::Unavailable);
    assert!(!gateway.answers_trusted());
}

#[tokio::test]
async fn before_the_first_check_nothing_is_trusted() {
    let (_server, gateway) = laya(Right).await;
    assert!(!gateway.answers_trusted());
}

#[tokio::test]
async fn keep_checked_runs_at_once_and_stops() {
    let (server, gateway) = laya(Right).await;
    gateway.keep_answers_checked(std::future::ready(())).await;
    assert_eq!(
        server.received_requests().await.unwrap().len(),
        check::cases().len()
    );
    assert!(gateway.answers_trusted());

    // No decide role: nothing to check.
    let embed_only = Gateway::new(AiConfig {
        roles: [(
            Role::Embed,
            ProviderConfig::new(format!("{}/v1", server.uri()).parse().unwrap(), "e"),
        )]
        .into(),
        ..AiConfig::default()
    })
    .unwrap();
    embed_only
        .keep_answers_checked(std::future::pending())
        .await;
    assert_eq!(embed_only.answers_check(), AnswersCheck::Pending);
}
