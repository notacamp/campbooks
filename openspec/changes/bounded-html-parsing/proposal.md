## Why
A message's HTML is parsed by html5ever's tree builder three or four times
every time it is read (the style inliner's measure, two ammonia passes, a
fail-closed pass), and once more when search reduces it to text. The tree
builder's work grows with the depth of its stack of open elements times the
tags it reads, so HTML built for it (a long run of `<div></span>`, or of
unclosed `<div>`) costs time quadratic in its size. Measured in a release
build: 58 KiB takes 0.16 s per parse, 585 KiB 15 s, and 2 MiB (the most the
reader reads) 140 s per parse and 180 s for one ammonia pass, so one message
costs about ten minutes of CPU on every read. The read runs in
`spawn_blocking`; the request's 30 s timeout ends the response but not the
work, so repeated reads pile up. Search's 512 KiB cap allows about 20 s, on an
async worker. The caps that exist (nesting, elements) are checked only after
the whole parse. Tracking issue: campbooks-cloud#205.

Ordinary mail is far from this: a 1.5 MiB newsletter of 60,000 elements
parses in 31 ms; sloppy real-world HTML (unclosed `p`, `td`, `li`, `font`)
in under 10 ms.

## What Changes
- Before any tree is built for the reader, the HTML is measured by one
  parse that counts its work as the parsers after it would do it, fed a
  chunk at a time and stopped as soon as the work passes a budget
  (10,000,000 units, about seven times what a dense 2 MiB newsletter costs) or the tree is deeper than 4,096 levels. HTML within the budget is
  sanitised as today. HTML past it is shown as its text: the characters the
  tokenizer reads, outside scripts, styles and the head, with line breaks at
  blocks, escaped, in one `div` that keeps the line breaks. No remote images,
  no `css`.
- Search reduces HTML to text with html5ever's tokenizer alone (no tree, no
  ammonia), in time linear in its size, keeping today's rules: no scripts,
  styles, head or title, no `<blockquote>`s, line breaks at blocks.
- The style inliner reads the measure's numbers instead of parsing again.

## Capabilities
- conversations: modified `Sanitised email HTML`, `Conversations as a search
  source`.

## Impact
- Module: `conversations` only (`domain::sanitize`, `domain::plain_text`).
  No new module, dependency, migration, endpoint, field or event; the API's
  shape is unchanged, so clients are unchanged.
- Ordinary mail renders exactly as before; the measure adds one parse
  (milliseconds) to messages without a style sheet.

## Non-goals
- No new client copy for a message shown as text (a follow-up can add a note
  once there is a field for it).
- No change to what is stored at ingest or to the 2 MiB read cap.
- No change to the inliner's own limits (256 KiB of style text, 20,000
  elements, 256 levels, the work products).
