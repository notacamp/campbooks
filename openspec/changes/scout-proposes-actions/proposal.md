## Why

campbooks-cloud#305 asks that a chat with Scout can "answer user questions
and take actions". Phase 1 (`continue-with-scout`) answers and looks things
up, changing nothing. A member who learns in the chat that Marta's email is
waiting on them, or that a newsletter should never reach Home, still has to
close the panel and find the button. The approved design: Scout may propose
a change, and nothing changes until the member says yes on a card that
names exactly what and where.

## What changes

- Scout gets three proposing tools: `propose_change` (one low-risk change
  to one conversation, from a fixed list: archive, read, star, label, move
  to a stream, trash, spam, a need's Done / Not needed / Already paid /
  Still to pay / Yes, Later), `propose_rule` (a sentence for Teach Scout)
  and `propose_reply` (an intent for a reply draft). The server never runs
  a change: it checks the proposal, builds the exact request and its undo
  from the catalogue, and stores it with Scout's answer, at most three per
  answer.
- Each proposal is a card under Scout's answer, written by the client from
  the action and the conversation's own facts, with Confirm and Not now.
  Confirm sends the member's own request through the normal API, then
  offers Undo. A rule card opens Teach Scout with the sentence, where the
  rule is confirmed as today. A reply card opens the Reply dialog with the
  intent; Scout never sends.
- `assistant.settle_proposal` records what the member did with a card, so
  Scout knows in the next answers.
- The instruction, the handbook's core section and the panel's line change
  from "changes nothing" to "asks before it changes anything".

## Non-goals

- Changes Scout makes without a yes, even under "act without asking".
- High-risk operations, sending, drafts saved by Scout, rules saved without
  Teach Scout's own confirmation.
- Changes to calendar events and tasks: their modules add proposal entries
  in their own changes (the same seam as the about registry).
- Phase 3: other entry points, recent chats, ⌘K, the activity page.

## Capabilities

### Modified capabilities

- `assistant-chat`: proposing tools, proposals stored with the answer,
  `settle_proposal`, the instruction.
- `assistant-chat-ui`: confirmation cards, rule and reply cards, the
  panel's line.

## Modules

`assistant` (server and client). The client composes the Reply dialog
opener through `lib/`, as it composes the panel's.
