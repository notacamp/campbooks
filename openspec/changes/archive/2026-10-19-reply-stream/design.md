# Design

## 1. Decisions taken

- **The look is approved.** The owner chose "Stream it" with its preview
  (a thinking block on top, the reply typing in below) on 2026-10-19.
  Screens built on the approved Next design system are reviewed in the PR
  (owner, 2026-10-13), so there is no separate prototype round.
- **Plain text for the written reply.** Pulling a JSON string out of a
  stream means handling escapes split across chunks and repairing broken
  JSON, and schema adherence is patchy on reasoning models served through
  a router. The mail is still JSON data in the user message, and the answer
  is still bounded and filtered (`prompt::reply`: 4,000 characters, no
  control or invisible direction characters) and only ever shown as text,
  so safety is unchanged. Likely replies keep their schema.
- **Reasoning effort is a setting, unset by default.** Streaming already
  fixes how slow it feels; whether `low` keeps the register-matching
  quality is measured on demo threads before an instance turns it on.
- **The desktop app stays on JSON.** Its bridge hands back whole answers;
  a streaming bridge is a separate piece of work. Its timeout goes from 30
  to 100 seconds so the 90-second route limit applies there too.

## 2. Alternatives

a) **Only raise the limit.** Ends the cut-off but leaves the member staring
at a spinner for up to a minute and a half. Rejected on its own; kept for
the JSON routes.

b) **A job and polling.** The work would outlive the request, which needs a
place to keep the answer (stored model text) and a poll loop. More moving
parts, nothing stored today. Rejected.

c) **Server-Sent Events over the POST.** One request, the same checks, the
same body, an answer that starts within a second. Chosen.

d) **A WebSocket.** Two-way is not needed, the BFF passes HTTP bodies
through already, and the change feed is SSE. Rejected.

## 3. The operation

`POST /api/v1/w/{workspace_id}/conversations/{conversation_id}/reply/draft/stream`,
`conversations.stream_reply`, scope **`conversations:send`**, access
`conversations.inbox` / `send`, a low-risk command (not mutating anything:
nothing is stored or sent). Request body: `WriteReply`, as
`conversations.write_reply`.

Before the response head, as ordinary problem answers: the scope, read
access to the conversation (404), send access to its inbox (403
`inbox_read_only`), a `complete` role that may read mail (409
`reply_unavailable`), the member's hourly allowance (429), the body's
bounds (400 `reply_ask_invalid`), and the daily AI limit (503
`ai_daily_limit_reached`, from the gateway's admission, which runs inside
the usage scope before the stream exists). A provider that refuses the
request (a non-2xx status) or does not answer within 30 seconds is also an
ordinary problem (503 `reply_failed` / `reply_timed_out`).

Then `200 text/event-stream` with `x-accel-buffering: no` and a comment
every 10 seconds. Events, each `data` a JSON object:

| event | data |
|---|---|
| `thinking` | `{"delta"}`: the model's reasoning text, filtered; forwarding stops after 8,000 characters |
| `text` | `{"delta"}`: the reply's words, filtered; forwarding stops after 4,000 characters |
| `done` | `ReplyText` `{message_id, text}`: the whole reply as `prompt::reply` keeps it |
| `error` | a problem (`reply_failed` or `reply_timed_out`) in the caller's language |

The stream ends after `done` or `error`. Its limits: 30 seconds with no
sign of life from the model (any chunk counts, including reasoning it does
not show, which the gateway passes on as `ChatEvent::Alive` and the module
never forwards), 2 minutes from the head. An empty reply, one the model
stopped for length, or a stream cut off before the model said why it
stopped (no `[DONE]` and no `finish_reason`) ends with `reply_failed`.

`Idempotency-Key` changes nothing for it: an event-stream answer is never
stored, and its key is released as soon as the head is sent.

## 4. Time limits

The server's limit becomes `kernel::time_limit`: every request gets a
`Deadline` (30 seconds), which a handler may raise for itself up to
`MAX` = 100 seconds, never lower. `MAX` stays under the idempotency layer's
2 minutes, after which a pending key counts as abandoned. The limit covers
the response head only, so a stream past it is not cut. The BFF carries the
same `Deadline` into the in-process request (extensions are kept).

| route | route limit | model timeout |
|---|---|---|
| `suggest_replies`, `write_reply` | 90 s | 80 s, then `reply_timed_out` |
| `stream_reply` head | 60 s (reading the conversation, then the model) | 30 s to the first byte, then `reply_timed_out` |
| `stream_reply` body | n/a | 30 s idle, 2 min from the head (module), 160 s from the request (gateway: head wait + body + margin) |

## 5. The gateway

`Gateway::chat_stream(role, request)` posts with `stream: true` and
`stream_options.include_usage`, OpenAI-compatible providers only. A server
that ignores `stream` and answers JSON is read as one text event. The SSE
parser reads `delta.content` and the reasoning from `delta.reasoning`, else
`delta.reasoning_content`, else the text of `delta.reasoning_details`
entries (encrypted ones skipped). Text starting with `<think>` is read as
reasoning up to `</think>` (text hygiene for self-hosted servers without a
reasoning parser, not a decision). A mid-stream `error` chunk ends the
stream without its text. The buffer is capped at 1 MiB.

Metering: admission runs inside the caller's usage scope before anything is
sent; the call is settled with its usage when the stream ends, as failed
when it errs, and as failed (`error.type=cancelled`) when it is dropped
early (the member closed the dialog). Each stream logs one line when it
ends: outcome, time to first token, elapsed time, characters of reasoning
and text, why it stopped and token counts. Never content.

Reasoning effort: `CAMPBOOKS_AI__<ROLE>__REASONING_EFFORT` (`minimal`,
`low`, `medium`, `high`) sends `reasoning: {"effort": …}`, the form
documented by OpenRouter, on chat and streamed chat calls. Only for a
provider that documents it: a strict OpenAI-compatible server rejects it.

## 6. Data ownership and events

Nothing new is stored: no table, no migration, no event, no job. The thinking
and the reply exist only in the SSE response. The prompt, the deltas, the
thinking and the reply text never appear in logs, spans or metrics.

## 7. AI use

Role `complete`. When it is not configured or may not read mail, the reply
is unavailable (409 `reply_unavailable`) exactly as before and the member
writes it. The result is a draft the member reads, edits and sends; nothing
is sent or saved for them, so there is nothing to undo.

## 8. The client

`readEventStream` reads frames from a response body. `streamReply` calls the
stream on the web (through the BFF, with its CSRF header) and falls back to
`writeReply` on the desktop. The dialog keeps an `AbortController` per
request and aborts it on a new ask, Back and close. While Scout thinks, a
`ThinkingTrace` (muted, about five lines, pinned to the bottom, `role=status`
for its label only) shows the thinking; on the first word the dialog moves
to the draft, which types in until `done` replaces it. A rewrite keeps the
old text until the first word, and puts it back on failure. The trace
collapses into a native disclosure once the reply is written.

## 9. Risks

- Proxies that buffer: nginx honours `x-accel-buffering: no`; Caddy streams
  `text/event-stream`. The change feed already streams in production.
- Thinking may quote the mail. It is shown only to someone who may read the
  conversation, as plain muted text, and never kept.
