# Proposal

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/168

## Why

The owner, 2026-10-17: "why does it, when i click reply, not propose an ai
reply?" Today Reply (R, the Reply button on a Home item, the reader's Reply)
opens the plain composer. "Draft it for me" exists only as a shortcut label
and in the `lib/ui` Composer, wired to nothing. The approved Home actions
prototype (campbooks-cloud#164, v5) has Reply open a dialog where Scout
offers likely replies and writes the one the member picks.

## What Changes

- **Reply opens a dialog**, on Home and in the reader: "Reply to Marta",
  two or three likely replies as short intents ("Yes, Thursday 10:00
  works", "Ask to move it to the afternoon"), a box to say it in your own
  words, and "Write my own", which opens the plain composer as today. Keys
  1 to 3 pick an intent, Esc closes with nothing changed, and below 48rem
  it is a bottom sheet.
- **Scout writes the draft only on a click.** Picking an intent (or typing
  your own words) asks the `complete` role to write the reply for it, in
  the conversation's language, signed with the member's name. Nothing is
  pre-generated.
- **The draft is shown in the dialog, editable**, with "Shorter" and
  "Change…" (a plain-words instruction such as "warmer"), both rewritten by
  the `complete` role and marked with the visor. "Send" saves it as a reply
  draft and sends it through the existing send path with its undo window:
  the dialog shows "Sending to Marta" for a moment, the Home item folds with
  its confirmation, the toast keeps Undo (Z), and an undone send brings the
  dialog back with the draft. "Open in composer" moves the draft into the
  regular composer. Nothing is ever sent without the member's Send.
- **Two new operations in `conversations`**:
  `conversations.suggest_replies` (likely replies for a message of a
  conversation) and `conversations.write_reply` (the reply for an intent,
  or a rewrite of the member's draft). Both read the thread with the same
  checks as reading the conversation, need send rights on its inbox (a
  read-only or kept inbox gets 403 `inbox_read_only`), count against the
  workspace's daily AI limit (interactive share) and a per-member hourly
  limit, and answer 409 `reply_unavailable` when no `complete` model may
  read mail on this server; the dialog then offers only "Write my own" with
  a line saying why.
- **Home offers no Reply on an item whose inbox the member may only read.**

## Non-goals

- No "Say yes and add to calendar" combination and no calendar write: the
  calendar is not in the rewrite yet (a later issue of the epic).
- No ranking of Home actions by the model (campbooks-cloud#166 owns that):
  Reply stays the need's own first action.
- No writing-style learning and no owner preferences in prompts. A
  preference becomes a smart rule, never prompt text.
- No pre-drafted replies, no background drafting, no stored suggestions.
- Reply all and Forward keep opening the plain composer.
- No streaming of the draft as it is written.

## Impact

- Modules: `conversations` (server: a new `reply` feature slice with two
  operations, its catalogue reasons in four languages; client: the
  `ReplyDialog`, its API hooks, the reader's Reply), `attention` (client:
  Home's Reply opens the dialog). No new module, no cross-module
  dependency, no migration, no event.
- `lib/ui`: a new `ReplyDraft` component set (the editable draft with its
  rewrite buttons, the ask line, the quoted context), with stories and
  tests.
- AI: `complete` role only, through `campbooks-ai`; metered like every call
  (campbooks-cloud#152); the external-content gate applies.
- OpenAPI, generated client types, access matrix, CHANGELOG, a
  `reply-dialog` recording scenario and a fake `complete` model for it
  (`scripts/fake-complete.ts`).
