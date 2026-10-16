# Tasks

## 1. Kernel

- [x] 1.1 `config::RateLimitsConfig` (`CAMPBOOKS_RATE_LIMITS__*`), defaults, startup check naming the bad setting; in `Config`, `testing::config` and `Config::check`
- [x] 1.2 In-process token bucket (GCRA) over `kernel::clock`, keyed by a keyed hash, swept and capped; unit tests (burst, refill, `Retry-After`, recovery, sweep, cap, class off)
- [x] 1.3 `Operation::rate_limit(RateLimit::Send)` and the catalogue's class of an operation
- [x] 1.4 The middleware: principal (in-process or verified token, attached for reuse), class, key, Postgres for sends, 429; `invoke` attaches the caller kind
- [x] 1.5 Installed by `operations::compose`; `AppState::api_limits` built from the config, `with_api_limits` for tests
- [x] 1.6 One log line per limited bucket, `campbooks.rate_limit.refusals` counter

## 2. Server and modules

- [x] 2.1 The admin router counts in the admin class
- [x] 2.2 `conversations.send_draft` declares the send class
- [x] 2.3 API and admin API descriptions mention the limits; OpenAPI regenerated
- [x] 2.4 `cargo xtask arch` accepts `rate_limit: send` on mutating operations only
- [x] 2.5 Integration tests: read limit + recovery (clock), writes and sends at their own thresholds, two people, a client, two apps, the BFF, automation and the assistant, off switch, a class off

## 3. Close

- [x] 3.1 `deploy/.env.example`, `deploy/README.md`, CHANGELOG
- [x] 3.2 Web client checked against a 429 (no change needed, or a minimal fix)
- [x] 3.3 Gates green; terminal recording
- [x] 3.4 Archive this change
