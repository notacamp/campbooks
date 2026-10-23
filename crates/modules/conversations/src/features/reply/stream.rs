//! The streamed reply's events (campbooks-cloud#288): the model's stream in,
//! Server-Sent Events out, within the stream's own limits.
//!
//! [`drive`] is a pure function of the model's events and Tokio's clock, so
//! its limits are tested with a paused clock. It forwards the model's
//! reasoning (`thinking`) and the reply's words (`text`) as they arrive,
//! filtered like the reply and bounded, and ends with `done` (the whole
//! reply, kept as `write_reply` keeps it) or `error` (a problem). Dropping
//! it (the caller hung up) drops the model's stream, which stops and
//! settles the call.
//!
//! The thinking and the reply are data: they reach the SSE response and
//! nothing else, never a log line, a span or a metric.

use std::{convert::Infallible, pin::Pin, time::Duration};

use axum::response::sse::Event;
use campbooks_ai::{ChatEvent, Error as AiError};
use campbooks_kernel::{Error, i18n::Locale};
use futures_util::{Stream, StreamExt as _};
use tokio::time::Instant;

use super::{prompt, types::ReplyText};
use crate::model::MessageId;

/// Longest the model may go without a sign of life: any chunk it sends
/// counts (reasoning, words, or work it does not show).
pub(super) const IDLE: Duration = Duration::from_secs(30);
/// Longest a streamed reply may take in all.
pub(super) const TOTAL: Duration = Duration::from_mins(2);
/// How often a comment keeps the connection open.
pub(super) const KEEP_ALIVE: Duration = Duration::from_secs(10);
/// Most characters of reasoning forwarded; more still counts as a sign of
/// life.
pub(super) const MAX_THINKING_CHARS: usize = 8_000;
/// Most bytes of the reply's words collected: past this the model is only
/// running on, and the reply kept is shorter anyway.
const MAX_COLLECTED_BYTES: usize = 64 * 1024;

/// The stream's limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Limits {
    /// Longest the model may go without an event.
    pub idle: Duration,
    /// Longest the whole stream may take.
    pub total: Duration,
    /// Most characters of reasoning forwarded.
    pub thinking_chars: usize,
    /// Most characters of the reply's words forwarded.
    pub text_chars: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            idle: IDLE,
            total: TOTAL,
            thinking_chars: MAX_THINKING_CHARS,
            text_chars: prompt::MAX_REPLY_CHARS,
        }
    }
}

/// The SSE events of a reply written by `chat`, for `message`, with
/// problems in `locale`.
pub(super) fn drive<S>(
    chat: S,
    limits: Limits,
    message: MessageId,
    locale: Locale,
    rewrite: bool,
) -> impl Stream<Item = Result<Event, Infallible>> + Send + 'static
where
    S: Stream<Item = Result<ChatEvent, AiError>> + Send + 'static,
{
    let started = Instant::now();
    let state = Driver {
        chat: Box::pin(chat),
        limits,
        message,
        locale,
        started,
        reply: String::new(),
        thinking_sent: 0,
        text_sent: 0,
        finished: false,
        report: Report {
            started,
            first: None,
            outcome: None,
            reason: None,
            rewrite,
            prompt_tokens: None,
            completion_tokens: None,
        },
    };
    futures_util::stream::unfold(state, |mut driver| async move {
        let event = driver.next().await?;
        Some((Ok::<_, Infallible>(event), driver))
    })
}

struct Driver<S> {
    chat: Pin<Box<S>>,
    limits: Limits,
    message: MessageId,
    locale: Locale,
    started: Instant,
    /// The reply's words so far.
    reply: String,
    thinking_sent: usize,
    text_sent: usize,
    /// `done` or `error` went out: nothing follows.
    finished: bool,
    report: Report,
}

impl<S> Driver<S>
where
    S: Stream<Item = Result<ChatEvent, AiError>> + Send,
{
    /// The next event to send, or `None` once the stream is over.
    async fn next(&mut self) -> Option<Event> {
        loop {
            if self.finished {
                return None;
            }
            let left = self.limits.total.saturating_sub(self.started.elapsed());
            let wait = self.limits.idle.min(left);
            let Ok(next) = tokio::time::timeout(wait, self.chat.next()).await else {
                let reason = if left <= self.limits.idle {
                    "total"
                } else {
                    "idle"
                };
                return Some(self.fail("timed_out", reason, "reply_timed_out"));
            };
            match next {
                None => return Some(self.fail("failed", "provider", "reply_failed")),
                Some(Err(error)) => {
                    let code = if matches!(&error, AiError::Transport(e) if e.is_timeout()) {
                        "reply_timed_out"
                    } else {
                        "reply_failed"
                    };
                    return Some(self.fail("failed", "provider", code));
                }
                Some(Ok(ChatEvent::Reasoning(delta))) => {
                    self.started_writing();
                    let room = self
                        .limits
                        .thinking_chars
                        .saturating_sub(self.thinking_sent);
                    if let Some(delta) = forwarded(&delta, room) {
                        self.thinking_sent += delta.chars().count();
                        return Some(delta_event("thinking", &delta));
                    }
                }
                Some(Ok(ChatEvent::Text(delta))) => {
                    self.started_writing();
                    if self.reply.len() < MAX_COLLECTED_BYTES {
                        self.reply.push_str(&delta);
                    }
                    let room = self.limits.text_chars.saturating_sub(self.text_sent);
                    if let Some(delta) = forwarded(&delta, room) {
                        self.text_sent += delta.chars().count();
                        return Some(delta_event("text", &delta));
                    }
                }
                // Nothing to send; the idle wait starts again. A reply is
                // asked for with no tools, so a tool call is only a sign of
                // life, never run.
                Some(Ok(ChatEvent::Alive | ChatEvent::ToolCall(_))) => {}
                Some(Ok(ChatEvent::Done(summary))) => {
                    self.report.prompt_tokens = summary.usage.map(|u| u.prompt_tokens);
                    self.report.completion_tokens = summary.usage.map(|u| u.completion_tokens);
                    if summary.finish_kind() == "length" {
                        return Some(self.fail("failed", "truncated", "reply_failed"));
                    }
                    let Some(text) = prompt::reply(&self.reply) else {
                        return Some(self.fail("failed", "empty", "reply_failed"));
                    };
                    self.finished = true;
                    self.report.outcome = Some("done");
                    let done = ReplyText {
                        message_id: self.message,
                        text,
                    };
                    return Some(
                        Event::default()
                            .event("done")
                            .data(serde_json::to_string(&done).unwrap_or_default()),
                    );
                }
            }
        }
    }

    fn started_writing(&mut self) {
        if self.report.first.is_none() {
            self.report.first = Some(self.started.elapsed());
        }
    }

    /// The last event: a problem with `code`, logged as `outcome` for
    /// `reason`.
    fn fail(&mut self, outcome: &'static str, reason: &'static str, code: &'static str) -> Event {
        self.finished = true;
        self.report.outcome = Some(outcome);
        self.report.reason = Some(reason);
        let problem = Error::Unavailable(crate::MESSAGES.reason(code)).to_problem(self.locale);
        Event::default()
            .event("error")
            .data(serde_json::to_string(&problem).unwrap_or_default())
    }
}

/// What of `delta` goes out: its kept characters, at most `room` of them;
/// `None` when nothing is left.
fn forwarded(delta: &str, room: usize) -> Option<String> {
    if room == 0 {
        return None;
    }
    let kept: String = prompt::kept(delta).chars().take(room).collect();
    (!kept.is_empty()).then_some(kept)
}

/// A `thinking` or `text` event carrying `delta`.
fn delta_event(name: &'static str, delta: &str) -> Event {
    Event::default()
        .event(name)
        .data(serde_json::json!({ "delta": delta }).to_string())
}

/// One log line per streamed reply, however it ends (a drop is the caller
/// hanging up): how, why, and how long, never what was written.
struct Report {
    started: Instant,
    /// Time to the model's first reasoning or words.
    first: Option<Duration>,
    outcome: Option<&'static str>,
    reason: Option<&'static str>,
    rewrite: bool,
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
}

impl Drop for Report {
    fn drop(&mut self) {
        tracing::info!(
            outcome = self.outcome.unwrap_or("cancelled"),
            reason = self.reason,
            ttft_ms = self.first.map(millis),
            elapsed_ms = millis(self.started.elapsed()),
            rewrite = self.rewrite,
            prompt_tokens = self.prompt_tokens,
            completion_tokens = self.completion_tokens,
            "reply streamed"
        );
    }
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests;
