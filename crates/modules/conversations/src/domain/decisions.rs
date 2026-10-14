//! Storing the decide pass's answers.

use campbooks_ai::decide::{
    AnswerValue, Question, QuestionKind, signals::email::triage, strategy::Decided,
};
use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::{Value, json};
use sqlx::PgConnection;

use super::{
    decide::{STATE_VERSION, Source},
    policy::Policy,
};
use crate::model::{ConversationId, DecisionId, MessageId};

/// One strategy's verdicts on one message, with the switch applied.
#[derive(Debug)]
pub struct Recorded<'a> {
    /// The message.
    pub message: MessageId,
    /// Its conversation.
    pub conversation: ConversationId,
    /// What kind of strategy answered.
    pub source: Source,
    /// The strategy's name.
    pub strategy: &'a str,
    /// The questions asked.
    pub questions: &'a [Question],
    /// One answer (or abstention) per question, with its reasons.
    pub answers: &'a [Option<Decided>],
    /// Which answers may act.
    pub policy: &'a Policy,
}

impl Recorded<'_> {
    /// The answer to `question` that the policy lets act, if any.
    pub fn accepted(&self, question: &str) -> Option<&Decided> {
        let switch = self.policy.switch(question, self.source);
        self.questions
            .iter()
            .zip(self.answers)
            .find(|(q, _)| q.id == question)
            .and_then(|(_, verdict)| verdict.as_ref())
            .filter(|decided| switch.accepts(&decided.answer))
    }

    /// Store every non-abstaining verdict. An answer is marked `acted` when
    /// the policy accepts it; the stream answer only when it actually placed
    /// the conversation (`placed_stream`; later messages never re-sort). A
    /// verdict already stored for the same message, question and source (a
    /// replayed delivery) is kept.
    pub async fn store(
        &self,
        conn: &mut PgConnection,
        workspace: WorkspaceId,
        placed_stream: bool,
    ) -> Result<(), sqlx::Error> {
        for (question, decided) in self.questions.iter().zip(self.answers) {
            let Some(decided) = decided else { continue };
            let acted = if question.id == triage::STREAM {
                placed_stream
            } else {
                self.accepted(&question.id).is_some()
            };
            let answer = &decided.answer;
            let reasons: Vec<_> = decided
                .reasons
                .iter()
                .map(|r| match &r.sentence {
                    Some(sentence) => {
                        json!({"rule": r.rule, "evidence": r.evidence, "sentence": sentence})
                    }
                    None => json!({"rule": r.rule, "evidence": r.evidence}),
                })
                .collect();
            let latency_ms = i32::try_from(answer.latency.as_millis()).unwrap_or(i32::MAX);
            sqlx::query!(
                r#"
                INSERT INTO conversations.decisions
                    (id, workspace_id, message_id, conversation_id, question, source, strategy,
                     value, options, probabilities, confidence, model, model_version,
                     latency_ms, reasons, acted, state_version)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
                ON CONFLICT (message_id, question, source) DO NOTHING
                "#,
                DecisionId::new() as DecisionId,
                workspace.as_uuid(),
                self.message as MessageId,
                self.conversation as ConversationId,
                question.id,
                self.source.as_str(),
                self.strategy,
                value_json(&answer.value),
                options_json(question),
                answer.probabilities.as_ref().map(|p| json!(p)),
                answer.confidence,
                answer.model,
                answer.model_version,
                latency_ms,
                json!(reasons),
                acted,
                STATE_VERSION,
            )
            .execute(&mut *conn)
            .await?;
        }
        Ok(())
    }
}

fn value_json(value: &AnswerValue) -> Value {
    match value {
        AnswerValue::Choice(label) => json!(label),
        AnswerValue::Score(level) => json!(level),
        AnswerValue::Noul(holds) => json!(holds),
    }
}

/// The options in the order the probabilities are given.
fn options_json(question: &Question) -> Value {
    match &question.kind {
        QuestionKind::Choice { options, .. } => {
            json!(options.iter().map(|o| o.label.as_str()).collect::<Vec<_>>())
        }
        QuestionKind::Score { levels, .. } => json!(levels),
        QuestionKind::Noul { .. } => json!(["false", "true"]),
    }
}
