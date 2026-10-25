# Proposal

## Why

The owner, 2026-10-25 (campbooks-cloud#355): a thread they had already
answered kept telling them to reply. The thread's need had cleared as
Replied, and Home and Tasks were clean, but the reader's note still said
"Campbooks' read, as suggestions: it needs a reply" and offered Reply.

With no open need, the note falls back to the decide pass's answers. Those
answers are about the conversation's newest inbound message, and the note
never looked at whether the member had written since. A reply clears a
Reply or Do need (the attention handbook, "Replied"), but the note kept
suggesting the reply that had already been sent.

## What changes

- When the member sent a message after the conversation's newest inbound
  one, the note says so: "You replied on 23 Oct, so nothing here waits on
  you." It no longer says the message needs a reply, asks for something,
  names a date or looks urgent, and it offers no Reply.
- A warning stands: if the decide pass found the message suspicious, the
  note still says it after the reply.
- With an open need (a wait, a nudge, a bill) the note is unchanged: the
  need says what is open.

Modules touched: `conversations` (client only: `repliedSince` in the model,
the reader's note, one new string in en, pt-PT, es-ES and fr). No endpoint,
event, scope, job, migration, dependency or secret.
