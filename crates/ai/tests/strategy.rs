//! Strategies that call a model server, against mocks: the `decide` model as
//! a strategy, k-NN over the `embed` role, and a cascade that never reaches
//! the model when the facts already decided.

// Helpers outside `#[test]` fns are not covered by clippy's test exemptions.
#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use campbooks_ai::{
    AiConfig, Gateway, Protocol, ProviderConfig, Role,
    decide::{
        AnswerValue, Question,
        knn::{GatewayEmbedder, InMemoryNeighbours, Knn},
        signals::{
            email::{Envelope, Party, triage, triage_questions},
            facts::EmailFacts,
        },
        strategy::{Cascade, Model, Stage, Strategy},
    },
};
use serde_json::json;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_partial_json, method, path},
};

fn gateway(role: Role, server: &MockServer, protocol: Protocol) -> Gateway {
    let provider = ProviderConfig::new(format!("{}/v1", server.uri()).parse().unwrap(), "m")
        .with_protocol(protocol);
    Gateway::new(AiConfig {
        roles: [(role, provider)].into(),
        ..AiConfig::default()
    })
    .unwrap()
}

fn newsletter() -> Envelope {
    let mut env = Envelope {
        from: Party {
            name: "Livraria".into(),
            address: "news@livraria.example".into(),
        },
        subject: "Novidades de outubro".into(),
        body: "Os livros do mês.".into(),
        ..Envelope::default()
    };
    env.headers.insert(
        "List-Unsubscribe".into(),
        "<https://livraria.example/u>".into(),
    );
    env
}

fn suspicious_question() -> Vec<Question> {
    triage_questions()
        .into_iter()
        .filter(|q| q.id == triage::SUSPICIOUS)
        .collect()
}

#[tokio::test]
async fn the_model_strategy_asks_decide_once_with_the_envelope_state() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .and(body_partial_json(json!({
            "questions": {"suspicious": {"type": "noul"}}
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": "laya-rl-agent",
            "answers": {"suspicious": {
                "type": "noul", "noul": 0.1, "confidence": 0.9, "answer_confidence": 0.9
            }},
            "routing": {"repo": "convaiinnovations/laya/multilingual"}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let model = Model::new(
        "laya",
        gateway(Role::Decide, &server, Protocol::Laya),
        "conversations.triage",
    );
    let questions = suspicious_question();
    let out = model.decide(&newsletter(), &questions).await.unwrap();
    assert_eq!(out.remote_calls, 1);
    let decided = out.answers[0].as_ref().unwrap();
    assert_eq!(decided.answer.noul(), Some(false));
    assert_eq!(decided.reasons[0].rule, "model.decide");

    let requests = server.received_requests().await.unwrap();
    let body: serde_json::Value = requests[0].body_json().unwrap();
    let state = body["state"].as_str().unwrap();
    assert!(state.contains("Subject: Novidades de outubro"), "{state}");
    assert!(state.contains("list-unsubscribe:"), "{state}");
}

#[tokio::test]
async fn a_cascade_does_not_call_the_model_when_the_facts_decided() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;
    let model = Model::new(
        "laya",
        gateway(Role::Decide, &server, Protocol::Laya),
        "conversations.triage",
    );
    let questions: Vec<Question> = triage_questions()
        .into_iter()
        .filter(|q| q.id == triage::NEEDS_REPLY)
        .collect();
    let cascade = Cascade::new(vec![
        Stage {
            strategy: Arc::new(EmailFacts),
            thresholds: [(triage::NEEDS_REPLY.to_owned(), 0.9)].into(),
        },
        Stage {
            strategy: Arc::new(model),
            thresholds: [(triage::NEEDS_REPLY.to_owned(), 0.0)].into(),
        },
    ]);
    assert_eq!(cascade.name(), "facts>laya");
    let out = cascade.decide(&newsletter(), &questions).await.unwrap();
    let decided = out.answers[0].as_ref().unwrap();
    assert_eq!(decided.answer.value, AnswerValue::Noul(false));
    assert_eq!(out.remote_calls, 0);
}

#[tokio::test]
async fn knn_embeds_through_the_embed_role() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/embeddings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [{"index": 0, "embedding": [3.0, 4.0]}]
        })))
        .expect(1)
        .mount(&server)
        .await;
    let mut store = InMemoryNeighbours::default();
    for _ in 0..3 {
        store.push(
            vec![0.6, 0.8],
            [(triage::SUSPICIOUS.to_owned(), AnswerValue::Noul(false))].into(),
        );
    }
    let knn = Knn::new(
        GatewayEmbedder::new(gateway(Role::Embed, &server, Protocol::OpenAi), "embed"),
        store,
    );
    assert_eq!(knn.name(), "knn-embed");
    let questions = suspicious_question();
    let out = knn.decide(&newsletter(), &questions).await.unwrap();
    assert_eq!(out.remote_calls, 1);
    let decided = out.answers[0].as_ref().unwrap();
    assert_eq!(decided.answer.noul(), Some(false));
    // Three neighbours at similarity 1, smoothed: (3 + 0.5) / (3 + 1).
    assert!((decided.answer.confidence.unwrap() - 0.875).abs() < 1e-6);
}
