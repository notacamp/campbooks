# Proposal

## Why

"Search everything" is one of the needs users had on the Rails app
(`docs/domain-map.md`, Appendix A), and M1 ("an inbox that sorts itself")
lists a `search` module: one index over every record a member may see.
Today Campbooks Next can list a stream and open a conversation, but a member
cannot find last spring's invoice from Ana, a phrase they remember, or every
unread receipt with an attachment.

Search is a projection (ADR 0011): it is rebuilt from the records other
modules own and must never show a member something they could not open.
It also has to stay light for self-hosters (one container plus Postgres,
ADR 0006) and work in the four languages Campbooks ships in.

This change is the first slice: the engine decision (ADR 0016), the `search`
module with a lexical index of conversations, the query language and the
query operation. The search screens, the semantic layer and query
suggestions follow as their own changes (see design → *Slicing*).

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/40.

## What Changes

- **ADR 0016: search runs inside Postgres.** Full-text search with
  per-language configurations (en, pt-PT, es-ES, fr) and `unaccent`,
  `pg_trgm` for partial matches on names and addresses, and, in a later
  change, pgvector embeddings from the `embed` role fused with the lexical
  ranking. No search service, no index outside the database.
- **Kernel.** A `kernel__search_extensions` migration installs `pg_trgm`,
  `unaccent` and `vector` (platform extensions, shared by every module that
  needs them). A new port, `SearchSource` (`kernel::search`), through which
  a module hands search the current searchable form of its records, and a
  registry of sources on `AppState`. The change-feed `Visibility` port gains
  `visible_containers`, so a query can be limited to the containers a member
  may see in one step. The record-type-to-read-scope rules of the change feed
  move to a shared kernel helper.
- **New module `search`** (`crates/modules/search`, schema `search`), scope
  `search:read`:
  - a projection of *groups* (the unit a result is shown as: a conversation)
    and *entries* (the text inside it: its messages), with the text, a
    weighted `tsvector` in the entry's detected language, people for
    `from:`/`to:` and the non-personal facets filters need;
  - kept current by consuming `conversations.message_received`,
    `conversations.message_sent` and `conversations.conversation_changed`
    (ids only) and pulling the group's current state through the source
    port, coalesced per group;
  - a lazy backfill per workspace and source (started by the first event or
    the first query), resumable, plus `campbooks-server search reindex`;
  - deleted with the workspace on `workspaces.workspace_deleting`;
  - the operation `search.query`:
    `GET /api/v1/w/{workspace_id}/search?q=…`, with Gmail-style operators
    (`from:`, `to:`, `subject:`, `has:attachment`, `in:`, `is:`, `label:`,
    `before:`/`after:`/`older_than:`/`newer_than:`, `"phrases"`, `-term`),
    as-you-type prefix matching, relevance or newest order, opaque cursors,
    and highlighted title and snippet returned as text segments, never HTML.
- **`conversations`** implements `SearchSource` for
  `conversations.conversation` (the conversation's subject, labels and
  facets, and each message's people, plain text with quoted history removed,
  attachment names and mailbox) and `visible_containers` for its inboxes.
- **Server** registers the sources and the module.
- **Docs.** `docs/domain-map.md` → *search* and *How contexts talk* describe
  the pull model and the `SearchSource` port; `docs/architecture.md` lists
  the module and the port.

## Capabilities

### New Capabilities

- `search`: one index over the records a member may see, kept current from
  domain events through source ports, and the query operation over it.

### Modified Capabilities

- `conversations`: the module is a search source for its conversations and
  answers which inboxes a member may see as a list.

## Impact

- Adds the module approved by `docs/domain-map.md` (M1, *search*). No
  cross-module crate dependency: `search` mirrors the three conversations
  events it consumes and `workspaces.workspace_deleting`, and reaches
  conversations' data only through the kernel's `SearchSource` port, which
  the server registers. No `architecture.toml` entry.
- New ADR 0016. ADR 0011's "search is a projection of the event log" holds;
  0016 adds that the projection pulls current state through a port instead
  of carrying content in events.
- Postgres needs the `pg_trgm`, `unaccent` and `vector` extensions. The
  shipped image (`pgvector/pgvector:pg17`) has all three; a self-hoster on
  managed Postgres needs them allowed (they are on every major provider's
  list). The migration fails with a clear message otherwise.
- Storage: search keeps its own copy of message text (capped at 32 KiB per
  message, quoted history removed) and its index, roughly the size of the
  text again. Backups grow by that; nothing else to operate.
- Self-hosters: nothing to do. The first search in an existing workspace
  starts its backfill; results say "still indexing" until it ends.

## Non-goals

- The search screens (`/`, the ⌘K hand-off, the results page): the
  `search-ui` change, built on the design system, right after this one.
- Embeddings and hybrid ranking (`search-semantic`), and query suggestions
  through `decide` plus `from:` autocomplete (`search-suggestions`): later
  changes, each with its own tracking issue.
- Indexing drafts, attachment contents, calendars, tasks, documents or
  parties: those modules become sources when they exist. Attachment
  *names* are indexed now.
- Saved searches, search history on the server, search analytics. Recent
  searches, when the UI adds them, stay on the device.
- Cross-workspace search.
- A search service (OpenSearch, Meilisearch) or an embedded index
  (tantivy); see ADR 0016.
- Natural-language query parsing applied without the member's say.
