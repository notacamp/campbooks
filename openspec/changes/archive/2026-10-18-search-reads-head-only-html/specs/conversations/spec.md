## MODIFIED Requirements

### Requirement: Conversations as a search source
The module SHALL implement the kernel's `SearchSource` port for the group
type `conversations.conversation`: a conversation's container (its inbox),
time (its last message), subject, label names, facets (stream, unread,
starred, archived, label ids) and, per message, its people (from, to, cc
with names and addresses), plain text (the text part when it has words, or
the HTML part reduced to text when there is none or it holds blanks only,
quoted history removed, at most
32 KiB; the HTML is reduced by html5ever's tokenizer alone, with no tree,
fed 1 KiB at a time and stopped after 500 ms with the words read so far, keeping the characters outside scripts, styles, the head, the
title, the markup a `noscript`, `iframe`, `noembed` or `noframes` holds,
`template`s and `blockquote`s, with a line break at each `br` and at the end of
each block; the head ends where the tree builder would end it, at `</head>`,
`<body>`, any start tag that cannot stand in a head, or words outside its
raw-text elements, so HTML with no `<body>` tag still gives its words),
attachment names, mailbox (`inbox`, `archive`, `spam`, `trash`), direction
and a content stamp made of the hashes of the blobs it is read from, plus,
for a message with an HTML part, the version of how HTML becomes text. The
same reading SHALL give every other reader of a message its words: its
snippet and Scout's read at ingest, the decide pass, the labelling export,
reply context, need summaries, and the text `conversations.get` returns for
each message, which replies and forwards quote. HTML SHALL be read as text
off the async workers; `conversations.get` SHALL read it within one second
for all its messages together, reading at most 256 KiB of each message's
HTML, and a message past that SHALL come without `text`, marked
`text_unread` (the HTML is still whole). Words read from HTML SHALL never
mark a text `truncated`. A conversation that no longer exists SHALL be reported as gone.
Drafts SHALL NOT be returned.

#### Scenario: HTML-only message
- **WHEN** search asks for a message that has only an HTML part
- **THEN** it gets the message's text without tags, scripts or styles

#### Scenario: Quoted history
- **WHEN** a reply quotes the previous message under "On Tue, Ana wrote:"
- **THEN** the text handed to search ends before the quote

#### Scenario: A flag change
- **WHEN** a conversation is marked read and search refreshes it
- **THEN** the content stamps of its messages are unchanged, so no body is read again

#### Scenario: HTML built to be slow to parse
- **WHEN** search asks for an HTML-only message whose HTML would take minutes to build as a tree
- **THEN** it gets the message's text, or the words read within 500 ms

#### Scenario: A head and no body tag
- **WHEN** search asks for an HTML-only message `<head><meta charset=utf-8><title>T</title><p>Invoice due Friday`
- **THEN** it gets "Invoice due Friday" and nothing of the title, styles or scripts, as for a doctype, a full document, a closed head with no body tag and Outlook's HTML

#### Scenario: A text part of blanks only
- **WHEN** a message stored earlier holds a text part of three line breaks beside its HTML
- **THEN** search, its snippet and Scout read the words of its HTML, and its stamp names how HTML is read, so the backfill reads it again

#### Scenario: Forwarding HTML-only mail
- **WHEN** a member forwards or replies to a message that is HTML alone, or one whose text part is blanks
- **THEN** the message's text in `conversations.get` holds the words of its HTML, so the quote carries them

#### Scenario: Crafted HTML threaded into one conversation
- **WHEN** a sender threads a dozen HTML-only messages, each slow to tokenize, into one conversation and a member opens it
- **THEN** the answer comes within a few seconds, each message with its words or marked `text_unread`, none marked `truncated`
