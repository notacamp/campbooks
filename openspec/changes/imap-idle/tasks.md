# Tasks

## 1. Schema and settings
- [ ] 1.1 Migration `20261030020000_connectors__imap_idle_leases.sql`: the lease table, row-level security enabled and forced, `workspace_isolation`, the two maintenance policies, the keep-owner trigger, grants
- [ ] 1.2 `ImapConfig` gains `idle_enabled`, `idle_sessions`, `idle_debounce_seconds`, `idle_refresh_minutes`, `idle_quiet_days`, `idle_poll_seconds`, `provider_sessions_per_host`, `workspace_idle_sessions_per_host`, checked; `deploy/.env.example`
- [ ] 1.3 `architecture.toml`: `[[access.maintenance_scope]]` for `features/imap_idle/lease.rs`; the server's maintenance-policies test lists the two new policies

## 2. The IMAP client
- [ ] 2.1 `syntax`: `* n EXPUNGE` read as `Untagged::Expunge`
- [ ] 2.2 `session`: `idle`, `idle_wait` (cancel-safe, no per-read timeout), `idle_read` (one bounded response), `idle_done`; the tagged-answer reader shared with `command`
- [ ] 2.3 `ispdb`: whether a host is a fixed IMAP host of the bundled database
- [ ] 2.4 The cursor keeps whether the server offers IDLE (`idle`), set by each run

## 3. The pool
- [ ] 3.1 `features/imap_idle/lease.rs`: claim (advisory lock, live counts, caps), renew, release, give up, live counts per host for runs (maintenance scope); offer and withdraw (workspace)
- [ ] 3.2 The caps as a pure choice over live and claimable leases
- [ ] 3.3 The session: load in a workspace transaction, sign in through the guard, `CAPABILITY`, `EXAMINE INBOX`, IDLE with the period caps, refresh with `NOOP`, debounce (leading and one trailing), stop on request
- [ ] 3.4 Reconnects: backoff with jitter, give up after six with a cooldown; refusals counted with the runs' (`health::count_login_refusal`), `needs_reauth` at three; busy only backs off
- [ ] 3.5 The pool loop: renew and claim every minute, stop sessions whose lease is lost, stop all after two minutes without renewing, release on shutdown
- [ ] 3.6 The server starts the pool in the `worker` and `all` roles

## 4. Sync runs
- [ ] 4.1 After a run: offer or withdraw the lease (IDLE offered by the server, changed within the quiet days); the safety poll while held
- [ ] 4.2 Run caps: known providers take `PROVIDER_SESSIONS_PER_HOST`; a run needs a slot beyond the live IDLE sessions (host, and workspace on small hosts)

## 5. Tests
- [ ] 5.1 The fake IMAP server offers IDLE: pushes `EXISTS`/`EXPUNGE`/`FETCH` as its mailbox changes, can flood, hang up, and counts `IDLE`, `DONE` and `NOOP`
- [ ] 5.2 One test per scenario of the spec delta, plus the caps' choice, the debounce, the backoff and the settings

## 6. Finish
- [ ] 6.1 OpenAPI and events unchanged (CI's drift checks); no tracked event or Coverage row added
- [ ] 6.2 Changelog entry `changelog/unreleased/added/318-imap-idle.md` (still off by default)
- [ ] 6.3 Gates green in CI; recording (lease table and row-level security, the CI run of the IDLE tests)
