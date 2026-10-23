# Proposal

## Why

The owner, 2026-10-23 (campbooks-cloud#307): "i dont like how the reply
button is at the bottom far from other action buttons". From 48rem the
reader shows Reply all and Forward in its header, but Reply sat alone in the
footer pinned at the bottom of the reader as "Reply to <name>". The three
ways to answer a message were split across the screen, and the most used
one was the farthest from the others.

## What changes

- From 48rem Reply (R) sits in the reader's header, right before Reply all
  and Forward, in the same style. The three stay together, right after
  Done: when the header is too narrow for one row, Delete, Spam, Pin,
  Unread and Move wrap, not the ways to answer.
- The footer pinned at the bottom holds only the open composer. While the
  composer is closed nothing is pinned there, so the thread gets that room.
- Phones are unchanged: Reply and Done float above the tab bar, and Reply
  all and Forward stay in the header as icons.
- What Reply does is unchanged: the reply dialog first (Scout's likely
  replies), then the composer in the pinned area.

Modules touched: `conversations` (client only). No endpoint, event, scope,
job, migration, dependency or secret.
