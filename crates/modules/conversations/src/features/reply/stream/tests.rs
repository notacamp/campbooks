//! The streamed reply's limits and bounds, on a paused clock, against
//! stand-in model streams.

use std::time::Duration;

use axum::response::{IntoResponse as _, sse::Sse};
use campbooks_ai::{ChatEvent, ChatSummary, Error as AiError, Usage};
use campbooks_kernel::i18n::Locale;
use futures_util::{Stream, StreamExt as _};
use serde_json::{Value, json};
use tokio::time::Instant;

use super::{Limits, drive};
use crate::model::MessageId;

/// The model's end, having stopped for `finish`.
fn done(finish: &str) -> ChatEvent {
    ChatEvent::Done(ChatSummary {
        model: "test-model".into(),
        finish_reason: Some(finish.into()),
        usage: Some(Usage {
            prompt_tokens: 120,
            completion_tokens: 40,
        }),
        ttft: None,
    })
}

fn reasoning(text: &str) -> ChatEvent {
    ChatEvent::Reasoning(text.into())
}

fn text(text: &str) -> ChatEvent {
    ChatEvent::Text(text.into())
}

/// The SSE events `chat` makes, as (event, data), and how long they took.
async fn run<S>(chat: S, limits: Limits) -> (Vec<(String, Value)>, Duration)
where
    S: Stream<Item = Result<ChatEvent, AiError>> + Send + 'static,
{
    let started = Instant::now();
    let message = MessageId::new();
    let response = Sse::new(drive(chat, limits, message, Locale::En, false)).into_response();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();
    let events = body
        .split("\n\n")
        .filter(|block| !block.trim().is_empty())
        .map(|block| {
            let mut name = String::new();
            let mut data = String::new();
            for line in block.lines() {
                if let Some(value) = line.strip_prefix("event: ") {
                    name = value.to_owned();
                } else if let Some(value) = line.strip_prefix("data: ") {
                    data.push_str(value);
                }
            }
            (name, serde_json::from_str(&data).unwrap())
        })
        .collect();
    (events, started.elapsed())
}

fn names(events: &[(String, Value)]) -> Vec<&str> {
    events.iter().map(|(name, _)| name.as_str()).collect()
}

#[tokio::test(start_paused = true)]
async fn thinking_then_words_then_the_whole_reply() {
    let chat = futures_util::stream::iter(vec![
        Ok(reasoning("Maya asks about Friday.")),
        Ok(text("Hi Maya,\r\n")),
        Ok(text("Friday works.\u{0007}\u{202E}")),
        Ok(done("stop")),
    ]);
    let (events, _) = run(chat, Limits::default()).await;
    assert_eq!(names(&events), ["thinking", "text", "text", "done"]);
    assert_eq!(events[0].1, json!({"delta": "Maya asks about Friday."}));
    assert_eq!(events[1].1, json!({"delta": "Hi Maya,\n"}));
    assert_eq!(
        events[2].1,
        json!({"delta": "Friday works."}),
        "control and direction characters never go out"
    );
    assert_eq!(events[3].1["text"], "Hi Maya,\nFriday works.");
    assert!(events[3].1["message_id"].is_string());
}

#[tokio::test(start_paused = true)]
async fn a_model_that_goes_quiet_times_out() {
    let chat = futures_util::stream::iter(vec![Ok(reasoning("Thinking"))])
        .chain(futures_util::stream::pending());
    let (events, took) = run(chat, Limits::default()).await;
    assert_eq!(names(&events), ["thinking", "error"]);
    assert_eq!(events[1].1["code"], "reply_timed_out");
    assert_eq!(events[1].1["status"], 503);
    assert!(
        took >= Duration::from_secs(30) && took < Duration::from_secs(31),
        "{took:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn a_model_that_never_stops_thinking_times_out_in_all() {
    // A sign every 20 seconds: never idle, never done.
    let chat = futures_util::stream::unfold((), |()| async {
        tokio::time::sleep(Duration::from_secs(20)).await;
        Some((Ok(reasoning("…")), ()))
    });
    let (events, took) = run(chat, Limits::default()).await;
    assert_eq!(events.last().unwrap().0, "error");
    assert_eq!(events.last().unwrap().1["code"], "reply_timed_out");
    assert!(
        took >= Duration::from_mins(2) && took < Duration::from_secs(121),
        "{took:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn an_empty_or_cut_short_reply_fails() {
    for chat in [
        vec![Ok(reasoning("Hm")), Ok(text("  \n")), Ok(done("stop"))],
        vec![Ok(text("Hi Maya, Fri")), Ok(done("length"))],
    ] {
        let (events, _) = run(futures_util::stream::iter(chat), Limits::default()).await;
        assert_eq!(events.last().unwrap().0, "error", "{events:?}");
        assert_eq!(events.last().unwrap().1["code"], "reply_failed");
        assert!(!names(&events).contains(&"done"));
    }
}

#[tokio::test(start_paused = true)]
async fn a_failing_or_vanishing_model_fails() {
    for chat in [
        vec![
            Ok(text("Hi")),
            Err(AiError::InvalidResponse("mid-stream".into())),
        ],
        vec![Ok(text("Hi"))],
    ] {
        let (events, _) = run(futures_util::stream::iter(chat), Limits::default()).await;
        assert_eq!(names(&events), ["text", "error"]);
        assert_eq!(events[1].1["code"], "reply_failed");
    }
}

#[tokio::test(start_paused = true)]
async fn thinking_and_words_are_forwarded_within_their_bounds() {
    let limits = Limits {
        thinking_chars: 10,
        text_chars: 5,
        ..Limits::default()
    };
    let chat = futures_util::stream::iter(vec![
        Ok(reasoning("abcdefgh")),
        Ok(reasoning("ijklmnop")),
        Ok(reasoning("qrstuvwx")),
        Ok(text("Hello world")),
        Ok(text("!")),
        Ok(done("stop")),
    ]);
    let (events, _) = run(chat, limits).await;
    assert_eq!(names(&events), ["thinking", "thinking", "text", "done"]);
    assert_eq!(events[0].1["delta"], "abcdefgh");
    assert_eq!(events[1].1["delta"], "ij");
    assert_eq!(events[2].1["delta"], "Hello");
    // The reply itself is whole, as `write_reply` keeps it.
    assert_eq!(events[3].1["text"], "Hello world!");
}
