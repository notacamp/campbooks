# Tasks

## 1. Data and settings
- [ ] 1.1 Migration `20261104010000_connectors__address_verification.sql`: `connectors.address_checks` (row-level security enabled and forced, `workspace_isolation`, minimal grants, cascade with the connection)
- [ ] 1.2 `CAMPBOOKS_CONNECTORS__IMAP__ENABLED` defaults to true; IDLE stays off; without the instance secret key IMAP is not offered unless asked for explicitly
- [ ] 1.3 Tests: the defaults start with production's settings (poll 60–120, no IMAP variable) with and without the key; an explicit `ENABLED=true` without the key stops the server

## 2. The address check
- [ ] 2.1 `domain/address_checks.rs`: request (nonce, locale, daily budget), the code and its `Message-ID`, the waiting check of a connection, recording found / not found, the view's state (stale after 30 minutes)
- [ ] 2.2 `features/send_address_check`: the job: the message (plain text, the member's locale, `Auto-Submitted`), the SMTP submission with the sending budgets and session slot, the outcomes
- [ ] 2.3 `imap/check.rs`: search the Inbox, fetch and compare the `Message-ID`, remove exactly that UID (UIDPLUS-guarded expunge, else MOVE to Trash, else leave), deselect
- [ ] 2.4 `sync_stream`: the waiting check found over the run's session after the push and before the read; the stream due at the poll's minimum while a check waits
- [ ] 2.5 `connect_imap`: start the check when the address is not verified; keep a verified address on reconnect
- [ ] 2.6 `features/check_address`: `POST …/connections/{connection_id}/address-check`, `ConnectionView.address_check`
- [ ] 2.7 i18n: the reasons and the check message in en, pt-PT, es-ES and fr
- [ ] 2.8 Tests: found → verified and sending works; not found → failed and sending refused; a lying server; another check's code; the cleanup touches that UID only (and leaves it when another message is deleted, or with neither UIDPLUS nor MOVE); the SMTP server refuses the address; the daily limit; the retry; row-level security

## 3. The connect screen
- [ ] 3.1 `api/queries.ts`: discover, connect (no cached mutation for the password), check again
- [ ] 3.2 `ConnectMailbox` dialog: address → route (Google / Zoho sign-in, Outlook note, app password, Advanced) → connected → checking → verified / not verified
- [ ] 3.3 Provider guides (`guides.ts`): Yahoo, AOL, iCloud, Fastmail, GMX, web.de, Yandex, own domain, with their official links
- [ ] 3.4 Errors per code; step-up; the password cleared; keyboard (⇧C, ↵, Esc, Back)
- [ ] 3.5 Mailbox rows: "Checking your address…" / "We couldn't verify this address: sending stays off" with Try again
- [ ] 3.6 Strings in en, pt-PT, es-ES, fr
- [ ] 3.7 Tests: every route, each provider's how-to, Advanced, errors per code, password cleared, keyboard flow
- [ ] 3.8 Storybook screen stories for each step, each with a play-less twin for recording

## 4. Docs
- [ ] 4.1 `deploy/.env.example`, `deploy/README.md`: IMAP on by default, the address check
- [ ] 4.2 Zoho notes (cloud #334): the four mail scopes and labels synced, in `deploy/README.md` and the connectors spec

## 5. Finish
- [ ] 5.1 OpenAPI regenerated, client types regenerated
- [ ] 5.2 Tracking plan: no Coverage row (connectors not instrumented yet, campbooks-cloud#332)
- [ ] 5.3 Changelog entry `changelog/unreleased/added/314-imap-connect-screen.md`
- [ ] 5.4 Gates green in CI; recording at 375px and desktop (the lead)
