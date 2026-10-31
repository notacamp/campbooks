## 0. Design gate

- [x] 0.1 Proposal, design with alternatives, spec deltas; prototype boards "Save a thread to a page" and the Scout panel (approved 2026-10-25)
- [x] 0.2 Owner sign-off ("go ahead", 2026-10-28); split into part 1 (server and Scout) and part 2 (screens)
- [x] 0.3 Operation names re-checked against what shipped (#672, #680, #681, #685); the flag `CAMPBOOKS_PAGES__MAIL_SCOUT__ENABLED` with its flag-off production-values test

## 1. Kernel and conversations

- [x] 1.1 `AttachmentBlobs` port in the kernel, registered in `AppState`; `conversations` implements it with the `get_attachment` read check; tests incl. another member's inbox (None) and a wrong message id (None)

## 2. Pages operations

- [x] 2.1 `pages.attach_from_message` and `pages.detach_files` (files rows with `source`, blob attach/detach, all-or-nothing 404); tests
- [x] 2.2 Markdown → schema blocks parser (server), shared by `pages.append` and `pages.create { markdown }`; tests incl. script links, raw HTML, oversize
- [x] 2.3 `pages.append` / `pages.unappend` with `append_id` and the edited-blocks-stay rule; tests
- [x] 2.4 `pages.read_text` (Markdown, unresolved mentions, 64 KiB cut) and `pages.list?q=`; `.assistant()` opt-ins; tests
- [x] 2.5 `crates/modules/pages/handbook.md` + `HANDBOOK`, composed in `crates/server/src/lib.rs`

## 3. Scout

- [x] 3.1 `find_pages` and `read_page` tools (`OPERATIONS`, schemas, `invoke`, bounded results); tests incl. the unreadable page
- [x] 3.2 About registry `pages.page` (server); the client's `SCOUT_CHAT_ABOUT` and "Ask Scout" are part 2
- [x] 3.3 `propose_page_change` (`append_section`, `new_page`) in `ROWS` with undo, store-time checks; tests incl. unreadable source and unwritable page refused (the client `RUNNABLE` and `PageChangeCard` are part 2)

## 4. Reader

- [x] 4.1 None in this part: the reader dialog, copy, summary hand-off and Undo are part 2 (`pages-from-mail-and-scout-ui`)

## 5. Finish

- [x] 5.1 Tracked events and Coverage rows (design.md)
- [x] 5.2 OpenAPI and client types regenerated (CI artifacts)
- [x] 5.3 Changelog entry `changelog/unreleased/added/364-pages-from-mail-and-scout.md`
- [x] 5.4 Gates green in CI
- [x] 5.5 Recording: a vhs clip of the CI tests (server-only part)
- [x] 5.6 `openspec archive pages-from-mail-and-scout --yes` before merge
