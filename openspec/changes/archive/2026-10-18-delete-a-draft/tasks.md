# Tasks

## 1. Client

- [x] 1.1 `useDeleteDraft`: delete, update the cached lists, "Draft deleted" with Undo that recreates the draft
- [x] 1.2 Composer Discard goes through it, snapshotting what is on screen and settling pending autosave first
- [x] 1.3 Reader: every editable draft as a named chip with Delete draft
- [x] 1.4 Reply dialog: Delete draft once a draft exists
- [x] 1.5 Strings in en, pt-PT, es-ES and fr

## 2. Tests

- [x] 2.1 Chip delete, two drafts listed and individually deletable, Undo recreates
- [x] 2.2 Discard then Undo restores unsaved text
- [x] 2.3 Reply dialog: delete a resumed draft with Undo, not offered before a save

## 3. Docs

- [x] 3.1 Recording scenario `delete-draft`
- [x] 3.2 CHANGELOG
- [x] 3.3 OpenAPI and client types unchanged; gates green in CI
