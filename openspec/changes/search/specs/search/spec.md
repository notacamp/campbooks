## ADDED Requirements

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
group SHALL be coalesced and run one at a time, and an entry's text SHALL be
re-read only when its content stamp changed. Event payloads and job payloads
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
treated as text and reported in `query.warnings`, never as an error. A query
over 512 characters, 16 terms or 8 operators SHALL get 422 `query_too_long`.
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
with an opaque `next_cursor`; a cursor SHALL only be valid for the query and
order that produced it (422 `cursor_mismatch` otherwise), and relevance
results SHALL stop after 1,000.

#### Scenario: Next page
- **WHEN** the member asks for the page after `next_cursor` with the same query
- **THEN** the results continue without repeating a conversation

#### Scenario: Cursor from another query
- **WHEN** a cursor from the query "invoice" is sent with the query "receipt"
- **THEN** the response is 422 with code `cursor_mismatch`

### Requirement: Highlights without HTML
Each hit SHALL carry the group, its container, the best-matching entry, the
title and a snippet of the best-matching entry as segments of plain text
each marked as a hit or not, the sender of that entry, its time and the
group's facets (stream, unread, starred, message count, attachment). The
response SHALL contain no HTML, and text from mail SHALL NOT be able to
create or end a highlight.

#### Scenario: Markup in a message
- **WHEN** a matching message's text contains `<b>urgent</b>` and the private-use characters the server uses as highlight delimiters
- **THEN** the snippet's segments contain the text `<b>urgent</b>` verbatim, the delimiters are gone, and only the query's words are marked as hits

### Requirement: Backfill
A workspace's index SHALL be built per source in the background, in pages,
resuming where it stopped, starting at the first event or the first query
seen for that workspace, and again whenever the module's index version
changes. While a backfill runs, queries SHALL answer from what is indexed and
say `indexing: true`. The command `campbooks-server search reindex
[--workspace <id>]` SHALL start one by hand.

#### Scenario: First search in an existing workspace
- **WHEN** a member searches a workspace whose mail arrived before the module existed
- **THEN** the response says `indexing: true`, and once the backfill ends the same query returns the older mail with `indexing: false`

### Requirement: Workspace purge
On `workspaces.workspace_deleting` the module SHALL schedule the deletion of
every row of the workspace in the `search` schema for its `purge_after`.

#### Scenario: Purged
- **WHEN** a deleted workspace's grace period ends
- **THEN** the `search` schema holds no row of it

### Requirement: Queries are not kept
The server SHALL NOT store query text, and SHALL NOT put it in logs, spans
or metrics.

#### Scenario: Telemetry
- **WHEN** a member searches "boiler repair"
- **THEN** no log line, span field or metric attribute contains that text
