# Suggest people as you type in To, Cc and Bcc

Modules: `conversations` (a new read, `conversations.list_correspondents`)
and the `conversations` client module with the design system's
`RecipientField` (the composer). Tracking:
https://github.com/notacamp/campbooks-cloud/issues/304. No new module and no
cross-module dependency.

## Why

Writing to someone means typing their whole address from memory today. The
composer's To, Cc and Bcc fields take only what is typed: an address you
mistype goes nowhere, and the person you wrote to last week is no easier to
reach than a stranger. Every other mail client suggests the people you
exchange mail with as you type.

`conversations.list_senders` only reads inbound mail, so someone you have
written to but who never wrote back cannot be found, and its ranking (the
senders you replied to most) answers a different question.

## What changes

- **A new read, `conversations.list_correspondents`**
  (`GET /api/v1/w/{workspace_id}/correspondents?q=&limit=`): the people the
  caller exchanges mail with, from the inboxes they can read. Both
  directions: who writes to those inboxes (the sender of inbound mail) and
  who those inboxes wrote to (To and Cc of sent mail). Addresses, display
  names and two counts only (`wrote`, `received`), never a subject or any
  text. `q` is required (2 to 100 characters, every word in the address or
  the name, ignoring case and accents), at most 10 people, no cursor. Bounded
  like `list_senders`: the last year, at most 5,000 received and 5,000 sent
  messages, newest first. Ranked by how often the caller wrote to them, then
  how often they wrote. The caller's own inbox addresses are never listed.
- **Safe to write to, unless you already did.** Someone the caller wrote to
  is always suggested. Someone who only writes in is suggested only when
  `list_senders` with `reachable` would list them: never a no-reply address,
  nor a sender whose newest mail fails its checks or pretends to be them, nor
  one with mail reported, under review or found suspicious and not said safe.
- **The composer suggests people.** From two characters typed in To, Cc or
  Bcc, a list under the field shows up to 8 people (avatar, name, address).
  ↑ and ↓ move, Enter or Tab pick the highlighted one, Esc closes it, a click
  or a tap picks one. People already in To, Cc or Bcc are left out. With the
  list closed, a typed address is added as today. New message, Reply, Reply
  all and Forward's composer all get it.

## Non-goals

- Workspace teammates as suggestions (a follow-up).
- Suggestions before anything is typed.
- Contacts from the provider's address book.
- Teach Scout's "who" picker moving to this read (it keeps `list_senders`).
- Any rule that reads what an email says (ADR 0018): this reads addresses and
  display names only.

## Impact

- `conversations`: the `list_correspondents` feature, its route and
  operation (`conversations:read`, record `conversations.inbox`), the
  `invalid_correspondents_query` reason in the four server catalogues, and a
  shared domain helper for "safe to write to" used by `list_senders` and
  `list_correspondents`. No migration: it reads through the existing
  `messages_inbound_recent_idx`, `messages_outbound_idx` and
  `messages_inbound_sender_idx`.
- Client: `RecipientField` becomes an ARIA combobox when given suggestions;
  `useCorrespondents` and a per-field suggestions hook in the
  `conversations` module, wired into `DraftComposer`; strings in en, pt-PT,
  es-ES and fr.
- OpenAPI document and generated client types.
