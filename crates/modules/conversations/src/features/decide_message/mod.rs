//! `conversations.decide_message` — the model pass over each new inbound
//! message: the triage questions in one `decide` call
//! (`campbooks_ai::decide::strategy::Model`), every answer stored in
//! `decisions` with the policy applied (no model answer acts under the
//! baseline policy; campbooks-cloud#59).
//!
//! Skipped when the `decide` role is not configured or has not passed its
//! answers check (`campbooks_ai::decide::check`), for messages sent before
//! the backfill window (`CAMPBOOKS_CONVERSATIONS__DECIDE_BACKFILL_DAYS`),
//! and for messages already answered (a replayed delivery). The model reads
//! whose inbox the message is in: the inbox's address and name, and whether
//! several members read it.

use std::sync::Arc;

use campbooks_ai::decide::{
    signals::email::{Envelope, Recipient, triage_questions},
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
        needs::{self, NeedSettings},
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
    needs: NeedSettings,
}

impl DecideMessage {
    /// A pass asking `model`, for messages sent within `backfill_days`,
    /// marking the answers `policy` accepts and raising needs as `needs`
    /// says.
    pub fn new(
        model: Option<Arc<dyn Strategy>>,
        backfill_days: u32,
        policy: Policy,
        needs: NeedSettings,
    ) -> Self {
        Self {
            model,
            backfill: TimeDelta::days(i64::from(backfill_days)),
            policy,
            needs,
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
            // The model failed its answers check: the pass does what it
            // does when no model is configured, and stores nothing.
            Err(campbooks_ai::Error::AnswersNotTrusted) => return Ok(()),
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
        let ctx_events = ctx.state().events.clone();
        let tx = ctx.tx().await?;
        // The model pass never re-sorts: the stream was set when the message
        // was stored.
        recorded.store(tx, workspace, false).await?;
        // The model's answers may put a question to the member.
        needs::refresh(tx, &ctx_events, &facts.workspace, conversation, &self.needs).await?;
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
               i.address AS inbox_address, i.display_name AS inbox_name,
               (SELECT count(*) FROM conversations.inbox_members im
                 WHERE im.inbox_id = i.id) AS "inbox_members!",
               ARRAY(SELECT a.filename FROM conversations.attachments a
                      WHERE a.message_id = m.id ORDER BY a.position) AS "filenames!",
               ARRAY(SELECT a.content_type FROM conversations.attachments a
                      WHERE a.message_id = m.id ORDER BY a.position) AS "content_types!"
          FROM conversations.messages m
          JOIN conversations.inboxes i ON i.id = m.inbox_id
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
    let mut envelope = envelope(MessageParts {
        from: &from,
        to: &recipients.to,
        cc: &recipients.cc,
        reply_to: &recipients.reply_to,
        subject: &row.subject,
        headers: &headers,
        attachments: &attachments,
        body: &body,
    });
    envelope.recipient = Some(Recipient {
        name: row.inbox_name.unwrap_or_default(),
        addresses: vec![row.inbox_address],
        shared: row.inbox_members > 1,
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
    use campbooks_ai::{
        AiConfig, Gateway, Protocol, ProviderConfig, Role,
        decide::check::{CheckOutcome, CheckReport},
    };
    use campbooks_kernel::{db::BeginFor as _, tenancy::WorkspaceId};
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

    /// A finished answers check with `outcome`.
    fn check(outcome: CheckOutcome) -> CheckReport {
        CheckReport {
            outcome,
            right: if outcome == CheckOutcome::Passed {
                33
            } else {
                8
            },
            total: 33,
            model: Some("laya-test".into()),
            checked_at: chrono::Utc::now(),
        }
    }

    async fn kit_with_model(db: PgPool, server: &MockServer, backfill_days: u32) -> Kit {
        kit_with_checked_model(db, server, backfill_days, CheckOutcome::Passed).await
    }

    /// A Laya gateway on `server` whose answers check ended in `outcome`.
    fn checked_gateway(server: &MockServer, outcome: CheckOutcome) -> Gateway {
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
        gateway.record_answers_check(check(outcome));
        gateway
    }

    async fn kit_with_checked_model(
        db: PgPool,
        server: &MockServer,
        backfill_days: u32,
        outcome: CheckOutcome,
    ) -> Kit {
        let deps = ConversationsDeps {
            ai: Some(checked_gateway(server, outcome)),
            config: ConversationsConfig {
                decide_backfill_days: backfill_days,
                ..ConversationsConfig::default()
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
        let rows: Vec<(String, String, Value, bool, i16, Option<String>)> = sqlx::query_as(
            "SELECT m.provider_message_id, d.question, d.value, d.acted, d.state_version,
                    d.model_version
               FROM conversations.decisions d
               JOIN conversations.messages m ON m.id = d.message_id
              WHERE d.source = 'model' ORDER BY 1, 2",
        )
        .fetch_all(&db)
        .await
        .unwrap();
        assert_eq!(rows.len(), 3 * 6);
        // Recorded with the envelope version (context 1) and the stamp of
        // the passed answers check.
        assert!(rows.iter().all(|(.., acted, version, model_version)| !acted
            && *version == crate::STATE_VERSION
            && *version == 2
            && model_version.as_deref() == Some("checked:ctx1")));
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

    /// The model as it answered a code host's notification in production:
    /// it asks for an action (0.875) with a deadline (0.976), no reply.
    fn notification_answers() -> Value {
        let noul = |v: f64, c: f64| json!({"type": "noul", "noul": v, "confidence": c});
        json!({
            "model": "laya-test",
            "answers": {
                "stream": {"type": "choice", "choice": "updates",
                    "probabilities": {"primary": 0.2, "updates": 0.7, "receipts": 0.05,
                                      "newsletters": 0.05}},
                "needs_reply": noul(0.1, 0.9),
                "asks_action": noul(0.875, 0.875),
                "has_deadline": noul(0.976, 0.976),
                "urgency": {"type": "score", "score": 1.0,
                    "probabilities": {"0": 0.3, "1": 0.5, "2": 0.2}},
                "suspicious": noul(0.02, 0.98)
            }
        })
    }

    /// Sam's inbox holding `fixture`, read by the rules and the model above.
    async fn read_by_both(db: &PgPool, fixture: &str) -> (Kit, WorkspaceId) {
        let recorded = serde_json::from_str(&crate::testkit::fixture(fixture)).unwrap();
        read_recorded(db, &recorded).await
    }

    /// Sam's inbox holding the recorded batch `recorded`, read by the rules
    /// and the model above.
    async fn read_recorded(db: &PgPool, recorded: &Value) -> (Kit, WorkspaceId) {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/systemone"))
            .respond_with(ResponseTemplate::new(200).set_body_json(notification_answers()))
            .expect(1)
            .mount(&server)
            .await;
        let kit = kit_with_model(db.clone(), &server, 100_000).await;
        let workspace = WorkspaceId::new();
        let inbox = kit.inbox(workspace, "sam@example.com", &[]).await;
        let batch = crate::dev::record(&kit.state, workspace, inbox, recorded)
            .await
            .unwrap();
        crate::dev::submit(&kit.state, workspace, &batch)
            .await
            .unwrap();
        kit.run_jobs().await;
        let model: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM conversations.decisions WHERE source = 'model'",
        )
        .fetch_one(db)
        .await
        .unwrap();
        assert_eq!(model, 6, "the model pass ran");
        (kit, workspace)
    }

    async fn needs(db: &PgPool) -> Vec<(String, String, String, String)> {
        sqlx::query_as(
            "SELECT reason, trust, basis, state FROM conversations.needs ORDER BY updated_at",
        )
        .fetch_all(db)
        .await
        .unwrap()
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn a_security_alert_is_a_sure_do_in_updates(db: PgPool) {
        let (kit, _) = read_by_both(&db, "dependabot").await;
        assert_eq!(
            kit.stream_of("dependabot-1").await,
            ("updates".to_owned(), "rules".to_owned())
        );
        assert_eq!(
            needs(&db).await,
            [(
                "do".to_owned(),
                "sure".to_owned(),
                "rules".to_owned(),
                "open".to_owned()
            )]
        );
        // The rules' "no date" acted: the model's deadline is no due date.
        let due: Option<chrono::NaiveDate> =
            sqlx::query_scalar("SELECT due_on FROM conversations.needs")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(due, None);
    }

    /// The Dependabot fixture as a sender could make it look: `edit`ed.
    fn dependabot_as(edit: impl Fn(&mut Value)) -> Value {
        let mut recorded: Value =
            serde_json::from_str(&crate::testkit::fixture("dependabot")).unwrap();
        edit(&mut recorded["changes"][0]["message"]);
        recorded
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn an_unauthenticated_alert_is_no_to_do(db: PgPool) {
        // The same alert, its From spoofed through a provider that does not
        // check (no results), or failing DMARC.
        for results in [
            Value::Null,
            json!(
                "mx.google.com; dkim=fail header.i=@github.com; dmarc=fail header.from=github.com"
            ),
        ] {
            let recorded =
                dependabot_as(|m| m["headers"]["authentication_results"] = results.clone());
            let (_, _) = read_recorded(&db, &recorded).await;
            let raised = needs(&db).await;
            assert!(
                raised.iter().all(|(reason, ..)| reason != "do"),
                "{raised:?}"
            );
            sqlx::query("DELETE FROM conversations.inboxes")
                .execute(&db)
                .await
                .unwrap();
        }
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn a_look_alike_platform_is_no_to_do(db: PgPool) {
        // It signs its own mail and names its own domain; it is not a listed
        // platform, so its alert never acts.
        let recorded = dependabot_as(|m| {
            m["from"] =
                json!({"name": "GitHub Security", "address": "alerts@githubsecurity.example"});
            m["reply_to"] = json!([]);
            m["headers"]["authentication_results"] = json!(
                "mx.google.com; dkim=pass header.i=@githubsecurity.example; \
                 dmarc=pass header.from=githubsecurity.example"
            );
        });
        let (_, _) = read_recorded(&db, &recorded).await;
        let raised = needs(&db).await;
        assert!(
            raised
                .iter()
                .all(|(reason, trust, ..)| reason != "do" && trust != "sure"),
            "{raised:?}"
        );
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn a_notification_the_rules_settled_is_no_question(db: PgPool) {
        let (_, _) = read_by_both(&db, "github-comment").await;
        assert_eq!(needs(&db).await, []);
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn a_question_raised_before_the_fix_is_withdrawn(db: PgPool) {
        let (kit, workspace) = read_by_both(&db, "github-comment").await;
        let conversation = kit.conversation_of("github-comment-1").await;
        let message = kit.message_of("github-comment-1").await;
        // What the earlier `raise` stored: the model's suggestion put as a
        // question although the rules had settled it.
        sqlx::query(
            "INSERT INTO conversations.needs
                 (conversation_id, workspace_id, message_id, reason, trust, basis, question,
                  state, latest_inbound_id)
             VALUES ($1::uuid, $2, $3::uuid, 'review', 'check', 'model', 'asks_action', 'open',
                     $3::uuid)",
        )
        .bind(&conversation)
        .bind(workspace.as_uuid())
        .bind(&message)
        .execute(&db)
        .await
        .unwrap();

        let refresh = |kit: &Kit| {
            let state = kit.state.clone();
            let conversation = conversation.clone();
            async move {
                let mut tx = state.db.begin_for(workspace).await.unwrap();
                crate::domain::needs::refresh(
                    &mut tx,
                    &state.events,
                    &workspace.to_string(),
                    conversation.parse().unwrap(),
                    &crate::domain::needs::NeedSettings::new(
                        36_500,
                        &crate::domain::policy::Policy::baseline(),
                    ),
                )
                .await
                .unwrap();
                tx.commit().await.unwrap();
            }
        };
        refresh(&kit).await;
        let (state, cause): (String, Option<String>) =
            sqlx::query_as("SELECT state, cleared_because FROM conversations.needs")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(
            (state.as_str(), cause.as_deref()),
            ("cleared", Some("superseded"))
        );
        let events = kit.events("conversations.need_changed").await;
        assert_eq!(events.len(), 1, "{events:?}");
        assert_eq!(events[0]["need"], Value::Null);
        assert_eq!(events[0]["cleared_because"], "superseded");

        // Again: nothing more to say.
        refresh(&kit).await;
        assert_eq!(kit.events("conversations.need_changed").await.len(), 1);
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

    #[sqlx::test(migrations = "../../../migrations")]
    async fn an_off_network_decide_role_without_opt_in_leaves_sorting_to_rules(db: PgPool) {
        // `decide.example.com` is off the instance's network: no mail goes
        // there (nothing could answer anyway), rules alone sort.
        let provider = ProviderConfig::new(
            "https://decide.example.com/v1".parse().unwrap(),
            "convaiinnovations/laya-multilingual",
        )
        .with_protocol(Protocol::Laya);
        let gateway = Gateway::new(AiConfig {
            roles: [(Role::Decide, provider)].into(),
            ..AiConfig::default()
        })
        .unwrap();
        let deps = ConversationsDeps {
            ai: Some(gateway),
            ..sorting_deps()
        };
        let kit = Kit::with_deps(db.clone(), deps).await;
        let workspace = WorkspaceId::new();
        let inbox = kit.inbox(workspace, "sam@example.com", &[]).await;
        kit.ingest(workspace, inbox, "en").await;

        let by_source: Vec<(String, i64)> = sqlx::query_as(
            "SELECT source, count(*) FROM conversations.decisions GROUP BY 1 ORDER BY 1",
        )
        .fetch_all(&db)
        .await
        .unwrap();
        assert!(
            by_source.iter().all(|(source, _)| source == "rules"),
            "{by_source:?}"
        );
        assert!(!by_source.is_empty(), "rules still answered");
        let failed: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM kernel.jobs WHERE state = 'failed' OR last_error IS NOT NULL",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(failed, 0);
    }

    /// What a workspace's sorting came to, by fixture message: every
    /// decision, the conversation's stream and its need.
    async fn sorting(db: &PgPool, workspace: WorkspaceId) -> Vec<String> {
        sqlx::query_scalar(
            "SELECT concat_ws(' | ', m.provider_message_id, c.stream, c.stream_source,
                    (SELECT string_agg(concat_ws(':', d.question, d.source, d.value::text, d.acted),
                                       ',' ORDER BY d.question, d.source)
                       FROM conversations.decisions d WHERE d.message_id = m.id),
                    (SELECT concat_ws(':', n.reason, n.trust, n.basis, n.state)
                       FROM conversations.needs n WHERE n.conversation_id = c.id))
               FROM conversations.messages m
               JOIN conversations.conversations c ON c.id = m.conversation_id
              WHERE m.workspace_id::text = $1 ORDER BY m.provider_message_id",
        )
        .bind(workspace.to_string())
        .fetch_all(db)
        .await
        .unwrap()
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn a_model_that_failed_its_answers_check_is_as_if_absent(db: PgPool) {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(laya_answers()))
            .expect(0)
            .mount(&server)
            .await;
        let failing =
            kit_with_checked_model(db.clone(), &server, 100_000, CheckOutcome::Failed).await;
        let with_failing = WorkspaceId::new();
        let inbox = failing.inbox(with_failing, "sam@example.com", &[]).await;
        failing.ingest(with_failing, inbox, "en").await;

        let none = Kit::with_deps(db.clone(), sorting_deps()).await;
        let without = WorkspaceId::new();
        let inbox = none.inbox(without, "sam@example.com", &[]).await;
        none.ingest(without, inbox, "en").await;

        let failing_sorted = sorting(&db, with_failing).await;
        assert!(!failing_sorted.is_empty());
        assert_eq!(failing_sorted, sorting(&db, without).await);
        let failed: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM kernel.jobs WHERE state = 'failed' OR last_error IS NOT NULL",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(failed, 0, "skipped, not failed");
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn model_answers_no_check_vouched_for_are_kept_but_not_read(db: PgPool) {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/systemone"))
            .respond_with(ResponseTemplate::new(200).set_body_json(laya_answers()))
            .mount(&server)
            .await;
        // Rules that only record a reply or an action leave the model's
        // "yes" to be put as a question.
        let deps = ConversationsDeps {
            ai: Some(checked_gateway(&server, CheckOutcome::Passed)),
            config: ConversationsConfig {
                decide_backfill_days: 100_000,
                ..ConversationsConfig::default()
            },
            ..sorting_deps()
        };
        let kit = Kit::with_deps(
            db.clone(),
            deps.with_policy(
                crate::Policy::baseline()
                    .with(
                        campbooks_ai::decide::signals::email::triage::STREAM,
                        crate::domain::decide::Source::Rules,
                        crate::Switch::ActAbove(0.9),
                    )
                    .with(
                        campbooks_ai::decide::signals::email::triage::NEEDS_REPLY,
                        crate::domain::decide::Source::Rules,
                        crate::Switch::Record,
                    )
                    .with(
                        campbooks_ai::decide::signals::email::triage::ASKS_ACTION,
                        crate::domain::decide::Source::Rules,
                        crate::Switch::Record,
                    ),
            ),
        )
        .await;
        let workspace = WorkspaceId::new();
        let (token, sam) = kit.member("sam", workspace);
        let inbox = kit
            .inbox(
                workspace,
                "sam@example.com",
                &[(sam, crate::model::InboxRole::Manager)],
            )
            .await;
        kit.ingest(workspace, inbox, "en").await;

        // The model read whose inbox it was.
        let requests = server.received_requests().await.unwrap();
        let body: Value = serde_json::from_slice(&requests[0].body).unwrap();
        assert!(
            body["state"]
                .as_str()
                .unwrap()
                .contains("The recipient is sam@example.com, who reads this personal inbox."),
            "{body}"
        );

        let id = kit.conversation_of("en-1").await;
        let uri = format!("/api/v1/w/{workspace}/conversations/{id}");
        let sources = |json: &Value| -> Vec<String> {
            json["decisions"]
                .as_array()
                .unwrap()
                .iter()
                .map(|d| d["source"].as_str().unwrap().to_owned())
                .collect()
        };
        let checked = kit.get(&token, &uri).await;
        assert!(
            sources(&checked.json).iter().any(|s| s == "model"),
            "{:?}",
            checked.json
        );
        let questions_before: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM conversations.needs WHERE basis = 'model' AND state = 'open'",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert!(questions_before > 0, "the model's yes put questions");

        // What the migration does to answers from before the check.
        sqlx::query(
            "UPDATE conversations.decisions SET model_version = 'laya-0.3.21-unchecked'
              WHERE source = 'model'",
        )
        .execute(&db)
        .await
        .unwrap();
        let unchecked = kit.get(&token, &uri).await;
        assert!(
            sources(&unchecked.json).iter().all(|s| s == "rules"),
            "{:?}",
            unchecked.json
        );
        let kept: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM conversations.decisions WHERE source = 'model'",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert!(kept > 0, "kept, not deleted");

        // Read again on the next refresh, the open questions they raised are
        // withdrawn (superseded), and none is raised again.
        let conversations: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT conversation_id::text FROM conversations.needs
              WHERE basis = 'model' AND state = 'open'",
        )
        .fetch_all(&db)
        .await
        .unwrap();
        let settings = crate::domain::needs::NeedSettings::new(36_500, &crate::Policy::baseline());
        let refresh_all = || async {
            for conversation in &conversations {
                let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
                crate::domain::needs::refresh(
                    &mut tx,
                    &kit.state.events,
                    &workspace.to_string(),
                    conversation.parse().unwrap(),
                    &settings,
                )
                .await
                .unwrap();
                tx.commit().await.unwrap();
            }
        };
        refresh_all().await;
        let (open, superseded): (i64, i64) = sqlx::query_as(
            "SELECT count(*) FILTER (WHERE state = 'open'),
                    count(*) FILTER (WHERE cleared_because = 'superseded')
               FROM conversations.needs WHERE basis = 'model'",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(open, 0);
        assert_eq!(superseded, questions_before);

        sqlx::query("DELETE FROM conversations.needs")
            .execute(&db)
            .await
            .unwrap();
        refresh_all().await;
        let raised_again: i64 =
            sqlx::query_scalar("SELECT count(*) FROM conversations.needs WHERE basis = 'model'")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(raised_again, 0);
    }
}
