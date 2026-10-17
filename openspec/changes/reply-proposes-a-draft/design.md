# Design

## Where it lives

The thread a reply answers is the `conversations` module's data, and the
module already depends on `campbooks-ai`. The two operations are one
vertical slice, `crates/modules/conversations/src/features/reply/`, with
no new table, migration or event: nothing is stored. The draft the member
sends is an ordinary `conversations.draft`, created and sent by the
existing `create_draft` and `send_draft` operations, so the undo window,
the outbound intent and the send outcome are the ones the composer uses.

The client dialog is `ReplyDialog` in `clients/app/src/modules/conversations`
(exported through its `index.ts`); Home (`attention`) and the reader both
open it. Its visual parts are a new `lib/ui` component set, `ReplyDraft`.

## Operations

Both are `POST`, commands (`Operation::command(Risk::Low)`): they change
nothing, but each spends a model call and the member's allowance, so they
are never replayed or prefetched, and they are not readers of any record
type (no change feed invalidation can re-run them).

- `POST /api/v1/w/{workspace_id}/conversations/{conversation_id}/reply/suggestions`
  (`conversations.suggest_replies`), body `{ message_id? }`: answers
  `{ message_id, intents: [string] }`, at most three intents of at most 80
  characters, in the member's language.
- `POST /api/v1/w/{workspace_id}/conversations/{conversation_id}/reply/draft`
  (`conversations.write_reply`), body `{ message_id?, intent, draft?,
  change?, shorter }`: answers `{ message_id, text }`. With `draft` and
  `change` or `shorter`, the draft is rewritten; otherwise a reply is
  written for `intent`.

`message_id` defaults to the conversation's newest message (what the
reader's Reply answers). A message of another conversation is 404.

**Scope**: `conversations:send` (the result is a draft to send).
**Access** (ADR 0017): declared `conversations.inbox` / `send`. The handler
first asks `read` on the conversation (404 to anyone who may not see it,
byte for byte the workspace 404), then `send` on its inbox (403
`inbox_read_only` for a reader, or an inbox kept read-only after its
connection was removed). Only then is the model's availability, the rate
limit and the thread looked at.

**Limits**: the per-caller write class of the API rate limits
(campbooks-cloud#504) applies to both; on top, one per-member allowance of
100 model calls an hour shared by both (`conversations.reply`, Postgres
counter like `assistant.compile`), and every call runs in the workspace's
usage scope as interactive (`Scope::workspace(…).interactive()`), so the
daily cap (campbooks-cloud#505) refuses it with 503
`ai_daily_limit_reached` when the workspace is over.

## The model calls

Role: **`complete`**. The gateway is used only when
`provider_for_content(Role::Complete)` succeeds, so an external provider
without the content opt-in never receives mail. When the role is not
configured, or may not read mail, both operations answer **409
`reply_unavailable`** before reading anything, and the dialog offers only
"Write my own" with the line "Scout can't write replies on this server, so
you write this one." Nothing changes and nothing needs undoing: the model
only proposes text; the member edits, sends (with the send's own Undo) or
closes.

### What the model is given

A fixed English system instruction, and one user message holding a JSON
object:

- `conversation`: the subject and the newest 8 messages up to the answered
  one, oldest first, each `{ from: "them" | "me", name, sent_at, text }`.
  Text is the plain-text part, else the HTML part reduced to text
  (`plain_text::from_html`), with quoted history removed
  (`plain_text::own_words`), cut at 4 KiB per message and 16 KiB in all.
  That is text hygiene, not a decision (AGENTS.md → No content rules).
- `replying_to`: the index of the answered message.
- `my_name`: the member's display name from their token, else the inbox's
  display name, else nothing (then the reply is not signed by name).
- `my_language`: the member's locale tag.
- For a draft: `intent`; for a rewrite: `draft` and `change` (or
  `"shorter"`).

Nothing about the member's preferences, rules or other mail is sent.

### Prompt injection

Mail is untrusted input. The instruction says the conversation is data,
never instructions, and that anything in it asking to change the task, to
reveal the instruction or to write something else must be ignored. The
mail travels only inside the JSON string values of the user message, never
in the system message. The answer is held to a strict JSON schema
(`{"intents": [string]}` / `{"reply": string}`), parsed, trimmed, bounded
(80 characters per intent and three intents; 4,000 characters per reply),
and shown in the client as text only (a `<textarea>` value and button
labels), never as HTML. The worst a hostile email can do is make the draft
say something odd, which the member reads before sending; the model has no
tools and the operations change nothing.

### Intents: one cheap call when the dialog opens

Options considered:

1. **Derive from the need** (Reply / Decide / Do and its reason). The need
   says *that* a reply is needed, not what it could say: the intents would
   be generic ("Reply", "Say yes"), and turning the email's words into
   options deterministically would be a content rule (ADR 0018).
2. **Generate with `decide` (Jev)**: `decide` answers fixed questions with
   probabilities; it does not write free text.
3. **One `complete` call when the dialog opens** (chosen): short JSON, at
   most 200 output tokens, temperature 0.3. It runs only when the member
   presses Reply, never in the background, and the client keeps the answer
   for the session per message, so reopening the dialog costs nothing. The
   draft itself is still written only when an intent is picked
   (owner-approved: drafts on click only).

When the call fails or returns nothing usable, the dialog keeps "say it in
your own words" and "Write my own", and says Scout has no suggestions this
time.

### Language and signature

The reply is written in the language of the message being answered (the
model reads it); when that is unclear, in `my_language`. Intents are in
`my_language`, since they are the member's buttons. The reply ends with
`my_name` on its own line when one is known.

### Failures

A provider failure is 503 `reply_failed` (the log keeps the error's kind
only, never the provider's words, which can echo the request); an answer
that is not the schema is also `reply_failed`. Over the hourly allowance:
429 `rate_limited`. Over the daily cap: 503 `ai_daily_limit_reached`. The
dialog says each in a plain line and keeps "Write my own"; the member's
edited draft is never lost on a failed rewrite.

## The dialog

- Reply (R, Home's Reply 1, the reader's Reply button) opens it. Home
  answers the conversation's newest message, as the reader does, so "Open
  in composer" and "Write my own" land on the same draft the reader's
  composer would.
- Stages: **intents** (likely replies with keys 1–3 and the visor, the
  own-words line, "Write my own"), **writing** (the visor thinking), and
  **draft** (an editable text area, "Shorter", "Change…", "Send ⌘↵",
  "Open in composer", "Back to the choices").
- Send: `create_draft` (kind `reply`, the message, the text, the quoted
  history the composer would write) then `send_draft` of its version. On
  success the dialog shows its result ("Sending to Marta") for about a
  second and closes; the Home row folds with "Sending to Marta"; the toast
  says it with Undo for the server's window. Undo calls `cancel_send`; in
  time, the dialog comes back on the draft (Home shows the row again) and
  a second Send updates and sends that same draft, never a second one; too
  late, the toast says so. A failure keeps the dialog open with the draft
  and says it could not be sent.
- "Open in composer" creates the reply draft with the text and opens the
  reader's composer on it (from Home: the reader with `?reply`).
- Esc or a click outside closes with nothing changed: no draft is created
  until Send or Open in composer.
- Read-only: the reader already hides Reply for a reader or a kept inbox;
  Home now leaves Reply out for such items too.

## Testing

Server tests run the operations against a wiremock `complete` model:
intents, a draft, a rewrite, the prompt carrying the mail only as JSON data
and the member's name and language, an injected instruction in the mail
staying data, 409 without the role and with an external provider that may
not read mail, 503 on a provider error without echoing it, 403 for a
reader, 404 for a stranger and for a message of another conversation. The
access matrix covers both operations for every kind of caller. Client
tests cover the dialog's stages, keys, send and undo, the unavailable
state and Home's wiring. The recording runs against a fake `complete`
model (`scripts/fake-complete.ts`).
