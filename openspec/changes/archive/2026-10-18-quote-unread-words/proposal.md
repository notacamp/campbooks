## Why

`conversations.get` reads each message's HTML as text within one shared
second, at most 256 KiB each (campbooks-cloud#218). A message past that comes
without `text`, marked `text_unread`. The client did not read that flag:
Reply and Forward quoted such a message with an empty quote, silently, and
its folded line in the reader was blank (campbooks-cloud#250).

## What Changes

- `conversations.get_message_body` takes `words=true`: the message's words
  alone, as `text` (its text part when it has words, else its HTML read as
  text with the usual half-second bound), no `html`, `truncated` when they
  were cut at 256 KiB. Same access as the body; no new operation.
- The client asks for those words, once per message (they never change),
  whenever a message is marked `text_unread`: before quoting it in the
  composer's reply, reply all or forward, before Home's Forward picker
  starts its draft, in the reply dialog before saving its reply, and for the
  message's folded line in the reader.
- While they are read, a neutral line says so ("Reading the email to quote
  it…", "Reading…"). When they cannot be read, nothing is started or sent
  and the member is told why; a forward never goes out with an empty quote.
- A debug-only `CAMPBOOKS_DEV_TEXT_DEADLINE_MS` replaces the one-second
  deadline of `conversations.get`, so a recording can force `text_unread`
  deterministically (0 marks every message read from HTML). Release builds
  never read it, and it is off when unset. No production knob.

## Impact

- Specs: conversations (A message's words alone), inbox-ui (Quoting mail
  whose words came later).
- Code: `features/get_message_body`, `features/get_conversation`,
  `openapi.json`, the client's `queries.ts`, `ReplyComposer`, `ForwardDialog`,
  `ReplyDialog`, `ThreadMessage`; i18n en, pt-PT, es-ES, fr.
- Recording scenario `forward-text-unread`. No migration.
