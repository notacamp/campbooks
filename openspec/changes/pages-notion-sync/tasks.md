## 0. Design gate

- [x] 0.1 Owner decisions (direction, scope, who); proposal, design with alternatives, spec deltas; prototype board "Sync with Notion"
- [x] 0.2 Owner sign-off on the prototype and design ("looks good", 2026-10-25, relayed by the coordinating session)
- [ ] 0.3 Owner: public Notion integration registered (redirect URI, capabilities), client id and secret plus webhook verification token in Infisical `/campbooks-next`

## 1. Connectors: provider and auth

- [ ] 1.1 Migration: provider `notion`, capability `pages`; config `CAMPBOOKS_NOTION__*` + `CAMPBOOKS_CONNECTORS__NOTION__ENABLED` with the flag-off production-values test
- [ ] 1.2 Connectors-owned OAuth (start + `/auth/callback/notion`, state/PKCE flow storage, Basic client auth) → `create_or_refresh_connection`; refresh rotation in one transaction; `invalid_grant` → needs reconnect; tests incl. replayed state and client-token refusal
- [ ] 1.3 Notion client (fixed host, pinned `Notion-Version`, id validation, bounded answers, no redirects, Retry-After cap, `notion:{connection}` bucket); fake Notion for tests

## 2. Connectors: streams, hook, outbound

- [ ] 2.1 `notion.pages` stream: block tree read (paged, caps), normalized snapshot + content hashes, `external` kinds, file download to blobs; polling fallback via search
- [ ] 2.2 `POST /hooks/notion`: verification post, constant-time signature check on the raw body, limits, maintenance-scope lookup → streams due; tests incl. forged signature and oversize body
- [ ] 2.3 Outbound `notion.block_update|append|delete` with MAC binding, base edit-time check, never archive pages; tests incl. "Notion changed it first"

## 3. Pages

- [ ] 3.1 Block ids (`bid`) in the schema and checker; lazy backfill on server apply
- [ ] 3.2 `pages.external_links` / `pages.external_blocks` migration with forced RLS; `pages.import_from_notion`; tests incl. someone else's connection (404)
- [ ] 3.3 Snapshot consumer: kind mapping table, three-way compare, `conflict` and `external` blocks, one checked Yjs update; tests for each of the four cases and removals
- [ ] 3.4 Debounced outbound diff job; mentions → "Campbooks link"; never send `external`/unresolved `conflict`
- [ ] 3.5 `pages.resolve_conflict` (with undo), `pages.stop_sync`; stop on disconnect / trash / un-share / `forget_member`; tests

## 4. Client

- [ ] 4.1 Settings → Connections → Notion card and connect flow; reconnect state
- [ ] 4.2 "Import from Notion" dialog; sync badge, Open in Notion, Stop syncing; "Kept in Notion" block; conflict block with the three choices and Undo; Share dialog warning
- [ ] 4.3 i18n en/pt-PT/es-ES/fr; Storybook screen stories (idle twins); `pnpm test:fit` 375/1280; tests for coverage ≥ 80%

## 5. Finish

- [ ] 5.1 Tracked events and Coverage rows
- [ ] 5.2 OpenAPI and client types regenerated (CI artifacts)
- [ ] 5.3 Changelog entry `changelog/unreleased/added/376-pages-notion-sync.md`
- [ ] 5.4 Gates green in CI
- [ ] 5.5 Recording at 375 px and desktop (Storybook, fake Notion data only)
- [ ] 5.6 `openspec archive pages-notion-sync --yes` before merge
