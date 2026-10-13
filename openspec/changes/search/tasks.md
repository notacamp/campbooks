# Tasks

## 1. Design

- [x] 1.1 Proposal, design with alternatives per decision, spec deltas, ADR 0016
- [ ] 1.2 Design gate: owner's sign-off on engine, data flow, query model, API/UI and slicing (answers recorded in design.md)

## 2. Kernel

- [ ] 2.1 `<ts>_kernel__search_extensions.sql`: `pg_trgm`, `unaccent`, `vector` (`IF NOT EXISTS`), failing with a message naming the missing extension; `cargo xtask arch` still passes
- [ ] 2.2 `kernel::search`: `SearchSource` trait, `Group`, `Entry`, `Facets`, `Person`, `Page`, `SourceContext`; `AppState::with_search_sources` and lookup by group type (fail closed); unit tests with a fake source
- [ ] 2.3 `Visibility::visible_containers` (default: none, fail closed); `NoContainers` and the change feed unchanged in behaviour
- [ ] 2.4 Move the change feed's record-type → read-scope rules (`ReadRules`) to a shared kernel helper used by the feed and by search; feed tests unchanged

## 3. Conversations as a source

- [ ] 3.1 `visible_containers` for `conversations.inbox` over `visible_inboxes`, with the spec's scenario as a test
- [ ] 3.2 HTML-to-text and quoted-history stripping for search text (reusing the module's sanitiser; no new crate unless justified), with en/pt-PT/es-ES/fr fixtures
- [ ] 3.3 `SearchSource` for `conversations.conversation`: `group`, `entries`, `page`; stamps from blob hashes; drafts excluded; tests for gone, HTML-only, quoted history, flag change without body reads

## 4. The `search` module

- [ ] 4.1 `cargo xtask new-module search`; scope `search:read`; i18n catalogue (errors `query_too_long`, `cursor_mismatch`) in all four locales
- [ ] 4.2 Migration: text search configurations (`search.en|pt|es|fr|simple` with `unaccent`), `groups`, `entries`, `backfills`, indexes, forced RLS, grants
- [ ] 4.3 Language detection (`whatlang`, exact version ≥ 2 weeks old, MIT; `cargo deny` passes) limited to the four, with a confidence floor
- [ ] 4.4 `search.refresh_group` job (unique and ordered per group): upsert group and entries, reconcile removed entries, delete gone groups; `tsvector` weights
- [ ] 4.5 Event handlers for `conversations.message_received`, `message_sent`, `conversation_changed` (mirror types); out-of-order and removal tests
- [ ] 4.6 Backfills: lazy start, paged `search.backfill` on its own queue, resume from cursor, `INDEX_VERSION` bump, `campbooks-server search reindex [--workspace]`
- [ ] 4.7 Purge on `workspaces.workspace_deleting`
- [ ] 4.8 Query parser (pure, unit-tested): terms, phrases, negation, OR, operators, dates with `tz`, limits, warnings; property test that no input panics
- [ ] 4.9 Query execution: bound-parameter `tsquery` per term × configuration, prefix, trigram people filters, container and read-scope filters, relevance and newest, cursors
- [ ] 4.10 Snippets and titles as segments via `ts_headline` with private-use delimiters, delimiters stripped from stored text; markup test
- [ ] 4.11 `search.query` handler with OpenAPI (`x-campbooks`, security, records `search.result`), rate limit, `indexing` flag; scope, 404 and visibility tests
- [ ] 4.12 Telemetry test: query text never in logs, spans or metrics
- [ ] 4.13 Benchmark test (`#[ignore]`, documented command): 100 k generated fictional messages, instant-query p95

## 5. Server and docs

- [ ] 5.1 Compose the module; register conversations' search source
- [ ] 5.2 `docs/domain-map.md` (search, ports) and `docs/architecture.md` (module, port) updated
- [ ] 5.3 Tracking issues for `search-ui`, `search-semantic`, `search-suggestions` under #40 (lead)

## 6. Wrap-up

- [ ] 6.1 OpenAPI and events catalogue regenerated; client types regenerated
- [ ] 6.2 CHANGELOG updated (search, Postgres extensions for self-hosters on managed Postgres)
- [ ] 6.3 Gates green (fmt, clippy, tests, arch, openapi, events, deny, machete, `.sqlx`, OpenSpec validate)
- [ ] 6.4 Terminal recording (`vhs`): ingest, accent/stem queries, operators, segments, another inbox's member, purge
- [ ] 6.5 Archive the change into `openspec/specs/`
