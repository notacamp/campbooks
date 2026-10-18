## MODIFIED Requirements

### Requirement: Conversations as a search source
The module SHALL implement the kernel's `SearchSource` port for the group
type `conversations.conversation`: a conversation's container (its inbox),
time (its last message), subject, label names, facets (stream, unread,
starred, archived, label ids) and, per message, its people (from, to, cc
with names and addresses), plain text (the text part, or the HTML part
reduced to text when there is none, quoted history removed, at most
32 KiB; the HTML is reduced by html5ever's tokenizer alone, with no tree,
fed 1 KiB at a time and stopped after 500 ms with the words read so far, keeping the characters outside scripts, styles, the head, the
title, the markup a `noscript`, `iframe`, `noembed` or `noframes` holds, and
`blockquote`s, with a line break at each `br` and at the end of
each block; the head ends where the tree builder would end it, at `</head>`,
`<body>`, any start tag that cannot stand in a head, or words outside its
raw-text elements, so HTML with no `<body>` tag still gives its words),
attachment names, mailbox (`inbox`, `archive`, `spam`, `trash`), direction
and a content stamp made of the hashes of the blobs it is read from, plus,
for a message read from its HTML, the version of how HTML becomes text. A conversation that no longer exists SHALL be reported as gone.
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
