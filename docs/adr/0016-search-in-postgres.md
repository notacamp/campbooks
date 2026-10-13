# 0016 — Search runs inside Postgres, as a projection that pulls through source ports

**Status:** Proposed · 2026-10-13

## Context
Members need one search over everything they may see (`docs/domain-map.md`
→ *search*), in English, Portuguese, Spanish and French, limited to the
inboxes (later calendars, folders) they are a member of. ADR 0011 makes
search a projection of the event log; ADR 0006 keeps self-hosting at one
container plus Postgres; the event log will get a retention window (#65), so
a projection cannot depend on replaying it from the beginning. Production
runs on one ARM host.

## Decision
- **Engine: Postgres.** Full-text search with one text search configuration
  per shipped language plus a neutral one, all with `unaccent`; each entry
  indexed in its detected language, each query term expanded across all of
  them. `pg_trgm` for partial names and addresses. Semantic search is a later
  layer on pgvector (`embed` role), fused with the lexical ranking and
  optional: without `embed`, search is lexical.
- **Extensions are platform infrastructure**: a kernel migration installs
  `pg_trgm`, `unaccent` and `vector`.
- **Ownership: one `search` module owns the index.** It reacts to the
  sources' ordinary domain events (ids only) and pulls the current
  searchable form of a record through a kernel port, `SearchSource`, that
  each source module implements and the server registers. Because it always
  reads current state, out-of-order deliveries converge, and backfills and
  reindexes page through the same port instead of replaying the log.
- **Access at query time** through the `Visibility` port's
  `visible_containers`, plus the read scopes of the record types searched.
- **No external index.** Search text, vectors and their indexes live in the
  `search` schema, under row-level security, in the database backup.

## Consequences
- Nothing new to run or back up; isolation, purges and freshness come from
  the database. Self-hosters on managed Postgres need the three extensions
  allowed.
- Ranking is `ts_rank_cd`, weaker than BM25. Field weights, recency and the
  semantic layer compensate; the next step if that is not enough is BM25
  inside Postgres, then an external engine behind the module's one query
  function.
- Message text is stored twice (blobs and the search table), capped per
  message.
- Each source module implements one port and search mirrors its event names;
  adding a source does not change the engine or the query path.
- `docs/domain-map.md`'s earlier sketch ("`*Indexed` / `*Removed` events")
  is replaced by the pull model.

## Alternatives considered
- **An embedded index (tantivy) in the worker**: better ranking, but files
  outside the database that `api` replicas can't share without a volume or a
  dedicated role, isolation only in code, separate backups and purges.
- **A search service (OpenSearch, Meilisearch)**: one more service for every
  self-hoster, isolation by filters instead of row-level security, a second
  copy to keep in sync. Reusing the legacy stack's OpenSearch would tie the
  rewrite to infrastructure removed at cutover.
- **Hybrid lexical and semantic from the start**: makes an `embed` endpoint a
  requirement for search to work at all, and embedding every mailbox on a CPU
  host the first cost.
- **Sources pushing search documents** (a blob and an `*_indexed` event per
  change): generic, but a blob per change, explicit delete events, and a
  version per record to survive out-of-order deliveries.
- **Each module indexing its own tables** (federated search): fresh, but
  every module re-implements parsing, ranking and snippets, and results
  across modules can't be ranked together.
