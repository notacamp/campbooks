## Why

campbooks-cloud#288. On 2026-10-19 a written reply failed in production with
503 `request_timed_out` after 30 002 ms: the `complete` model (a reasoning
model) was still thinking when the server's 30-second request limit
stopped it. The member saw nothing for half a minute, then an error. The
same model usually answers in 10 to 40 seconds, so the limit cut off a
large share of replies, and even a reply that arrived inside it gave no
sign of life while it was written.

The dialog also sends only the words the member typed. When they write
"1 and 3, warmer", the model never saw the likely replies those numbers
point at.

## What changes

- New operation `conversations.stream_reply`
  (`POST …/conversations/{conversation_id}/reply/draft/stream`): the same
  request, checks and answer as `conversations.write_reply`, answered as a
  Server-Sent Events stream. Scout's thinking (when the model shows it) and
  the reply's words arrive as they are written; the stream ends with the
  whole reply, bounded as before, or a problem. Every check (scope, access,
  allowance, a model that may read mail, the daily AI limit) still runs
  before the stream opens, as an ordinary problem answer.
- The stream has its own limits: 30 seconds without a sign from the model,
  2 minutes in all. The thinking shown is bounded and is never stored or
  logged.
- The JSON operations (`suggest_replies`, `write_reply`) may run for 90
  seconds instead of the server-wide 30, and a model that takes too long
  answers 503 `reply_timed_out`, in the member's language, instead of the
  generic `request_timed_out`.
- `WriteReply` takes `suggestions`: the likely replies the member was shown,
  in order, so "1 and 3" or "the second, warmer" means something to the
  model. They are options, never facts.
- A written reply is plain text from the model, no longer a JSON schema, so
  it can be shown as it arrives. It is bounded and filtered exactly as
  before.
- The reply dialog streams on the web: a short, muted trace of Scout's
  thinking, then the reply typing into the draft. The desktop app keeps the
  JSON route, whose limit now fits (its bridge waits 100 seconds).
- Platform: the server's 30-second limit becomes a default an operation may
  raise up to 100 seconds (`kernel::time_limit`). The AI gateway streams
  chat completions (`Gateway::chat_stream`), metered and settled however the
  stream ends, and any role may ask for less reasoning
  (`CAMPBOOKS_AI__<ROLE>__REASONING_EFFORT`, unset by default). An
  event-stream answer is never stored by the idempotency layer.

## Capabilities

### New Capabilities

### Modified Capabilities

- `conversations`: replies stream; suggestions as input; plain-text replies;
  `reply_timed_out`.
- `home-ui`: the reply dialog shows Scout thinking and the reply typing in.
- `ai-usage`: streamed calls are metered and settled; reasoning effort.
- `operation-catalogue`: per-operation time limits; event streams are never
  stored for replay.
- `web-bff`: streamed answers pass through unbuffered.
- `observability`: AI calls are timed in the logs.

## Impact

- Module `conversations` (`features/reply`: `mod.rs`, new `stream.rs`,
  `prompt.rs`, `types.rs`, tests, catalogues).
- Platform crate `campbooks-ai` (`gateway.rs`, new `stream.rs`,
  `config.rs`, `telemetry.rs`).
- `campbooks-kernel`: new `time_limit`, `idempotency`.
- `campbooks-server`: uses the kernel's time limit; access matrix.
- Client: `lib/api` (event-stream reader), `lib/ui` (`ThinkingTrace`),
  `modules/conversations` (`streamReply`, `ReplyDialog`, i18n).
- Desktop: the Tauri bridge's timeout goes from 30 to 100 seconds.
- OpenAPI: one new path, `WriteReply.suggestions`, `reply_timed_out` in
  the 503 descriptions. No migration, no new event, no stored data, no new
  crate. ADR 0019.
- No new module and no cross-module dependency.

## Non-goals

- A native streaming bridge for the desktop app (it stays on JSON).
- Streaming likely replies.
- Storing, showing later, or logging Scout's thinking.
- Choosing a reasoning effort for the hosted instance (an ops decision,
  after a side-by-side on demo threads).
