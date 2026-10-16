//! The `decide` role against a real provider. Ignored by default: it needs a
//! running model server. With the Laya image from `deploy/laya`:
//!
//! ```sh
//! docker build -t campbooks-laya deploy/laya
//! docker run -d --name laya -e LAYA_API_KEY=dev-key -p 127.0.0.1:18000:8000 \
//!   -v laya-models:/models campbooks-laya        # first start downloads ~680 MB
//! CAMPBOOKS_AI__DECIDE__PROTOCOL=laya \
//! CAMPBOOKS_AI__DECIDE__BASE_URL=http://127.0.0.1:18000/v1 \
//! CAMPBOOKS_AI__DECIDE__MODEL=convaiinnovations/laya-multilingual \
//! CAMPBOOKS_AI__DECIDE__API_KEY=dev-key \
//!   cargo test -p campbooks-ai --test decide_live -- --ignored --nocapture
//! ```
//!
//! Any `decide` configuration works (an instruct model too). The test checks
//! the shape of the answers and prints them; whether they are *right* is what
//! the per-question evals are for.

use campbooks_ai::{
    AiConfig, Gateway,
    decide::{DecisionContext, DecisionState, Question},
};

const EMAILS: [(&str, &str); 2] = [
    (
        "en",
        "From: Ana Silva <ana@example.com>\nSubject: October invoice\n\nHi, attached is invoice \
         no. 2026-118 for EUR 450.00, due on 31 October. Could you confirm you received it and \
         tell me when you expect to pay?",
    ),
    (
        "pt-PT",
        "From: Ana Silva <ana@example.com>\nSubject: Fatura de outubro\n\nOlá, segue em anexo a \
         fatura n.º 2026-118 de 450,00 EUR, com vencimento a 31 de outubro. Pode confirmar a \
         receção e indicar a data prevista de pagamento?",
    ),
];

#[tokio::test]
#[ignore = "needs a running decide provider; see the module docs"]
async fn three_questions_about_a_sample_email() {
    let gateway = Gateway::new(AiConfig::from_env().expect("decide config")).expect("gateway");
    let questions = [
        Question::choice(
            "category",
            "What kind of email is this?",
            [
                ("invoice", "A bill or invoice asking for payment"),
                ("newsletter", "Marketing or a newsletter"),
                ("personal", "A personal message from a friend or family"),
                ("meeting", "A meeting invitation or scheduling"),
            ],
        ),
        Question::score(
            "urgency",
            "How soon does this email need a reply?",
            ["no reply needed", "reply this week", "reply today"],
        ),
        Question::noul(
            "asks_user",
            "The sender asks the recipient to do something or to reply.",
        ),
    ];

    for (lang, email) in EMAILS {
        let answers = gateway
            .decide(
                &DecisionContext::new("test.live"),
                &DecisionState::from(email),
                &questions,
            )
            .await
            .expect("decide");
        assert_eq!(answers.len(), questions.len());
        for answer in &answers {
            if let Some(p) = &answer.probabilities {
                let total: f64 = p.iter().sum();
                assert!((total - 1.0).abs() < 1e-6, "{p:?}");
            }
            println!(
                "[{lang}] {:<9} {:?}  p={:?}  confidence={:?}  model={} latency={:?}",
                answer.question_id,
                answer.value,
                answer.probabilities,
                answer.confidence,
                answer.model,
                answer.latency,
            );
        }
    }
}

/// The answers check (`check::cases`) against the configured `decide`
/// role, exactly as the server runs it. With Jev through OpenRouter:
///
/// ```sh
/// CAMPBOOKS_AI__DECIDE__PROTOCOL=laya \
/// CAMPBOOKS_AI__DECIDE__BASE_URL=https://openrouter.ai/api/alpha \
/// CAMPBOOKS_AI__DECIDE__PATH=decisions \
/// CAMPBOOKS_AI__DECIDE__MODEL=typesafe/jev-1.13 \
/// CAMPBOOKS_AI__DECIDE__API_KEY=… CAMPBOOKS_AI__DECIDE__EXTERNAL_CONTENT=allow \
///   cargo test -p campbooks-ai --test decide_live answers_check -- --ignored --nocapture
/// ```
#[tokio::test]
#[ignore = "needs a running decide provider; see the module docs"]
async fn answers_check_against_the_configured_model() {
    let gateway = Gateway::new(AiConfig::from_env().expect("decide config")).expect("gateway");
    let report = gateway.run_answers_check().await;
    println!("{report:?}");
    assert!(report.passed(), "{report:?}");
}
