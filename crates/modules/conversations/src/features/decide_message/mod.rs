//! `conversations.decide_message` — the model pass over each new inbound
//! message: the triage questions in one `decide` call
//! (`campbooks_ai::decide::strategy::Model`), every answer stored in
//! `decisions` with the policy applied. Under the shipped policy the
//! model's answers act where the eval shows they hold (ADR 0018): an
//! accepted stream answer places the conversation (stream source `model`)
//! when the message is its first inbound one and nobody placed it yet, and
//! accepted answers raise its need.
//!
//! Skipped when the `decide` role is not configured or has not passed its
//! answers check (`campbooks_ai::decide::check`), for messages sent before
//! the backfill window (`CAMPBOOKS_CONVERSATIONS__DECIDE_BACKFILL_DAYS`),
//! and for messages a checked model already answered (a replayed delivery).
//! The model reads whose inbox the message is in: the inbox's address and
//! name, and whether several members read it.
//!
//! `conversations.decide_backlog` is the same pass for one stored message,
//! enqueued by the needs backfill (`backfill_needs`) for recent mail that a
//! trusted model never answered, or answered before a triage question was
//! added: mail stored before the upgrade to the facts (ADR 0018), or before
//! the money question (campbooks-cloud#167). It asks every question again;
//! the checked answers already stored are kept, and only the missing ones
//! are added. A backlog pass waits while the answers check has not
//! run or the provider did not answer it (about a day at most); when the
//! check failed on the model's answers, the backfill queues nothing.
//!
//! Every pass's call counts against the workspace's daily AI limit
//! (`campbooks_ai::usage`, campbooks-cloud#152). Over the limit the pass
//! stores nothing, as when no model is configured, and the message keeps
//! what was decided when it was stored (the facts, taught rules); it is not
//! re-queued. When the call cannot be counted, it is not made and the job
//! retries.

use std::sync::Arc;

use campbooks_ai::{
    Gateway,
    decide::{
        Question,
        check::{AnswersCheck, CheckOutcome},
        signals::{
            actions::{action_questions, likelihoods},
            email::{Envelope, Recipient, triage_questions},
        },
        strategy::Strategy,
    },
    usage::Scope,
};
use campbooks_kernel::{
    Event, EventConsumer, JobContext, JobError, JobHandler, blobs::BlobRef, db::BeginFor as _,
    tenancy::WorkspaceId,
};
use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    config::ActionSettings,
    domain::{
        actions,
        blobs::read_text,
        conversation::{Changed, provider_message_ids},
        decide::{ENVELOPE_BODY_BYTES, MessageParts, PURPOSE, Source, envelope},
        decisions::Recorded,
        needs::{self, NeedSettings},
        policy::Policy,
        redact,
        streams::{Inbound, sort_first_inbound},
    },
    events::{Change, MessageReceived, Origin},
    ingest::Headers,
    model::{Address, ConversationId, InboxId, MessageId, StreamSource},
};

/// How long a backlog pass waits for the answers check's first run.
const CHECK_PENDING_RETRY: std::time::Duration = std::time::Duration::from_mins(2);
/// How long a backlog pass waits when the provider did not answer the
/// check: the check runs again on this cadence.
const CHECK_UNAVAILABLE_RETRY: std::time::Duration = campbooks_ai::decide::check::RECHECK_FAILED;

/// The model the pass asks: the `decide` role's strategy and, when it is
/// the role's, its gateway (whose answers check says whether to ask).
#[derive(Clone)]
pub struct ModelPass {
    strategy: Arc<dyn Strategy>,
    gateway: Option<Gateway>,
}

impl ModelPass {
    /// A pass asking `strategy`; `gateway` is the `decide` role it calls,
    /// when it does.
    pub fn new(strategy: Arc<dyn Strategy>, gateway: Option<Gateway>) -> Self {
        Self { strategy, gateway }
    }

    /// Whether its answers may be used now: a strategy given in place of the
    /// role always; the role while its last answers check passed.
    fn trusted(&self) -> bool {
        self.gateway.as_ref().is_none_or(Gateway::answers_trusted)
    }

    /// How long to wait before its answers may be trusted, when they may
    /// soon be: the check has not run yet, or the provider did not answer it
    /// (down, overloaded) and it runs again shortly.
    fn wait_for_check(&self) -> Option<std::time::Duration> {
        match self.gateway.as_ref()?.answers_check() {
            AnswersCheck::Pending => Some(CHECK_PENDING_RETRY),
            AnswersCheck::Done(report) if report.outcome == CheckOutcome::Unavailable => {
                Some(CHECK_UNAVAILABLE_RETRY)
            }
            AnswersCheck::Done(_) => None,
        }
    }

    /// Whether the role's last answers check failed on its answers: they
    /// are not used, so there is no point asking. A provider that did not
    /// answer the check is not refused: it may be back shortly.
    pub fn refused(&self) -> bool {
        self.gateway.as_ref().is_some_and(|g| {
            matches!(g.answers_check(), AnswersCheck::Done(report)
                if report.outcome == CheckOutcome::Failed)
        })
    }
}

/// When a message counts as answered already.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Answered {
    /// A checked model answered any question about it (a live delivery,
    /// replayed).
    Any,
    /// A checked model answered every triage question about it (the
    /// backlog, after a question was added).
    Every,
}

/// How one pass over a message ended.
enum Passed {
    /// Answered and stored, or nothing to do.
    Done,
    /// The model's answers are not trusted (its check failed or has not
    /// run): nothing stored.
    NotTrusted,
}

/// What every model pass shares: the model, the window, the policy and the
/// need settings.
struct Pass {
    model: Option<ModelPass>,
    backfill: TimeDelta,
    policy: Policy,
    needs: NeedSettings,
    /// The action catalogue (campbooks-cloud#166): off unless configured.
    actions: ActionSettings,
}

impl Pass {
    /// The questions asked about `envelope`: the triage questions, then,
    /// with the catalogue on, its own (each in a slot of its own). With it
    /// off, exactly the triage questions.
    fn asked(&self, triage: &[Question], envelope: &Envelope) -> Vec<Question> {
        let mut asked = triage.to_vec();
        if self.actions.enabled {
            asked.extend(action_questions(envelope));
        }
        asked
    }

    /// The catalogue's rating of every action from `answers` (and the model
    /// that gave them), with the catalogue on.
    fn rate(
        &self,
        envelope: &Envelope,
        answers: &[Option<campbooks_ai::decide::strategy::Decided>],
    ) -> Option<(
        Vec<(campbooks_ai::decide::signals::actions::ActionId, f64)>,
        String,
    )> {
        self.actions.enabled.then(|| {
            let given: Vec<&campbooks_ai::decide::Answer> =
                answers.iter().flatten().map(|d| &d.answer).collect();
            let model = given.first().map(|a| a.model.clone()).unwrap_or_default();
            (likelihoods(envelope, &given), model)
        })
    }

    /// Ask the model about `message` and store its answers: sort the
    /// conversation when its stream answer is accepted and the message is
    /// its first inbound one, then refresh its need.
    async fn run(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        message: MessageId,
        answered: Answered,
    ) -> Result<Passed, JobError> {
        let Some(model) = &self.model else {
            return Ok(Passed::Done);
        };
        // Not trusted: nothing is read, nothing is asked.
        if !model.trusted() {
            return Ok(Passed::NotTrusted);
        }
        let questions = triage_questions();
        let Some(loaded) = load(ctx, workspace, message, &questions, answered).await? else {
            return Ok(Passed::Done);
        };
        if loaded.sent_at < Utc::now() - self.backfill {
            return Ok(Passed::Done);
        }

        // Counted against the workspace's daily AI limit (campbooks-cloud#152).
        let usage = Scope::workspace(ctx.state().db.clone(), workspace, PURPOSE);
        let all = self.asked(&questions, &loaded.envelope);
        let asked = model.strategy.decide(&loaded.envelope, &all);
        let mut answers = match usage.run(asked).await {
            Ok(decisions) => decisions.answers,
            // The model failed (or has not passed) its answers check: the
            // pass does what it does when no model is configured, and
            // stores nothing.
            Err(campbooks_ai::Error::AnswersNotTrusted) => return Ok(Passed::NotTrusted),
            // The workspace reached its daily AI limit: the same, and the
            // message is not asked again later.
            Err(campbooks_ai::Error::OverDailyCap { .. }) => {
                tracing::debug!(workspace = %workspace, "over the daily AI limit: no model pass");
                return Ok(Passed::Done);
            }
            Err(error) => return Err(redact::ai(&error)),
        };
        // The catalogue's answers are rated, never stored as decisions:
        // the triage answers are recorded exactly as without it.
        let rated = self.rate(&loaded.envelope, &answers);
        answers.truncate(questions.len());
        let recorded = Recorded {
            message,
            conversation: loaded.conversation,
            source: Source::Model,
            strategy: model.strategy.name(),
            questions: &questions,
            answers: &answers,
            policy: &self.policy,
        };
        let events = ctx.state().events.clone();
        let ws = workspace.to_string();
        let tx = ctx.tx().await?;
        // Holding the inbox `FOR SHARE` until this commits, as a batch does:
        // a disconnect removing or freezing it waits for the answers, or the
        // answers see it closed (or the message gone) and keep nothing.
        let open = sqlx::query_scalar!(
            r#"SELECT i.detached_at IS NULL AS "open!" FROM conversations.inboxes i
                WHERE i.id = $1 FOR SHARE"#,
            loaded.inbox as InboxId,
        )
        .fetch_optional(&mut **tx)
        .await?;
        let present = sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM conversations.messages WHERE id = $1) AS "e!""#,
            message as MessageId,
        )
        .fetch_one(&mut **tx)
        .await?;
        if open != Some(true) || !present {
            return Ok(Passed::Done);
        }
        // Sorted only by its first inbound message, and never over a
        // member's move or an earlier placement.
        let placed = match recorded.accepted_stream() {
            Some(stream) => {
                sort_first_inbound(
                    tx,
                    loaded.conversation,
                    stream,
                    StreamSource::Model,
                    Inbound::Stored(message),
                )
                .await?
            }
            None => false,
        };
        recorded.store(tx, workspace, placed).await?;
        if let Some((rated, model_name)) = &rated {
            actions::store(
                tx,
                (*workspace.as_uuid(), message),
                rated,
                self.actions.floor(),
                model_name,
            )
            .await?;
        }
        if placed {
            // Not a member's change: nothing is sent back to the provider.
            let provider_message_ids = provider_message_ids(tx, loaded.conversation).await?;
            Changed {
                workspace: ws.clone(),
                inbox: loaded.inbox,
                conversation: loaded.conversation,
                origin: Origin::Provider,
                changes: vec![Change::Stream],
                provider_message_ids,
            }
            .publish(&events, tx)
            .await?;
        }
        needs::refresh(tx, &events, &ws, loaded.conversation, &self.needs).await?;
        Ok(Passed::Done)
    }
}

/// The model pass over every new inbound message, composed with the
/// module's model (none when the `decide` role is not configured), backfill
/// window and policy.
pub struct DecideMessage(Pass);

impl DecideMessage {
    /// A pass asking `model`, for messages sent within `backfill_days`,
    /// marking the answers `policy` accepts and raising needs as `needs`
    /// says.
    pub fn new(
        model: Option<ModelPass>,
        backfill_days: u32,
        policy: Policy,
        needs: NeedSettings,
    ) -> Self {
        Self(Pass {
            model,
            backfill: TimeDelta::days(i64::from(backfill_days)),
            policy,
            needs,
            actions: ActionSettings::default(),
        })
    }

    /// The same pass, rating the action catalogue as `actions` says.
    #[must_use]
    pub fn with_actions(mut self, actions: ActionSettings) -> Self {
        self.0.actions = actions;
        self
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
        let facts = event.payload.message;
        if self.0.model.is_none() || facts.sent_at < Utc::now() - self.0.backfill {
            return Ok(());
        }
        let workspace: WorkspaceId = facts
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("message_received names an invalid workspace"))?;
        self.0
            .run(ctx, workspace, facts.message, Answered::Any)
            .await?;
        Ok(())
    }
}

/// A stored message for the model pass (`conversations.decide_backlog`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Backlog {
    /// The message.
    pub message: MessageId,
}

impl Backlog {
    /// Its enqueue options in `workspace`: one pending job per message,
    /// behind live work.
    pub fn options(&self, workspace: WorkspaceId) -> campbooks_kernel::jobs::EnqueueOptions {
        campbooks_kernel::jobs::EnqueueOptions::for_workspace(workspace.to_string())
            .unique(format!("conversations.decide_backlog:{}", self.message))
            .priority(-10)
    }
}

/// The model pass over one stored message that a trusted model never
/// answered.
pub struct DecideBacklog(Pass);

impl DecideBacklog {
    /// The same pass as [`DecideMessage::new`].
    pub fn new(
        model: Option<ModelPass>,
        backfill_days: u32,
        policy: Policy,
        needs: NeedSettings,
    ) -> Self {
        let DecideMessage(pass) = DecideMessage::new(model, backfill_days, policy, needs);
        Self(pass)
    }

    /// The same pass, rating the action catalogue as `actions` says.
    #[must_use]
    pub fn with_actions(mut self, actions: ActionSettings) -> Self {
        self.0.actions = actions;
        self
    }
}

impl JobHandler for DecideBacklog {
    type Payload = Backlog;
    const NAME: &'static str = "conversations.decide_backlog";
    const QUEUE: &'static str = "ai";
    /// About a day of a provider that does not answer its check, at
    /// [`CHECK_UNAVAILABLE_RETRY`].
    const MAX_ATTEMPTS: u32 = 100;

    async fn run(&self, ctx: &mut JobContext, backlog: Backlog) -> Result<(), JobError> {
        let workspace: WorkspaceId = ctx
            .workspace()
            .and_then(|w| w.parse().ok())
            .ok_or_else(|| JobError::fatal("decide backlog enqueued outside a workspace"))?;
        let passed = self
            .0
            .run(ctx, workspace, backlog.message, Answered::Every)
            .await?;
        // Right after a start the check may not have run yet, or the
        // provider may have been down when it ran: ask again once it has
        // passed, rather than lose the message.
        if matches!(passed, Passed::NotTrusted)
            && let Some(wait) = self.0.model.as_ref().and_then(ModelPass::wait_for_check)
        {
            return Err(JobError::retry_after(
                "the decide answers check has not passed yet",
                wait,
            ));
        }
        Ok(())
    }
}

/// A stored inbound message, read for the model pass.
struct Loaded {
    conversation: ConversationId,
    inbox: InboxId,
    sent_at: DateTime<Utc>,
    envelope: Envelope,
}

/// The message's conversation and envelope, or `None` when there is nothing
/// to do (gone, its inbox detached, outbound, or already answered by a
/// checked model, as `answered` counts it, about `questions`).
async fn load(
    ctx: &JobContext,
    workspace: WorkspaceId,
    message: MessageId,
    questions: &[campbooks_ai::decide::Question],
    answered: Answered,
) -> Result<Option<Loaded>, JobError> {
    let ids: Vec<String> = questions.iter().map(|q| q.id.clone()).collect();
    let state = ctx.state();
    let mut tx = state.db.begin_for(workspace).await?;
    let row = sqlx::query!(
        r#"
        SELECT m.conversation_id AS "conversation: ConversationId",
               m.inbox_id AS "inbox: InboxId", m.direction, m.sent_at,
               m.sender, m.recipients, m.subject, m.headers, m.bodies,
               (SELECT count(DISTINCT d.question) FROM conversations.decisions d
                 WHERE d.message_id = m.id AND d.source = 'model'
                   AND d.model_version LIKE 'checked:%'
                   AND d.question = ANY($2)) AS "answered!",
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
           -- A kept, read-only inbox (its connection removed) is never sent
           -- to a model again.
           AND i.detached_at IS NULL
        "#,
        message as MessageId,
        &ids,
    )
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    let Some(row) = row else { return Ok(None) };
    let done = match answered {
        Answered::Any => row.answered > 0,
        Answered::Every => usize::try_from(row.answered).is_ok_and(|n| n >= ids.len()),
    };
    if done || row.direction != "inbound" {
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
            match read_text(
                &state.blobs,
                &workspace.to_string(),
                text,
                ENVELOPE_BODY_BYTES,
            )
            .await
            {
                Ok((body, _)) => body,
                // Its blob went with the message (its inbox removed):
                // nothing to do.
                Err(_) if gone(state, workspace, message).await? => return Ok(None),
                Err(error) => return Err(error.into()),
            }
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
    Ok(Some(Loaded {
        conversation: row.conversation,
        inbox: row.inbox,
        sent_at: row.sent_at,
        envelope,
    }))
}

/// Whether `message` is gone.
async fn gone(
    state: &campbooks_kernel::AppState,
    workspace: WorkspaceId,
    message: MessageId,
) -> Result<bool, JobError> {
    let mut tx = state.db.begin_for(workspace).await?;
    let present = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM conversations.messages WHERE id = $1) AS "e!""#,
        message as MessageId,
    )
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(!present)
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
mod actions_tests;
#[cfg(test)]
mod usage_tests;

#[cfg(test)]
mod tests {
    use campbooks_ai::{
        AiConfig, Gateway, Protocol, ProviderConfig, Role,
        decide::{
            check::{CheckOutcome, CheckReport},
            signals::email::triage,
        },
    };
    use campbooks_kernel::{db::BeginFor as _, tenancy::WorkspaceId};
    use serde_json::{Value, json};
    use sqlx::PgPool;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
    };

    use crate::{
        ConversationsConfig, ConversationsDeps, Policy, Switch,
        domain::decide::Source,
        testkit::{Gated, Kit, Scripted, sorting_deps},
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
                "suspicious": noul(0.05),
                "money": {"type": "choice", "choice": "neither",
                    "probabilities": {"owed": 0.02, "paid": 0.02, "neither": 0.96}}
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

    /// The test kit's settings with no model of its own: the pass asks the
    /// `decide` role, if any.
    fn without_model() -> ConversationsDeps {
        ConversationsDeps {
            model: None,
            ..sorting_deps()
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

    #[tokio::test]
    async fn a_provider_down_at_its_check_is_waited_for_and_a_failed_one_refused() {
        let server = MockServer::start().await;
        let pass = |outcome| {
            super::ModelPass::new(
                std::sync::Arc::new(Scripted::fixtures()),
                Some(checked_gateway(&server, outcome)),
            )
        };
        let down = pass(CheckOutcome::Unavailable);
        assert!(!down.trusted() && !down.refused());
        assert_eq!(down.wait_for_check(), Some(super::CHECK_UNAVAILABLE_RETRY));
        let failed = pass(CheckOutcome::Failed);
        assert!(failed.refused());
        assert_eq!(
            failed.wait_for_check(),
            None,
            "a failed check is not waited for"
        );
        let passed = pass(CheckOutcome::Passed);
        assert!(passed.trusted() && !passed.refused());
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
                ..without_model().config
            },
            ..without_model()
        };
        Kit::with_deps(db, deps).await
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn model_answers_act_where_the_policy_accepts_them(db: PgPool) {
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
        assert_eq!(rows.len(), 3 * 7);
        // Recorded with the envelope version (context 2, the facts) and the
        // stamp of the passed answers check.
        assert!(rows.iter().all(
            |(.., version, model_version)| *version == crate::STATE_VERSION
                && *version == 3
                && model_version.as_deref() == Some("checked:ctx2")
        ));
        // Acted where the policy (0.9 for every model answer here) accepts
        // it: the stream (0.97), the reply (0.9) and "not suspicious"
        // (0.95); the rest are suggestions.
        let acted: Vec<&str> = rows
            .iter()
            .filter(|(m, _, _, acted, ..)| m == "en-1" && *acted)
            .map(|(_, q, ..)| q.as_str())
            .collect();
        assert_eq!(acted, ["needs_reply", "stream", "suspicious"]);
        // The model's confident "primary" placed the newsletter: zero-shot
        // answers can be wrong, which is why its operating points come from
        // the eval.
        assert_eq!(
            kit.stream_of("en-3").await,
            ("primary".to_owned(), "model".to_owned())
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
    async fn the_model_sorting_a_conversation_is_announced(db: PgPool) {
        let kit = Kit::new(db.clone()).await;
        let workspace = WorkspaceId::new();
        let inbox = kit.inbox(workspace, "sam@example.com", &[]).await;
        kit.ingest(workspace, inbox, "en").await;
        let newsletter = kit.conversation_of("en-3").await;
        let changed = kit.events("conversations.conversation_changed").await;
        let sorted: Vec<&Value> = changed
            .iter()
            .filter(|e| e["conversation"] == newsletter.as_str())
            .collect();
        assert_eq!(sorted.len(), 1, "{changed:?}");
        assert_eq!(sorted[0]["changes"], json!(["stream"]));
        assert_eq!(sorted[0]["state"]["stream"], "newsletters");
        // Primary is placed too, with nothing to announce: it did not move.
        assert_eq!(
            kit.stream_of("en-1").await,
            ("primary".to_owned(), "model".to_owned())
        );
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn the_model_replaces_a_placement_by_the_retired_rules(db: PgPool) {
        let model = Gated::closed(Scripted::fixtures());
        let kit = Kit::with_deps(db.clone(), sorting_deps().with_model(model.clone())).await;
        let workspace = WorkspaceId::new();
        let (_, sam) = kit.member("sam", workspace);
        let inbox = kit
            .inbox(
                workspace,
                "sam@example.com",
                &[(sam, crate::model::InboxRole::Manager)],
            )
            .await;
        kit.ingest(workspace, inbox, "en").await;
        // The content rules placed the newsletter before the upgrade.
        sqlx::query(
            "UPDATE conversations.conversations c
                SET stream = 'receipts', stream_source = 'rules'
               FROM conversations.messages m
              WHERE m.conversation_id = c.id AND m.provider_message_id = 'en-3'",
        )
        .execute(&db)
        .await
        .unwrap();

        model.open();
        kit.state
            .events
            .replay(
                &kit.state.db,
                "conversations.decide_message",
                campbooks_kernel::events::ReplayFrom::Beginning,
            )
            .await
            .unwrap();
        kit.run_jobs().await;
        assert_eq!(
            kit.stream_of("en-3").await,
            ("newsletters".to_owned(), "model".to_owned())
        );
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn a_members_move_sticks(db: PgPool) {
        // The mail is stored while the model is not trusted yet (so in
        // Primary), and a member moves the newsletter to Updates before the
        // model answers.
        let model = Gated::closed(Scripted::fixtures());
        let kit = Kit::with_deps(db.clone(), sorting_deps().with_model(model.clone())).await;
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
        let id = kit.conversation_of("en-3").await;
        let moved = kit
            .post(
                &token,
                &format!("/api/v1/w/{workspace}/conversations/{id}/stream"),
                Some(json!({"stream": "updates"})),
            )
            .await;
        assert_eq!(moved.status, axum::http::StatusCode::OK, "{:?}", moved.json);

        // Then the model answers "newsletters", confidently.
        model.open();
        kit.state
            .events
            .replay(
                &kit.state.db,
                "conversations.decide_message",
                campbooks_kernel::events::ReplayFrom::Beginning,
            )
            .await
            .unwrap();
        kit.run_jobs().await;
        assert_eq!(
            kit.stream_of("en-3").await,
            ("updates".to_owned(), "member".to_owned())
        );
        let (value, acted): (Value, bool) = sqlx::query_as(
            "SELECT d.value, d.acted FROM conversations.decisions d
               JOIN conversations.messages m ON m.id = d.message_id
              WHERE m.provider_message_id = 'en-3' AND d.question = 'stream'
                AND d.source = 'model'",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!((value, acted), (json!("newsletters"), false));
        // The others it sorted.
        assert_eq!(
            kit.stream_of("en-4").await,
            ("updates".to_owned(), "model".to_owned())
        );
    }

    /// The model reading a code host's security alert: in Updates, asking
    /// for an action, no reply.
    fn alert_answers() -> Value {
        let noul = |p: f64| json!({"type": "noul", "noul": p});
        json!({
            "model": "laya-test",
            "answers": {
                "stream": {"type": "choice", "choice": "updates",
                    "probabilities": {"primary": 0.01, "updates": 0.97, "receipts": 0.01,
                                      "newsletters": 0.01}},
                "needs_reply": noul(0.05),
                "asks_action": noul(0.95),
                "has_deadline": noul(0.1),
                "urgency": {"type": "score", "score": 1.0,
                    "probabilities": {"0": 0.2, "1": 0.6, "2": 0.2}},
                "suspicious": noul(0.02),
                "money": {"type": "choice", "choice": "neither",
                    "probabilities": {"owed": 0.02, "paid": 0.02, "neither": 0.96}}
            }
        })
    }

    /// The model reading a comment on a watched issue as needing a reply,
    /// confidently: it reads its question mark as one for the recipient.
    fn comment_answers() -> Value {
        let noul = |p: f64| json!({"type": "noul", "noul": p});
        json!({
            "model": "laya-test",
            "answers": {
                "stream": {"type": "choice", "choice": "updates",
                    "probabilities": {"primary": 0.2, "updates": 0.7, "receipts": 0.05,
                                      "newsletters": 0.05}},
                "needs_reply": noul(0.95),
                "asks_action": noul(0.1),
                "has_deadline": noul(0.05),
                "urgency": {"type": "score", "score": 0.2,
                    "probabilities": {"0": 0.8, "1": 0.2, "2": 0.0}},
                "suspicious": noul(0.02),
                "money": {"type": "choice", "choice": "neither",
                    "probabilities": {"owed": 0.02, "paid": 0.02, "neither": 0.96}}
            }
        })
    }

    /// Sam's inbox holding the recorded batch `recorded`, read by the facts
    /// and by a model answering `answers`; and the state the model read.
    async fn read_recorded(
        db: &PgPool,
        recorded: &Value,
        answers: Value,
    ) -> (Kit, WorkspaceId, String) {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/systemone"))
            .respond_with(ResponseTemplate::new(200).set_body_json(answers))
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
        assert_eq!(model, 7, "the model pass ran");
        let request = &server.received_requests().await.unwrap()[0];
        let body: Value = serde_json::from_slice(&request.body).unwrap();
        let state = body["state"].as_str().unwrap().to_owned();
        (kit, workspace, state)
    }

    fn fixture(name: &str) -> Value {
        serde_json::from_str(&crate::testkit::fixture(name)).unwrap()
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
    async fn a_notification_the_model_reads_as_a_to_do_is_no_need_without_a_deadline(db: PgPool) {
        let (kit, _, state) = read_recorded(&db, &fixture("dependabot"), alert_answers()).await;
        assert_eq!(
            kit.stream_of("dependabot-1").await,
            ("updates".to_owned(), "model".to_owned())
        );
        assert!(
            needs(&db).await.is_empty(),
            "automated mail asks something only with a time limit"
        );
        // The model was told the facts, as hints.
        assert!(
            state.contains(
                "Facts from the headers, not decisions: list mail (List-Id \
                 \"example-org/demo-shop <demo-shop.example-org.github.com>\"); \
                 service sender address."
            ),
            "{state}"
        );
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn a_notification_asking_by_a_deadline_is_a_sure_do_in_updates(db: PgPool) {
        let mut answers = alert_answers();
        answers["answers"]["has_deadline"] = json!({"type": "noul", "noul": 0.9});
        read_recorded(&db, &fixture("dependabot"), answers).await;
        assert_eq!(
            needs(&db).await,
            [(
                "do".to_owned(),
                "sure".to_owned(),
                "model".to_owned(),
                "open".to_owned()
            )]
        );
        let due: Option<chrono::NaiveDate> =
            sqlx::query_scalar("SELECT due_on FROM conversations.needs")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(due, None, "no due date until one is extracted");
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn the_model_is_told_when_the_senders_domain_failed_authentication(db: PgPool) {
        let mut recorded = fixture("dependabot");
        recorded["changes"][0]["message"]["headers"]["authentication_results"] = json!(
            "mx.google.com; dkim=fail header.i=@github.com; dmarc=fail header.from=github.com"
        );
        let (_, _, state) = read_recorded(&db, &recorded, alert_answers()).await;
        assert!(
            state.contains("the sender's domain failed authentication"),
            "{state}"
        );
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn the_facts_no_wins_over_the_models_yes_on_a_notification(db: PgPool) {
        let (..) = read_recorded(&db, &fixture("github-comment"), comment_answers()).await;
        assert_eq!(needs(&db).await, []);
        let answers: Vec<(String, Value, bool)> = sqlx::query_as(
            "SELECT source, value, acted FROM conversations.decisions
              WHERE question = 'needs_reply' ORDER BY source",
        )
        .fetch_all(&db)
        .await
        .unwrap();
        assert_eq!(
            answers,
            [
                ("model".to_owned(), json!(true), true),
                ("rules".to_owned(), json!(false), true),
            ]
        );
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn a_question_its_answers_no_longer_support_is_withdrawn(db: PgPool) {
        let (kit, workspace, _) =
            read_recorded(&db, &fixture("github-comment"), comment_answers()).await;
        let conversation = kit.conversation_of("github-comment-1").await;
        let message = kit.message_of("github-comment-1").await;
        // What an earlier `raise` stored: a question the answers on record
        // no longer support.
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
    async fn an_off_network_decide_role_without_opt_in_leaves_only_the_facts(db: PgPool) {
        // `decide.example.com` is off the instance's network: no mail goes
        // there (nothing could answer anyway), only the facts read mail.
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
            ..without_model()
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
        assert_eq!(by_source, [("rules".to_owned(), 2)], "the facts answered");
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

        let none = Kit::with_deps(db.clone(), without_model()).await;
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
        // A policy that only records the model's reply and action answers
        // leaves its "yes" to be put as a question.
        let deps = ConversationsDeps {
            ai: Some(checked_gateway(&server, CheckOutcome::Passed)),
            config: ConversationsConfig {
                decide_backfill_days: 100_000,
                ..without_model().config
            },
            ..without_model()
        };
        let policy: Policy = deps
            .policy
            .clone()
            .with(triage::NEEDS_REPLY, Source::Model, Switch::Record)
            .with(triage::ASKS_ACTION, Source::Model, Switch::Record);
        let kit = Kit::with_deps(db.clone(), deps.with_policy(policy.clone())).await;
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
        let settings = crate::domain::needs::NeedSettings::new(36_500, &policy);
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

        // Asked again, a trusted model's answers replace the unchecked ones.
        kit.state
            .events
            .replay(
                &kit.state.db,
                "conversations.decide_message",
                campbooks_kernel::events::ReplayFrom::Beginning,
            )
            .await
            .unwrap();
        kit.run_jobs().await;
        let versions: Vec<Option<String>> = sqlx::query_scalar(
            "SELECT DISTINCT model_version FROM conversations.decisions WHERE source = 'model'",
        )
        .fetch_all(&db)
        .await
        .unwrap();
        assert_eq!(versions, [Some("checked:ctx2".to_owned())]);
    }
}
