//! The model pass under the daily AI limit (campbooks-cloud#152): counted
//! against the message's workspace, rules only over the limit, nothing
//! re-queued, and no call when it cannot be counted. The meter here is in
//! memory; the ledger is the `system` module's (its own tests, and the
//! server's end-to-end test).

use std::sync::{Arc, Mutex};

use campbooks_ai::{
    AiConfig, Gateway, Protocol, ProviderConfig, Role,
    decide::{
        check::{CheckOutcome, CheckReport},
        strategy::BoxFuture,
    },
    usage::{Admission, Meter, MeterError, MeteredCall, Outcome},
};
use campbooks_kernel::tenancy::WorkspaceId;
use chrono::NaiveDate;
use serde_json::json;
use sqlx::PgPool;
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method};

use crate::{
    ConversationsConfig, ConversationsDeps,
    testkit::{Kit, sorting_deps},
};

/// Admits `cap` calls a day, or fails to count any.
#[derive(Debug, Default)]
struct Ledger {
    cap: u32,
    broken: Mutex<bool>,
    admitted: Mutex<u32>,
    calls: Mutex<Vec<(WorkspaceId, Role, String)>>,
}

impl Ledger {
    /// The next day: the count starts again.
    fn new_day(&self) {
        *self.admitted.lock().unwrap() = 0;
    }
}

impl Meter for Ledger {
    fn admit<'a>(
        &'a self,
        _db: &'a PgPool,
        call: MeteredCall<'a>,
    ) -> BoxFuture<'a, Result<Admission, MeterError>> {
        Box::pin(async move {
            if *self.broken.lock().unwrap() {
                return Err("the ledger is down".into());
            }
            self.calls
                .lock()
                .unwrap()
                .push((call.workspace, call.role, call.feature.to_owned()));
            let mut admitted = self.admitted.lock().unwrap();
            if *admitted >= self.cap {
                return Ok(Admission::Refused { cap: self.cap });
            }
            *admitted += 1;
            Ok(Admission::Admitted {
                day: NaiveDate::from_ymd_opt(2026, 10, 16).unwrap(),
            })
        })
    }

    fn settle<'a>(
        &'a self,
        _db: &'a PgPool,
        _call: MeteredCall<'a>,
        _day: NaiveDate,
        _outcome: Outcome,
    ) -> BoxFuture<'a, Result<(), MeterError>> {
        Box::pin(async { Ok(()) })
    }
}

/// laya-serve on `server`, its answers check passed, metered by `ledger`.
fn metered_gateway(server: &MockServer, ledger: Arc<Ledger>) -> Gateway {
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
    .with_meter(ledger);
    gateway.record_answers_check(CheckReport {
        outcome: CheckOutcome::Passed,
        right: 33,
        total: 33,
        model: Some("laya-test".into()),
        checked_at: chrono::Utc::now(),
    });
    gateway
}

async fn laya(expected_calls: u64) -> MockServer {
    let noul = json!({"type": "noul", "noul": 0.1});
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": "laya-test",
            "answers": {
                "stream": {"type": "choice", "choice": "primary",
                    "probabilities": {"primary": 0.97, "updates": 0.01, "receipts": 0.01,
                                      "newsletters": 0.01}},
                "needs_reply": noul, "asks_action": noul, "has_deadline": noul,
                "urgency": {"type": "score", "score": 0.2,
                    "probabilities": {"0": 0.8, "1": 0.2, "2": 0.0}},
                "suspicious": noul,
                "money": {"type": "choice", "choice": "neither", "probabilities": {"owed": 0.1, "paid": 0.1, "neither": 0.8}},
                "has_dated_thing": noul
            }
        })))
        .expect(expected_calls)
        .mount(&server)
        .await;
    server
}

async fn kit(db: PgPool, server: &MockServer, ledger: Arc<Ledger>) -> Kit {
    let deps = ConversationsDeps {
        ai: Some(metered_gateway(server, ledger)),
        config: ConversationsConfig {
            decide_backfill_days: 100_000,
            ..ConversationsConfig::default()
        },
        // The pass asks the metered `decide` role, not the kit's model.
        model: None,
        ..sorting_deps()
    };
    Kit::with_deps(db, deps).await
}

/// Messages the model answered, and every stored inbound message.
async fn answered(db: &PgPool) -> (Vec<String>, Vec<String>) {
    let model = sqlx::query_scalar(
        "SELECT DISTINCT m.provider_message_id FROM conversations.decisions d
           JOIN conversations.messages m ON m.id = d.message_id
          WHERE d.source = 'model' ORDER BY 1",
    )
    .fetch_all(db)
    .await
    .unwrap();
    let inbound = sqlx::query_scalar(
        "SELECT provider_message_id FROM conversations.messages
          WHERE direction = 'inbound' ORDER BY 1",
    )
    .fetch_all(db)
    .await
    .unwrap();
    (model, inbound)
}

/// Model passes still waiting to run: deliveries of `decide_message` and
/// `decide_backlog` jobs. Only those: other work may legitimately wait (the
/// change feed's `kernel.sequence_changes` leaves a short-delayed successor
/// when producers are in flight).
async fn waiting_model_passes(db: &PgPool) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM kernel.jobs
          WHERE state IN ('pending', 'running')
            AND kind IN ('conversations.decide_message', 'conversations.decide_backlog')",
    )
    .fetch_one(db)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn over_the_daily_limit_the_rest_gets_no_model_pass(db: PgPool) {
    // Three inbound messages, a limit of two calls a day.
    let server = laya(2).await;
    let ledger = Arc::new(Ledger {
        cap: 2,
        ..Ledger::default()
    });
    let kit = kit(db.clone(), &server, ledger.clone()).await;
    let workspace = WorkspaceId::new();
    let inbox = kit.inbox(workspace, "sam@example.com", &[]).await;
    let stats = kit.ingest(workspace, inbox, "en").await;
    assert_eq!((stats.failed, stats.retried), (0, 0), "{stats:?}");

    let (model, inbound) = answered(&db).await;
    assert_eq!(model.len(), 2, "the first two got the model: {model:?}");
    assert_eq!(inbound, ["en-1", "en-3", "en-4"], "every one was stored");
    let skipped = inbound.iter().find(|m| !model.contains(m)).unwrap();
    // Left as it was stored, as with no model at all.
    assert_ne!(kit.stream_of(skipped).await.1, "model");
    // Every call was the workspace's, for triage.
    assert_eq!(
        ledger.calls.lock().unwrap().clone(),
        vec![(workspace, Role::Decide, "conversations.triage".to_owned()); 3]
    );

    // Nothing waits for the next day: no burst when it starts.
    assert_eq!(waiting_model_passes(&db).await, 0);
    ledger.new_day();
    kit.run_jobs().await;
    assert_eq!(answered(&db).await.0.len(), 2);
    assert_eq!(ledger.calls.lock().unwrap().len(), 3, "nothing asked again");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_call_that_cannot_be_counted_is_not_made_and_asked_again(db: PgPool) {
    let server = laya(3).await;
    let ledger = Arc::new(Ledger {
        cap: 10,
        broken: Mutex::new(true),
        ..Ledger::default()
    });
    let kit = kit(db.clone(), &server, ledger.clone()).await;
    let workspace = WorkspaceId::new();
    let inbox = kit.inbox(workspace, "sam@example.com", &[]).await;
    let stats = kit.ingest(workspace, inbox, "en").await;
    assert_eq!((stats.failed, stats.retried), (0, 3), "{stats:?}");
    // Stored, nothing sent to the model.
    let (model, inbound) = answered(&db).await;
    assert!(model.is_empty());
    assert_eq!(inbound.len(), 3);
    assert!(server.received_requests().await.unwrap().is_empty());

    // Once the ledger is back, the retries ask.
    *ledger.broken.lock().unwrap() = false;
    sqlx::query("UPDATE kernel.jobs SET run_at = now() WHERE state = 'pending'")
        .execute(&db)
        .await
        .unwrap();
    kit.run_jobs().await;
    assert_eq!(answered(&db).await.0.len(), 3);
}
