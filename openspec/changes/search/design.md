# Design

## Context

What exists on `next`:

- `conversations` keeps metadata in Postgres (`messages.subject`, `snippet`,
  `sender_address`, flags, `bodies` as blob refs) and the text in blobs: the
  plain-text part, the HTML part and a per-message **participants document**
  (from, to, cc, reply-to, subject). Its events carry ids and blob pointers,
  never personal data. A conversation's flags (stream, unread, starred,
  archived, trashed) travel in `conversation_changed.state`; labels don't.
- Every workspace table has forced row-level security on
  `campbooks.workspace_id`. Within a workspace, inbox membership decides what
  a user sees (`visible_inboxes`); a client token bound to the workspace sees
  every inbox. The change feed asks the kernel's `Visibility` port one
  container at a time.
- The durable event bus fans out to consumers at publish time; deliveries of
  one event type are not ordered relative to each other (ADR 0012). The event
  log will get a retention window and a per-workspace purge (#65), so replay
  cannot be the only way to rebuild a projection.
- `campbooks-ai` serves `embed` over the OpenAI-compatible API and `decide`
  over Laya. Neither is guaranteed to be configured; on a CPU host `embed` is
  cheap per call but not per mailbox.
- Postgres is 17 from `pgvector/pgvector:pg17`, which also ships the contrib
  extensions. No extension is installed yet.
- Production runs Next on one ARM host. The legacy Rails stack beside it has
  OpenSearch; Next does not use it.

## Decision 1: the engine

**Chosen: Postgres full-text search, with pgvector added later for a hybrid
ranking (ADR 0016).**

- Text search configurations `search.en`, `search.pt`, `search.es`,
  `search.fr` (copies of the built-in Snowball configurations with
  `unaccent` before the stemmer) and `search.simple` (`unaccent` +
  `simple`, for names, codes and text in no detected language).
- Each entry is indexed once, in its own language, detected from its text
  with `whatlang` limited to the four (below a confidence floor:
  `search.simple`). A query term is expanded to the OR of its forms under the
  five configurations, so a Portuguese query finds an English message and
  "faturas" finds "fatura".
- `pg_trgm` indexes the people text (names and addresses) and titles, for
  `from:ana`, partial addresses and, later, typo tolerance.
- Freshness is a job away (sub-second to seconds), in the same database,
  under the same row-level security, in the same backup.

| | Postgres FTS (+ pgvector later) | Embedded index (tantivy in the worker) | A service (OpenSearch / Meilisearch) | Hybrid lexical + semantic from day one |
| --- | --- | --- | --- | --- |
| Self-host weight | none: same Postgres | none to run, but an index directory per process to keep and back up | one more service, volume, version and backup; OpenSearch wants a 1–2 GB JVM heap | embed model server required for the semantic half |
| Multilingual mail | Snowball stemmers for all four, `unaccent`; ranking is `ts_rank_cd`, not BM25 | BM25, stemmers for all four, better ranking out of the box | OpenSearch: analyzers per language, BM25. Meilisearch: no stemming, typo tolerance | best recall across languages and wording |
| Isolation | row-level security on every row, plus the query's container filter | in code only; one index per workspace or a filter on every query | in code only (filters, tenant tokens) | as the lexical engine it rides on |
| Freshness | transactional with the projection | needs a writer process; readers in the `api` role see new segments after a commit/reload | asynchronous sync to keep consistent, plus its own failure modes | embedding lag on top |
| Scale shape (ADR 0011) | any number of `api`/`worker` processes share it | breaks "same binary, any role": `api` replicas need the worker's files (shared volume or a search role) | scales on its own | — |
| Deletion and purge | `DELETE` in the workspace's transaction | delete-by-term, segment merges before bytes go | deletes through its API, eventually | vectors deleted with rows |
| ARM | yes | yes | yes (both publish arm64) | yes |
| Backups | in the Postgres backup | separate, or rebuilt | separate, or rebuilt | in Postgres |

**Why not the others.** The embedded index is the strongest ranking per CPU,
but its files live outside the database: the `api` role can't read them
without a shared volume, isolation is only in our code, and backups and
purges become two systems. A service doubles the moving parts self-hosters
run and gives up row-level security; reusing the legacy OpenSearch would tie
Next to infrastructure that goes away at cutover. Starting hybrid makes an
`embed` endpoint a requirement for search to work at all, and embedding
every message on a CPU host is the expensive part; semantic search is added
as a layer that degrades to lexical (Decision 3, *Semantic layer*).

**Escape hatch.** Everything goes through one query function in the module.
If `ts_rank_cd` ranking or GIN speed is not enough at cloud scale, the next
step is BM25 inside Postgres (ParadeDB's `pg_search`: needs its own Postgres
image and is AGPL-3.0, so not a default for self-hosters), and only then an
external engine behind the same function.

## Decision 2: ownership and data flow

**Chosen: one `search` module owns a projection it keeps current from the
sources' ordinary domain events (ids only), pulling the current searchable
form of a record through a kernel port, `SearchSource`, that each source
module implements.**

### The model

- A **group** is what a result is shown as and what access applies to: a
  conversation. It has a container (its inbox), a time (its last message),
  a title (the subject), keywords (label names) and facets.
- An **entry** is text inside a group: a message. It has people
  (from/to/cc with names and addresses), plain text, attachment names, a time
  and its own facets (mailbox, direction, attachments).
- **Facets** are non-personal values filters need: `stream`, `unread`,
  `starred`, `archived`, `labels` (ids) on the group; `mailbox`
  (`inbox`/`archive`/`spam`/`trash`), `direction`, `attachments` (count) on
  the entry. They are typed in the port and stored as `jsonb`.

### The port

```rust
// campbooks-kernel, crate::search
pub trait SearchSource: Debug + Send + Sync + 'static {
    /// The group record type it serves, e.g. "conversations.conversation".
    fn group_type(&self) -> &'static str;
    /// The group's current searchable form in the transaction's workspace,
    /// with each entry's content stamp; `None` once the group is gone.
    fn group<'a>(&'a self, ctx: SourceContext<'a>, id: &'a str) -> SourceFuture<'a, Option<Group>>;
    /// The full text of these entries (the ones whose stamp changed).
    fn entries<'a>(&'a self, ctx: SourceContext<'a>, ids: &'a [String]) -> SourceFuture<'a, Vec<Entry>>;
    /// A page of group ids after `cursor`, for backfills.
    fn page<'a>(&'a self, ctx: SourceContext<'a>, after: Option<&'a str>, limit: u32) -> SourceFuture<'a, Page>;
}
```

`SourceContext` carries the job's workspace transaction and the blob store.
An entry's **stamp** is the hashes of the blobs it is read from (they are
content-addressed), so a flag change re-reads the group's row and facets but
never a body. The server registers sources with
`AppState::with_search_sources`; `search` looks one up by group type and
fails closed (nothing indexed) when it is missing.

### Keeping it current

```
conversations.message_received ─┐
conversations.message_sent ─────┼─► search.on_* handlers ─► search.refresh_group { group }
conversations.conversation_changed┘        (unique per group: bursts coalesce)
                                                 │
                       SearchSource::group ◄─────┤ current facets, title, entry stamps
                       SearchSource::entries ◄───┘ only entries whose stamp changed
                                                 │
                                   upsert group + entries, delete entries not returned,
                                   delete the group when the source says it is gone
```

- The handlers mirror the three event types (as every consumer does) and
  enqueue `search.refresh_group` with `unique_key = group` and
  `ordering_key = group`. A refresh always reads the **current** state, so
  out-of-order or repeated deliveries converge: no versions to compare.
- Message removals need no message ids: `conversation_changed` with
  `messages` makes the refresh reconcile the group's entries against the
  source.
- Freshness: the time from the event to the job, typically under two
  seconds. Screens that need read-your-own-writes read conversations, as
  ADR 0011 says.

### Backfill and reindex

- `search.backfills` holds one row per workspace and source: state,
  cursor, the index version it builds and when it started.
- **Lazy start.** A workspace's backfill starts on the first event the
  module sees from it, or on the first query, whichever comes first. Idle
  workspaces cost nothing (design goal 1). A query while a backfill runs
  answers what is indexed and says `indexing: true`.
- `search.backfill` pages through `SearchSource::page` (500 groups a page,
  low priority, its own queue so live refreshes are never behind it),
  refreshes each group and re-enqueues itself with the next cursor. A worker
  that dies resumes from the stored cursor.
- **Reindex.** `INDEX_VERSION` in the module names the shape of what it
  stores (configurations, weights, fields). When it changes, the next
  touch of a workspace starts a backfill at the new version; queries keep
  using the existing rows meanwhile, since each refresh overwrites in place.
  A change that only needs new `tsvector`s is rebuilt from the stored text
  without the source. `campbooks-server search reindex [--workspace <id>]`
  starts one by hand.
- Event-log retention (#65) does not matter to search: it never replays.

### Deletion, purge and retention

- A group the source no longer returns is deleted with its entries in the
  same transaction.
- An inbox that loses its members is invisible at once (query-time access).
  An inbox that is deleted is cleaned up by the refreshes of its
  conversations; a container-level delete arrives with conversations' inbox
  removal event (follow-up, not in this module's control).
- `workspaces.workspace_deleting` schedules `search.purge_workspace` at
  `purge_after`, which deletes every row of the workspace, like the other
  modules. Nothing of search is in the event log or in blobs.
- Search keeps nothing longer than its source: retention policies of a
  source (future) reach search as removals.
- Queries are never stored and never logged (telemetry rules); metrics carry
  latency and result-count buckets only.

### Alternatives considered

| | (a) Events + read ports, pull current state (chosen) | (b) Sources push search documents | (c) Each module indexes its own data |
| --- | --- | --- | --- |
| How | sources' existing events trigger a pull through `SearchSource` | each module writes a search-document blob and publishes `<module>.record_indexed` / `record_removed` | `tsvector` columns in each module's tables; search fans a query out to every module and merges |
| New events | none | two per module | none |
| Personal data | copied into `search` tables only | a document blob per change, plus the copy in `search` | none copied |
| Ordering | converges (always reads current state) | needs a version per record: two deliveries can arrive out of order (ADR 0012) | n/a |
| Backfill | page through the port | a "reindex requested" event every module must answer | none needed |
| Ranking across types | one index, one ranking | one index, one ranking | per-module scores are not comparable; a merge per keystroke |
| Cost per source | one port impl | a document writer and two events | parser, ranking, snippets, embeddings, all per module |
| Coupling | search mirrors a source's event names | search is generic over one event shape | search knows every module's query API |

(b) is what `docs/domain-map.md` sketched ("`*Indexed` / `*Removed`
events"). It keeps search generic, but every change writes a blob, deletes
need explicit events, and out-of-order deliveries would need a version per
record. (a) costs one small handler per source event in `search` and gives
convergence, backfill and reindex for free; the domain map is updated in
this change. (c) is the federated design: fresh by construction, but every
module would rebuild search, and one ranked list across mail, documents and
tasks needs one index.

**Projecting inbox membership into search** (instead of asking the port at
query time) was rejected: a member removed from an inbox would keep seeing
its results until the projection caught up.

## Data ownership

Schema `search`, owned by this module. Every table has `workspace_id`,
`ENABLE` + `FORCE ROW LEVEL SECURITY` and the workspace policy; the migration
grants `campbooks_app` DML. Other modules' records are referenced by
`(record_type, record_id)`, never by foreign key.

| Table | Holds |
| --- | --- |
| `groups` | id, record_type, record_id, container_type, container_id, occurred_at, title, keywords (`text[]`), facets (`jsonb`), title_tsv (`tsvector`, weight A), index_version, refreshed_at. Unique `(workspace_id, record_type, record_id)`. |
| `entries` | id, group_id (→ groups, cascade), record_type, record_id, occurred_at, language, people (`jsonb`), people_text (folded "name address" lines), body (≤ 32 KiB), attachments (`text[]`), facets (`jsonb`), stamp, tsv (`tsvector`: people B, attachment names B, body C). Unique `(workspace_id, record_type, record_id)`. |
| `backfills` | source group type, state (`running`/`done`), cursor, index_version, started_at, finished_at. Primary key `(workspace_id, group_type)`. |

Indexes: GIN on `entries.tsv` and `groups.title_tsv`; GIN `gin_trgm_ops` on
`entries.people_text` and `groups.title`; `(workspace_id, container_id,
occurred_at DESC, id DESC)` on `groups` for newest-first and filters;
`(group_id)` on `entries`.

Extensions are database-wide platform infrastructure, so a kernel migration
installs them (`CREATE EXTENSION IF NOT EXISTS` `pg_trgm` and `unaccent`;
`vector` arrives with the semantic layer);
the `search` migration creates its text search configurations.

**Events.** Consumed: `conversations.message_received` v1,
`conversations.message_sent` v1, `conversations.conversation_changed` v1,
`workspaces.workspace_deleting` v1. Published: none. Search results are not
change-feed records; the client refreshes an open result list from the
conversations changes it already receives.

## Decision 3: the query model

### Syntax

Free text with Gmail's operators, which members already know. Operators are
English everywhere (they are syntax, like `⌘K`); the UI's filter chips write
them, so nobody has to type them.

| Form | Meaning |
| --- | --- |
| `word`, `"a phrase"`, `-word`, `a OR b` | text in the subject, people, body or attachment names; phrase in order; excluded; either |
| `from:ana`, `to:`, `cc:` | people, partial and accent-insensitive (`from:joao` finds João) |
| `subject:` | the title only |
| `has:attachment` | an entry with attachments |
| `in:primary` … `in:<stream>`, `in:inbox`, `in:archive`, `in:spam`, `in:trash`, `in:anywhere` | stream or mailbox; spam and trash are excluded unless asked for |
| `is:unread`, `is:read`, `is:starred` | group flags |
| `label:<name>` | a label, by its folded name |
| `before:2026-10-01`, `after:`, `older_than:7d`, `newer_than:2w` (`d`/`w`/`m`/`y`) | time, in the caller's `tz` (IANA, default UTC) |

The parser is deterministic Rust in the module. It never fails a query: an
unknown operator or a bad value is kept as text and reported in
`query.warnings`, so typing `from:` never errors mid-keystroke. Limits: 512
characters, 16 terms, 8 operators (422 beyond). User text reaches SQL only as
bound parameters, through `plainto_tsquery`/`phraseto_tsquery` per term and
configuration, combined in SQL; it is never spliced into `tsquery` syntax.

**As you type** (`prefix=true`): the last bare term of 2+ characters matches
as a prefix (`:*`).

### Ranking

- `relevance` (default for the instant panel): `ts_rank_cd` with weights
  title A 1.0, people and attachment names B 0.6, body C 0.3, normalised by
  length, times a recency factor `1 / (1 + age_days / 180)`. A group scores
  its best entry or its title, whichever is higher. To bound the cost of
  common words, ranking runs over the newest 2,000 matching groups.
- `newest` (default for the results page, like the inboxes): by time, then
  id, straight from the index.
- The weights and the decay are constants tuned against the fictional
  fixture set and recorded with it.

### Snippets and highlighting

`ts_headline` on the best entry's body (and on the title, highlighting every
hit), only for the page being returned, with delimiters taken from Unicode's
private-use area; any such characters are removed from the stored text first,
so mail can't forge a highlight. The server splits the result into segments
`[{ "text": "…", "hit": true }]`. Clients render segments as text nodes with a
`<mark>`; no HTML from mail is ever produced or interpreted. Without a body
hit, the snippet is the start of the body, unhighlighted.

### Who sees what

- Row-level security limits every query to the workspace.
- The query is limited to `container_id = ANY($visible)`, from the
  `Visibility` port's new `visible_containers(access, container_type)` (for a
  user: the inboxes they are a member of; for a client token: all). Asked once
  per request.
- A group type is searched only if the caller also holds a scope of an
  operation that reads it (the change feed's rule, moved to a shared kernel
  helper): `search:read` alone never reveals mail.
- A result is therefore always something the caller can open; opening one
  they lost access to in between answers 404 from conversations.

### Pagination

Opaque cursors. `newest`: keyset on `(occurred_at, id)`. `relevance`: an
offset into the ranked candidates, carried with a hash of the query (a
cursor from another query is 422). 20 results a page by default, 50 at most;
relevance stops at 1,000 results ("refine your search").

### Semantic layer (next change, decided here)

- When `embed` is configured, each entry's text is chunked (two chunks of up
  to 512 tokens at most) and embedded in the background; vectors live in
  `search.embeddings` as `halfvec(1024)`, zero-padded for smaller models
  (zero padding keeps cosine similarity unchanged), so one column and one
  HNSW index serve any model up to 1,024 dimensions without runtime DDL.
- Each embedding carries its **generation** (model id, dimensions). A model
  change creates a new generation, re-embeds from search's stored text in the
  background, keeps answering from the old one, and switches per workspace
  when the new one is complete; the old rows are then deleted. No downtime.
- Hybrid ranking fuses the lexical top 200 and the vector top 200
  (pgvector iterative index scans, filtered by workspace and container) by
  reciprocal rank fusion, on submitted queries only; as-you-type stays
  lexical, so no embedding call per keystroke.
- Without `embed` (or when it fails) search is lexical, silently. Nothing is
  mutated, so there is nothing to undo.

### Query parsing with `decide` (later change, decided here)

The issue asks for filters parsed by `decide`. ADR 0009's note says
zero-shot answers are not reliable yet, and a model call per keystroke is
too slow. So: operators and chips are deterministic; on a submitted query of
three or more words with no operators, typed questions (`choice` stream,
`choice` time window, `noul` "looking for an attachment") produce
**suggested** chips the member can click ("Only Receipts?"), never applied
silently. Without `decide`, no suggestions.

### Alternatives considered

- **`websearch_to_tsquery` in one configuration** (the member's locale):
  simplest, but a Portuguese member searching English mail misses stems, and
  it can't express our operators. Rejected for the per-term expansion.
- **Indexing every entry in all five configurations**: no detection needed,
  but five times the `tsvector`s. Rejected; detection is cheap and the
  query-side expansion covers misdetections.
- **Natural language only** (no operators, `decide` or `complete` turns the
  sentence into filters): friendlier on paper, but slow, unreliable today and
  hard to correct. Rejected as the primary path; kept as suggestions.
- **Relevance-only order**: mail search is usually about the recent; the
  results page defaults to newest, the instant panel to relevance.

## Decision 4: API and UI

### Operation (this change)

`GET /api/v1/w/{workspace_id}/search` — `operation_id = "search.query"`,
`security(("bearer" = ["search:read"]))`, `Operation::query().assistant()`
(the assistant may search; automation has no use for it yet), records
`["search.result"]`. Rate-limited per caller.

Parameters: `q`, `sort` (`relevance`|`newest`), `prefix` (bool), `limit`,
`cursor`, `tz`.

```json
{
  "hits": [{
    "record": { "type": "conversations.conversation", "id": "…" },
    "container": { "type": "conversations.inbox", "id": "…" },
    "matched": { "type": "conversations.message", "id": "…" },
    "title": [{ "text": "Invoice ", "hit": false }, { "text": "October", "hit": true }],
    "snippet": [{ "text": "…", "hit": false }],
    "from": { "name": "Ana Lopes", "address": "ana@example.com" },
    "occurred_at": "2026-10-02T09:05:00Z",
    "facets": { "stream": "receipts", "unread": true, "starred": false, "count": 3, "attachment": true }
  }],
  "next_cursor": "…",
  "indexing": false,
  "query": { "text": "invoice october", "filters": [{ "operator": "from", "value": "ana" }], "warnings": [] }
}
```

A hit carries what a conversation row needs, so a result list renders from
one response. Errors are problem+json: 422 `query_too_long`,
`cursor_mismatch`; 404 for a workspace the caller is not in.

**Alternatives.** (a) Return refs only and let the client fetch the rows from
conversations (composition in the client): no duplicated display data, but
two round trips per keystroke and a new batch-get operation. Rejected for the
instant panel. (b) `POST /search` with a JSON query AST: unambiguous for
machines, but the string is what people and the assistant type, and the parser
is shared. Rejected; the parsed form comes back in `query`.

### UI (the `search-ui` change, designed here)

Built from `lib/ui` and the shortcut registry, no separate prototype:

- **`/` opens search** from anywhere in a workspace (registry id
  `search.open`, shown in hints and the `?` sheet). On desktop it opens a
  search overlay (the CommandMenu's `Overlay`): a field with the parsed
  filters as removable chips under it, instant results as conversation rows
  with highlighted subject and snippet, ↑ ↓ (and J K) to move, ↵ to open the
  conversation in the inbox reader, ⌘↵ for all results, Esc to close.
  Requests are debounced (~120 ms), the previous one aborted, and the last
  results stay on screen while the next load, so nothing flickers.
- **⌘K hands off**: with a query typed, its last rows offer "Search
  conversations for '<query>'" next to "Ask", which opens the overlay with it.
- **Results page** `/w/:workspace/search?q=…`: the same field and chips
  (From, Has attachment, Unread, Stream, Date), a list of conversation rows
  newest first with a "Most relevant" toggle, and the reader beside it on
  wide screens; the inbox's J/K, ↵, E, S, U and Z work on it unchanged.
- **Phones (375 px)**: a search button in the top bar opens the results page
  full screen; chips wrap; the reader is its own screen, as in the inbox.
- **States**: empty query shows the device's recent searches (stored on the
  device, clearable) and operator tips; no results suggests removing a chip;
  `indexing` says "Still indexing older mail"; errors say so plainly.
- **New `lib/ui` pieces**, each with a story, a test and axe in both themes:
  `Highlight` (segments → text and `<mark>`), a `SearchField` (field,
  chips, clear, shortcut hint), and `ConversationRow` accepting highlighted
  subject and snippet. Copy in en, pt-PT, es-ES and fr.

**Alternatives.** (a) Search *inside* ⌘K (commands and conversations in one
list): one surface, but commands and mail compete for the same rows and the
menu grows a second purpose. (b) A search field fixed in every stream's
header: discoverable, but takes space on phones and needs a results mode in
every list. (c) **`/` for a dedicated overlay plus a results page, ⌘K handing
off (chosen)**: one keystroke to search, commands stay commands.

## Decision 5: slicing

One OpenSpec change per PR, each archiving itself:

1. **`search`** (this change): ADR 0016, kernel extensions migration,
   `SearchSource` and `visible_containers`, the module, conversations as a
   source, `search.query`. Recording: a `vhs` terminal clip: fictional mail
   in four languages ingested, then `curl` queries showing accent and stem
   matching, operators, highlighted segments, a member of another inbox
   getting nothing, and a purge.
2. **`search-ui`**: the overlay, ⌘K hand-off, results page, `lib/ui` pieces,
   i18n. Recording: 375 px and desktop, `/`, typing, chips, opening a result,
   archiving from the results.
3. **`search-semantic`**: embeddings, generations, hybrid ranking,
   re-embedding on model change. Recording: terminal (a fake `embed` server)
   plus the results page finding a message by meaning.
4. **`search-suggestions`**: `decide` filter suggestions and `from:`/`to:`
   autocomplete (`search.suggest`). Recording: 375 px and desktop.

Tracked as #108 (`search-ui`), #109 (`search-semantic`, after the first
release, with #20) and #110 (`search-suggestions`).

## AI use

None in this change. Later: `embed` for entries and queries (fallback:
lexical only; nothing to undo, nothing is mutated) and `decide` for
suggested filters (fallback: no suggestions; a suggestion is applied only by
the member and removed like any chip).

## Copied from the Rails app on `main`?

No. The Rails app searched through OpenSearch beside Postgres; this design
deliberately does not. Chunked embeddings resemble what it did for semantic
lookups, but here they ride on Postgres with model generations, which it did
not have. The Gmail-style operator syntax is copied from Gmail, on the merits:
members already know it.

## Risks / Trade-offs

- **Ranking quality.** `ts_rank_cd` is weaker than BM25. Mitigated by field
  weights, recency, the newest-first default and, later, semantic fusion;
  the escape hatch is in Decision 1.
- **Big workspaces.** GIN plus a container filter is fine to hundreds of
  thousands of messages; beyond that, partition `entries` by workspace hash
  (ADR 0011). A benchmark test with 100 k generated messages tracks the
  instant query (target p95 under 150 ms on the reference ARM host).
- **Language detection** is wrong on short messages; the query-side
  expansion over all configurations absorbs it.
- **Storage.** Text is stored twice (blob and search). Capped per message and
  stripped of quoted history.
- **Extensions on managed Postgres** must be allowed; the migration fails
  early with a message naming them.
- **Pull load.** A refresh reads the group from conversations; flag changes
  never re-read bodies (stamps), and bursts coalesce per group.
- **ADR numbering.** 0015 is taken on another branch; this change uses 0016.

## Decisions at the gate (owner, 2026-10-13)

1. **Order:** the results page sorts newest first with a "Most relevant"
   toggle; the instant `/` panel sorts by relevance. `search.query`
   defaults to `relevance`; the results page asks for `newest`.
2. **`decide`** only suggests filter chips, never parses or applies filters
   (#110).
3. **Semantic search** comes after the first release, with AI setup (#20):
   no `embed` endpoint now (#109). The `vector` extension is therefore not
   installed by this change; `search-semantic` adds it.

Follow-up changes: `search-ui` (#108), `search-semantic` (#109),
`search-suggestions` (#110).
