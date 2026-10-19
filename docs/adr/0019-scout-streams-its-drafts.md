# 0019 — Scout streams its drafts

- Status: accepted
- Date: 2026-10-19
- Tracking: notacamp/campbooks-cloud#288
- Change: `openspec/changes/reply-stream`

## Context

On 2026-10-19 a written reply failed in production with 503
`request_timed_out` after 30 002 ms. The `complete` role runs a reasoning
model: it thinks before it writes, and a reply took 10 to 40 seconds. Every
request had one limit, 30 seconds, applied by the server to the whole
handler. Replies that crossed it were stopped; replies that did not still
left the member watching a spinner with no sign of progress.

The written reply was also held to a JSON schema (`{"reply": "…"}`), which
cannot be shown while it is written without parsing partial JSON, and
reasoning models served through a router keep to schemas unevenly.

## Decision

1. **A streamed operation for the written reply.**
   `conversations.stream_reply` takes the same request as
   `conversations.write_reply` and answers Server-Sent Events over the
   POST: `thinking` and `text` deltas as the model writes, then `done` with
   the whole bounded reply, or `error` with a problem. Every check runs
   before the response head, so a refusal is an ordinary problem answer.
   The stream has its own limits (30 seconds without a sign of life from the
   model, counting reasoning it does not show; 2 minutes from the head).
2. **The written reply is plain text.** The instruction asks for the body
   only; the answer is filtered and bounded as before. The mail stays JSON
   data in the user message. Likely replies keep their schema.
3. **Per-route deadlines.** `kernel::time_limit` gives every request a
   `Deadline` of 30 seconds that its handler may raise for itself up to 100
   seconds, below the idempotency layer's 2-minute abandonment. The limit
   covers the response head only.
4. **Streaming in the gateway.** `Gateway::chat_stream` speaks the
   OpenAI-compatible streaming protocol, reads reasoning from the fields
   providers use, is admitted against the daily cap before anything is
   sent, and settles its usage however the stream ends (drop included).
5. **A pure stream driver in the module.** The module turns the gateway's
   events into SSE frames in one testable function, with time limits that
   run on Tokio's clock (paused in tests).

## Consequences

- The desktop app stays on the JSON route: its bridge returns whole
  answers. Its timeout rises to 100 seconds so the route's 90 seconds
  apply.
- The idempotency layer releases the key of any `text/event-stream`
  answer instead of buffering it to store; such answers are never replayed.
- Proxies must not buffer `text/event-stream` (nginx: the
  `x-accel-buffering: no` header the operation sends; Caddy streams it).
- A reasoning model's thinking reaches the member's screen. It is shown
  only to someone who may read the conversation, as text, and is never
  stored or logged.
- A role may ask for less reasoning (`REASONING_EFFORT`), unset by default.

## Alternatives considered

- **Only raise the limit.** Ends the cut-off, keeps the silent wait.
  Kept for the JSON routes as the fallback, not as the answer.
- **A background job and polling.** Needs the model's answer stored
  somewhere and a poll loop; nothing is stored today.
- **Extracting the JSON string as it streams.** Escapes and `\u` sequences
  split across chunks, and repairs for broken JSON: complexity for a
  schema that buys no safety here.
- **A WebSocket.** Two-way is not needed; the BFF already passes HTTP
  bodies through and the change feed is SSE.
