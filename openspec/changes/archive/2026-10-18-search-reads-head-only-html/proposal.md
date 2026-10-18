## Why

Search indexed no text at all for mail whose HTML opens with a head and
never says `<body>` (`<head><meta charset=utf-8><p>Invoice due`): the text
reader left head mode only at `</head>` or `<body>`, unlike the HTML tree
builder (campbooks-cloud#218). The reader shares the reading for its plain
view, so it was affected too.

## What Changes

- The connector stores a text part only when the message has a real
  `text/plain` part. Before, its MIME parser made one out of the HTML with
  its own reading, which for HTML with no `<body>` gave a few line breaks
  that hid the HTML from every reader (found while recording).
- Every reader of a message's words (search, the snippet and Scout's read
  at ingest, the decide pass, the labelling export, reply context, need
  summaries, and the text `conversations.get` returns, which replies and
  forwards quote) reads them one way (`blobs::words`): the text part when it has
  words, else the HTML through `plain_text::text_of`. A text part of blanks
  only counts as none, which also fixes the rows already stored.

- `plain_text::text_of` (search, the reader's plain text, Scout's reply and
  need summaries) ends the head where the tree builder would: at any start
  tag that cannot stand in a head, or at words outside its raw-text
  elements, besides `</head>` and `<body>`. Still the real tokenizer, never
  a regex; scripts, styles, the title and the head's own elements stay out.
- A message with an HTML part carries the reading's version in its search
  stamp, and the search index version goes to 2: each workspace's next
  search or event starts a background backfill, which reads those messages
  again; queries answer from the old rows meanwhile.
- Tests for a head with no body tag, words after the head's tags, a doctype,
  a closed head with no body tag, a link then a div, and Outlook's HTML.

## Impact

- Specs: conversations (Conversations as a search source), search
  (Backfill), connectors (Gmail messages adapter).
- Code: `domain/plain_text.rs`, `features/search_source`, `search::domain::index`.
- No migration. On screen: such mail is found by search, has a preview, and
  its words are quoted when it is forwarded or replied to (before, only the
  forward's header block came along).
