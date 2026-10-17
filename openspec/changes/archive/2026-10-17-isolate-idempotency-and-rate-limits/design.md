# Design

## Context

`kernel.idempotency` (migration `20261013170000`, `containers` added by
`20261020040000`) and `kernel.rate_limits` (`20261014110000`) were created
without row-level security, with `SELECT, INSERT, UPDATE, DELETE` granted to
the runtime role, on the reasoning that keys and counters belong to callers,
not workspaces. The kernel-scope change (#512) and the instance-rows change
(#526) closed every other kernel table; #526 put `kernel.schedules` under a
`kernel_scope_only` policy. This change closes the last two and adds a test
so a new kernel table cannot be left open.

Data ownership is unchanged: the kernel owns schema `kernel`. No event is
published or consumed, no endpoint is added, no AI is used. Five existing
operations change only their replay (their scopes are unchanged:
`identity:clients` for the two client operations, `workspaces:admin` for
invitations, `identity:write` for links, `connectors:write` for
connections).

## Every reader and writer

Found by reading every query naming either table in the kernel, the server,
the modules, `xtask` and the migrations, and every caller of
`RateLimits::check` / `hit` / `exhaust`.

### `kernel.idempotency`

| # | Path | Where | Before | Now | Knows |
|---|------|-------|--------|-----|-------|
| 1 | Claim a key (`INSERT … ON CONFLICT`, then read the existing row) | `idempotency::claim` | pool, autocommit statements | one transaction naming the caller (`begin_as_idempotency_caller`) | the caller (`user:<subject>` / `client:<id>`), the route's workspace if any |
| 2 | Store a settled answer | `idempotency::store` (was inline in `finish`) | pool | the caller's transaction | the caller |
| 3 | Keep only that a secret's key was used (new) | `idempotency::withhold` | — | the caller's transaction | the caller |
| 4 | Release a key (server error, 401/403/409/429, too large) | `idempotency::release` | pool | the caller's transaction | the caller |
| 5 | Hourly prune (`kernel.prune_idempotency`, queue `maintenance`) | `PruneIdempotency::run` | `ctx.pool()` | `begin_kernel`, one batch per transaction; sees and deletes expired keys only | nothing (instance work) |
| 6 | Replay authorization (scopes, membership, containers) | `Idempotency::replay_refusal` | pool | unchanged; reads no key | the caller, the route's workspace |
| 7 | Invoke by id (automation, the assistant) and the BFF's `/bff/api/*` | `operations::invoke`, `bff::handlers` | through the middleware | the same as 1–4 | the in-process principal |
| 8 | Admin | — | none | none | — |
| 9 | Tests | kernel `operations/tests.rs`; server `identity_tests.rs`, `bff/tests.rs`, `revocation_tests.rs` | owner's pool for setup and inspection; HTTP for the rest | unchanged; the new tests run the app as the runtime role | — |

Requests outside the composed module router never reach the layer: the
issuer's protocol endpoints (`/oauth2/*`, `/auth/*`), the BFF's own
operations and the admin API (`/admin/v1/*`). A request with no valid token
is passed through untouched, as before.

### `kernel.rate_limits`

| # | Path | Where | Before | Now | Key |
|---|------|-------|--------|-----|-----|
| 1 | A hit (`RateLimits::check` / `hit`) | `rate_limit::RateLimits::hit` | one `INSERT … ON CONFLICT … RETURNING` on the pool | one `SELECT … FROM kernel.take_rate_limit($1, $2, 1)` on the pool | an HMAC of the limit and the key |
| 1a | Sign-in, token, authorize, consent, end session, passkeys, dynamic registration (per address, and `instance`) | identity `protocol::*` | as 1 | as 1 | `ip:` / `text:instance` |
| 1b | Links, passkey registration | identity `features::{start_link, start_passkey_registration}` | as 1 | as 1 | `text:account:<id>` |
| 1c | Metadata documents fetched for the whole instance | identity `metadata` | as 1 | as 1 | `text:instance` |
| 1d | Web sign-in | server `bff::handlers` | as 1 | as 1 | `ip:` |
| 1e | Assistant compile | assistant `features::compile` | as 1 | as 1 | `text:<workspace>:<user>` |
| 1f | Search (query, scan) | search `features::query` | as 1 | as 1 | `text:<caller>` |
| 1g | Change feed stream opens | kernel `changes::stream` | as 1 | as 1 | `text:<caller>` |
| 1h | Per-person send class (#504) | kernel `rate_limit::api` | as 1 | as 1 | `text:user:<sub>` / `client:<id>` |
| 2 | Use up a budget (tests) | `RateLimits::exhaust` (`test-util`) | `INSERT … ON CONFLICT` setting `hits = max` | `kernel.take_rate_limit(…, cost = max)` | as 1 |
| 3 | Hourly prune (`kernel.prune_rate_limits`) | `PruneRateLimits::run` → `prune` | `DELETE` batches on `ctx.pool()` | `SELECT kernel.prune_rate_limits($1)` batches | — |
| 4 | Admin | — | none | none | — |
| 5 | Tests | kernel `rate_limit` tests, server `rate_limit_tests.rs` (expire a window) | owner's pool | owner's pool for the expiry; the new test as the runtime role | — |

Every hit runs on the pool, never inside a caller's workspace transaction;
the AI caps (#505) live in `system.ai_usage*`, which already have
workspace row-level security.

## Decisions

### Idempotency keys: bound to their caller

Options considered:

1. **A workspace column and a workspace policy.** Keys are per caller and
   many idempotent operations have no workspace (`identity.start_link`,
   `identity.sign_out_everywhere`, `workspaces.create`,
   `workspaces.accept_invitation`, passkeys, consents): their rows would need
   an unset workspace admitted by some other rule. A caller's key also spans
   workspaces (the same key on two workspaces' routes is one key, answered
   422 for the second). The replay already re-checks the workspace and its
   containers (`replay_refusal`), so the column would add nothing.
2. **Kernel scope only** (`kernel_scope_only`, like `kernel.schedules`):
   the layer's own transactions would see every caller's keys. Simple, but
   wider than needed.
3. **Chosen: bound to the caller.** The `principal` column already names
   the caller. A crate-private setting, `campbooks.idempotency_caller`, set
   only by `db::begin_as_idempotency_caller` (called only by the layer),
   and one policy, `caller_isolation` (`FOR ALL`, `USING` and `WITH CHECK`
   `principal = <caller>`), admit that caller's keys only. It works the same
   with and without a workspace (sign-in-less operations, client
   registration). The kernel scope gets two narrower policies for the
   prune: `kernel_scope_reads_expired` (`FOR SELECT`) and
   `kernel_scope_prunes_expired` (`FOR DELETE`), both `expires_at <= now()`:
   the kernel's machinery can neither read a live answer nor write one.

A handler's own transaction (`begin_for`) never carries the caller setting,
so it sees no key at all, its own caller's included. `cargo xtask arch`
rejects the setting's name, its last word on its own and the constant
`IDEMPOTENCY_CALLER_SETTING` outside the kernel, like the scopes' (no
reviewed list).

The claim keeps its `ON CONFLICT (principal, key)`: the conflicting row
always has the caller's own principal, so the policy never hides it.

### Secrets: kept as `withheld`, a replay is 409 (option A)

Options considered:

- **B. Not caching those operations.** A retried `register_client` would
  register a second app with a second secret, the first one's secret lost
  with the dropped answer: an orphan credential. A retried invitation would
  send a second email; a retried link would mint a second ticket.
- **A, chosen. Store nothing of the answer, keep the key.** The key still
  makes the operation run at most once. For an operation declaring
  `secret_once`, a successful answer is stored as state `withheld` with its
  status and `Location` and an empty body (`response_body = ''`,
  `response_content_type IS NULL`, enforced by the check
  `idempotency_withheld_body_check`). A replay first passes the same
  authorization as any replay (scopes, workspace, containers), then answers
  409 problem+json `idempotency_secret_shown_once` with
  `Idempotency-Replayed: true`: the client learns the operation is done and
  that the secret will not be shown again. Non-success answers of those
  operations (400, 404, 422) hold no secret and are stored as before.

The flag is declarative, in the catalogue (`Operation::secret_once()` →
`x-campbooks.secret_once: true`), so clients and automation can read it
from the OpenAPI document; `cargo xtask arch` accepts it only as a boolean
on a mutating operation.

The operations that declare it, found by reading every response type of a
mutating operation for a secret, a token, a code or a ticket:

| Operation | What its answer shows once |
|---|---|
| `identity.register_client` | a confidential app's `client_secret` |
| `identity.rotate_client_secret` | the new `client_secret` |
| `workspaces.invite_member` | `accept_url`, carrying the invitation token |
| `identity.start_link` | `url`, carrying a single-use, two-minute ticket |
| `connectors.start_connection` | `authorize_url`, carrying the same kind of ticket |

Reviewed and not flagged: `identity.start_passkey_registration` (WebAuthn
creation options: the challenge is public by design and bound to the
account server-side), `workspaces.accept_invitation` (the token is in the
request, which is never stored, only its SHA-256 with the method and path;
the token is random, so the hash reveals nothing), every other mutating
operation's answer (records, ids, states). Outside the layer: the issuer's
`/oauth2/token` and `/oauth2/register`, and the admin API's signup codes.

### Existing cached secrets

The migration deletes every row of the five operations, before row-level
security is enabled (so a migration login that is not a superuser still
sees them). The rows are a day's cache of answers, not user data: the only
effect is that a retry of one of those requests made in the previous 24
hours runs the operation again instead of replaying it. Rows of other
operations are kept; they hold no secret.

### Rate-limit counters: functions only

Options considered:

1. **Kernel scope only**, with `hit` opening `begin_kernel`: three more
   round trips per hit, and any kernel code in the kernel scope could still
   read or reset every counter.
2. **A workspace policy for workspace keys.** Buckets are HMACs: the
   database cannot tell a workspace's key from an address's, and only one
   key (the assistant's) names a workspace at all.
3. **Chosen: `SECURITY DEFINER` functions, no table privilege.** `REVOKE
   ALL ON kernel.rate_limits FROM campbooks_app`; the runtime role calls
   `kernel.take_rate_limit(bucket, window_seconds, cost)` (add `cost` hits to
   one bucket, starting a new window when the last ended; answers `hits` and
   `retry_after`) and `kernel.prune_rate_limits(batch)` (delete up to
   `batch` ended windows; answers how many). This is a privilege boundary,
   not a setting: even SQL running as the runtime role cannot list, read,
   lower or reset a counter. It can only add hits to a bucket it names, and
   naming one takes the HMAC key, which is derived from the instance secret
   and never reaches the database.

The functions:

- are owned by the migration login (the table's owner) and run as it;
- pin `search_path = pg_catalog, pg_temp` and qualify every name, so a
  caller's objects cannot shadow anything in their bodies;
- refuse a bucket that is not 32 bytes (an HMAC-SHA-256), a window outside
  (0, 86400] seconds (the configured windows' bound; a longer one would let
  a caller pin a counter for good) and a cost below 1 (`22023`);
- turn the kernel scope on for their own statement and restore the
  caller's value before returning. A `SET` clause on the function would do
  this automatically but needs a superuser to name a custom setting, which
  the migration login need not be. On an error the caller's statement (or
  its savepoint) is rolled back, which undoes the setting too;
- are executable by the runtime role only (`REVOKE ALL … FROM PUBLIC`).

The table also gets forced row-level security with one
`kernel_scope_only` policy, for the owner and any role given privileges
later. `RateLimits::exhaust` (tests) adds `max` hits through the same
function instead of overwriting the row. The replaced `query!` macros'
offline cache entries are removed; the new statements are runtime queries
(no new cache entries).

`cargo xtask arch` forbids `search_path` in migrations; it now allows
exactly a function's pinned `SET search_path = pg_catalog, pg_temp` (not
followed by `;`), and still rejects any statement changing the session's
path.

### The coverage test

`every_kernel_table_has_row_level_security_or_a_reason` (server) lists every
table of schema `kernel` and asserts that each enables and forces row-level
security and has a policy, or is on an explicit instance-only list with a
reason; a listed table must grant the runtime role no write. The list holds
one table:

- `kernel.instance_rows_from_workspaces`: the instance-wide rows a workspace
  may add, read by the queue's and the event log's policies; written by
  migrations only, the runtime role may only read it, and it holds nothing
  of any caller or workspace.

No other kernel table lacks row-level security after this change, so no
follow-up issue is needed for the kernel. (The BFF's tables, in schema
`bff`, have none either; they are outside this test and this issue.)

The server's policy test now admits the kernel scope in
`kernel.rate_limits` `kernel_scope_only` and in the two expired-only
policies of `kernel.idempotency`, and the caller setting only in
`kernel.idempotency` `caller_isolation`. The runtime role's privilege test
lists `kernel.rate_limits` as reachable only through the two functions.

## Threat model

As for the workspace and scope settings (#526): the caller setting is a
defence against **code mistakes**. A module's raw SQL that reads or writes
`kernel.idempotency` by mistake, in its workspace transaction or none, now
sees and writes nothing. SQL injection running as the runtime role could
still set the caller setting to a principal it knows and read that caller's
cached answers, which now hold no secret, or plant a forged answer for a
later replay. Closing that needs the layer behind functions or a separate
role, like the separate-roles follow-up for the kernel's machinery. The
rate-limit counters are already behind a privilege boundary.

## Risks

- **Background work that stops.** The two prunes now run in the kernel
  scope (idempotency, expired keys only) and through the function (rate
  limits). Both are tested as the runtime role. A missed reader would see
  no rows rather than fail; the caller list above and the whole workspace's
  tests (identity's flows and the BFF run as the runtime role) cover it.
- **Deploy order.** The migration and the code ship together. A process of
  the previous build still running after the migration would fail every
  rate-limited request (sign-in, token, the BFF's login: permission denied
  on the table) and every request with an idempotency key (API clients,
  and every mutation automation or the assistant invokes by id; the web
  app sends none) until it is replaced. The server migrates on start, so
  a single-instance restart has no such window; with several instances,
  replace them together. An old worker's
  `kernel.prune_rate_limits` fails and retries (harmless).
- **A limit outside the function's bounds.** A `Limit` with a window of
  zero or longer than a day would now fail its check with a database error
  (503) instead of counting. Every window in the code is at most an hour,
  and the configured ones are bounded to a day at startup.
- **Round trips.** Claim, store and release each open a short transaction
  (`BEGIN`, the setting, the statement, `COMMIT`): about four more round
  trips per idempotent request. Rate limits are unchanged (one statement).
- **A superuser migration login** (development, some self-hosted installs)
  owns the functions, which then run as a superuser. Their bodies are fixed
  statements on one table, with validated parameters, no dynamic SQL and a
  pinned path.
- **Lock timeout.** The migration alters both tables (`ACCESS EXCLUSIVE`);
  `lock_timeout = 5s` fails it (and the start) on a busy table instead of
  queueing every request behind it; the next start tries again.

## Rollback

sqlx has no down migrations, and refuses to start with applied migrations
it does not know. **Ops step: before redeploying an image older than this
change, run this once as the migration login.** It restores both tables to
the shape of `20261021040000` (columns, constraints, grants, no row-level
security), drops the two functions, deletes the keys kept as `withheld`
(the older image would answer 409 "in flight" for them until they expire)
and forgets this change's migration, so a later upgrade applies it again.
Checked against a database migrated only to the base: identical columns,
constraints, row-level security flags, policies, grants, functions and
indexes in schema `kernel`, the same `_sqlx_migrations`, and re-upgrading
applies cleanly. The cached answers the migration deleted are not restored
(they were a day's cache).

```sql
-- Roll back campbooks-cloud#210 BEFORE redeploying an image older than it.
-- Run once, as the migration login (the tables' owner). Restores
-- kernel.idempotency and kernel.rate_limits to the shape of 20261021040000
-- (columns, constraints, grants, no row-level security), drops the two
-- rate-limit functions, and forgets this change's migration, so the older
-- image starts and a later upgrade applies it again. Keys kept as
-- `withheld` are deleted first: the older image would answer 409 "in
-- flight" for them until they expire.
BEGIN;
SET LOCAL lock_timeout = '5s';
DROP POLICY caller_isolation ON kernel.idempotency;
DROP POLICY kernel_scope_reads_expired ON kernel.idempotency;
DROP POLICY kernel_scope_prunes_expired ON kernel.idempotency;
ALTER TABLE kernel.idempotency NO FORCE ROW LEVEL SECURITY;
ALTER TABLE kernel.idempotency DISABLE ROW LEVEL SECURITY;
DELETE FROM kernel.idempotency WHERE state = 'withheld';
ALTER TABLE kernel.idempotency DROP CONSTRAINT idempotency_withheld_body_check;
ALTER TABLE kernel.idempotency DROP CONSTRAINT idempotency_state_check;
ALTER TABLE kernel.idempotency
    ADD CONSTRAINT idempotency_state_check CHECK (state IN ('pending', 'completed'));
DROP FUNCTION kernel.take_rate_limit(bytea, double precision, integer);
DROP FUNCTION kernel.prune_rate_limits(integer);
DROP POLICY kernel_scope_only ON kernel.rate_limits;
ALTER TABLE kernel.rate_limits NO FORCE ROW LEVEL SECURITY;
ALTER TABLE kernel.rate_limits DISABLE ROW LEVEL SECURITY;
GRANT SELECT, INSERT, UPDATE, DELETE ON kernel.rate_limits TO campbooks_app;
DELETE FROM _sqlx_migrations WHERE version = 20261021060000;
COMMIT;
```

Every process must run the same side of the change: upgrade (or roll back)
the API and the worker together.
