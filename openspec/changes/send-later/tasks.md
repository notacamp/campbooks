## 1. Server
- [x] 1.1 `send_at` on `SendDraft`, range check → 400 `send_at_invalid` (en/pt/es/fr problem messages), schedule at it; tests for every scenario incl. cancel and not-assistant
## 2. Client
- [ ] 2.1 Time helpers (tomorrow 08:00, next working day, pick-a-time validation) with unit tests across a Friday, a Sunday and a DST change
- [ ] 2.2 Split Send with menu (top layer) + Pick-a-time form; `DraftComposer` passes `send_at`; toast "Scheduled for …" with Undo → cancel_send
- [ ] 2.3 Drafts: Scheduled group (> 2 min ahead), Cancel send, "Cancel send and edit"
- [ ] 2.4 i18n en / pt-PT / es-ES / fr; `pnpm test:fit`; stories + tests; recording scenario
## 3. Finish
- [ ] 3.1 OpenAPI and client types regenerated
- [ ] 3.2 Tracking plan regenerated (no change expected)
- [ ] 3.3 Changelog entry `changelog/unreleased/added/392-send-later.md`
- [ ] 3.4 Gates green in CI
- [ ] 3.5 Recording at 375 px and desktop
- [ ] 3.6 `openspec archive send-later --yes` before merge
