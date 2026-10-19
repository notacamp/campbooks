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
            moves,
        },
    },
};
use serde_json::{Value, json};
use sqlx::PgPool;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

use campbooks_kernel::{db::BeginFor as _, tenancy::WorkspaceId};

use crate::{
    ActionSettings, ConversationsDeps,
    domain::{actions, conversation::summary},
    model::ConversationId,
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

/// How the stand-in answers: Laya's answers object, or the content of an
/// OpenAI-compatible chat completion (no log-probabilities).
#[derive(Clone)]
enum Answering {
    Laya(Value),
    OpenAi(Value),
}

/// The triage answers as an OpenAI-compatible model writes them, and every
/// catalogue answer `true`.
fn openai_answers() -> Value {
    let mut object = json!({
        "stream": "primary", "needs_reply": true, "asks_action": true,
        "has_deadline": false, "urgency": 1, "suspicious": false, "money": "neither"
    });
    for id in question::ALL {
        object[id] = json!(true);
    }
    object
}

/// A gateway on `server` speaking `protocol`, whose answers check passed.
fn gateway(server: &MockServer, protocol: Protocol) -> Gateway {
    let mut provider = ProviderConfig::new(
        format!("{}/v1", server.uri()).parse().unwrap(),
        "convaiinnovations/laya-multilingual",
    )
    .with_protocol(protocol);
    provider.logprobs = false;
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
    /// A listed conversation with a need, and its workspace.
    with_need: Option<(WorkspaceId, ConversationId)>,
    kit: Kit,
}

async fn run(db: PgPool, enabled: bool) -> Run {
    run_with(db, enabled, Answering::Laya(answers())).await
}

async fn run_with(db: PgPool, enabled: bool, answering: Answering) -> Run {
    let server = MockServer::start().await;
    let protocol = match &answering {
        Answering::Laya(answers) => {
            Mock::given(method("POST"))
                .and(path("/v1/systemone"))
                .respond_with(ResponseTemplate::new(200).set_body_json(answers))
                .mount(&server)
                .await;
            Protocol::Laya
        }
        Answering::OpenAi(content) => {
            Mock::given(method("POST"))
                .and(path("/v1/chat/completions"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                    "model": "instruct-test",
                    "choices": [{"message": {"role": "assistant",
                                             "content": content.to_string()}}]
                })))
                .mount(&server)
                .await;
            Protocol::OpenAi
        }
    };
    let mut deps = ConversationsDeps {
        ai: Some(gateway(&server, protocol)),
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
    let with_needs: Vec<&Value> = listed.json["conversations"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| !c["need"].is_null())
        .collect();
    let needs = with_needs.iter().map(|c| c["need"].clone()).collect();
    let with_need = with_needs
        .first()
        .map(|c| (workspace, c["id"].as_str().unwrap().parse().unwrap()));
    Run {
        bodies,
        decisions,
        rated,
        needs,
        with_need,
        kit,
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
        // Whose move it is rides along with them; this stand-in answers
        // neither, which records nothing and fails nothing.
        assert!(after.contains_key(moves::question::NEXT_MOVE));
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
        let actions = n["actions"].as_array().map_or(&[][..], Vec::as_slice);
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

    // Switched off, what was rated is not read: the flag is a kill switch.
    let (workspace, id) = on.with_need.expect("a conversation with a need");
    let mut tx = on.kit.state.db.begin_for(workspace).await.unwrap();
    let mut summaries = [summary(&mut tx, None, id).await.unwrap()];
    actions::attach(&mut tx, ActionSettings::default(), &mut summaries)
        .await
        .unwrap();
    assert!(summaries[0].need.as_ref().unwrap().actions.is_none());
    actions::attach(
        &mut tx,
        ActionSettings {
            enabled: true,
            ..ActionSettings::default()
        },
        &mut summaries,
    )
    .await
    .unwrap();
    assert!(summaries[0].need.as_ref().unwrap().actions.is_some());
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
    // No one but the sender is named: only `next_move` is asked, right
    // after triage, then the catalogue.
    assert_eq!(asked[triage.len()].id, moves::question::NEXT_MOVE);
    assert_eq!(asked.len(), triage.len() + 1 + question::ALL.len());
    assert_eq!(on.0.moves(&envelope).len(), 1);
    assert!(pass_off_moves().is_empty());
}

fn pass_off_moves() -> Vec<campbooks_ai::decide::Question> {
    let pass = super::DecideMessage::new(
        None,
        14,
        crate::Policy::baseline(),
        crate::domain::needs::NeedSettings::new(14, &crate::Policy::baseline()),
    );
    let envelope: Envelope = serde_json::from_value(json!({
        "from": {"name": "Ana", "address": "ana@example.com"},
        "to": [{"name": "Rui", "address": "rui@example.com"}]
    }))
    .unwrap();
    pass.0.moves(&envelope)
}

#[test]
fn with_people_to_pick_awaited_is_asked_after_next_move() {
    let on = super::DecideMessage::new(
        None,
        14,
        crate::Policy::baseline(),
        crate::domain::needs::NeedSettings::new(14, &crate::Policy::baseline()),
    )
    .with_actions(ActionSettings {
        enabled: true,
        ..ActionSettings::default()
    });
    let envelope: Envelope = serde_json::from_value(json!({
        "from": {"name": "Joao", "address": "joao@studio.example"},
        "to": [{"name": "Ana Costa", "address": "ana@studio.example"}],
        "cc": [{"address": "no-reply@tickets.example.com"}]
    }))
    .unwrap();
    let triage = triage_questions();
    let asked: Vec<String> =
        on.0.asked(&triage, &envelope)
            .into_iter()
            .skip(triage.len())
            .map(|q| q.id)
            .collect();
    assert_eq!(
        asked[..2],
        [moves::question::NEXT_MOVE, moves::question::AWAITED]
    );
}

/// The catalogue's answers broken three ways: Remind missing, Forward not a
/// number, the paid question out of range.
fn broken(mut answers: Value) -> Value {
    let map = answers["answers"].as_object_mut().unwrap();
    map.remove(question::REMIND);
    map.insert(
        question::FORWARD.into(),
        json!({"type": "noul", "noul": "NaN"}),
    );
    map.insert(
        question::PAID_AUTOMATICALLY.into(),
        json!({"type": "noul", "noul": 1.5, "confidence": 1.5}),
    );
    answers
}

/// Each listed need as triage left it (question, reason, trust, basis,
/// urgency and due date), sorted: ids differ between runs.
fn need_shapes(run: &Run) -> Vec<String> {
    let mut shapes: Vec<String> = run
        .needs
        .iter()
        .map(|need| {
            [
                "question", "reason", "trust", "basis", "urgent", "overdue", "due_on",
            ]
            .iter()
            .map(|field| format!("{field}={}", need[*field]))
            .collect::<Vec<_>>()
            .join(" ")
        })
        .collect();
    shapes.sort();
    shapes
}

/// The actions rated in `run`.
async fn rated_actions(run: &Run) -> Vec<String> {
    sqlx::query_scalar("SELECT DISTINCT action FROM conversations.action_likelihoods ORDER BY 1")
        .fetch_all(&run.kit.db)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_catalogue_answer_that_does_not_fit_drops_only_its_rating(db: PgPool) {
    let off = run(db.clone(), false).await;
    let on = run_with(db, true, Answering::Laya(broken(answers()))).await;
    // The job went through: triage is stored as with the catalogue off.
    assert!(!on.decisions.is_empty());
    assert_eq!(off.decisions, on.decisions);
    // The needs themselves, not just how many: what each asks, why, how
    // sure and how urgent are the same.
    assert_eq!(need_shapes(&off), need_shapes(&on));
    // Only the actions whose answers fit are rated: Open attachment (its
    // answer was fine), Reply (needs_reply), Report and It's safe
    // (suspicious); not Remind, Forward or the paid pair.
    let rated = rated_actions(&on).await;
    assert_eq!(
        rated,
        ["open_attachment", "reply", "report", "safe"],
        "{rated:?}"
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn over_openai_a_catalogue_answer_that_does_not_fit_fails_nothing(db: PgPool) {
    let mut content = openai_answers();
    content.as_object_mut().unwrap().remove(question::REMIND);
    content[question::FORWARD] = json!("maybe");
    content[question::PAID_AUTOMATICALLY] = json!(7);
    let off = run_with(db.clone(), false, Answering::OpenAi(openai_answers())).await;
    let on = run_with(db, true, Answering::OpenAi(content)).await;
    assert!(!on.decisions.is_empty());
    assert_eq!(off.decisions, on.decisions);
    // Without log-probabilities nothing has a likelihood to rate.
    assert!(rated_actions(&on).await.is_empty());
}

#[test]
fn an_image_the_body_shows_is_no_attachment_to_open() {
    let row = |name: &str, content_type: &str, inline: bool, cid: bool| {
        (name.to_owned(), content_type.to_owned(), inline, cid)
    };
    let files = super::files_only(vec![
        // A signature logo: inline, with a Content-ID, a picture.
        row("logo.png", "image/png", true, true),
        row("lease.pdf", "application/pdf", false, false),
        // A picture sent as a file, and an inline PDF: files both.
        row("photo.jpg", "image/jpeg", false, false),
        row("terms.pdf", "application/pdf", true, true),
    ]);
    let names: Vec<&str> = files.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["lease.pdf", "photo.jpg", "terms.pdf"]);
    // Only the logo: nothing to open, so Open attachment is not asked.
    let logo_only = Envelope {
        attachments: super::files_only(vec![row("logo.png", "image/png", true, true)]),
        ..Envelope::default()
    };
    assert!(
        !campbooks_ai::decide::signals::actions::action_questions(&logo_only)
            .iter()
            .any(|q| q.id == question::OPEN_ATTACHMENT)
    );
}
