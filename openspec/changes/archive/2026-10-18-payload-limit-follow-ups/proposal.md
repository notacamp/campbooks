## Why

The reviews of public #594 and #593 left follow-ups (campbooks-cloud#265):

- **Jobs retried over-limit payloads.** A job failing with `PayloadTooLarge`
  was retried, though it can never fit.
- **The passkey operations' 413 was undeclared.** OpenAPI and the client
  types did not carry it.
- **The CHANGELOG overpromised.** It said every 413's message names the limit,
  but only a file's does; a JSON body's limit is not known where the body is
  refused.
- **There was no plan for keyed uploads.** The idempotency layer caps keyed
  bodies at 10 MiB, while blobs may be 50 MiB.
- **The language lookup was unbounded.** After the time limit moved inside
  `localize`, the saved-locale lookup that renders a timed-out request's
  problem had no limit of its own.

The decision on campbooks-cloud#266 (parked) also asks for a rule:
irreversible effects need a domain guard.

## What Changes

- `JobError::from(Error::PayloadTooLarge)` is fatal.
- The four passkey ceremony operations declare 413 `body_too_large`, and
  their 400s no longer mention size. `openapi.json` and the generated client
  types are updated.
- The CHANGELOG says a file's message names the limit.
- **Keyed uploads:** the plan is documented on `MAX_REQUEST_BYTES` and in the
  spec. A large file is stored in its own unkeyed step (blobs are content
  addressed, so a retry is the same blob), then attached in a small keyed
  operation. Raising the cap would hold that much memory per keyed request.
- **The language lookup:** `localize` waits at most two seconds for the
  caller's saved locale (token check and lookup), then uses `Accept-Language`,
  so 30 s stays the cap.
- **The rule:** an operation with an irreversible external effect has a
  domain-level guard (a state machine or a unique key on the effect) and does
  not rely on idempotency alone. It is in the spec and the architecture notes.

## Impact

- Specs: blob-storage, operation-catalogue, server-i18n.
- Code: kernel `jobs`, `idempotency` (docs), `i18n::request`; identity
  `protocol::passkey` (OpenAPI only); `openapi.json`; the generated client
  types.
- No migration.
