## 1. Server

- [x] 1.1 `source` on `DraftAttachment` / `StoredAttachment` (serde default `forwarded`); keep-list drops either kind; detach a dropped uploaded blob
- [x] 1.2 Filename cleaning + blocked-extension list + content-type check in `domain/attachments.rs`, unit tests
- [x] 1.3 `features/upload_draft_attachment`: route, body limit, version/token/state/limits checks under a row lock, blob put + attach, version bump, problems; tests for every spec scenario (author-only 404, stale 409, too large 413, too many 422, blocked 400, empty 400, path name cleaned, not assistant-callable)
- [x] 1.4 `release_send` test: an uploaded file is in the outgoing document

## 2. Client

- [x] 2.1 `uploadAttachment` in `api/drafts.ts`; `useDraftEditor.attach(files)` queued with saves, 409 handled as a save's
- [x] 2.2 Composer: Attach button + `compose.attach` (⇧⌘U), hidden file input, drop zone, paste of files, chips (uploading / done / remove), total line
- [x] 2.3 Client-side refusals (empty, total, blocked) with toasts; server refusal codes mapped to the same toasts
- [x] 2.4 i18n en / pt-PT / es-ES / fr; `pnpm test:fit`; stories (idle, uploading, full, refused) + component tests; recording scenario

## 3. Finish

- [x] 3.1 OpenAPI and client types regenerated
- [x] 3.2 Tracking plan regenerated (Coverage row for the new operation)
- [x] 3.3 Changelog entry `changelog/unreleased/added/391-attach-files.md`
- [ ] 3.4 Gates green in CI
- [ ] 3.5 Recording at 375 px and desktop
- [ ] 3.6 `openspec archive draft-attachments --yes` before merge
