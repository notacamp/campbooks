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
  and Forward, the three right after Done.
- Every action after Done is an icon button whose tooltip names it and its
  key ("Reply (R)"). With Reply added, the labelled buttons wrapped onto a
  second row; the owner asked for icons with tooltips or a "more" menu.
  Icons alone still wrapped below about 1280px, so the less used actions
  (Delete, Report spam, Pin, Read/Unread, Previous, Next) fold into one
  "More actions" (…) menu when the header has no room for them: a new
  `lib/ui` component, `OverflowActions`, measures the room it has, so it
  holds in every language and window size. The header is one row at every
  width, phones included (they used to wrap too). Keys work either way.
- Move comes right after Forward; the phone-only copies of Reply all and
  Forward merge into the same group.
- The footer pinned at the bottom holds only the open composer. While the
  composer is closed nothing is pinned there, so the thread gets that room.
- Phones are unchanged: Reply and Done float above the tab bar, and Reply
  all and Forward stay in the header as icons.
- What Reply does is unchanged: the reply dialog first (Scout's likely
  replies), then the composer in the pinned area.

Modules touched: `lib/ui` (new `OverflowActions`, with story and test) and
`conversations` (client only; one new string, "More actions", in en, pt-PT,
es-ES and fr). No endpoint, event, scope,
job, migration, dependency or secret.
