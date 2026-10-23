//! `…/assistant/chats` — Scout's chat (campbooks-cloud#305: "Ask about
//! this"). A member asks Scout about what they are looking at; Scout
//! explains and looks things up, as that member, and may propose changes
//! the member confirms on a card ([`propose`]). The server never makes one:
//! the member's own request does.
//!
//! A chat belongs to one member and is about one record of a registered
//! type ([`about`]: a conversation, a rule, one of Scout's suggestions) or
//! nothing (asked from the command menu; always a new chat). The member's
//! chats are listed by their first question, newest first. Each answer is written by the `complete`
//! role, through the provider that may receive mail content, from a fixed
//! instruction and the handbook ([`crate::handbook`]), the question's
//! context and what the chat is about (read again as the member for every
//! question), the chat so far, read-only [`tools`] each backed by a
//! catalogue operation the assistant may call, and the proposing tools.
//! Every answer carries the steps Scout took, built by the server from the
//! tools that ran, and the proposals it made, each checked as the member
//! and built from the catalogue and the record's own facts.
//!
//! Only on the member's own request: none of these operations is open to
//! the assistant or automation, and a client token acting for nobody gets
//! no chat. Logs say how a question went in sizes and names, never what
//! was asked, read or written.

mod about;
mod answer;
mod member;
mod project;
mod prompt;
mod propose;
mod store;
mod tools;
mod types;
mod words;

#[cfg(test)]
mod tests;

use std::{
    convert::Infallible,
    time::{Duration, Instant},
};

use axum::{
    Extension, Json,
    extract::State,
    http::{HeaderValue, StatusCode},
    response::{
        IntoResponse as _, Response,
        sse::{Event, KeepAlive, Sse},
    },
};
use campbooks_ai::{Gateway, Role, usage::Scope};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    authz::{self, Action},
    db::BeginFor as _,
    extract,
    i18n::Locale,
    rate_limit::{Limit, RateKey},
    tenancy::{UserId, Workspace, WorkspaceId},
    time_limit::Deadline,
};
use futures_util::Stream;
use serde::Deserialize;
use tokio::sync::mpsc;

use self::types::{
    AskChat, ChatFinish, ChatId, ChatList, ChatMessageId, ChatMessageView, ChatProposal, ChatTool,
    ChatView, ProposalId, SettleProposal, StartChat,
};
use self::{
    about::Seen,
    answer::{Answer, Asking, Failure, Mode, Quiet, Sink},
    member::{Member, Unread},
    prompt::Now,
    store::{ChatRow, MAX_QUESTIONS},
    tools::Toolbox,
};
use crate::{
    domain::resources::{self, CHAT_RECORD},
    handbook::Handbook,
    scopes::{AssistantRead, AssistantWrite},
};

/// Questions per member and workspace per hour.
const LIMIT: Limit = Limit::new("assistant.chat", 30, Duration::from_hours(1));
/// The usage scope's feature: every model call of a question.
const FEATURE: &str = "assistant.chat";
/// How long a whole question may run, past the server's default limit.
const ASK_ROUTE_LIMIT: Duration = Duration::from_secs(100);
/// How long a whole question's model calls may take, all of them: less
/// than the route's limit, so a slow model is answered `chat_failed`.
const ASK_BUDGET: Duration = Duration::from_secs(92);
/// How often a comment keeps a stream open.
const KEEP_ALIVE: Duration = Duration::from_secs(10);
/// Longest time zone name taken.
const MAX_ZONE_CHARS: usize = 64;

/// What chats are answered with: the AI gateway, when one is composed, and
/// the handbook the server composed.
#[derive(Debug, Clone)]
pub struct Chatter {
    ai: Option<Gateway>,
    handbook: Handbook,
}

impl Chatter {
    /// Answering through `ai`'s `complete` role, from `handbook`.
    pub fn new(ai: Option<Gateway>, handbook: Handbook) -> Self {
        Self { ai, handbook }
    }

    /// The gateway, if its `complete` role is configured and may receive
    /// mail; otherwise 409 `chat_unavailable`, never the gateway's words.
    fn gateway(&self) -> Result<&Gateway> {
        self.ai
            .as_ref()
            .filter(|gateway| {
                gateway
                    .config()
                    .provider_for_content(Role::Complete)
                    .is_ok()
            })
            .ok_or_else(|| Error::Conflict(crate::MESSAGES.reason("chat_unavailable")))
    }
}

/// `…/assistant/chats/{chat_id}`.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct ChatPath {
    /// The chat.
    pub chat_id: ChatId,
}

fn needs_member() -> Error {
    Error::Denied(crate::MESSAGES.reason("chat_needs_member"))
}

/// `…/assistant/chats/{chat_id}/proposals/{proposal_id}`.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct ProposalPath {
    /// The chat.
    pub chat_id: ChatId,
    /// The proposal.
    pub proposal_id: ProposalId,
}

/// The member a chat operation acts for: a client token acting for nobody
/// gets 403 `chat_needs_member`.
fn caller(workspace_access: &campbooks_kernel::tenancy::Access) -> Result<UserId> {
    workspace_access.user.ok_or_else(needs_member)
}

/// Start a chat with Scout, or continue yours about the same record.
/// Without `about`, a new chat about nothing in particular, every time.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/assistant/chats",
    operation_id = "assistant.start_chat",
    tag = "assistant",
    security(("bearer" = ["assistant:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).records(["assistant.chat"]).access("assistant.chat", campbooks_kernel::authz::Action::Create)))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    request_body = StartChat,
    responses(
        (status = OK, description = "Your chat about that record, continued.", body = ChatView),
        (status = CREATED, description = "A new chat.", body = ChatView),
        (status = BAD_REQUEST, description = "Scout cannot chat about that type of record (`about_unsupported`).",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or not a member (`chat_needs_member`).",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such workspace or record, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "No language model may read mail here (`chat_unavailable`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn start(
    State(state): State<AppState>,
    Extension(chatter): Extension<Chatter>,
    workspace: Workspace<AssistantWrite>,
    extract::Json(body): extract::Json<StartChat>,
) -> Result<(StatusCode, Json<ChatView>)> {
    authz::require_type(
        &state,
        &workspace.access,
        Action::Create,
        CHAT_RECORD,
        needs_member,
    )?;
    let user = caller(&workspace.access)?;
    if let Some(about) = &body.about
        && about::entry(&about.record_type).is_none()
    {
        return Err(Error::BadRequest(
            crate::MESSAGES.reason("about_unsupported"),
        ));
    }
    chatter.gateway()?;
    if let Some(about) = &body.about {
        let member = Member {
            state: state.clone(),
            principal: workspace.principal.clone(),
            workspace: workspace.id,
        };
        about::check(&member, about)
            .await
            .map_err(|unread| match unread {
                Unread::NotVisible | Unread::Refused => Error::NotFound,
                Unread::Broken(kind) => Error::internal(format!("about read failed: {kind}")),
            })?;
    }
    let mut tx = state.db.begin_for(workspace.id).await?;
    let existing = match &body.about {
        Some(about) => store::chat_about(&mut tx, user, about).await?,
        None => None,
    };
    let (status, row) = if let Some(row) = existing {
        (StatusCode::OK, row)
    } else if let Some(row) =
        store::create(&mut tx, workspace.id, user, body.about.as_ref()).await?
    {
        (StatusCode::CREATED, row)
    } else {
        // Made meanwhile, by the same member in another tab.
        let about = body.about.as_ref().ok_or(Error::NotFound)?;
        let row = store::chat_about(&mut tx, user, about)
            .await?
            .ok_or(Error::NotFound)?;
        (StatusCode::OK, row)
    };
    let view = store::view(&mut tx, &row).await?;
    tx.commit().await?;
    Ok((status, Json(view)))
}

/// Your chats with Scout, the newest message first: at most 30, each with
/// your first question and when its last message was written. A chat with
/// no question yet is not listed. What a chat is about is never read again
/// here: open it to see whether Scout still can.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/assistant/chats",
    operation_id = "assistant.list_chats",
    tag = "assistant",
    security(("bearer" = ["assistant:read"])),
    extensions(("x-campbooks" = json!(Operation::query().records(["assistant.chat"]).access("assistant.chat", campbooks_kernel::authz::Action::Read)))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    responses(
        (status = OK, description = "Your chats.", body = ChatList),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or not a member (`chat_needs_member`).",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such workspace.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn list(
    State(state): State<AppState>,
    workspace: Workspace<AssistantRead>,
) -> Result<Json<ChatList>> {
    authz::require_type(
        &state,
        &workspace.access,
        Action::Read,
        CHAT_RECORD,
        needs_member,
    )?;
    let user = caller(&workspace.access)?;
    let mut tx = state.db.begin_for(workspace.id).await?;
    let chats = store::list(&mut tx, user).await?;
    tx.commit().await?;
    Ok(Json(ChatList { chats }))
}

/// Your chat, what it is about, and its messages, oldest first.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/assistant/chats/{chat_id}",
    operation_id = "assistant.get_chat",
    tag = "assistant",
    security(("bearer" = ["assistant:read"])),
    extensions(("x-campbooks" = json!(Operation::query().records(["assistant.chat"]).access("assistant.chat", campbooks_kernel::authz::Action::Read)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("chat_id" = ChatId, Path, description = "The chat."),
    ),
    responses(
        (status = OK, description = "The chat.", body = ChatView),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such chat, or it is not yours.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn get(
    State(state): State<AppState>,
    workspace: Workspace<AssistantRead>,
    extract::Path(path): extract::Path<ChatPath>,
) -> Result<Json<ChatView>> {
    let mut tx = state.db.begin_for(workspace.id).await?;
    authz::require(
        &state,
        &mut tx,
        &workspace.access,
        Action::Read,
        &resources::chat(&path.chat_id),
        needs_member,
    )
    .await?;
    let row = store::chat(&mut tx, path.chat_id)
        .await?
        .ok_or(Error::NotFound)?;
    let view = store::view(&mut tx, &row).await?;
    tx.commit().await?;
    Ok(Json(view))
}

/// Delete your chat and its messages. There is no undo.
#[utoipa::path(
    delete,
    path = "/api/v1/w/{workspace_id}/assistant/chats/{chat_id}",
    operation_id = "assistant.delete_chat",
    tag = "assistant",
    security(("bearer" = ["assistant:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).records(["assistant.chat"]).access("assistant.chat", campbooks_kernel::authz::Action::Delete)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("chat_id" = ChatId, Path, description = "The chat."),
    ),
    responses(
        (status = NO_CONTENT, description = "Deleted."),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such chat, or it is not yours.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn delete(
    State(state): State<AppState>,
    workspace: Workspace<AssistantWrite>,
    extract::Path(path): extract::Path<ChatPath>,
) -> Result<StatusCode> {
    let mut tx = state.db.begin_for(workspace.id).await?;
    authz::require(
        &state,
        &mut tx,
        &workspace.access,
        Action::Delete,
        &resources::chat(&path.chat_id),
        needs_member,
    )
    .await?;
    store::delete(&mut tx, path.chat_id).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Say what you did with one of Scout's proposals: `done`, `declined` or
/// `failed` for a pending card, `undone` for a done one.
///
/// It records only: the change itself was your own request, sent by your
/// app. Scout reads what you did in the chat's next answers.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/assistant/chats/{chat_id}/proposals/{proposal_id}",
    operation_id = "assistant.settle_proposal",
    tag = "assistant",
    security(("bearer" = ["assistant:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).records(["assistant.chat"]).access("assistant.chat", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("chat_id" = ChatId, Path, description = "The chat."),
        ("proposal_id" = ProposalId, Path, description = "The proposal."),
    ),
    request_body = SettleProposal,
    responses(
        (status = OK, description = "The proposal, settled.", body = ChatProposal),
        (status = BAD_REQUEST, description = "Not a state a card is settled with.", body = Problem,
            content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such chat or proposal, or it is not yours.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "It cannot become that state (`proposal_settled`): \
            only a pending card is settled, and only a done one undone.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn settle(
    State(state): State<AppState>,
    workspace: Workspace<AssistantWrite>,
    extract::Path(path): extract::Path<ProposalPath>,
    extract::Json(body): extract::Json<SettleProposal>,
) -> Result<Json<ChatProposal>> {
    let mut tx = state.db.begin_for(workspace.id).await?;
    authz::require(
        &state,
        &mut tx,
        &workspace.access,
        Action::Write,
        &resources::chat(&path.chat_id),
        needs_member,
    )
    .await?;
    let (message, mut proposals) =
        store::proposals_for_update(&mut tx, path.chat_id, path.proposal_id)
            .await?
            .ok_or(Error::NotFound)?;
    let proposal = proposals
        .iter_mut()
        .find(|proposal| proposal.id == path.proposal_id)
        .ok_or(Error::NotFound)?;
    if !proposal.state.may_become(body.state) {
        return Err(Error::Conflict(crate::MESSAGES.reason("proposal_settled")));
    }
    proposal.state = body.state.into();
    let settled = proposal.clone();
    store::set_proposals(&mut tx, message, &proposals).await?;
    tx.commit().await?;
    tracing::info!(
        kind = ?settled.kind,
        state = ?settled.state,
        "chat proposal settled"
    );
    Ok(Json(settled))
}

/// Ask Scout, and get its answer whole (the desktop app, where answers
/// don't stream).
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/assistant/chats/{chat_id}/questions",
    operation_id = "assistant.ask_chat",
    tag = "assistant",
    security(("bearer" = ["assistant:write"])),
    // A command: it stores the question and the answer, and spends model
    // calls and the member's hourly allowance.
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).records(["assistant.chat"]).access("assistant.chat", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("chat_id" = ChatId, Path, description = "The chat."),
    ),
    request_body = AskChat,
    responses(
        (status = OK, description = "Scout's answer, as stored.", body = ChatMessageView),
        (status = BAD_REQUEST, description = "An empty question (`question_empty`) or one over 2,000 characters (`question_too_long`).",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such chat, or it is not yours.",
            body = Problem, content_type = "application/problem+json"),
        (status = TOO_MANY_REQUESTS, description = "Too many questions this hour.", body = Problem,
            content_type = "application/problem+json"),
        (status = CONFLICT, description = "The chat holds 40 questions already (`chat_full`): start over; \
            or no language model may read mail here (`chat_unavailable`).",
            body = Problem, content_type = "application/problem+json"),
        (status = SERVICE_UNAVAILABLE, description = "Scout couldn't answer (`chat_failed`), \
            or the workspace used its whole daily AI limit (`ai_daily_limit_reached`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn ask(
    State(state): State<AppState>,
    Extension(chatter): Extension<Chatter>,
    workspace: Workspace<AssistantWrite>,
    locale: Locale,
    deadline: Option<Extension<Deadline>>,
    extract::Path(path): extract::Path<ChatPath>,
    extract::Json(body): extract::Json<AskChat>,
) -> Result<Json<ChatMessageView>> {
    allow(deadline.as_ref(), ASK_ROUTE_LIMIT);
    let until = Instant::now() + ASK_BUDGET;
    let mut question =
        Question::prepare(&state, &chatter, &workspace, path.chat_id, &body, locale).await?;
    let outcome = question
        .asking
        .run(Mode::Whole { until }, None, &mut Quiet)
        .await;
    let outcome = match outcome {
        // Past the cap before anything was answered: nothing is stored.
        Err(Failure::OverCap(error)) if question.asking.calls <= 1 => {
            question.report("over_cap", "over_cap", false);
            return Err(error.into());
        }
        other => other,
    };
    let answer_id = question.store_question().await?;
    match outcome {
        Ok(answer) => {
            let view = question.store_answer(answer_id, &answer).await?;
            question.report(answer.finish.as_str(), "none", false);
            Ok(Json(view))
        }
        Err(failure) => {
            question.report("failed", failure.kind(), false);
            question.store_failed(answer_id).await?;
            Err(problem_of(failure))
        }
    }
}

/// Ask Scout, and get its answer as it is written.
///
/// The same request as `assistant.ask_chat`, answered as Server-Sent
/// Events once every check passed (a refusal is an ordinary problem
/// answer). Events, each `data` a JSON object: `thinking` (`{"delta"}`:
/// Scout's reasoning as it arrives, at most 8,000 characters in all),
/// `step` (`{"tool"}`: a tool starting), `text` (`{"delta"}`: the answer's
/// words as they arrive; words before a `step` are not part of the answer),
/// then exactly one of `done` (the stored Scout message, as `ask_chat`
/// answers it) or `error` (a problem: `chat_failed` or
/// `ai_daily_limit_reached`). A comment every 10 seconds keeps the
/// connection open. The question is stored when the stream opens; a
/// stream that ends before `done` leaves Scout's message stored as
/// `failed`, with no text.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/assistant/chats/{chat_id}/questions/stream",
    operation_id = "assistant.stream_chat",
    tag = "assistant",
    security(("bearer" = ["assistant:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).records(["assistant.chat"]).access("assistant.chat", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("chat_id" = ChatId, Path, description = "The chat."),
    ),
    request_body = AskChat,
    responses(
        (status = OK, description = "The answer as it is written: `thinking`, `step`, `text`, then `done` or `error`.",
            content_type = "text/event-stream", body = String),
        (status = BAD_REQUEST, description = "An empty question (`question_empty`) or one over 2,000 characters (`question_too_long`).",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such chat, or it is not yours.",
            body = Problem, content_type = "application/problem+json"),
        (status = TOO_MANY_REQUESTS, description = "Too many questions this hour.", body = Problem,
            content_type = "application/problem+json"),
        (status = CONFLICT, description = "The chat holds 40 questions already (`chat_full`): start over; \
            or no language model may read mail here (`chat_unavailable`).",
            body = Problem, content_type = "application/problem+json"),
        (status = SERVICE_UNAVAILABLE, description = "Scout couldn't answer (`chat_failed`), \
            or the workspace used its whole daily AI limit (`ai_daily_limit_reached`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn stream(
    State(state): State<AppState>,
    Extension(chatter): Extension<Chatter>,
    workspace: Workspace<AssistantWrite>,
    locale: Locale,
    deadline: Option<Extension<Deadline>>,
    extract::Path(path): extract::Path<ChatPath>,
    extract::Json(body): extract::Json<AskChat>,
) -> Result<Response> {
    // The head waits for the model's first byte at most `IDLE`, after the
    // reads (given up to 30 s more); the body has its own limits.
    allow(deadline.as_ref(), answer::IDLE + Duration::from_secs(30));
    let mut question =
        Question::prepare(&state, &chatter, &workspace, path.chat_id, &body, locale).await?;
    let opened = match question.asking.open().await {
        Ok(opened) => opened,
        Err(Failure::OverCap(error)) => {
            question.report("over_cap", "over_cap", true);
            return Err(error.into());
        }
        Err(failure) => {
            question.report("failed", failure.kind(), true);
            let answer_id = question.store_question().await?;
            question.store_failed(answer_id).await?;
            return Err(problem_of(failure));
        }
    };
    let answer_id = question.store_question().await?;
    let (frames, rx) = mpsc::unbounded_channel::<Event>();
    let work = async move {
        let mut sink = Frames(frames.clone());
        if let Some(line) = question.lost_line() {
            sink.text(&format!("{line}\n\n"));
        }
        let outcome = question
            .asking
            .run(Mode::Stream, Some(opened), &mut sink)
            .await;
        let last = match outcome {
            Ok(answer) => match question.store_answer(answer_id, &answer).await {
                Ok(view) => {
                    question.report(answer.finish.as_str(), "none", true);
                    Event::default()
                        .event("done")
                        .data(serde_json::to_string(&view).unwrap_or_default())
                }
                Err(error) => {
                    tracing::warn!(error = error.code(), "chat: the answer could not be stored");
                    question.report("failed", "store", true);
                    error_event(
                        &Error::Unavailable(crate::MESSAGES.reason("chat_failed")),
                        locale,
                    )
                }
            },
            Err(failure) => {
                question.report("failed", failure.kind(), true);
                if let Err(error) = question.store_failed(answer_id).await {
                    tracing::warn!(
                        error = error.code(),
                        "chat: the failure could not be stored"
                    );
                }
                error_event(&problem_of(failure), locale)
            }
        };
        let _ = frames.send(last);
    };
    let mut response = Sse::new(frames_until_done(work, rx))
        .keep_alive(KeepAlive::new().interval(KEEP_ALIVE))
        .into_response();
    // Tell buffering reverse proxies (nginx) to pass frames through at once.
    response
        .headers_mut()
        .insert("x-accel-buffering", HeaderValue::from_static("no"));
    Ok(response)
}

/// The SSE events `work` sends through `rx`, then the end. Dropping the
/// stream (the caller hung up) drops `work` with it, and the model's stream
/// inside it, which stops and settles the call.
fn frames_until_done<W>(
    work: W,
    rx: mpsc::UnboundedReceiver<Event>,
) -> impl Stream<Item = std::result::Result<Event, Infallible>> + Send + 'static
where
    W: std::future::Future<Output = ()> + Send + 'static,
{
    let state = (Box::pin(work), false, rx);
    futures_util::stream::unfold(state, |(mut work, mut finished, mut rx)| async move {
        loop {
            if finished {
                return rx
                    .recv()
                    .await
                    .map(|event| (Ok(event), (work, finished, rx)));
            }
            tokio::select! {
                biased;
                Some(event) = rx.recv() => return Some((Ok(event), (work, finished, rx))),
                () = &mut work => finished = true,
            }
        }
    })
}

/// A streamed question's sink: frames to the caller.
struct Frames(mpsc::UnboundedSender<Event>);

impl Sink for Frames {
    fn thinking(&mut self, delta: &str) {
        let _ = self.0.send(delta_event("thinking", delta));
    }

    fn text(&mut self, delta: &str) {
        let _ = self.0.send(delta_event("text", delta));
    }

    fn step(&mut self, tool: ChatTool) {
        let _ = self.0.send(
            Event::default()
                .event("step")
                .data(serde_json::json!({ "tool": tool }).to_string()),
        );
    }
}

/// A `thinking` or `text` event carrying `delta`.
fn delta_event(name: &'static str, delta: &str) -> Event {
    Event::default()
        .event(name)
        .data(serde_json::json!({ "delta": delta }).to_string())
}

/// An `error` event carrying `error`'s problem, in `locale`.
fn error_event(error: &Error, locale: Locale) -> Event {
    Event::default()
        .event("error")
        .data(serde_json::to_string(&error.to_problem(locale)).unwrap_or_default())
}

/// The problem for a question with no answer.
fn problem_of(failure: Failure) -> Error {
    match failure {
        Failure::OverCap(error) => error.into(),
        Failure::Failed(_) => Error::Unavailable(crate::MESSAGES.reason("chat_failed")),
    }
}

/// Let the request run `limit` past the server's default, when it has a
/// deadline (the server's routes; not a bare module in tests).
fn allow(deadline: Option<&Extension<Deadline>>, limit: Duration) {
    if let Some(Extension(deadline)) = deadline {
        deadline.allow(limit);
    }
}

/// One question, checked and ready for the model.
struct Question {
    state: AppState,
    workspace: WorkspaceId,
    chat: ChatId,
    text: String,
    locale: Locale,
    /// The line an answer starts with, when Scout can no longer see what
    /// the chat is about (its message key).
    lost: Option<&'static str>,
    /// The type of record the chat is about, or `none`: logged, never
    /// the record itself.
    about: &'static str,
    handbook_version: String,
    asking: Asking,
}

impl Question {
    /// Check the caller, the chat and the question, read what the chat is
    /// about as the member, and build the model's conversation: shared by
    /// `ask_chat` and `stream_chat`. Nothing is stored and no model is
    /// called.
    async fn prepare(
        state: &AppState,
        chatter: &Chatter,
        workspace: &Workspace<AssistantWrite>,
        chat: ChatId,
        body: &AskChat,
        locale: Locale,
    ) -> Result<Self> {
        let access = &workspace.access;
        let row = {
            let mut tx = state.db.begin_for(workspace.id).await?;
            authz::require(
                state,
                &mut tx,
                access,
                Action::Write,
                &resources::chat(&chat),
                needs_member,
            )
            .await?;
            let row = store::chat(&mut tx, chat).await?.ok_or(Error::NotFound)?;
            tx.commit().await?;
            row
        };
        let text = words::question(&body.text).map_err(|refusal| {
            Error::BadRequest(
                crate::MESSAGES
                    .reason(refusal.code())
                    .arg("max", words::MAX_QUESTION_CHARS),
            )
        })?;
        if row.questions() >= MAX_QUESTIONS {
            return Err(Error::Conflict(
                crate::MESSAGES
                    .reason("chat_full")
                    .arg("max", MAX_QUESTIONS),
            ));
        }
        let gateway = chatter.gateway()?.clone();
        let user = caller(access)?;
        state
            .rate_limits
            .check(&state.db, &LIMIT, &rate_key(workspace.id, user))
            .await?;
        let (transcript, now, utc_offset) = {
            let mut tx = state.db.begin_for(workspace.id).await?;
            let transcript = store::transcript(&mut tx, chat).await?;
            let (now, offset) = local_now(&mut tx, body.time_zone.as_deref()).await?;
            tx.commit().await?;
            (transcript, now, offset)
        };
        let member = Member {
            state: state.clone(),
            principal: workspace.principal.clone(),
            workspace: workspace.id,
        };
        let seen = match row.about() {
            Some(about) => Some(about::read(&member, &about).await.unwrap_or_else(|unread| {
                // Not an access answer: the read itself failed. Answer
                // without it, as for a record no longer seen.
                tracing::warn!(
                    error = unread.kind(),
                    "chat: what the chat is about could not be read"
                );
                Seen::Lost
            })),
            None => None,
        };
        let lost = match (&seen, row.about()) {
            (Some(Seen::Lost), Some(about)) => Some(
                about::entry(&about.record_type).map_or("chat.about_lost", |entry| entry.lost_line),
            ),
            _ => None,
        };
        let messages = prompt::messages(
            &chatter.handbook,
            &now,
            locale,
            seen.as_ref(),
            &transcript,
            &text,
        );
        let usage = Scope::workspace(state.db.clone(), workspace.id, FEATURE).interactive();
        Ok(Self {
            state: state.clone(),
            workspace: workspace.id,
            chat: row.id,
            text,
            locale,
            lost,
            about: row
                .about()
                .and_then(|about| about::entry(&about.record_type))
                .map_or("none", |entry| entry.record_type),
            handbook_version: chatter.handbook.version(),
            asking: Asking::new(gateway, usage, Toolbox::new(member, utc_offset), messages),
        })
    }

    /// The fixed line an answer starts with when Scout can no longer see
    /// what the chat is about.
    fn lost_line(&self) -> Option<String> {
        self.lost
            .map(|key| crate::MESSAGES.t(self.locale, key, &[]))
    }

    /// Store the question and Scout's answer to come (as `failed`, until
    /// written); the chat must still have room. The answer's id.
    async fn store_question(&self) -> Result<ChatMessageId> {
        let mut tx = self.state.db.begin_for(self.workspace).await?;
        let row: ChatRow = store::chat_for_update(&mut tx, self.chat).await?;
        if row.questions() >= MAX_QUESTIONS {
            return Err(Error::Conflict(
                crate::MESSAGES
                    .reason("chat_full")
                    .arg("max", MAX_QUESTIONS),
            ));
        }
        let answer = store::ask(&mut tx, self.workspace, self.chat, &self.text).await?;
        tx.commit().await?;
        Ok(answer)
    }

    /// Write Scout's answer, the fixed line first when it can no longer see
    /// what the chat is about.
    async fn store_answer(
        &self,
        answer_id: ChatMessageId,
        answer: &Answer,
    ) -> Result<ChatMessageView> {
        let text = match self.lost_line() {
            Some(line) => format!("{line}\n\n{}", answer.text),
            None => answer.text.clone(),
        };
        let mut tx = self.state.db.begin_for(self.workspace).await?;
        let view = store::finish(
            &mut tx,
            self.chat,
            answer_id,
            &text,
            &answer.steps,
            &self.asking.toolbox.proposer.proposals,
            answer.finish,
        )
        .await?
        .ok_or(Error::NotFound)?;
        tx.commit().await?;
        Ok(view)
    }

    /// Keep Scout's answer as failed, with no text.
    async fn store_failed(&self, answer_id: ChatMessageId) -> Result<()> {
        let mut tx = self.state.db.begin_for(self.workspace).await?;
        store::finish(
            &mut tx,
            self.chat,
            answer_id,
            "",
            &[],
            &[],
            ChatFinish::Failed,
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// One log line per question: how it went, in sizes and names only.
    fn report(&self, outcome: &'static str, reason: &'static str, streamed: bool) {
        tracing::info!(
            outcome,
            reason,
            streamed,
            model_calls = self.asking.calls,
            tools = %self.asking.tools.join(","),
            proposals = self.asking.toolbox.proposer.proposals.len(),
            refusals = %self.asking.toolbox.proposer.refusals.join(","),
            result_bytes = self.asking.toolbox.used,
            finish_reason = self.asking.finish,
            prompt_tokens = self.asking.prompt_tokens,
            completion_tokens = self.asking.completion_tokens,
            question_chars = self.text.chars().count(),
            about = self.about,
            about_lost = self.lost.is_some(),
            handbook = %self.handbook_version,
            elapsed_ms = self.asking.elapsed_ms(),
            "chat question"
        );
    }
}

/// One counter per member and workspace.
fn rate_key(workspace: WorkspaceId, user: UserId) -> RateKey {
    RateKey::text(&format!("{workspace}:{user}"))
}

/// The current time, and the member's date, time and weekday in
/// `time_zone` when the database knows it (else UTC), with its offset from
/// UTC in minutes.
async fn local_now(conn: &mut sqlx::PgConnection, time_zone: Option<&str>) -> Result<(Now, i32)> {
    let asked = time_zone
        .map(str::trim)
        .filter(|zone| !zone.is_empty() && zone.chars().count() <= MAX_ZONE_CHARS);
    let known = match asked {
        Some(zone) => sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM pg_timezone_names WHERE name = $1)",
        )
        .bind(zone)
        .fetch_one(&mut *conn)
        .await?
        .then_some(zone),
        None => None,
    };
    let zone = known.unwrap_or("UTC");
    let (utc, local_date, local_time, weekday, offset): (String, String, String, String, i32) =
        sqlx::query_as(
            "SELECT to_char(now() AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"'), \
                    to_char(now() AT TIME ZONE $1, 'YYYY-MM-DD'), \
                    to_char(now() AT TIME ZONE $1, 'HH24:MI'), \
                    trim(to_char(now() AT TIME ZONE $1, 'Day')), \
                    round(extract(epoch FROM (now() AT TIME ZONE $1) - (now() AT TIME ZONE 'UTC')) \
                          / 60)::int",
        )
        .bind(zone)
        .fetch_one(&mut *conn)
        .await?;
    Ok((
        Now {
            utc,
            time_zone: zone.to_owned(),
            local_date,
            local_time,
            weekday,
        },
        offset,
    ))
}
