# Tasks

## 1. Kernel
- [x] 1.1 Move `identity::metadata::guard` to `campbooks_kernel::net::guard`; identity re-exports it; its tests move with it
- [x] 1.2 Add `guard::resolve_socket(host, port, Policy { allow_private, allow_any_port })` returning the checked `SocketAddr`; tests for every refused range, rebinding (resolver called once), ports
- [x] 1.3 Move the step-up rule to `campbooks_kernel::auth::step_up` (`require_recent`, `require_fresh`); identity calls it; messages stay in identity's catalogue and connectors gets its own `step_up_required` reason

## 2. connectors: schema and credential
- [x] 2.1 Migration `20261023130000_connectors__imap.sql`: provider CHECK + `imap`; `credentials.refresh_token` nullable, `password bytea`, exactly-one CHECK; `connectors.imap_servers` with RLS
- [x] 2.2 `Provider::Imap` and every exhaustive match (start_connection offered list, credentials refresh = none, intake none, removal revoke = none, sync_stream idle arm, send_message `provider_not_configured`, request_changes none, schedule budget key, metrics, events); `contracts/*` enums + `openapi/events.json`
- [x] 2.3 `credentials::store_password` / `credentials::password` with AAD `password`; previous-key re-seal like tokens; RLS test
- [x] 2.4 `ImapConfig` (`CAMPBOOKS_CONNECTORS__IMAP__*`) and `Deps::imap_enabled`

## 3. connectors: discovery
- [x] 3.1 `cargo xtask ispdb` vendors `thundernest/autoconfig` `ispdb/*.xml` into `crates/modules/connectors/ispdb/` with its MPL-2.0 LICENSE; loaded with `include_dir`/`include_str!` and parsed once with `quick-xml`
- [x] 3.2 Domain autoconfig over HTTPS through the guard (no redirects, 64 KiB cap, 5 s)
- [x] 3.3 SRV (`_imaps._tcp`, `_imap._tcp`, `_submissions._tcp`, `_submission._tcp`) and MX via `hickory-resolver`; MX → ISPDB by MX domain; MX routing to google / zoho / microsoft
- [x] 3.4 App-password table (yahoo, aol, icloud, gmx, web.de, yandex, mail.ru, fastmail)
- [x] 3.5 Feature `features/discover_imap`: operation, rate limit, tests with a fake resolver and fake HTTP

## 4. connectors: verify and connect
- [x] 4.1 `imap/` adapter module: `Session` wrapper over `async-imap` on `tokio-rustls` (TLS, STARTTLS-required, AUTH PLAIN / LOGIN, timeouts); refusal codes only, never server text
- [x] 4.2 SMTP verify with `lettre` (TLS / STARTTLS, AUTH, no send)
- [x] 4.3 Fake IMAP + SMTP servers in `testkit_imap.rs` (scripted, in-process, self-signed cert trusted in tests): login ok / refused / no STARTTLS / LOGINDISABLED / not IMAP
- [x] 4.4 Feature `features/connect_imap`: auth (member, scope, ADR 0017, step-up), rate limits (member, workspace, target-address failures), verify, `create_or_refresh_connection`, reconnect rules; tests for every scenario in the spec delta
- [x] 4.5 i18n reasons in en, pt-PT, es-ES, fr

## 5. Finish
- [ ] 5.1 `cargo xtask openapi` regenerated; client types regenerated
- [x] 5.2 `docs/self-hosting` note: the IMAP settings and the password-at-rest trade-off
- [x] 5.3 Changelog entry `changelog/unreleased/added/310-imap-connection.md`
- [ ] 5.4 Gates green in CI; vhs terminal recording of discover + connect against the fake servers in the PR
