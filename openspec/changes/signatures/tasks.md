## 1. Server
- [ ] 1.1 Migration: `conversations.signatures` (RLS) + `drafts.signature`; forget_member / purge_workspace delete signatures
- [ ] 1.2 `get_signature` / `set_signature` (roles, member-only, not assistant, 2000 limit, problems in 4 languages); tests incl. shared inbox isolation
- [ ] 1.3 Drafts: `create_draft` fills it, `DraftView.signature`, `DraftChanges.signature: Option<bool>`, `content_hash` only when non-empty (test: old token stays valid)
- [ ] 1.4 `format::message(body, signature, quoted)` + `release_send`; tests for plain and HTML parts
## 2. Client
- [ ] 2.1 API helpers + Settings → Mailboxes "Your signature" editor (compact composer editor), Save / ⌘↵, toast
- [ ] 2.2 Composer signature block: Leave off / Add signature / Edit in Settings / "Add a signature in Settings"
- [ ] 2.3 i18n en / pt-PT / es-ES / fr; `pnpm test:fit`; stories + tests; recording scenario
## 3. Finish
- [ ] 3.1 OpenAPI and client types regenerated
- [ ] 3.2 Tracking plan regenerated (Coverage rows for both operations)
- [ ] 3.3 Changelog entry `changelog/unreleased/added/393-signatures.md`
- [ ] 3.4 Gates green in CI
- [ ] 3.5 Recording at 375 px and desktop
- [ ] 3.6 `openspec archive signatures --yes` before merge
