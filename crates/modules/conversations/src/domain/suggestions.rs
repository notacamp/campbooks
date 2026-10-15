//! The decide pass's answers as the client shows them: suggestions, with the
//! stored reasons behind them ("Why?"). Read-only views of `decisions`.

use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use utoipa::ToSchema;

use campbooks_kernel::{Error, Result};

use super::{
    conversation::ConversationSummary,
    decide::Source,
    policy::{Policy, Switch},
};
use crate::model::{ConversationId, MessageId, Stream, StreamSource};

/// What kind of strategy gave an answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DecisionSource {
    /// The shared email signals.
    Rules,
    /// The `decide` model.
    Model,
    /// Nearest neighbours of corrected mail.
    Knn,
    /// Several strategies in turn.
    Cascade,
}

impl DecisionSource {
    fn parse(source: &str) -> Result<Self> {
        match source {
            "rules" => Ok(Self::Rules),
            "model" => Ok(Self::Model),
            "knn" => Ok(Self::Knn),
            "cascade" => Ok(Self::Cascade),
            other => Err(Error::internal(format!(
                "unknown decision source `{other}`"
            ))),
        }
    }
}

/// One reason behind an answer: a stable rule id the client translates,
/// and what it matched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ReasonView {
    /// `<strategy>.<rule>`, e.g. `signals.list_header`.
    #[schema(example = "signals.list_header")]
    pub rule: String,
    /// What it matched (a header name, a word), possibly empty.
    pub evidence: String,
    /// The sender's sentence holding the evidence, when it came from the
    /// text. Untrusted: show it as quoted plain text only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sentence: Option<String>,
}

/// Where the decide pass would put a conversation, when that is not where it
/// is: a suggestion the member can take or leave.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct StreamSuggestion {
    /// The suggested stream.
    pub stream: Stream,
    /// Who suggested it.
    pub source: DecisionSource,
    /// How sure, 0 to 100, when the source says.
    pub confidence: Option<u8>,
    /// Why.
    pub reasons: Vec<ReasonView>,
}

/// An answer's value: a label (choice), a level (score) or yes/no.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum AnswerView {
    /// Yes or no.
    Flag(bool),
    /// A level on the question's scale.
    Level(f64),
    /// One of the question's options.
    Label(String),
}

/// One answer of the decide pass about the newest inbound message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct DecisionView {
    /// The question: `stream`, `needs_reply`, `asks_action`,
    /// `has_deadline`, `urgency` or `suspicious`.
    #[schema(example = "needs_reply")]
    pub question: String,
    /// The answer.
    pub value: AnswerView,
    /// Who answered.
    pub source: DecisionSource,
    /// How sure, 0 to 100, when the source says.
    pub confidence: Option<u8>,
    /// Whether it changed anything. In this version only a confident rules
    /// stream answer may; every other answer is a suggestion.
    pub acted: bool,
    /// The confidence, 0 to 100, from which an answer of this source to
    /// this question may act; `None` when it never acts (a suggestion).
    pub threshold: Option<u8>,
    /// Why.
    pub reasons: Vec<ReasonView>,
}

/// The confidence from which `view`'s source may act on its question.
pub fn threshold(policy: &Policy, view: &DecisionView) -> Option<u8> {
    let source = match view.source {
        DecisionSource::Rules => Source::Rules,
        DecisionSource::Model => Source::Model,
        DecisionSource::Knn | DecisionSource::Cascade => return None,
    };
    match policy.switch(&view.question, source) {
        Switch::Record => None,
        // The smallest whole percent at or above the switch's point.
        Switch::ActAbove(at) => (0..=100_u8).find(|p| f64::from(*p) >= at * 100.0 - 1e-9),
    }
}

struct DecisionRow {
    conversation_id: ConversationId,
    question: String,
    value: serde_json::Value,
    source: String,
    confidence: Option<i16>,
    acted: bool,
    reasons: serde_json::Value,
}

impl DecisionRow {
    fn confidence(&self) -> Option<u8> {
        self.confidence.and_then(|c| u8::try_from(c).ok())
    }

    fn reasons(&self) -> Result<Vec<ReasonView>> {
        serde_json::from_value(self.reasons.clone())
            .map_err(|e| Error::internal(super::redact::json(&e)))
    }
}

/// Fill in each summary's [`StreamSuggestion`]: the newest stream answer on
/// record (the model's before the rules' for the same message; a model's
/// only when given under a passed answers check), when it did
/// not act, names another stream than the conversation's, and no member
/// placed the conversation.
pub async fn attach(conn: &mut PgConnection, summaries: &mut [ConversationSummary]) -> Result<()> {
    let ids: Vec<ConversationId> = summaries
        .iter()
        .filter(|s| s.stream_source != StreamSource::Member)
        .map(|s| s.id)
        .collect();
    if ids.is_empty() {
        return Ok(());
    }
    let rows = sqlx::query_as!(
        DecisionRow,
        r#"
        SELECT DISTINCT ON (d.conversation_id)
               d.conversation_id AS "conversation_id: ConversationId", d.question, d.value,
               d.source, round(d.confidence * 100)::smallint AS confidence, d.acted, d.reasons
          FROM conversations.decisions d
          JOIN conversations.messages m ON m.id = d.message_id
         WHERE d.conversation_id = ANY($1) AND d.question = 'stream'
           AND (d.source <> 'model' OR d.model_version LIKE 'checked:%')
         ORDER BY d.conversation_id, m.sent_at DESC, (d.source = 'model') DESC, d.created_at DESC
        "#,
        &ids as &[ConversationId],
    )
    .fetch_all(conn)
    .await?;
    for row in rows {
        let Some(summary) = summaries.iter_mut().find(|s| s.id == row.conversation_id) else {
            continue;
        };
        let Some(stream) = row.value.as_str().and_then(|v| v.parse::<Stream>().ok()) else {
            continue;
        };
        if row.acted || stream == summary.stream {
            continue;
        }
        summary.suggestion = Some(StreamSuggestion {
            stream,
            source: DecisionSource::parse(&row.source)?,
            confidence: row.confidence(),
            reasons: row.reasons()?,
        });
    }
    Ok(())
}

/// Every question's answer about `message` (the model's before the rules';
/// a model's only when given under a passed answers check), in the
/// question's order of asking.
pub async fn for_message(conn: &mut PgConnection, message: MessageId) -> Result<Vec<DecisionView>> {
    let rows = sqlx::query_as!(
        DecisionRow,
        r#"
        SELECT DISTINCT ON (question)
               conversation_id AS "conversation_id: ConversationId", question, value, source,
               round(confidence * 100)::smallint AS confidence, acted, reasons
          FROM conversations.decisions
         WHERE message_id = $1 AND (source <> 'model' OR model_version LIKE 'checked:%')
         ORDER BY question, (source = 'model') DESC, created_at DESC
        "#,
        message as MessageId,
    )
    .fetch_all(conn)
    .await?;
    let mut views = rows
        .into_iter()
        .map(|row| {
            let value = serde_json::from_value(row.value.clone())
                .map_err(|e| Error::internal(super::redact::json(&e)))?;
            Ok(DecisionView {
                source: DecisionSource::parse(&row.source)?,
                confidence: row.confidence(),
                reasons: row.reasons()?,
                question: row.question,
                value,
                acted: row.acted,
                threshold: None,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    views.sort_by_key(|view| order(&view.question));
    Ok(views)
}

/// The order the questions are asked in (`campbooks_ai`'s triage set).
fn order(question: &str) -> usize {
    use campbooks_ai::decide::signals::email::triage;
    [
        triage::STREAM,
        triage::NEEDS_REPLY,
        triage::ASKS_ACTION,
        triage::HAS_DEADLINE,
        triage::URGENCY,
        triage::SUSPICIOUS,
    ]
    .iter()
    .position(|q| *q == question)
    .unwrap_or(usize::MAX)
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use sqlx::PgPool;

    use crate::{ConversationsDeps, Policy, model::InboxRole, testkit::Kit};

    #[sqlx::test(migrations = "../../../migrations")]
    async fn the_shipped_policy_shows_stream_answers_as_suggestions_with_reasons(db: PgPool) {
        let deps = ConversationsDeps::offline().with_policy(Policy::baseline());
        let kit = Kit::with_deps(db, deps).await;
        let workspace = campbooks_kernel::tenancy::WorkspaceId::new();
        let (sam, user) = kit.member("sam", workspace);
        let inbox = kit
            .inbox(workspace, "sam@example.com", &[(user, InboxRole::Manager)])
            .await;
        kit.ingest(workspace, inbox, "en").await;
        let base = format!("/api/v1/w/{workspace}/conversations");

        let list = kit.get(&sam, &base).await;
        assert_eq!(list.status, StatusCode::OK, "{:?}", list.json);
        let newsletter = list.json["conversations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["subject"].as_str().unwrap().starts_with("Issue 142"))
            .unwrap();
        // Not moved: still in Primary, with Newsletters suggested and why.
        assert_eq!(newsletter["stream"], "primary");
        assert_eq!(newsletter["suggestion"]["stream"], "newsletters");
        assert_eq!(newsletter["suggestion"]["source"], "rules");
        let reasons = newsletter["suggestion"]["reasons"].as_array().unwrap();
        assert!(
            reasons
                .iter()
                .any(|r| r["rule"].as_str().unwrap().starts_with("signals.")),
            "{reasons:?}"
        );

        let id = newsletter["id"].as_str().unwrap();
        let detail = kit.get(&sam, &format!("{base}/{id}")).await;
        let decisions = detail.json["decisions"].as_array().unwrap();
        assert_eq!(decisions[0]["question"], "stream");
        assert_eq!(decisions[0]["value"], "newsletters");
        assert_eq!(decisions[0]["acted"], false);
        assert!(decisions.len() > 1, "{decisions:?}");

        // A member's move ends the suggestion.
        let moved = kit
            .post(
                &sam,
                &format!("{base}/{id}/stream"),
                Some(serde_json::json!({"stream": "newsletters"})),
            )
            .await;
        assert_eq!(moved.status, StatusCode::OK, "{:?}", moved.json);
        assert!(moved.json["suggestion"].is_null(), "{:?}", moved.json);
    }
}
