## 1. Server
- [ ] 1.1 `update_draft` `inbox_id` (role on target, same workspace, not read-only, clear thread_key, From recomputed); tests for each scenario + assistant path
## 2. Client
- [ ] 2.1 From picker inside the composer (more than one sendable inbox); NewMessage drops its separate Select
- [ ] 2.2 Title mode menu: new draft of the new kind + text carried + old deleted; errors keep the old draft
- [ ] 2.3 Subject shown for every kind
- [ ] 2.4 Scout ▾ (⌘J): Shorter / Warmer / Fix spelling / Change… through the reply stream (nudge: writeNudge); plain-text insertion as text nodes; ThinkingTrace; Undo; hidden when unavailable
- [ ] 2.5 i18n en / pt-PT / es-ES / fr; `pnpm test:fit`; stories + tests (incl. worded-link scenario); recording scenario
## 3. Finish
- [ ] 3.1 OpenAPI and client types regenerated
- [ ] 3.2 Tracking plan regenerated (no change expected)
- [ ] 3.3 Changelog entry `changelog/unreleased/added/394-composer-controls.md`
- [ ] 3.4 Gates green in CI
- [ ] 3.5 Recording at 375 px and desktop
- [ ] 3.6 `openspec archive composer-controls --yes` before merge
