//! The action catalogue in the model pass (campbooks-cloud#166): off, the
//! request and every need are exactly as before; on, its questions go in
//! slots of their own in the same call, the triage answers are the same,
//! and each need offers its own actions.

use campbooks_ai::{
    AiConfig, Gateway, Protocol, ProviderConfig, Role,
    decide::{
        check::{CheckOutcome, CheckReport},
        signals::{
            actions::question,
            email::{Envelope, triage_questions},
        },
    },
};
use serde_json::{Value, json};
use sqlx::PgPool;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

use crate::{
    ActionSettings, ConversationsDeps,
    testkit::{Kit, sorting_deps},
};

/// A laya-serve answering every triage question, and every catalogue
/// question at 0.8 (ignored when not asked).
fn answers() -> Value {
    let noul = |p: f64| json!({"type": "noul", "noul": p, "confidence": p});
    let mut answers = json!({
        "model": "laya-test",
        "answers": {
            "stream": {"type": "choice", "choice": "primary",
                "probabilities": {"primary": 0.97, "updates": 0.01, "receipts": 0.01,
                                  "newsletters": 0.01}},
            "needs_reply": noul(0.9),
            "asks_action": noul(0.8),
            "has_deadline": noul(0.7),
            "urgency": {"type": "score", "score": 1.0,
                "probabilities": {"0": 0.2, "1": 0.6, "2": 0.2}},
            "suspicious": noul(0.05),
            "money": {"type": "choice", "choice": "neither",
                "probabilities": {"owed": 0.02, "paid": 0.02, "neither": 0.96}}
        }
    });
    for id in question::ALL {
        answers["answers"][id] = noul(0.8);
    }
    answers
}

/// A Laya gateway on `server` whose answers check passed.
fn gateway(server: &MockServer) -> Gateway {
    let provider = ProviderConfig::new(
        format!("{}/v1", server.uri()).parse().unwrap(),
        "convaiinnovations/laya-multilingual",
    )
    .with_protocol(Protocol::Laya);
    let gateway = Gateway::new(AiConfig {
        roles: [(Role::Decide, provider)].into(),
        ..AiConfig::default()
    })
    .unwrap();
    gateway.record_answers_check(CheckReport {
        outcome: CheckOutcome::Passed,
        right: 33,
        total: 33,
        model: Some("laya-test".into()),
        checked_at: chrono::Utc::now(),
    });
    gateway
}

/// What one run over Sam's English fixtures sent and kept, with the
/// catalogue `enabled` or not.
struct Run {
    /// Every decide request body, in order.
    bodies: Vec<Value>,
    /// Every stored decision: message, question, value, whether it acted.
    decisions: Vec<(String, String, Value, bool)>,
    /// Rows rated for the catalogue.
    rated: i64,
    /// Every listed conversation's need, as the API gives it.
    needs: Vec<Value>,
}

async fn run(db: PgPool, enabled: bool) -> Run {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .respond_with(ResponseTemplate::new(200).set_body_json(answers()))
        .mount(&server)
        .await;
    let mut deps = ConversationsDeps {
        ai: Some(gateway(&server)),
        model: None,
        ..sorting_deps()
    };
    deps.config.decide_backfill_days = 100_000;
    deps.config.actions = ActionSettings {
        enabled,
        ..ActionSettings::default()
    };
    let kit = Kit::with_deps(db, deps).await;
    let (workspace, _inbox, sam) = kit.sams_inbox().await;
    let bodies = server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .map(|r| serde_json::from_slice(&r.body).unwrap())
        .collect();
    let decisions = sqlx::query_as(
        "SELECT m.provider_message_id, d.question, d.value, d.acted
           FROM conversations.decisions d
           JOIN conversations.messages m ON m.id = d.message_id
          WHERE d.workspace_id = $1 AND d.source = 'model'
          ORDER BY 1, 2",
    )
    .bind(workspace.as_uuid())
    .fetch_all(&kit.db)
    .await
    .unwrap();
    let rated = sqlx::query_scalar(
        "SELECT count(*) FROM conversations.action_likelihoods WHERE workspace_id = $1",
    )
    .bind(workspace.as_uuid())
    .fetch_one(&kit.db)
    .await
    .unwrap();
    let listed = kit
        .get(
            &sam,
            &format!("/api/v1/w/{workspace}/conversations?stream=primary"),
        )
        .await;
    let needs = listed.json["conversations"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| !c["need"].is_null())
        .map(|c| c["need"].clone())
        .collect();
    Run {
        bodies,
        decisions,
        rated,
        needs,
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn off_the_request_and_every_need_are_as_before(db: PgPool) {
    let off = run(db, false).await;
    let triage: Vec<String> = triage_questions().into_iter().map(|q| q.id).collect();
    assert!(!off.bodies.is_empty());
    for body in &off.bodies {
        let mut asked: Vec<String> = body["questions"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        asked.sort();
        let mut want = triage.clone();
        want.sort();
        assert_eq!(asked, want, "only the triage questions are asked");
    }
    assert_eq!(off.rated, 0);
    assert!(!off.needs.is_empty());
    for need in &off.needs {
        assert!(need.get("actions").is_none(), "{need}");
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn on_its_questions_ride_along_and_change_no_triage_answer(db: PgPool) {
    let off = run(db.clone(), false).await;
    let on = run(db, true).await;
    assert_eq!(off.bodies.len(), on.bodies.len(), "no second call");
    // The jobs run in any order: each message's request, by its state.
    for after in &on.bodies {
        let before = off
            .bodies
            .iter()
            .find(|b| b["state"] == after["state"])
            .expect("the same message was asked about with the catalogue off");
        let (before, after) = (
            before["questions"].as_object().unwrap(),
            after["questions"].as_object().unwrap(),
        );
        for (id, asked) in before {
            assert_eq!(after.get(id), Some(asked), "{id} is asked as before");
        }
        assert!(after.contains_key(question::REMIND));
        assert!(after.contains_key(question::FORWARD));
        assert!(after.contains_key(question::PAID_AUTOMATICALLY));
    }
    // The same answers, stored and acted on alike; the catalogue's are not
    // decisions.
    assert_eq!(off.decisions, on.decisions);
    assert!(
        on.decisions
            .iter()
            .all(|(_, q, ..)| !q.starts_with("action_"))
    );
    assert!(on.rated > 0);
    // Every need offers its own: Reply first where the sender can be
    // answered (0.9), then what else cleared the floor; never a bill's or a
    // warning's answers on a need that is neither.
    assert!(on.needs.iter().all(|n| {
        let actions = n["actions"].as_array().unwrap();
        actions.len() <= 3
            && actions.iter().all(|a| {
                !["report", "safe", "already_paid", "still_to_pay"]
                    .contains(&a["actions"][0].as_str().unwrap())
            })
    }));
    let reply = on
        .needs
        .iter()
        .find(|n| n["actions"][0]["actions"][0] == "reply")
        .unwrap_or_else(|| panic!("a need offering Reply in {:?}", on.needs));
    assert_eq!(
        reply["actions"][0],
        json!({"actions": ["reply"], "likelihood": 90})
    );
    assert!(reply["actions"].as_array().unwrap().len() <= 3);
}

#[test]
fn off_the_pass_asks_exactly_the_triage_questions() {
    let pass = super::DecideMessage::new(
        None,
        14,
        crate::Policy::baseline(),
        crate::domain::needs::NeedSettings::new(14, &crate::Policy::baseline()),
    );
    let envelope: Envelope = serde_json::from_value(json!({
        "from": {"name": "Ana", "address": "ana@example.com"},
        "attachments": [{"name": "a.pdf", "content_type": "application/pdf"}]
    }))
    .unwrap();
    let triage = triage_questions();
    assert_eq!(pass.0.asked(&triage, &envelope), triage);
    let on = pass.with_actions(ActionSettings {
        enabled: true,
        ..ActionSettings::default()
    });
    let asked = on.0.asked(&triage, &envelope);
    assert_eq!(asked[..triage.len()], triage[..]);
    assert_eq!(asked.len(), triage.len() + question::ALL.len());
}
