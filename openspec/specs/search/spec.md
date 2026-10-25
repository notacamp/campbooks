# search Specification

## Purpose
One search over every record a member may open (conversations first): a
projection in Postgres kept current from the owning modules' events through
the kernel's `SearchSource` port, words matched in the four shipped
languages with accents ignored, Gmail-style operators, highlights as plain
text, and only the containers the caller may see, checked at query time
(ADR 0016).

## Requirements

### Requirement: Search scope
The `search` module SHALL declare the scope `search:read`, and the
operation `search.query` (`GET /api/v1/w/{workspace_id}/search`) SHALL
require it. A caller who is not a member of the workspace SHALL get 404.

#### Scenario: Anonymous caller
- **WHEN** a client calls `search.query` without an access token
- **THEN** the response is 401 as `application/problem+json`

#### Scenario: Another workspace
- **WHEN** a member of one workspace queries another workspace's search
- **THEN** the response is 404

### Requirement: Results only from what the caller may open
A query SHALL return only groups whose container the caller may see, as the
container-visibility port lists them for that caller at query time (a user:
the inboxes they are a member of; a client token bound to the workspace:
every inbox), and only of group types the caller holds a read scope for.
Every table of the `search` schema SHALL be isolated per workspace by forced
row-level security.

#### Scenario: A member of another inbox
- **WHEN** Sam's inbox holds a message about "boiler repair" and Rui, a member of the workspace but not of that inbox, searches "boiler"
- **THEN** Rui's results contain nothing from Sam's inbox

#### Scenario: Removed from an inbox
- **WHEN** a member is removed from an inbox and searches again at once
- **THEN** no result from that inbox is returned, without waiting for any reindex

#### Scenario: Search scope alone
- **WHEN** a client token holds `search:read` but no scope of an operation that reads conversations
- **THEN** its queries return no conversation results

### Requirement: Kept current from sources
The module SHALL keep its index current by consuming
`conversations.message_received`, `conversations.message_sent` and
`conversations.conversation_changed`, and for each, refreshing the affected
group from the current state the group's `SearchSource` returns: upserting
the group and its entries, deleting entries the source no longer returns,
and deleting the group when the source says it is gone. Refreshes of one
group SHALL run one at a time, and an entry's text SHALL be re-read only when
its content stamp changed. Event payloads and job payloads
SHALL hold ids only.

#### Scenario: A new message is findable
- **WHEN** a message with the subject "Fatura de outubro" is ingested
- **THEN** within seconds a search for "fatura" returns its conversation

#### Scenario: Deliveries out of order
- **WHEN** a conversation is starred and then unstarred, and the two `conversation_changed` deliveries run in reverse order
- **THEN** the index ends with the conversation not starred

#### Scenario: A removed message
- **WHEN** a message is removed from a conversation at the provider
- **THEN** its text no longer matches any query, and the conversation's other messages still do

### Requirement: Languages and partial matches
Entries SHALL be indexed in their detected language among English,
Portuguese, Spanish and French (or a language-neutral configuration), with
accents ignored. A query term SHALL match its stemmed forms in every one of
those languages and its unstemmed form. `from:`, `to:` and `cc:` SHALL match
part of a name or an address, accent-insensitively.

#### Scenario: Accents and stems
- **WHEN** a Portuguese message says "As faturas de João" and the member searches "fatura joao"
- **THEN** its conversation is returned

#### Scenario: Partial sender
- **WHEN** the member searches `from:lope`
- **THEN** conversations with messages from "Ana Lopes <ana.lopes@example.com>" are returned

### Requirement: Query syntax
`q` SHALL accept free words, `"phrases"`, `-word`, `a OR b` and the
operators `from:`, `to:`, `cc:`, `subject:`, `has:attachment`, `in:` (a
stream, `inbox`, `archive`, `spam`, `trash` or `anywhere`), `is:` (`unread`,
`read`, `starred`), `label:`, `before:`, `after:`, `older_than:` and
`newer_than:` (`d`, `w`, `m`, `y`), with dates in the caller's `tz`. Messages
in spam or trash SHALL be excluded unless `in:spam`, `in:trash` or
`in:anywhere` asks for them. An unknown operator or a bad value SHALL be
treated as text and reported in `query.warnings`, never as an error; dates
SHALL be years 1 to 9999. A query that only excludes (`-word` and no
filter) SHALL get 400 `query_only_excludes`. A query
over 512 characters, 16 terms or 8 operators SHALL get 400 `query_too_long`.
The response SHALL return the parsed filters in `query.filters`. User text
SHALL reach the database only as bound parameters.

#### Scenario: Operators combined
- **WHEN** the member searches `from:ana has:attachment is:unread after:2026-09-01 invoice`
- **THEN** only unread conversations with a message from Ana carrying an attachment, after 1 September 2026 in the caller's time zone, and matching "invoice" are returned, and `query.filters` lists the four filters

#### Scenario: Half-typed operator
- **WHEN** the member's query is `invoice from:`
- **THEN** the response is 200 with results for "invoice" and a warning for the empty `from:`

#### Scenario: Trash excluded
- **WHEN** a matching conversation's only message is in the trash
- **THEN** it is returned for `in:trash` and not for the same query without it

### Requirement: As you type
With `prefix=true`, the last free word of two or more characters SHALL match
as a prefix.

#### Scenario: Partial last word
- **WHEN** the member has typed "boiler rep" with `prefix=true`
- **THEN** conversations mentioning "boiler repair" are returned

### Requirement: Order and pages
`sort=newest` SHALL order groups by their time, newest first;
`sort=relevance` (the default) SHALL order them by text relevance weighted
by field (subject, then people and attachment names, then body) and by
recency. Results SHALL come in pages of `limit` (20 by default, at most 50)
with an opaque `next_cursor`, signed with a key derived from the instance
secret; a cursor SHALL only be valid for the caller, query and order that
produced it (400 `cursor_mismatch` otherwise, also when it was altered), and relevance
results SHALL stop after 1,000.

#### Scenario: Next page
- **WHEN** the member asks for the page after `next_cursor` with the same query
- **THEN** the results continue without repeating a conversation

#### Scenario: Someone else's cursor
- **WHEN** Rui sends a cursor Sam's search returned
- **THEN** the response is 400 with code `cursor_mismatch`

#### Scenario: Cursor from another query
- **WHEN** a cursor from the query "invoice" is sent with the query "receipt"
- **THEN** the response is 400 with code `cursor_mismatch`

### Requirement: Highlights without HTML
Each hit SHALL carry the group, its container, the best-matching entry, the
title and a snippet of the best-matching entry as segments of plain text
each marked as a hit or not, the sender of that entry, its time and the
group's facets (stream, unread, starred, message count, attachment). The
response SHALL contain no HTML, and text from mail SHALL NOT be able to
create or end a highlight.

#### Scenario: Markup in a message
- **WHEN** a matching message's text contains `<b>urgent</b>` and the private-use characters the server uses as highlight delimiters
- **THEN** the snippet's segments are plain text holding the word "urgent" without the tags, the delimiters are gone, and only the query's words are marked as hits

### Requirement: Backfill
A workspace's index SHALL be built per source in the background, in pages,
resuming where it stopped, starting at the first event or the first query
seen for that workspace, and again whenever the module's index version
changes (as when a source starts reading something it read wrongly before;
an entry whose content stamp changed is then read again). While a backfill runs, queries SHALL answer from what is indexed and
say `indexing: true`, without waiting for the page in progress. The command `campbooks-server search reindex
--workspace <id>` SHALL start one by hand.

#### Scenario: A search during a backfill page
- **WHEN** a backfill page is half-way through, its transaction open, and the member searches
- **THEN** the search answers at once with `indexing: true`

#### Scenario: First search in an existing workspace
- **WHEN** a member searches a workspace whose mail arrived before the module existed
- **THEN** the response says `indexing: true`, and once the backfill ends the same query returns the older mail with `indexing: false`

#### Scenario: A better reading of old mail
- **WHEN** the index version changes because HTML is read better, and a member searches a workspace indexed before
- **THEN** the search answers from the old rows with `indexing: true`, and once the backfill ends the words of mail read from HTML are findable

### Requirement: Workspace purge
On `workspaces.workspace_deleting` the module SHALL stop indexing the
workspace at once and schedule the deletion of its index for its
`purge_after`; a delivery that arrives later SHALL index nothing and start
no backfill.

#### Scenario: Purged
- **WHEN** a deleted workspace's grace period ends
- **THEN** the `search` schema holds no row of it

### Requirement: Queries are not kept
The server SHALL NOT store query text, and SHALL NOT put it in logs, spans
or metrics.

#### Scenario: Telemetry
- **WHEN** a member searches "boiler repair"
- **THEN** no log line, span field or metric attribute contains that text

### Requirement: A removed inbox leaves the index
On `conversations.inbox_removed`, the index SHALL forget every group in
that inbox.

#### Scenario: Disconnected and deleted
- **WHEN** an inbox is removed
- **THEN** none of its conversations is found by any caller, and other inboxes are untouched

### Requirement: Cursors and progress reveal nothing across scopes
A cursor SHALL be bound to its workspace as well as its caller and query,
and used in another workspace SHALL answer `cursor_mismatch`. Whether the
index is still being built (`indexing`) SHALL be reported only to a
caller who may see some container.

#### Scenario: The same caller, another workspace
- **WHEN** a member uses a cursor from workspace A in a search of workspace B
- **THEN** the answer is 400 `cursor_mismatch`

### Requirement: Pages are searchable
With pages enabled, search SHALL index each page's title and the plain text of
its blocks through the `pages` module's `SearchSource`, and SHALL answer a
page only to a caller who may read it. A trashed page SHALL leave the index,
and a restored one SHALL return to it.

#### Scenario: A page the caller may not read
- **WHEN** Rui searches for a word that appears only on Ana's private page
- **THEN** the page is not among Rui's results

#### Scenario: A trashed page
- **WHEN** Ana trashes a page and then searches for its title
- **THEN** the page is not among her results
