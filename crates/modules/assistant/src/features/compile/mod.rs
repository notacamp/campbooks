//! `POST /api/v1/w/{workspace_id}/assistant/compile` — turn a sentence the
//! member typed into taught rules for them to check (campbooks-cloud#127).
//!
//! Only on the member's own action. The `complete` role gets the sanitised
//! sentence, the member's own inbox addresses, the stream keys and their
//! label names, read as the member through the conversations operations, so
//! every access check applies; never anyone's mail. It proposes in a strict
//! schema with no tools; [`validate`] keeps only what the vocabulary allows
//! and the input supports; a person named without an address becomes a
//! question with candidates. An answer that isn't the schema's JSON is the
//! model's failure (503 `compile_failed`), never `unclear`, which would
//! blame the member's words. Nothing is saved: the member previews and saves
//! through `conversations.preview_rule` and `conversations.create_rule`.

mod prompt;
mod types;
mod validate;
mod words;

use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

use axum::{Extension, Json, extract::State};
use campbooks_ai::{Gateway, Role, usage::Scope};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    auth::Principal,
    authz::Action,
    db::BeginFor as _,
    extract,
    operations::{Caller, Invocation, InvokeError, invoke},
    rate_limit::{Limit, RateKey},
    tenancy::{UserId, Workspace, WorkspaceId},
};
use serde::Deserialize;
use serde_json::{Value, json};

use self::types::{
    Candidate, CompileRequest, CompileResult, Proposal, ProposalInbox, ProposedAction,
    ProposedCondition, ProposedHeader, WhoQuestion,
};
use self::{
    prompt::{ModelInput, ModelOutput, STREAMS},
    validate::{Context, InboxFacts, LabelFacts},
};
use crate::{domain::resources, scopes::AssistantWrite};

/// Compiles per member per hour.
const LIMIT: Limit = Limit::new("assistant.compile", 20, Duration::from_hours(1));
/// Most candidates offered for a `who`.
const MAX_CANDIDATES: usize = 5;
/// Words of a `who` phrase that name no one ("my", "o", "le"…).
const STOP_WORDS: [&str; 40] = [
    "my", "our", "the", "a", "an", "from", "of", "mine", "meu", "minha", "meus", "minhas", "nosso",
    "nossa", "o", "os", "as", "do", "da", "de", "mi", "mis", "nuestro", "nuestra", "el", "la",
    "los", "las", "del", "mon", "ma", "mes", "notre", "nos", "le", "les", "du", "des", "en", "et",
];

/// The AI gateway, when one is composed.
#[derive(Debug, Clone)]
pub struct Compiler {
    ai: Option<Gateway>,
}

impl Compiler {
    /// Compiling through `ai`'s `complete` role.
    pub fn new(ai: Option<Gateway>) -> Self {
        Self { ai }
    }

    /// The gateway, if its `complete` role is configured and may receive
    /// the member's words; otherwise the gateway's own reason.
    /// 409 `compile_unavailable` (no model may read sentences here: the
    /// member builds the rule by hand), never the gateway's own words.
    fn gateway(&self) -> Result<&Gateway> {
        self.ai
            .as_ref()
            .filter(|gateway| {
                gateway
                    .config()
                    .provider_for_content(Role::Complete)
                    .is_ok()
            })
            .ok_or_else(|| Error::Conflict(crate::MESSAGES.reason("compile_unavailable")))
    }
}

/// Turn your sentence into rules to check.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/assistant/compile",
    operation_id = "assistant.compile_instruction",
    tag = "assistant",
    security(("bearer" = ["assistant:write"])),
    // A command: it saves nothing, but it spends the member's hourly
    // allowance and a model call, so it is never replayed or prefetched.
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).records(["assistant.profile"]).access("assistant.profile", campbooks_kernel::authz::Action::Write)))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    request_body = CompileRequest,
    responses(
        (status = OK, description = "What Scout understood; nothing is saved.", body = CompileResult),
        (status = BAD_REQUEST, description = "Empty, too long, a link or pasted mail (`instruction_empty`, `instruction_too_long`, `instruction_has_link`, `instruction_pasted_mail`).",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or not a member (`profile_needs_member`).",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such workspace, or you are not a member.",
            body = Problem, content_type = "application/problem+json"),
        (status = TOO_MANY_REQUESTS, description = "Too many sentences this hour.", body = Problem,
            content_type = "application/problem+json"),
        (status = CONFLICT, description = "No language model may read sentences here (`compile_unavailable`): build the rule by hand.",
            body = Problem, content_type = "application/problem+json"),
        (status = SERVICE_UNAVAILABLE, description = "The language model failed or its answer could not be read (`compile_failed`), \
            or the workspace used its whole daily AI limit (`ai_daily_limit_reached`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    Extension(compiler): Extension<Compiler>,
    workspace: Workspace<AssistantWrite>,
    extract::Json(body): extract::Json<CompileRequest>,
) -> Result<Json<CompileResult>> {
    let started = Instant::now();
    let user = {
        let mut tx = state.db.begin_for(workspace.id).await?;
        let user =
            resources::require_own(&state, &mut tx, &workspace.access, Action::Write).await?;
        tx.commit().await?;
        user
    };
    let sentence = words::own_words(&body.text).map_err(|refusal| {
        Error::BadRequest(
            crate::MESSAGES
                .reason(refusal.code())
                .arg("max", words::MAX_CHARS),
        )
    })?;
    let gateway = compiler.gateway()?;
    state
        .rate_limits
        .check(&state.db, &LIMIT, &rate_key(workspace.id, user))
        .await?;

    let caller = Ask {
        state: &state,
        principal: &workspace.principal,
        workspace: workspace.id,
        senders: Mutex::default(),
    };
    let ctx = caller.context(sentence).await?;
    let input = ModelInput {
        sentence: &ctx.sentence,
        my_addresses: ctx.inboxes.iter().map(|i| i.address.as_str()).collect(),
        streams: STREAMS,
        labels: ctx.labels.iter().map(|l| l.name.as_str()).collect(),
    };
    // A person is waiting: the call may use the share of the workspace's
    // daily AI limit kept for people (campbooks-cloud#152).
    let usage = Scope::workspace(state.db.clone(), workspace.id, "assistant.compile").interactive();
    let asked = gateway.chat(Role::Complete, prompt::request(&input));
    let response = match usage.run(asked).await {
        Ok(response) => response,
        // The workspace used its whole daily AI limit: 503, saying so.
        Err(error @ campbooks_ai::Error::OverDailyCap { .. }) => return Err(error.into()),
        Err(error) => {
            // The kind only: a provider's error can echo the request.
            tracing::warn!(error = error_kind(&error), "compile: the model call failed");
            return Err(Error::Unavailable(crate::MESSAGES.reason("compile_failed")));
        }
    };
    // Sizes and the stop reason only, never what the model wrote.
    let finish_reason = finish_kind(response.finish_reason.as_deref());
    let completion_tokens = response.usage.map(|usage| usage.completion_tokens);
    let content_chars = response.content.chars().count();
    let Ok(output) = response.json::<ModelOutput>() else {
        // The model failed, not the member's words: 503, so the page offers
        // to try again or build the rule by hand, never "say it
        // differently" (campbooks-cloud#217). The kind only, never the
        // parser's message: it can quote the answer.
        let reason = if response.content.trim().is_empty() {
            "empty"
        } else if finish_reason == "length" {
            "truncated"
        } else {
            "not_json"
        };
        tracing::warn!(
            reason,
            finish_reason,
            completion_tokens,
            content_chars,
            latency_ms = elapsed_ms(started),
            "compile: the model's answer did not parse"
        );
        return Err(Error::Unavailable(crate::MESSAGES.reason("compile_failed")));
    };
    let checked = validate::check(&output, &ctx);
    let mut proposals = checked.proposals;
    for proposal in &mut proposals {
        if let Some(who) = proposal.who.as_mut() {
            caller.candidates(who).await?;
        }
    }
    tracing::info!(
        outcome = ?checked.outcome,
        proposals = proposals.len(),
        questions = proposals.iter().filter(|p| p.who.is_some()).count(),
        finish_reason,
        completion_tokens,
        content_chars,
        latency_ms = elapsed_ms(started),
        "compile"
    );
    Ok(Json(CompileResult {
        outcome: checked.outcome,
        unsupported: checked.unsupported,
        text: ctx.sentence,
        proposals,
    }))
}

/// One counter per member and workspace.
fn rate_key(workspace: WorkspaceId, user: UserId) -> RateKey {
    RateKey::text(&format!("{workspace}:{user}"))
}

/// Milliseconds since `started`, for the log.
fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// Why the model stopped, for the log: one of a few known words, never
/// whatever string the provider sent.
fn finish_kind(reason: Option<&str>) -> &'static str {
    match reason {
        None => "none",
        Some("stop") => "stop",
        Some("length") => "length",
        Some("content_filter") => "content_filter",
        Some("tool_calls") => "tool_calls",
        Some(_) => "other",
    }
}

/// What kind of failure, for the log: never the provider's words.
fn error_kind(error: &campbooks_ai::Error) -> &'static str {
    match error {
        campbooks_ai::Error::RoleNotConfigured(_) => "role_not_configured",
        campbooks_ai::Error::ExternalContentRefused { .. } => "external_content_refused",
        campbooks_ai::Error::Transport(_) => "transport",
        campbooks_ai::Error::Status { .. } => "status",
        campbooks_ai::Error::InvalidResponse(_) => "invalid_response",
        campbooks_ai::Error::InvalidQuestion(_) | campbooks_ai::Error::Config(_) => "config",
        campbooks_ai::Error::AnswersNotTrusted => "answers_not_trusted",
        campbooks_ai::Error::OverDailyCap { .. } => "over_daily_cap",
        campbooks_ai::Error::Unattributed => "unattributed",
        campbooks_ai::Error::Metering => "metering",
    }
}

/// The conversations operations, called as the member.
struct Ask<'a> {
    state: &'a AppState,
    principal: &'a Principal,
    workspace: WorkspaceId,
    /// `list_senders` answers by `q`, for this one sentence: several "who"
    /// questions share words and the fallback, and each answer reads the
    /// newest mail of every inbox the member can see.
    senders: Mutex<HashMap<Option<String>, Vec<Candidate>>>,
}

#[derive(Deserialize)]
struct InboxRow {
    id: String,
    address: String,
    role: Option<String>,
    #[serde(default)]
    shared: bool,
}

#[derive(Deserialize)]
struct LabelRow {
    id: String,
    inbox: String,
    name: String,
}

impl Ask<'_> {
    async fn call(&self, operation: &str, input: Value) -> Result<Option<Value>> {
        let invocation =
            Invocation::new(Caller::Assistant, self.principal.clone(), operation, input)
                .in_workspace(self.workspace.to_string());
        match invoke(self.state, invocation).await {
            Ok(output) => Ok(Some(output.body)),
            // The member's token can't read it (no conversations scope):
            // nothing of theirs to use.
            Err(InvokeError::Failed { status, .. })
                if status == axum::http::StatusCode::FORBIDDEN =>
            {
                Ok(None)
            }
            Err(error) => Err(Error::internal(error)),
        }
    }

    /// The member's inboxes and labels.
    async fn context(&self, sentence: String) -> Result<Context> {
        let inboxes: Vec<InboxRow> = self
            .call("conversations.list_inboxes", Value::Null)
            .await?
            .and_then(|body| serde_json::from_value(body["inboxes"].clone()).ok())
            .unwrap_or_default();
        let labels: Vec<LabelRow> = self
            .call("conversations.list_labels", Value::Null)
            .await?
            .and_then(|body| serde_json::from_value(body["labels"].clone()).ok())
            .unwrap_or_default();
        Ok(Context {
            sentence,
            inboxes: inboxes
                .into_iter()
                .map(|row| InboxFacts {
                    teachable: matches!(row.role.as_deref(), Some("sender" | "manager")),
                    id: row.id,
                    address: row.address.to_lowercase(),
                    shared: row.shared,
                })
                .collect(),
            labels: labels
                .into_iter()
                .map(|row| LabelFacts {
                    id: row.id,
                    inbox: row.inbox,
                    name: row.name,
                })
                .collect(),
        })
    }

    /// Senders matching `who`'s words, replied to most first; the people
    /// replied to most when none match. Found in the database, not by the
    /// model.
    async fn candidates(&self, who: &mut WhoQuestion) -> Result<()> {
        let words: Vec<String> = who
            .phrase
            .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '.')
            .map(str::to_lowercase)
            .filter(|w| w.chars().count() >= 2 && !STOP_WORDS.contains(&w.as_str()))
            .take(3)
            .collect();
        let mut found: Vec<Candidate> = Vec::new();
        for word in &words {
            for candidate in self.senders(Some(word)).await? {
                if !found.iter().any(|c| c.address == candidate.address) {
                    found.push(candidate);
                }
            }
        }
        found.sort_by(|a, b| b.replied.cmp(&a.replied).then(b.received.cmp(&a.received)));
        who.matched = !found.is_empty();
        if found.is_empty() {
            found = self.senders(None).await?;
        }
        found.truncate(MAX_CANDIDATES);
        who.candidates = found;
        Ok(())
    }

    /// `conversations.list_senders`, once per `q` in a compile.
    async fn senders(&self, q: Option<&str>) -> Result<Vec<Candidate>> {
        let key = q.map(str::to_owned);
        let cached = self
            .senders
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&key)
            .cloned();
        if let Some(found) = cached {
            return Ok(found);
        }
        let input = match q {
            Some(q) => json!({ "q": q, "limit": MAX_CANDIDATES }),
            None => json!({ "limit": MAX_CANDIDATES }),
        };
        let found: Vec<Candidate> = self
            .call("conversations.list_senders", input)
            .await?
            .and_then(|body| serde_json::from_value(body["senders"].clone()).ok())
            .unwrap_or_default();
        self.senders
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(key, found.clone());
        Ok(found)
    }
}

#[cfg(test)]
mod tests;
