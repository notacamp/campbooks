//! The SSE reader and the stream it feeds, without a provider.

use futures_util::StreamExt as _;

use super::*;
use crate::telemetry::Instruments;

/// Read `chunks` as one body; the events, and how it ended.
fn parse(chunks: &[&[u8]]) -> Result<Vec<ChatEvent>, Error> {
    let mut parser = Parser::default();
    for chunk in chunks {
        parser.push(chunk)?;
        if parser.done {
            break;
        }
    }
    parser.flush();
    Ok(parser.out.into_iter().collect())
}

/// Consecutive events of one kind joined: how a reader sees them.
fn joined(events: Vec<ChatEvent>) -> Vec<ChatEvent> {
    let mut out: Vec<ChatEvent> = Vec::new();
    for event in events {
        match (out.last_mut(), event) {
            (Some(ChatEvent::Reasoning(a)), ChatEvent::Reasoning(b))
            | (Some(ChatEvent::Text(a)), ChatEvent::Text(b)) => a.push_str(&b),
            (_, event) => out.push(event),
        }
    }
    out
}

fn reasoning(text: &str) -> ChatEvent {
    ChatEvent::Reasoning(text.to_owned())
}

fn text(text: &str) -> ChatEvent {
    ChatEvent::Text(text.to_owned())
}

const BODY: &str = "\
: OPENROUTER PROCESSING\n\
\n\
data: {\"model\":\"m-1\",\"choices\":[{\"delta\":{\"role\":\"assistant\",\"reasoning\":\"Marta asks\"}}]}\n\
\n\
data: {\"choices\":[{\"delta\":{\"reasoning\":\" about Thursday.\",\"reasoning_details\":[{\"type\":\"reasoning.text\",\"text\":\" about Thursday.\"}]}}]}\n\
\n\
data: {\"choices\":[{\"delta\":{\"content\":\"Hi Marta,\"}}]}\n\
\n\
data: {\"choices\":[{\"delta\":{\"content\":\" Thursday works.\"},\"finish_reason\":\"stop\"}]}\n\
\n\
data: {\"choices\":[],\"usage\":{\"prompt_tokens\":120,\"completion_tokens\":40,\"total_tokens\":160}}\n\
\n\
data: [DONE]\n\
\n";

#[test]
fn reads_reasoning_then_text_then_usage() {
    let mut parser = Parser::default();
    parser.push(BODY.as_bytes()).unwrap();
    assert!(parser.done);
    let events: Vec<ChatEvent> = parser.out.drain(..).collect();
    assert_eq!(
        events,
        [
            reasoning("Marta asks"),
            reasoning(" about Thursday."),
            text("Hi Marta,"),
            text(" Thursday works."),
        ],
        "the same reasoning in two fields is read once"
    );
    let summary = parser.summary(None);
    assert_eq!(summary.model, "m-1");
    assert_eq!(summary.finish_kind(), "stop");
    let usage = summary.usage.unwrap();
    assert_eq!((usage.prompt_tokens, usage.completion_tokens), (120, 40));
}

#[test]
fn a_body_split_anywhere_reads_the_same() {
    let whole = parse(&[BODY.as_bytes()]).unwrap();
    let bytes = BODY.as_bytes();
    for at in [1, 7, 30, 61, 100, bytes.len() - 3] {
        let (a, b) = bytes.split_at(at);
        assert_eq!(
            joined(parse(&[a, b]).unwrap()),
            joined(whole.clone()),
            "{at}"
        );
    }
}

#[test]
fn a_character_split_across_chunks_stays_whole() {
    let body =
        "data: {\"choices\":[{\"delta\":{\"content\":\"Olá, até já\"}}]}\n\ndata: [DONE]\n\n";
    let bytes = body.as_bytes();
    let at = body.find('á').unwrap() + 1;
    let events = parse(&[&bytes[..at], &bytes[at..]]).unwrap();
    assert_eq!(events, [text("Olá, até já")]);
}

#[test]
fn crlf_lines_and_multi_line_data_are_read() {
    let body = "data: {\"choices\":\r\ndata: [{\"delta\":{\"content\":\"Hi\"}}]}\r\n\r\ndata: [DONE]\r\n\r\n";
    assert_eq!(parse(&[body.as_bytes()]).unwrap(), [text("Hi")]);
}

#[test]
fn reasoning_content_and_reasoning_details_are_reasoning() {
    let body = "\
data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"First.\"}}]}\n\n\
data: {\"choices\":[{\"delta\":{\"reasoning_details\":[\
{\"type\":\"reasoning.encrypted\",\"data\":\"c2VjcmV0\"},\
{\"type\":\"reasoning.summary\",\"summary\":\" Second.\"},\
{\"type\":\"reasoning.text\",\"text\":\" Third.\"}]}}]}\n\n\
data: {\"choices\":[{\"delta\":{\"reasoning\":null,\"reasoning_details\":null,\"content\":null}}]}\n\n\
data: [DONE]\n\n";
    assert_eq!(
        joined(parse(&[body.as_bytes()]).unwrap()),
        [reasoning("First. Second. Third."), ChatEvent::Alive],
        "the encrypted entry is skipped; the empty delta is a sign of life"
    );
}

#[test]
fn hidden_reasoning_is_a_sign_of_life() {
    let body = "\
data: {\"choices\":[{\"delta\":{\"reasoning_details\":[{\"type\":\"reasoning.encrypted\",\"data\":\"c2VjcmV0\"}]}}]}\n\n\
data: {\"choices\":[{\"delta\":{\"content\":\"Hi\"}}]}\n\n\
data: [DONE]\n\n";
    assert_eq!(
        parse(&[body.as_bytes()]).unwrap(),
        [ChatEvent::Alive, text("Hi")]
    );
}

#[test]
fn a_blank_start_is_not_held_for_ever() {
    let blank = " ".repeat(MAX_THINK_LEAD + 1);
    let mut parser = Parser::default();
    parser.content(&blank);
    assert!(parser.held.is_empty(), "past the lead, it is the answer");
    assert_eq!(parser.out.pop_front(), Some(text(&blank)));
}

#[test]
fn many_lines_in_one_chunk_are_all_read() {
    let mut body = String::new();
    for _ in 0..1000 {
        body.push_str(": OPENROUTER PROCESSING\n");
    }
    body.push_str("data: {\"choices\":[{\"delta\":{\"content\":\"Hi\"}}]}\n\ndata: [DONE]\n\n");
    assert_eq!(parse(&[body.as_bytes()]).unwrap(), [text("Hi")]);
}

#[test]
fn a_provider_error_mid_stream_fails_without_its_words() {
    let body = "\
data: {\"choices\":[{\"delta\":{\"content\":\"Hi\"}}]}\n\n\
data: {\"error\":{\"message\":\"Subject: Q4 plan was refused\",\"code\":502}}\n\n";
    let error = parse(&[body.as_bytes()]).unwrap_err();
    assert!(matches!(error, Error::InvalidResponse(_)), "{error:?}");
    assert!(!error.to_string().contains("Q4 plan"), "{error}");
}

#[test]
fn a_line_longer_than_the_buffer_is_refused() {
    let long = vec![b'a'; MAX_BUFFER + 1];
    let error = parse(&[b"data: ", &long]).unwrap_err();
    assert!(matches!(error, Error::InvalidResponse(_)), "{error:?}");
}

#[test]
fn a_chunk_that_is_not_json_is_an_invalid_response() {
    let error = parse(&[b"data: {\"choices\": [\n\n"]).unwrap_err();
    assert!(matches!(error, Error::InvalidResponse(_)), "{error:?}");
}

/// The answer text of `deltas`, one content delta each.
fn contents(deltas: &[&str]) -> Vec<ChatEvent> {
    let mut parser = Parser::default();
    for delta in deltas {
        parser.content(delta);
    }
    parser.flush();
    joined(parser.out.into_iter().collect())
}

#[test]
fn a_leading_think_block_is_reasoning() {
    assert_eq!(
        contents(&[
            "<thi",
            "nk>Marta wants",
            " Thursday.</th",
            "ink>\n\nHi Marta",
            ", yes."
        ]),
        [reasoning("Marta wants Thursday."), text("Hi Marta, yes.")]
    );
    assert_eq!(
        contents(&["  \n<think>x</think>Hi"]),
        [reasoning("x"), text("Hi")]
    );
    // Never closed: all of it was thinking.
    assert_eq!(
        contents(&["<think>still thinking"]),
        [reasoning("still thinking")]
    );
}

#[test]
fn think_only_counts_at_the_very_start() {
    assert_eq!(contents(&["<p>Hi"]), [text("<p>Hi")]);
    assert_eq!(contents(&["<", "b>Hi"]), [text("<b>Hi")]);
    assert_eq!(
        contents(&["Hi <think>not this</think>"]),
        [text("Hi <think>not this</think>")]
    );
    assert_eq!(
        contents(&["<thi"]),
        [text("<thi")],
        "held, then let go at the end"
    );
}

/// A finisher for a call nobody meters.
fn finisher() -> Finisher {
    let provider = ProviderConfig::new("http://ollama:11434/v1".parse().unwrap(), "m");
    Finisher::new(
        Call::start(&Instruments::new(), "chat", Role::Complete, &provider),
        None,
    )
}

async fn collect(chunks: Vec<Result<Vec<u8>, Error>>) -> Vec<Result<ChatEvent, Error>> {
    events(futures_util::stream::iter(chunks), finisher())
        .collect()
        .await
}

#[tokio::test]
async fn the_stream_ends_with_its_summary() {
    let events = collect(vec![Ok(BODY.as_bytes().to_vec())]).await;
    let last = events.last().unwrap().as_ref().unwrap();
    let ChatEvent::Done(summary) = last else {
        panic!("{last:?}");
    };
    assert_eq!(summary.usage.unwrap().completion_tokens, 40);
    assert!(summary.ttft.is_some());
    assert_eq!(events.len(), 5);
}

#[tokio::test]
async fn a_body_that_ends_without_done_after_its_finish_still_ends() {
    let body =
        b"data: {\"choices\":[{\"delta\":{\"content\":\"Hi\"},\"finish_reason\":\"stop\"}]}\n\n"
            .to_vec();
    let events = collect(vec![Ok(body)]).await;
    assert!(matches!(events[0], Ok(ChatEvent::Text(ref t)) if t == "Hi"));
    assert!(matches!(events[1], Ok(ChatEvent::Done(_))));
    assert_eq!(events.len(), 2);
}

#[tokio::test]
async fn a_body_cut_off_before_its_finish_fails() {
    let body = b"data: {\"choices\":[{\"delta\":{\"content\":\"Hi\"}}]}\n\n".to_vec();
    let events = collect(vec![Ok(body)]).await;
    assert!(matches!(events[0], Ok(ChatEvent::Text(ref t)) if t == "Hi"));
    assert!(
        matches!(events[1], Err(Error::InvalidResponse(_))),
        "{events:?}"
    );
    assert_eq!(events.len(), 2);
}

#[tokio::test]
async fn a_failure_ends_the_stream() {
    let events = collect(vec![
        Ok(b"data: {\"choices\":[{\"delta\":{\"content\":\"Hi\"}}]}\n\n".to_vec()),
        Err(Error::InvalidResponse("connection reset".into())),
        Ok(b"data: [DONE]\n\n".to_vec()),
    ])
    .await;
    assert_eq!(events.len(), 2, "{events:?}");
    assert!(matches!(events[1], Err(Error::InvalidResponse(_))));
}
