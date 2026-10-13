//! `conversations.decide_message` — the model pass over each new inbound
//! message: the triage questions in one `decide` call
//! (`campbooks_ai::decide::strategy::Model`), every answer stored in
//! `decisions` with the policy applied (no model answer acts under the
//! baseline policy; campbooks-cloud#59).
//!
//! Skipped when the `decide` role is not configured, for messages sent
//! before the backfill window (`CAMPBOOKS_CONVERSATIONS__DECIDE_BACKFILL_DAYS`),
//! and for messages already answered (a replayed delivery).

use std::sync::Arc;

use campbooks_ai::decide::{
    signals::email::{Envelope, triage_questions},
    strategy::Strategy,
};
use campbooks_kernel::{
    Event, EventConsumer, JobContext, JobError, blobs::BlobRef, db::BeginFor as _,
    tenancy::WorkspaceId,
};
use chrono::{TimeDelta, Utc};

use crate::{
    domain::{
        blobs::read_text,
        decide::{ENVELOPE_BODY_BYTES, MessageParts, Source, envelope},
        decisions::Recorded,
        policy::Policy,
        redact,
    },
    events::MessageReceived,
    ingest::Headers,
    model::{Address, ConversationId, MessageId},
};

/// The model pass, composed with the module's model strategy (none when the
/// `decide` role is not configured), backfill window and policy.
pub struct DecideMessage {
    model: Option<Arc<dyn Strategy>>,
    backfill: TimeDelta,
    policy: Policy,
}

impl DecideMessage {
    /// A pass asking `model`, for messages sent within `backfill_days`,
    /// marking the answers `policy` accepts.
    pub fn new(model: Option<Arc<dyn Strategy>>, backfill_days: u32, policy: Policy) -> Self {
        Self {
            model,
            backfill: TimeDelta::days(i64::from(backfill_days)),
            policy,
        }
    }
}

impl EventConsumer for DecideMessage {
    type Event = MessageReceived;
    const NAME: &'static str = "conversations.decide_message";
    const QUEUE: &'static str = "ai";
    const MAX_ATTEMPTS: u32 = 5;

    async fn handle(
        &self,
        ctx: &mut JobContext,
        event: Event<MessageReceived>,
    ) -> Result<(), JobError> {
        let Some(model) = &self.model else {
            return Ok(());
        };
        let facts = event.payload.message;
        if facts.sent_at < Utc::now() - self.backfill {
            return Ok(());
        }
        let workspace: WorkspaceId = facts
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("message_received names an invalid workspace"))?;
        let Some((conversation, envelope)) = load(ctx, workspace, facts.message).await? else {
            return Ok(());
        };

        let questions = triage_questions();
        let answers = match model.decide(&envelope, &questions).await {
            Ok(decisions) => decisions.answers,
            Err(error) => return Err(redact::ai(&error)),
        };
        let recorded = Recorded {
            message: facts.message,
            conversation,
            source: Source::Model,
            strategy: model.name(),
            questions: &questions,
            answers: &answers,
            policy: &self.policy,
        };
        let tx = ctx.tx().await?;
        // The model pass never re-sorts: the stream was set when the message
        // was stored.
        recorded.store(tx, workspace, false).await?;
        Ok(())
    }
}

/// The message's conversation and envelope, or `None` when there is nothing
/// to do (gone, outbound, or already answered by the model).
async fn load(
    ctx: &JobContext,
    workspace: WorkspaceId,
    message: MessageId,
) -> Result<Option<(ConversationId, Envelope)>, JobError> {
    let state = ctx.state();
    let mut tx = state.db.begin_for(workspace).await?;
    let row = sqlx::query!(
        r#"
        SELECT m.conversation_id AS "conversation: ConversationId", m.direction,
               m.sender, m.recipients, m.subject, m.headers, m.bodies,
               EXISTS (SELECT 1 FROM conversations.decisions d
                        WHERE d.message_id = m.id AND d.source = 'model') AS "answered!",
               ARRAY(SELECT a.filename FROM conversations.attachments a
                      WHERE a.message_id = m.id ORDER BY a.position) AS "filenames!",
               ARRAY(SELECT a.content_type FROM conversations.attachments a
                      WHERE a.message_id = m.id ORDER BY a.position) AS "content_types!"
          FROM conversations.messages m
         WHERE m.id = $1
        "#,
        message as MessageId,
    )
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    let Some(row) = row else { return Ok(None) };
    if row.answered || row.direction != "inbound" {
        return Ok(None);
    }
    let decode =
        |e: serde_json::Error| JobError::fatal(format!("stored message: {}", redact::json(&e)));
    let from: Address = serde_json::from_value(row.sender).map_err(decode)?;
    let recipients: Recipients = serde_json::from_value(row.recipients).map_err(decode)?;
    let headers: Headers = serde_json::from_value(row.headers).map_err(decode)?;
    let bodies: Bodies = serde_json::from_value(row.bodies).map_err(decode)?;
    let body = match &bodies.text {
        Some(text) => {
            read_text(
                &state.blobs,
                &workspace.to_string(),
                text,
                ENVELOPE_BODY_BYTES,
            )
            .await?
            .0
        }
        None => String::new(),
    };
    let attachments: Vec<(String, String)> =
        row.filenames.into_iter().zip(row.content_types).collect();
    let envelope = envelope(MessageParts {
        from: &from,
        to: &recipients.to,
        cc: &recipients.cc,
        reply_to: &recipients.reply_to,
        subject: &row.subject,
        headers: &headers,
        attachments: &attachments,
        body: &body,
    });
    Ok(Some((row.conversation, envelope)))
}

/// `messages.recipients`.
#[derive(serde::Deserialize)]
struct Recipients {
    #[serde(default)]
    to: Vec<Address>,
    #[serde(default)]
    cc: Vec<Address>,
    #[serde(default)]
    reply_to: Vec<Address>,
}

/// `messages.bodies`.
#[derive(serde::Deserialize)]
struct Bodies {
    #[serde(default)]
    text: Option<BlobRef>,
}

#[cfg(test)]
mod tests {
    use campbooks_ai::{AiConfig, Gateway, Protocol, ProviderConfig, Role};
    use campbooks_kernel::tenancy::WorkspaceId;
    use serde_json::{Value, json};
    use sqlx::PgPool;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
    };

    use crate::{
        ConversationsConfig, ConversationsDeps,
        testkit::{Kit, sorting_deps},
    };

    /// A laya-serve answering every question the same way: the newsletter
    /// is "primary", confidently (zero-shot answers can be wrong).
    fn laya_answers() -> Value {
        let noul = |p: f64| json!({"type": "noul", "noul": p, "confidence": p});
        json!({
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
                "suspicious": noul(0.05)
            }
        })
    }

    async fn kit_with_model(db: PgPool, server: &MockServer, backfill_days: u32) -> Kit {
        let provider = ProviderConfig::new(
            format!("{}/v1", server.uri()).parse().unwrap(),
            "convaiinnovations/laya-multilingual",
        )
        .with_protocol(Protocol::Laya);
        let gateway = Gateway::new(AiConfig {
            roles: [(Role::Decide, provider)].into(),
        })
        .unwrap();
        let deps = ConversationsDeps {
            ai: Some(gateway),
            config: ConversationsConfig {
                decide_backfill_days: backfill_days,
            },
            ..sorting_deps()
        };
        Kit::with_deps(db, deps).await
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn model_answers_are_recorded_as_suggestions_only(db: PgPool) {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/systemone"))
            .respond_with(ResponseTemplate::new(200).set_body_json(laya_answers()))
            .expect(3)
            .mount(&server)
            .await;
        let kit = kit_with_model(db.clone(), &server, 100_000).await;
        let workspace = WorkspaceId::new();
        let inbox = kit.inbox(workspace, "sam@example.com", &[]).await;
        kit.ingest(workspace, inbox, "en").await;

        // One call per inbound message, none for the outbound one.
        let rows: Vec<(String, String, Value, bool, i16)> = sqlx::query_as(
            "SELECT m.provider_message_id, d.question, d.value, d.acted, d.state_version
               FROM conversations.decisions d
               JOIN conversations.messages m ON m.id = d.message_id
              WHERE d.source = 'model' ORDER BY 1, 2",
        )
        .fetch_all(&db)
        .await
        .unwrap();
        assert_eq!(rows.len(), 3 * 6);
        assert!(
            rows.iter()
                .all(|(.., acted, version)| !acted && *version == 1)
        );
        let stream = rows
            .iter()
            .find(|(m, q, ..)| m == "en-3" && q == "stream")
            .unwrap();
        assert_eq!(stream.2, json!("primary"));
        // The model's confident "primary" changed nothing: the rules placed
        // the newsletter.
        assert_eq!(
            kit.stream_of("en-3").await,
            ("newsletters".to_owned(), "rules".to_owned())
        );
        let (probabilities, model): (Value, String) = sqlx::query_as(
            "SELECT d.probabilities, d.model FROM conversations.decisions d
              WHERE d.source = 'model' AND d.question = 'urgency' LIMIT 1",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(probabilities.as_array().unwrap().len(), 3);
        assert_eq!(model, "laya-test");

        // A replayed delivery asks nothing again.
        let replayed = kit
            .state
            .events
            .replay(
                &kit.state.db,
                "conversations.decide_message",
                campbooks_kernel::events::ReplayFrom::Beginning,
            )
            .await
            .unwrap();
        assert_eq!(replayed, 3);
        kit.run_jobs().await;
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn mail_older_than_the_backfill_window_gets_no_model_pass(db: PgPool) {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(laya_answers()))
            .expect(0)
            .mount(&server)
            .await;
        // The fixtures were sent in early October 2026.
        let kit = kit_with_model(db.clone(), &server, 1).await;
        let workspace = WorkspaceId::new();
        let inbox = kit.inbox(workspace, "sam@example.com", &[]).await;
        kit.ingest(workspace, inbox, "en").await;
        let model: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM conversations.decisions WHERE source = 'model'",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(model, 0);
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn a_provider_failure_retries_without_echoing_the_body(db: PgPool) {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(400).set_body_string("Subject: Q4 plan"))
            .mount(&server)
            .await;
        let kit = kit_with_model(db.clone(), &server, 100_000).await;
        let workspace = WorkspaceId::new();
        let inbox = kit.inbox(workspace, "sam@example.com", &[]).await;
        let batch = kit.batch(workspace, inbox, "en").await;
        crate::dev::submit(&kit.state, workspace, &batch)
            .await
            .unwrap();
        campbooks_kernel::jobs::Worker::new(kit.state.clone())
            .run_until_idle()
            .await;
        let errors: Vec<String> = sqlx::query_scalar(
            "SELECT last_error FROM kernel.jobs
              WHERE kind = 'conversations.decide_message' AND last_error IS NOT NULL",
        )
        .fetch_all(&db)
        .await
        .unwrap();
        assert!(!errors.is_empty());
        assert!(
            errors.iter().all(|e| e == "decide provider answered 400"),
            "{errors:?}"
        );
    }
}
