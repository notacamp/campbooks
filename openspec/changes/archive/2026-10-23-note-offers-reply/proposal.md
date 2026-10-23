# Proposal

## Why

campbooks-cloud#306: the reader's note says "it needs a reply" but offers no
way to reply; Reply lives only in the pinned footer (and the phone action
bar). Where the sentence is, the action should be.

## What Changes

- The note in the reader offers **Reply** (R) first in its actions whenever it
  says a reply is needed: a `needs_reply` suggestion that holds (no open need),
  or an open `reply` need (not a question, not a wait/nudge). It opens the same
  Scout reply dialog as the footer and is shown only when the user can reply
  (not in read-only, spam or trash).
- No new strings (the shortcut label "Reply" is reused) and no new shortcut
  handler (the footer's R still does the work).

Modules touched: `clients/app` conversations (reader, model helper
`suggestsReply`). No API change.

## Impact

- Specs: `inbox-ui` (Suggestions shown as suggestions; Needs on rows and in the reader).
