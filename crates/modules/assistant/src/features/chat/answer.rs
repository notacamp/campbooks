//! The question loop: the model answers, or calls tools whose results it
//! reads before answering; at most [`MAX_ROUNDS`] rounds of tool calls, then
//! once more without tools. Whole (one answer per call, the desktop) or
//! streamed (reasoning, steps and words as they come).
//!
//! What the model thinks and writes and what the tools read are data: they
//! reach the caller's answer and nothing else, never a log line, a span or a
//! metric. One log line per question says how it went, in sizes and names.

use std::time::{Duration, Instant};

use campbooks_ai::{
    ChatEvent, ChatRequest, ChatResponse, ChatStream, Gateway, Message, Role, ToolCall,
    usage::Scope,
};
use futures_util::StreamExt as _;

use super::{
    prompt,
    tools::Toolbox,
    types::{ChatFinish, ChatStep, ChatTool},
    words,
};

/// Rounds of tool calls a question may make; then one more call, without
/// tools.
pub const MAX_ROUNDS: usize = 6;
/// Longest a streamed model may go without a sign of life.
pub const IDLE: Duration = Duration::from_secs(30);
/// Longest a streamed question may take, all its rounds.
pub const TOTAL: Duration = Duration::from_mins(3);
/// Longest one whole call may take.
const CALL: Duration = Duration::from_secs(80);
/// Most characters of reasoning forwarded per question.
const MAX_THINKING_CHARS: usize = 8_000;
/// Most bytes of one round's words collected.
const MAX_COLLECTED_BYTES: usize = 64 * 1024;

/// Where a streamed question's progress goes.
pub trait Sink: Send {
    /// Some of the model's reasoning.
    fn thinking(&mut self, delta: &str);
    /// Some of the answer's words.
    fn text(&mut self, delta: &str);
    /// A tool starting.
    fn step(&mut self, tool: ChatTool);
}

/// A sink for whole answers: nothing goes out until the end.
pub struct Quiet;

impl Sink for Quiet {
    fn thinking(&mut self, _: &str) {}
    fn text(&mut self, _: &str) {}
    fn step(&mut self, _: ChatTool) {}
}

/// How the model is called.
#[derive(Debug, Clone, Copy)]
pub enum Mode {
    /// Whole answers, all of the question done by `until`.
    Whole {
        /// When the question must be done.
        until: Instant,
    },
    /// Streamed answers.
    Stream,
}

/// Scout's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// Its words, cleaned and bounded.
    pub text: String,
    /// What it looked at.
    pub steps: Vec<ChatStep>,
    /// `done` or `cut_short`.
    pub finish: ChatFinish,
}

/// Why a question has no answer.
#[derive(Debug)]
pub enum Failure {
    /// The workspace used its whole daily AI limit.
    OverCap(campbooks_ai::Error),
    /// The model failed, took too long, or answered nothing usable: a fixed
    /// word for the log.
    Failed(&'static str),
}

/// One model call's answer.
#[derive(Debug, Default)]
struct Turn {
    text: String,
    calls: Vec<ToolCall>,
    finish: &'static str,
}

/// One question being answered.
pub struct Asking {
    /// The gateway (`complete` role).
    pub gateway: Gateway,
    /// The share of the daily limit the calls count in.
    pub usage: Scope,
    /// The tools, acting for the member.
    pub toolbox: Toolbox,
    /// The conversation with the model so far.
    pub messages: Vec<Message>,
    /// For the log: model calls made.
    pub calls: usize,
    /// For the log: the tools that ran, in order.
    pub tools: Vec<&'static str>,
    /// For the log: why the last call stopped.
    pub finish: &'static str,
    /// For the log: tokens.
    pub prompt_tokens: u32,
    /// For the log: tokens.
    pub completion_tokens: u32,
    thinking_sent: usize,
    started: std::time::Instant,
}

impl Asking {
    /// A question over `messages`.
    pub fn new(gateway: Gateway, usage: Scope, toolbox: Toolbox, messages: Vec<Message>) -> Self {
        Self {
            gateway,
            usage,
            toolbox,
            messages,
            calls: 0,
            tools: Vec::new(),
            finish: "none",
            prompt_tokens: 0,
            completion_tokens: 0,
            thinking_sent: 0,
            started: Instant::now(),
        }
    }

    /// The first streamed call, opened before the stream's head so that a
    /// workspace past its cap, or a model refusing the request, is an
    /// ordinary problem.
    ///
    /// # Errors
    /// [`Failure`] when it could not be opened.
    pub async fn open(&mut self) -> Result<ChatStream, Failure> {
        let request = self.request(true, None);
        self.calls += 1;
        let opened = self
            .usage
            .clone()
            .run(tokio::time::timeout(
                IDLE,
                self.gateway.chat_stream(Role::Complete, request),
            ))
            .await;
        match opened {
            Ok(Ok(stream)) => Ok(stream),
            Ok(Err(error)) => Err(failure(error)),
            Err(_) => Err(Failure::Failed("timed_out")),
        }
    }

    /// Answer: the rounds, then the answer. `opened`, the first streamed
    /// call when it was opened already.
    ///
    /// # Errors
    /// [`Failure`] when there is no answer.
    pub async fn run(
        &mut self,
        mode: Mode,
        mut opened: Option<ChatStream>,
        sink: &mut dyn Sink,
    ) -> Result<Answer, Failure> {
        let mut steps: Vec<ChatStep> = Vec::new();
        for round in 0..=MAX_ROUNDS {
            let with_tools = round < MAX_ROUNDS;
            let turn = match mode {
                Mode::Whole { until } => self.whole(with_tools, until).await?,
                Mode::Stream => {
                    let stream = match opened.take() {
                        Some(stream) => stream,
                        None => self.reopen(with_tools).await?,
                    };
                    self.streamed(stream, sink).await?
                }
            };
            self.finish = turn.finish;
            if with_tools && !turn.calls.is_empty() {
                self.messages.push(Message::assistant_calling(
                    turn.text.clone(),
                    turn.calls.clone(),
                ));
                for call in &turn.calls {
                    if let Some(tool) = Toolbox::tool_for(call) {
                        sink.step(tool);
                    }
                    let outcome = self.toolbox.run(call).await;
                    if let Some(step) = outcome.step {
                        self.tools.push(step.tool.name());
                        steps.push(step);
                    } else {
                        self.tools.push(outcome.kind);
                    }
                    self.messages
                        .push(Message::tool_result(call.id.clone(), outcome.result));
                }
                continue;
            }
            let text = words::answer(&turn.text);
            if text.is_empty() {
                return Err(Failure::Failed(if turn.finish == "length" {
                    "truncated"
                } else {
                    "empty"
                }));
            }
            let finish = if turn.finish == "length" {
                ChatFinish::CutShort
            } else {
                ChatFinish::Done
            };
            return Ok(Answer {
                text,
                steps,
                finish,
            });
        }
        Err(Failure::Failed("rounds"))
    }

    /// The request for the next call.
    fn request(&self, with_tools: bool, timeout: Option<Duration>) -> ChatRequest {
        let mut request = prompt::request(&self.messages, with_tools);
        request.timeout = timeout.or(Some(TOTAL + Duration::from_secs(10)));
        request
    }

    /// A whole call, within what is left of the question's time.
    async fn whole(&mut self, with_tools: bool, until: Instant) -> Result<Turn, Failure> {
        let left = until.saturating_duration_since(Instant::now());
        if left < Duration::from_secs(5) {
            return Err(Failure::Failed("timed_out"));
        }
        let request = self.request(with_tools, Some(left.min(CALL)));
        self.calls += 1;
        let response: ChatResponse = self
            .usage
            .clone()
            .run(self.gateway.chat(Role::Complete, request))
            .await
            .map_err(failure)?;
        let usage = response.usage.unwrap_or_default();
        self.prompt_tokens += usage.prompt_tokens;
        self.completion_tokens += usage.completion_tokens;
        Ok(Turn {
            finish: response.finish_kind(),
            text: response.content,
            calls: response.tool_calls,
        })
    }

    /// A later streamed call.
    async fn reopen(&mut self, with_tools: bool) -> Result<ChatStream, Failure> {
        let left = TOTAL.saturating_sub(self.started.elapsed());
        let request = self.request(with_tools, None);
        self.calls += 1;
        match tokio::time::timeout(
            IDLE.min(left),
            self.usage
                .clone()
                .run(self.gateway.chat_stream(Role::Complete, request)),
        )
        .await
        {
            Ok(Ok(stream)) => Ok(stream),
            Ok(Err(error)) => Err(failure(error)),
            Err(_) => Err(Failure::Failed("timed_out")),
        }
    }

    /// Read one streamed call to its end, forwarding its reasoning and
    /// words.
    async fn streamed(
        &mut self,
        mut chat: ChatStream,
        sink: &mut dyn Sink,
    ) -> Result<Turn, Failure> {
        let mut turn = Turn::default();
        loop {
            let left = TOTAL.saturating_sub(self.started.elapsed());
            if left.is_zero() {
                return Err(Failure::Failed("timed_out"));
            }
            let Ok(next) = tokio::time::timeout(IDLE.min(left), chat.next()).await else {
                return Err(Failure::Failed("timed_out"));
            };
            match next {
                None => return Err(Failure::Failed("provider")),
                Some(Err(error)) => return Err(failure(error)),
                Some(Ok(ChatEvent::Reasoning(delta))) => {
                    let room = MAX_THINKING_CHARS.saturating_sub(self.thinking_sent);
                    let kept: String = words::kept(&delta).chars().take(room).collect();
                    if !kept.is_empty() {
                        self.thinking_sent += kept.chars().count();
                        sink.thinking(&kept);
                    }
                }
                Some(Ok(ChatEvent::Text(delta))) => {
                    if turn.text.len() < MAX_COLLECTED_BYTES {
                        turn.text.push_str(&delta);
                    }
                    let kept = words::kept(&delta);
                    if !kept.is_empty() {
                        sink.text(&kept);
                    }
                }
                Some(Ok(ChatEvent::Alive)) => {}
                Some(Ok(ChatEvent::ToolCall(call))) => turn.calls.push(call),
                Some(Ok(ChatEvent::Done(summary))) => {
                    let usage = summary.usage.unwrap_or_default();
                    turn.finish = summary.finish_kind();
                    self.prompt_tokens += usage.prompt_tokens;
                    self.completion_tokens += usage.completion_tokens;
                    return Ok(turn);
                }
            }
        }
    }

    /// Milliseconds since the question started.
    pub fn elapsed_ms(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

/// A gateway error as a failure: the cap is said as such, anything else by
/// kind only (a provider's error can echo the request).
fn failure(error: campbooks_ai::Error) -> Failure {
    match error {
        error @ campbooks_ai::Error::OverDailyCap { .. } => Failure::OverCap(error),
        other => Failure::Failed(error_kind(&other)),
    }
}

/// What kind of failure, for the log: never the provider's words.
pub fn error_kind(error: &campbooks_ai::Error) -> &'static str {
    match error {
        campbooks_ai::Error::RoleNotConfigured(_) => "role_not_configured",
        campbooks_ai::Error::ExternalContentRefused { .. } => "external_content_refused",
        campbooks_ai::Error::Transport(e) if e.is_timeout() => "timeout",
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
