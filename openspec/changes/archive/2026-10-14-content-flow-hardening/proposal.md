# Proposal

## Why

The 2026-10-13 security audit of `next` (campbooks-cloud#117, batch 2 "content
flows") found no exploitable hole, but several ways mail could leave the
instance or outlive its use without anyone deciding it should:

- Any AI role could point at a hosted endpoint and receive mail content with no
  explicit decision by the operator.
- `AiError::Status` kept the provider's error body, which can echo the request
  (and so the mail in it); it was logged by the decide cascade and could reach
  `kernel.jobs.last_error`.
- Internal errors were logged with their `Debug` form; a Postgres error's
  message and `DETAIL` quote row values (`Key (email)=(…) already exists`).
- A stored message document that failed to decode was reported with serde's
  message, which quotes the offending value.
- API JSON carrying people's mail had no `Cache-Control`, so a browser or a
  shared cache could keep it.
- `kernel.events` was kept forever and failed jobs until someone deleted them
  (#65); a purged workspace left both behind.
- The recording tool would record any URL, production included.

## What Changes

- **No silent off-box mail.** Each AI role's endpoint is classified as on the
  instance's network or not, from explicit rules (loopback and private
  addresses, `localhost`, single-label compose names, and
  `CAMPBOOKS_AI__INTERNAL_HOSTS`; never DNS). A role off the network gets no
  content unless `CAMPBOOKS_AI__<ROLE>__EXTERNAL_CONTENT=allow`; the gateway
  refuses `chat`, `embed` and `decide` with `ExternalContentRefused`, and the
  conversations module does not compose the model pass, so rules alone sort.
  The health probe (no content) runs for every configured role. The admin
  System overview shows each role's location and whether it may receive mail.
- **Redaction.** `AiError::Status` holds the status only; provider and model
  answers that fail to parse are described by serde category and position. A
  new `campbooks_kernel::redact` describes database errors by SQLSTATE,
  constraint and table, and JSON errors by category and position; the kernel's
  internal-error log, `JobError::from(sqlx::Error)` and the connectors' mail
  reader use it.
- **`Cache-Control: no-store` + `Pragma: no-cache`** on every response under
  `/api/`, `/bff/` and `/admin/`, from one server middleware.
- **Retention.** `CAMPBOOKS_EVENTS__RETENTION_DAYS` (90) prunes the event log
  hourly (`kernel.prune_events`), never an event still being delivered;
  `CAMPBOOKS_JOBS__FAILED_RETENTION_DAYS` (30) prunes failed jobs. The
  workspace purge deletes the workspace's events and finished jobs with its
  change feed.
- **Recordings** (`pnpm record`) refuse any base URL that is not
  `localhost`, `127.0.0.1` or `[::1]` unless `--allow-remote` is passed.

## Capabilities

### New Capabilities

- `content-flows`: where mail may go (AI roles), what errors, logs and
  caches may keep of it, and how long the kernel keeps history.

### Modified Capabilities

None (the admin overview's new fields are additive and specified here).

## Impact

- Crates: `campbooks-ai` (config, error, gateway, health), `campbooks-kernel`
  (`redact`, error logging, `JobError`, config, maintenance jobs),
  `campbooks-conversations` (model pass composition, redaction helpers),
  `campbooks-connectors` (mail reader errors), `campbooks-system` (admin
  overview), `campbooks-workspaces` (purge), `campbooks-server` (no-store
  middleware), `xtask` (eval opts in: fictional data).
- Migration `kernel__retention`: indexes for pruning by age, by workspace and
  for "is this event still being delivered".
- OpenAPI: `AiRoleStatus` gains `location` and `content_allowed` (admin
  document only). Client: Admin → System shows them.
- **Self-hosters:** a role pointing outside your network (a hosted provider)
  stops receiving mail until you set `CAMPBOOKS_AI__<ROLE>__EXTERNAL_CONTENT=allow`.
  Events older than 90 days and failed jobs older than 30 days are deleted.
- ADR 0012 amended (event log retention).
- No new crates.

## Non-goals

- Per-workspace data keys and crypto-erasure (campbooks-cloud#57).
- An image proxy for "Show images" (#57).
- Redacting every error type in the codebase: this change covers the paths the
  audit named (AI errors, the kernel's internal-error log and `JobError` from
  database errors, the mail reader) plus the shared helpers to use elsewhere.
- Per-workspace or per-user AI opt-in: the opt-in is the operator's, per role.
- Shipping logs or traces anywhere (#57).
