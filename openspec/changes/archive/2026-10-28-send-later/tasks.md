## 1. Server
- [x] 1.1 `send_at` on `SendDraft`, range check → 400 `send_at_invalid` (en/pt/es/fr problem messages), schedule at it; tests for every scenario incl. cancel and not-assistant
## 2. Client
- [x] 2.1 Time helpers (tomorrow 08:00, next working day, pick-a-time validation) with unit tests across a Friday, a Sunday and a DST change
- [x] 2.2 Split Send with menu (top layer) + Pick-a-time form; `DraftComposer` passes `send_at`; toast "Scheduled for …" with Undo → cancel_send
- [x] 2.3 Drafts: Scheduled group (> 2 min ahead), Cancel send, "Cancel send and edit"
- [x] 2.4 i18n en / pt-PT / es-ES / fr; `pnpm test:fit`; stories + tests; recording scenario
## 3. Finish
- [x] 3.1 OpenAPI and client types regenerated
- [x] 3.2 Tracking plan regenerated (no change expected)
- [x] 3.3 Changelog entry `changelog/unreleased/added/392-send-later.md`
- [ ] 3.4 Gates green in CI
- [x] 3.5 Recording at 375 px and desktop
- [ ] 3.6 `openspec archive send-later --yes` before merge
