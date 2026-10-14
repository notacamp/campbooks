# Tasks

## 1. Kernel
- [x] 1.1 `kernel::mail`: `Mailer` port in `AppState` (`NoMailer` default, SMTP via lettre over rustls, `Outbox` for tests), `MailConfig` with `check()`; URL never logged or repeated; fake SMTP test on loopback
- [x] 1.2 `Admin<P>` 404s for bound principals; step-up window one hour
- [x] 1.3 `RateLimits::exhaust` (test util)

## 2. Identity
- [x] 2.1 `WayInAdded.way_in`; `identity.notify_way_in_added` consumer, `identity.notices` ledger, mail in four languages (text + HTML, one link, no images), audit without addresses; prune the ledger after 30 days
- [x] 2.2 `identity.sign_out_everywhere` + `identity.signed_out_everywhere`
- [x] 2.3 `require_fresh` (five minutes) on `start_passkey_registration`; `fresh_sign_in_required` in four languages
- [x] 2.4 Authorization requests bound to the browser (`__Host-campbooks_browser`): `/auth/requests/{id}`, passkey ceremonies, `complete_stored`, consent GET/POST
- [x] 2.5 Refresh families capped at the issuer session's expiry
- [x] 2.6 Rate limits: authorize, end_session; passkey sign-up per address + instance
- [x] 2.7 Test: a disabled account's passkey opens nothing
- [x] 2.8 NFKD folding of app names; client-credentials scope re-check; bound tokens 404 on account, app and held-grant routes

## 3. Server and web
- [x] 3.1 BFF: a rotated-away cookie after the grace ends the session everywhere; `/bff/login` rate-limited; consumer ending web sessions on sign-out everywhere
- [x] 3.2 Workspaces: `users.email_verified`; members' unverified addresses hidden; invitation member check on verified addresses only
- [x] 3.3 Server e2e: a bound token holding identity scopes gets 404 on account routes, workspace-app routes, search and attachments in another workspace
- [x] 3.4 Web: Sign out everywhere (confirmed in place), ways in added this week marked with a notice, fresh sign-in for passkeys; tests and stories; four languages

## 4. Docs and generated
- [x] 4.1 `deploy/.env.example`, `deploy/README.md` (Outgoing mail), CHANGELOG, ADR 0015 (one hour)
- [x] 4.2 OpenAPI, events, client types, `.sqlx`
