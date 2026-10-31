# Design

## Data ownership and events

`conversations.message_reads` is unchanged. No event is added or changed. The
tracking plan is unchanged (no event, no Coverage row).

## AI use

Role: `complete`. If the role is not configured, nothing changes (no read is
queued). The member undoes nothing here: it only changes how the model is
asked, and what the reader says when the model gave no answer.

## Gateway

- `ChatRequest.reasoning_effort: Option<ReasoningEffort>`; `WireChatRequest`
  sends the request's effort when the provider accepts the object
  (`ProviderConfig::accepts_reasoning`: `reasoning` is true, or the role has
  its own `reasoning_effort`), else the role's own effort, else nothing. The
  setting is opt-in because the existing rule stands: a strict
  OpenAI-compatible server rejects the object, and nothing detects a provider.
  Prod sets `CAMPBOOKS_AI__COMPLETE__REASONING=true` (OpenRouter takes it).
- `ChatResponse::ran_out_of_room`: `finish_reason=length`, no content, no tool
  calls.
- `Gateway::chat_with_room`: `chat`, and once more with
  `ChatRequest::with_more_room` (double `max_tokens`, low effort) when the
  answer ran out of room. Both calls are metered and held to the daily cap
  (#505) like any call; over the cap the job waits as it does now.

## Measurement

See the report. Chosen: low effort, `max_tokens` 8192 (arm D).

## Other features capped at 4096 on `complete`

Checked, left as they are, because prod showed no empty answers for them in
the same 7 days:

- `reply/prompt.rs` (`MAX_TOKENS` 4096): suggestions and the written reply.
- `assistant/.../compile/prompt.rs` (`MAX_TOKENS` 4096): Teach Scout.
- `assistant/.../chat/prompt.rs` is already 8192.

The `rule_notices` grouping had one empty answer in prod; it gets the same
low effort and 8192 as the read, with the same retry.

## A failed read on a newer message

A read belongs to a message. `needs::refresh` queues
`conversations.read_message` when a need opens on a message (a new
`need.message`) that the decide pass flagged, and the read gets its own row
and its own `MAX_CALLS`. So when a newer message arrives in the conversation
and raises the need, it is read afresh, whatever happened to the older
message's read; one read per new message, under the same daily cap. Nothing
queues a second read of the same message: `MAX_CALLS` stays 2 per message.

## API

`conversations.get` (scope `conversations:read`, unchanged) gains
`need_read_failed: boolean`, present (true) only when the open need's message
has a model read that is `failed` after both calls and the need is not a
phishing warning. A failed read with one call left, a pending or a finished
read says nothing.

## Client

The note of a conversation with an open need shows the muted line
`scout.readFailed` under the need's sentence when `need_read_failed` is true.
It uses the `Text` tertiary tone and the note's `after` slot. Storybook:
`Screens/Inbox` `ThreadReadFailed` (with a play) and
`ThreadReadFailedRecording` (no play, for the recording).
