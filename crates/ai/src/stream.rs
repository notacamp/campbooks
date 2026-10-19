//! Streamed chat completions (campbooks-cloud#288): the model's reasoning
//! and its answer as they are written, for a person waiting on them.
//!
//! [`Gateway::chat_stream`] posts the request with `stream: true` to an
//! OpenAI-compatible provider and reads its Server-Sent Events: each chunk's
//! `delta.content` is answer text; its reasoning comes from
//! `delta.reasoning`, else `delta.reasoning_content` (`vLLM`, `llama.cpp`,
//! `DeepSeek`), else the text of `delta.reasoning_details` entries (encrypted
//! ones are skipped). Answer text that starts with `<think>` is read as
//! reasoning up to `</think>`, only at the very start: text hygiene for a
//! self-hosted server without a reasoning parser, not a decision. A server
//! that ignores `stream` and answers JSON is read as one text event.
//!
//! The call is admitted against the daily cap (inside the caller's usage
//! scope) before anything is sent, and settled once however its stream
//! ends: with its usage after the last event, as failed after an error, and
//! as failed and `cancelled` when the stream is dropped before its end.
//! Each stream logs one line when it ends, with times, sizes and token
//! counts, never any content: the mail, the reasoning and the answer are
//! data.

use std::{
    collections::VecDeque,
    fmt,
    pin::Pin,
    task::{Context, Poll},
    time::{Duration, Instant},
};

use futures_util::{Stream, StreamExt as _};
use serde::Deserialize;
use tracing::Instrument as _;

use crate::{
    Error, Protocol, ProviderConfig, Role, Usage,
    gateway::{ChatRequest, Gateway, WireChatRequest, WireChatResponse, finish_kind},
    telemetry::Call,
    usage::Ticket,
};

/// Most bytes of the stream held while a line or an event is incomplete.
const MAX_BUFFER: usize = 1024 * 1024;

/// Most bytes of answer text held at the start while it may still open
/// `<think>` (blank lines before it, say): past this it is the answer.
const MAX_THINK_LEAD: usize = 64;

/// What a streamed call yields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatEvent {
    /// Some of the model's reasoning, as it thinks.
    Reasoning(String),
    /// Some of its answer, as it writes.
    Text(String),
    /// The model is at work but sent nothing to show (an encrypted
    /// reasoning chunk, an empty delta): a sign of life only.
    Alive,
    /// The end: why it stopped and what it cost. Nothing follows.
    Done(ChatSummary),
}

/// How a streamed call ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatSummary {
    /// The model that answered, as the provider reported it.
    pub model: String,
    /// Why it stopped (`stop`, `length`, …), as the provider said it.
    pub finish_reason: Option<String>,
    /// Token usage, when the provider reported it.
    pub usage: Option<Usage>,
    /// Time from the request to its first reasoning or answer text.
    pub ttft: Option<Duration>,
}

impl ChatSummary {
    /// Why the model stopped, for a log line: one of a few known words,
    /// never whatever string the provider sent.
    #[must_use]
    pub fn finish_kind(&self) -> &'static str {
        finish_kind(self.finish_reason.as_deref())
    }
}

/// The events of a streamed call. Dropping it before its end stops the call
/// (the connection closes) and settles it as cancelled.
pub struct ChatStream(Pin<Box<dyn Stream<Item = Result<ChatEvent, Error>> + Send>>);

impl fmt::Debug for ChatStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChatStream").finish_non_exhaustive()
    }
}

impl Stream for ChatStream {
    type Item = Result<ChatEvent, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.0.as_mut().poll_next(cx)
    }
}

impl Gateway {
    /// Run a chat completion with the provider configured for `role`,
    /// streamed: its reasoning and its answer come as they are written,
    /// then [`ChatEvent::Done`]. The same checks as [`Gateway::chat`] come
    /// first (a provider off the instance's network gets nothing unless the
    /// role opted in; the daily cap), and a provider that refuses the
    /// request (a non-success status) fails here, before any event.
    /// OpenAI-compatible providers only.
    pub async fn chat_stream(&self, role: Role, request: ChatRequest) -> Result<ChatStream, Error> {
        let provider = self.config().provider_for_content(role)?;
        if provider.protocol != Protocol::OpenAi {
            return Err(Error::Config("streaming needs the openai protocol".into()));
        }
        let ticket = self.admit(role).await?;
        let call = Call::start(self.instruments(), "chat", role, provider);
        let span = call.span();
        let mut finisher = Finisher::new(call, ticket);
        let opened = self
            .open_stream(role, provider, &request)
            .instrument(span)
            .await;
        let response = match opened {
            Ok(response) => response,
            Err(error) => {
                finisher.fail(&error);
                return Err(error);
            }
        };
        let json = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| {
                value
                    .trim_start()
                    .to_ascii_lowercase()
                    .starts_with("application/json")
            });
        if json {
            return whole(response, finisher).await;
        }
        Ok(events(response.bytes_stream(), finisher))
    }

    /// Send the streamed request; a non-success status keeps only its code
    /// (its body can echo the request, and so the mail in it).
    async fn open_stream(
        &self,
        role: Role,
        provider: &ProviderConfig,
        request: &ChatRequest,
    ) -> Result<reqwest::Response, Error> {
        let body = WireChatRequest::of(provider, request, true);
        let mut http = self.post(provider, "chat/completions")?.json(&body);
        if let Some(timeout) = request.timeout {
            http = http.timeout(timeout);
        }
        tracing::debug!(%role, model = %provider.model, "ai chat stream request");
        let response = http.send().await?;
        let status = response.status();
        if !status.is_success() {
            drop(response.bytes().await);
            return Err(Error::Status {
                status: status.as_u16(),
            });
        }
        Ok(response)
    }
}

/// A server that ignored `stream` and answered JSON: its answer as one
/// text event (reasoning first, when it starts with `<think>`), then done.
async fn whole(response: reqwest::Response, mut finisher: Finisher) -> Result<ChatStream, Error> {
    let read = async {
        let bytes = response.bytes().await?;
        let wire: WireChatResponse = serde_json::from_slice(&bytes)
            .map_err(|e| Error::InvalidResponse(campbooks_kernel::redact::json(&e)))?;
        let choice = wire
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| Error::InvalidResponse("no choices".into()))?;
        Ok::<_, Error>((
            choice.message.content.unwrap_or_default(),
            wire.model,
            choice.finish_reason,
            wire.usage,
        ))
    };
    let (content, model, finish_reason, usage) = match read.await {
        Ok(read) => read,
        Err(error) => {
            finisher.fail(&error);
            return Err(error);
        }
    };
    let mut parser = Parser {
        model,
        finish_reason,
        usage,
        ..Parser::default()
    };
    parser.content(&content);
    parser.flush();
    let mut events: Vec<Result<ChatEvent, Error>> = Vec::new();
    while let Some(event) = parser.out.pop_front() {
        finisher.note(&event);
        events.push(Ok(event));
    }
    let summary = parser.summary(finisher.first);
    finisher.done(&summary);
    events.push(Ok(ChatEvent::Done(summary)));
    Ok(ChatStream(Box::pin(futures_util::stream::iter(events))))
}

/// The events read from `bytes`, an SSE body, settled by `finisher` however
/// they end.
pub(crate) fn events<S, B, E>(bytes: S, finisher: Finisher) -> ChatStream
where
    S: Stream<Item = Result<B, E>> + Send + 'static,
    B: AsRef<[u8]> + Send + 'static,
    E: Into<Error> + Send + 'static,
{
    let state = State {
        bytes: Box::pin(bytes),
        parser: Parser::default(),
        finisher,
        ended: false,
    };
    let stream = futures_util::stream::unfold(state, |mut state| async move {
        loop {
            if let Some(event) = state.parser.out.pop_front() {
                return Some((Ok(event), state));
            }
            if state.ended {
                return None;
            }
            // Nothing is queued: read on.
            let read = match state.bytes.next().await {
                Some(Ok(chunk)) => state.parser.push(chunk.as_ref()),
                Some(Err(error)) => Err(error.into()),
                None if state.parser.finish_reason.is_some() => {
                    // The body ended without `[DONE]`, after the model said
                    // why it stopped: what came is the answer.
                    state.parser.done = true;
                    Ok(())
                }
                // Cut off (a proxy closing the line, say): what came is not
                // the whole answer.
                None => Err(Error::InvalidResponse(
                    "the stream ended before the answer did".into(),
                )),
            };
            if let Err(error) = read {
                state.ended = true;
                state.parser.out.clear();
                state.finisher.fail(&error);
                return Some((Err(error), state));
            }
            if state.parser.done {
                state.parser.flush();
            }
            // Counted as they arrive, so the summary below includes them.
            for event in &state.parser.out {
                state.finisher.note(event);
            }
            if state.parser.done {
                state.ended = true;
                let summary = state.parser.summary(state.finisher.first);
                state.finisher.done(&summary);
                state.parser.out.push_back(ChatEvent::Done(summary));
            }
        }
    });
    ChatStream(Box::pin(stream))
}

/// What the stream reads with.
struct State<S> {
    bytes: Pin<Box<S>>,
    parser: Parser,
    finisher: Finisher,
    /// Nothing more is read: the last events are queued, or it failed.
    ended: bool,
}

/// Whether answer text is still undecided between `<think>` and the answer.
#[derive(Debug, Default)]
enum Think {
    /// At the start: what came so far, all of it a prefix of `<think>` (or
    /// blank).
    #[default]
    Start,
    /// Inside `<think>`: the end of what came that may be the start of
    /// `</think>`.
    Inside,
    /// The answer.
    Text,
}

const THINK_OPEN: &str = "<think>";
const THINK_CLOSE: &str = "</think>";

/// The SSE reader: bytes in, events out.
#[derive(Debug, Default)]
struct Parser {
    /// Bytes of an incomplete line.
    buf: Vec<u8>,
    /// The data lines of the event being read.
    data: String,
    has_data: bool,
    think: Think,
    /// Answer text held back while [`Think`] is undecided.
    held: String,
    model: String,
    finish_reason: Option<String>,
    usage: Option<Usage>,
    /// `[DONE]` came, or the body ended.
    done: bool,
    out: VecDeque<ChatEvent>,
}

impl Parser {
    /// Read `bytes`: whole lines are parsed, the rest is kept. One pass
    /// over what came, however many lines it holds.
    fn push(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let mut buf = std::mem::take(&mut self.buf);
        buf.extend_from_slice(bytes);
        let mut start = 0;
        while let Some(offset) = buf[start..].iter().position(|b| *b == b'\n') {
            let end = start + offset;
            let line = &buf[start..end];
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            let line = std::str::from_utf8(line)
                .map_err(|_| Error::InvalidResponse("the stream is not UTF-8".into()))?;
            start = end + 1;
            self.line(line)?;
            if self.done {
                return Ok(());
            }
        }
        buf.drain(..start);
        self.buf = buf;
        if self.buf.len() + self.data.len() > MAX_BUFFER {
            return Err(Error::InvalidResponse(
                "a stream event is longer than 1 MiB".into(),
            ));
        }
        Ok(())
    }

    /// One line of the event stream.
    fn line(&mut self, line: &str) -> Result<(), Error> {
        if line.is_empty() {
            if self.has_data {
                self.has_data = false;
                let data = std::mem::take(&mut self.data);
                self.event(&data)?;
            }
            return Ok(());
        }
        if line.starts_with(':') {
            // A comment (`: OPENROUTER PROCESSING`), to keep the line open.
            return Ok(());
        }
        let (field, value) = line.split_once(':').unwrap_or((line, ""));
        if field == "data" {
            let value = value.strip_prefix(' ').unwrap_or(value);
            if self.has_data {
                self.data.push('\n');
            }
            self.data.push_str(value);
            self.has_data = true;
        }
        Ok(())
    }

    /// One event's data: a chunk, or `[DONE]`.
    fn event(&mut self, data: &str) -> Result<(), Error> {
        if data.trim() == "[DONE]" {
            self.done = true;
            return Ok(());
        }
        let chunk: WireChunk = serde_json::from_str(data)
            .map_err(|e| Error::InvalidResponse(campbooks_kernel::redact::json(&e)))?;
        if chunk.error.is_some() {
            // Never its text: a provider's error can echo the request.
            return Err(Error::InvalidResponse("provider error mid-stream".into()));
        }
        if !chunk.model.is_empty() {
            self.model = chunk.model;
        }
        if chunk.usage.is_some() {
            self.usage = chunk.usage;
        }
        let Some(choice) = chunk.choices.into_iter().next() else {
            return Ok(());
        };
        if let Some(delta) = choice.delta {
            let before = self.out.len();
            let reasoning = delta.reasoning();
            if !reasoning.is_empty() {
                self.out.push_back(ChatEvent::Reasoning(reasoning));
            }
            if let Some(content) = delta.content {
                self.content(&content);
            }
            if self.out.len() == before {
                self.out.push_back(ChatEvent::Alive);
            }
        }
        if choice.finish_reason.is_some() {
            self.finish_reason = choice.finish_reason;
        }
        Ok(())
    }

    /// Answer text, with a leading `<think>` block read as reasoning.
    fn content(&mut self, content: &str) {
        if content.is_empty() {
            return;
        }
        match self.think {
            Think::Text => self.out.push_back(ChatEvent::Text(content.to_owned())),
            Think::Start => {
                self.held.push_str(content);
                let start = self.held.trim_start();
                if let Some(inside) = start.strip_prefix(THINK_OPEN) {
                    let inside = inside.to_owned();
                    self.held.clear();
                    self.think = Think::Inside;
                    self.inside(&inside);
                } else if !THINK_OPEN.starts_with(start) || self.held.len() > MAX_THINK_LEAD {
                    self.think = Think::Text;
                    let text = std::mem::take(&mut self.held);
                    self.out.push_back(ChatEvent::Text(text));
                }
            }
            Think::Inside => self.inside(content),
        }
    }

    /// Text inside `<think>`: reasoning up to `</think>`, the answer after.
    fn inside(&mut self, content: &str) {
        self.held.push_str(content);
        if let Some(at) = self.held.find(THINK_CLOSE) {
            let reasoning = self.held[..at].to_owned();
            let rest = self.held[at + THINK_CLOSE.len()..].trim_start().to_owned();
            self.held.clear();
            self.think = Think::Text;
            if !reasoning.is_empty() {
                self.out.push_back(ChatEvent::Reasoning(reasoning));
            }
            if !rest.is_empty() {
                self.out.push_back(ChatEvent::Text(rest));
            }
            return;
        }
        // Keep back an end that may be the start of `</think>` (ASCII, so
        // the cut is on a character boundary).
        let keep = (1..THINK_CLOSE.len())
            .rev()
            .find(|n| self.held.ends_with(&THINK_CLOSE[..*n]))
            .unwrap_or(0);
        let ready = self.held.len() - keep;
        if ready > 0 {
            let reasoning: String = self.held.drain(..ready).collect();
            self.out.push_back(ChatEvent::Reasoning(reasoning));
        }
    }

    /// The end of the stream: whatever was held back goes out.
    fn flush(&mut self) {
        let held = std::mem::take(&mut self.held);
        if held.is_empty() {
            return;
        }
        match self.think {
            Think::Inside => self.out.push_back(ChatEvent::Reasoning(held)),
            Think::Start | Think::Text => self.out.push_back(ChatEvent::Text(held)),
        }
    }

    fn summary(&self, ttft: Option<Duration>) -> ChatSummary {
        ChatSummary {
            model: self.model.clone(),
            finish_reason: self.finish_reason.clone(),
            usage: self.usage,
            ttft,
        }
    }
}

/// One chunk of a streamed chat completion.
#[derive(Deserialize)]
struct WireChunk {
    #[serde(default)]
    model: String,
    #[serde(default)]
    choices: Vec<WireChunkChoice>,
    #[serde(default)]
    usage: Option<Usage>,
    #[serde(default)]
    error: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct WireChunkChoice {
    #[serde(default)]
    delta: Option<WireDelta>,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct WireDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
    #[serde(default)]
    reasoning_details: Option<Vec<WireReasoningDetail>>,
}

impl WireDelta {
    /// Its reasoning, from the first field that has some (providers send
    /// the same text in several).
    fn reasoning(&self) -> String {
        let plain = [&self.reasoning, &self.reasoning_content]
            .into_iter()
            .flatten()
            .find(|text| !text.is_empty());
        if let Some(text) = plain {
            return text.clone();
        }
        self.reasoning_details
            .iter()
            .flatten()
            .filter(|detail| !detail.kind.contains("encrypted"))
            .filter_map(|detail| detail.text.as_deref().or(detail.summary.as_deref()))
            .collect()
    }
}

#[derive(Deserialize)]
struct WireReasoningDetail {
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    summary: Option<String>,
}

/// Records and settles a streamed call exactly once, however it ends.
pub(crate) struct Finisher {
    call: Option<Call>,
    ticket: Option<Ticket>,
    started: Instant,
    /// Time to the first reasoning or answer text.
    first: Option<Duration>,
    reasoning_chars: usize,
    text_chars: usize,
}

impl Finisher {
    pub(crate) fn new(call: Call, ticket: Option<Ticket>) -> Self {
        Self {
            call: Some(call),
            ticket,
            started: Instant::now(),
            first: None,
            reasoning_chars: 0,
            text_chars: 0,
        }
    }

    /// Count an event on its way out.
    fn note(&mut self, event: &ChatEvent) {
        let chars = match event {
            ChatEvent::Reasoning(text) => {
                let n = text.chars().count();
                self.reasoning_chars += n;
                n
            }
            ChatEvent::Text(text) => {
                let n = text.chars().count();
                self.text_chars += n;
                n
            }
            ChatEvent::Done(_) | ChatEvent::Alive => 0,
        };
        if chars > 0 && self.first.is_none() {
            self.first = Some(self.started.elapsed());
        }
    }

    /// The stream ended normally.
    fn done(&mut self, summary: &ChatSummary) {
        if let Some(call) = self.call.take() {
            call.succeeded(Some(summary.model.as_str()), summary.usage);
        }
        settle(self.ticket.take(), false, summary.usage);
        self.log("done", summary.finish_kind(), summary.usage);
    }

    /// The call failed.
    fn fail(&mut self, error: &Error) {
        if let Some(call) = self.call.take() {
            call.failed(error);
        }
        settle(self.ticket.take(), true, None);
        self.log("failed", "none", None);
    }

    fn log(&self, outcome: &'static str, finish_reason: &'static str, usage: Option<Usage>) {
        tracing::info!(
            outcome,
            ttft_ms = self.first.map(millis),
            elapsed_ms = millis(self.started.elapsed()),
            reasoning_chars = self.reasoning_chars,
            text_chars = self.text_chars,
            finish_reason,
            prompt_tokens = usage.map(|u| u.prompt_tokens),
            completion_tokens = usage.map(|u| u.completion_tokens),
            "ai chat stream ended"
        );
    }
}

impl Drop for Finisher {
    /// Dropped before its end (the caller hung up or gave up): the call is
    /// cancelled and settled as failed.
    fn drop(&mut self) {
        if let Some(call) = self.call.take() {
            call.cancel();
            settle(self.ticket.take(), true, None);
            self.log("cancelled", "none", None);
        }
    }
}

/// Settle `ticket` off the stream: settling waits on the ledger, which a
/// stream's poll or a drop cannot. Outside a runtime there is nowhere to run
/// it, and only the tokens are lost (the call was counted when admitted).
fn settle(ticket: Option<Ticket>, failed: bool, usage: Option<Usage>) {
    let Some(ticket) = ticket else {
        return;
    };
    if let Ok(runtime) = tokio::runtime::Handle::try_current() {
        runtime.spawn(ticket.settle_as(failed, usage));
    }
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests;
