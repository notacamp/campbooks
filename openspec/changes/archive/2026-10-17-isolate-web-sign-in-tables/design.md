# Design

## Context

`bff.sessions` and `bff.login_transactions` (migration `20261013180000`),
`bff.retired_tokens` (`20261020000200`) and the scopes columns
(`20261020230000`) were created without row-level security, granting the
runtime role `SELECT, INSERT, UPDATE, DELETE`, on the reasoning that sessions
belong to people at the identity provider, not to a workspace, and only the
BFF reads them. #527 closed the kernel's last open tables the same way this
change closes the BFF's.

Data ownership is unchanged: the server's BFF owns schema `bff`. No event
is published or consumed anew (the two account handlers keep consuming
`identity.signed_out_everywhere` and `identity.account_suspended`), no
endpoint is added or changed, no AI is used. The BFF's operations keep their
security (`security(())` or the `session` cookie scheme, no OAuth scope).

## Every reader and writer

Found by reading every statement naming the three tables in the server, the
modules, `xtask`, the admin crate and the migrations, and every caller of
`bff::store`. The BFF's pool (`Inner.db`) is the application's pool, whose
connections run as the runtime role; the worker's `ctx.pool()` too.

| # | Path | Where | Operation | Key | Before | Now |
|---|------|-------|-----------|-----|--------|-----|
| 1 | Start a sign-in (`GET /bff/login`) | `handlers::start_login` → `store::insert_login` | insert a sign-in | state hash | `INSERT` on the pool | `bff.start_login` on the pool |
| 2 | Finish it (`GET /bff/callback`) | `handlers::finish_login` → `store::take_login` | redeem (delete, return) | state hash | `DELETE … RETURNING` | `bff.take_login` |
| 2a | … end the browser's earlier session | `store::end_session` (was `delete_by_token`) | delete | the earlier cookie's hash | `DELETE` by current token | `bff.end_session` (current, previous or retired token) |
| 2b | … start the session | `store::insert_session` | insert | new token hash | `INSERT` | `bff.create_session` |
| 3 | Every cookie request (`/bff/session`, `/bff/api/*`, `/bff/admin/*`, `/bff/logout`) | `Inner::lookup` → `store::find_session` | read the live session | presented hash | `SELECT` | `bff.find_session` |
| 3a | … a copied cookie, two rotations old | `store::is_retired` (was `retired_session`) | read | presented hash | `SELECT … JOIN` | `bff.retired_session` |
| 3b | … the new token never arrived | `store::reissue` | lock, retire, replace | presented (previous) hash | `SELECT … FOR UPDATE`, `INSERT`, `UPDATE` in a transaction | `bff.reissue_session` (one call) |
| 3c | … the current token arrived | `store::delivered` | update | presented hash | `UPDATE` by id and hash | `bff.mark_session_delivered` |
| 3d | … record use | `store::touch` | update | presented hash | `UPDATE` by id | `bff.touch_session` (live sessions only, never past the absolute deadline) |
| 3e | … rotate the cookie | `store::rotate` | lock, retire, replace | presented (current) hash | `SELECT … FOR UPDATE`, `INSERT`, `UPDATE` in a transaction | `bff.rotate_session` (one call) |
| 3f | … a session asking for fewer scopes than now | `Inner::retire` → `store::end_session` | delete | presented hash | `DELETE` by id | `bff.end_session` |
| 4 | Token refresh (single-flight) | `Inner::access_token` → `store::begin_refresh`, `lock_tokens` | lock the tokens until the transaction ends | presented hash | `SELECT … FOR UPDATE` by id, in a transaction on the pool | `bff.lock_session_tokens` in that transaction (the row lock outlives the call) |
| 4a | … store the new tokens | `store::update_tokens` | update | presented hash | `UPDATE` by id | `bff.store_session_tokens` |
| 4b | … refresh refused, tokens unreadable, or expired without a refresh token | `store::delete_session` / `Inner::end` | delete | presented hash | `DELETE` by id | `bff.end_session` |
| 5 | A copied cookie (retired or rotated away and presented after its successor) | `Inner::terminate` → `store::end_session` | delete, return the sealed refresh and ID tokens | presented hash | `DELETE … RETURNING` by id | `bff.end_session` |
| 6 | Sign out (`POST /bff/logout`) | `handlers::logout` → `Inner::terminate` | as 5 | presented hash | as 5 | as 5 |
| 7 | Back-channel logout (`POST /bff/backchannel-logout`) | `store::delete_for_logout` | delete by `sid` (and `sub`) or by `sub` | the verified logout token's `sub`/`sid` | `DELETE` | `bff.end_account_sessions` |
| 8 | `bff.end_signed_out_sessions` ← `identity.signed_out_everywhere` (queue `events`) | `EndSignedOutSessions` → `store::delete_for_logout` | delete one account's sessions | account | `DELETE` on `ctx.pool()` | `bff.end_account_sessions` on `ctx.pool()` |
| 9 | `bff.end_suspended_sessions` ← `identity.account_suspended` (queue `events`) | `EndSuspendedSessions` | as 8 | account | as 8 | as 8 |
| 10 | `bff.prune_sessions` (queue `maintenance`, every 15 minutes) | `PruneSessions` → `store::prune` | delete ended sessions and expired sign-ins | none (instance work) | two unbounded `DELETE`s with the process's clock, on `ctx.pool()` | `bff.prune_sessions` and `bff.prune_logins`, 5 000 per call until a short batch, the database's clock |
| 11 | Retired tokens | written in 3b and 3e, read in 3a and 5, deleted with their session (`ON DELETE CASCADE`) | — | — | statements in `store.rs` | inside the functions only |
| 12 | Admin API, admin CLI, `xtask` | — | none (`xtask` only checks the migrations' names and schemas) | — | — | — |
| 13 | Tests | `bff/tests.rs`, `identity_tests.rs`, the server's privilege, coverage and access-matrix tests | setup and inspection | — | the owner's pool; the app as the runtime role | unchanged; the reissue, prune, sign-out-everywhere and new boundary tests run as the runtime role |

The only file running these statements is still `crates/server/src/bff/store.rs`
(`architecture.toml` `[[access.pool_access]]`).

## What is stored at rest

| Table | Column | Stored as | Change |
|---|---|---|---|
| `sessions` | `token_hash`, `previous_token_hash` | SHA-256 of the cookie's 256-bit random token | none |
| `sessions` | `access_token`, `refresh_token`, `id_token` | sealed (AES-256-GCM, key derived from `CAMPBOOKS_SECRET_KEY`), bound to the row id and column | none |
| `retired_tokens` | `token_hash` | SHA-256 | none |
| `login_transactions` | `state_hash`, `browser_hash` | SHA-256 of the state and of the login cookie | none |
| `login_transactions` | `code_verifier` | sealed, bound to the state hash and column | none |
| `login_transactions` | `nonce` | **plain text** | **sealed**, bound to the state hash and column (`bytea`) |

Not credentials, kept as they are: `subject` (the account id), `idp_session_id`
(the issuer's session id; ending sessions by it needs a logout token signed
by the issuer), `name` and `email` (shown in the app), `user_agent`,
`return_to` (a same-origin path), `scopes` and the timestamps.

The nonce is sealed with the BFF's own `Cipher` (the one sealing its tokens
and the verifier), not the kernel's `Signer`, so one key and one format cover
every sealed value of the schema. It is opened at the callback and compared
by the ID token check as before; a nonce that does not open answers
`invalid_state`, like a verifier that does not.

**Existing rows.** Sessions are all kept: nothing in them was plain. The
migration deletes the sign-ins in flight before changing the column: they
last ten minutes (at most an hour by configuration), and one interrupted
answers `invalid_state` at its callback, the person starting again. **No
one is signed out by this change, the owner included.**

## Decisions

### Functions, not a BFF-only setting

Options considered:

1. **ENABLE + FORCE row-level security with a BFF-only setting**
   (`campbooks.bff_scope`, set only by `store.rs`), like the idempotency
   caller (#527). Simple, but the setting is a defence against code
   mistakes only: SQL injected anywhere in the runtime role can set it and
   then list every session, read every sealed refresh token (useless
   without the key, but still every account's), end anyone's sessions or
   delete the retired tokens that detect a copied cookie.
2. **Chosen: `SECURITY DEFINER` functions, no table privilege**, like
   `kernel.rate_limits`. This is a privilege boundary: SQL running as the
   runtime role can only name a session by the SHA-256 of its cookie's
   token, which it cannot learn (the token is 256 random bits, the database
   holds only its hash, and no function lists rows or answers a session's
   token hashes), or end one
   account's sessions. It can no longer list, read or rewrite anyone's
   session, nor delete retired tokens.

Option 2 was not much worse: fifteen short functions, and the store's
statements map onto them one to one. Rotation and reissue each become one
call instead of a three-statement transaction (fewer round trips); every
other operation stays one statement.

### Keyed by the presented token's hash

Every function acting on one session takes the hash of the token the
request presented (`presented`), never the session id: the id is not a
secret (it binds the sealed tokens and could appear in a log) and naming a
session by it would let any caller act on any session. The store's `Session`
keeps the presented hash (`Session::presented`) for the rest of the request.
Which tokens a function accepts:

- current or previous: find, touch, lock and store the tokens of a refresh
  (a request's token stays the current or previous one for the whole
  request: rotations are an hour apart);
- current only: mark delivered, rotate;
- previous only: reissue;
- retired only: find a retired token's session;
- current, previous or retired: end (the copied-cookie path presents a
  retired token).

Reads of liveness (find, the retired token's session, touch) use the
database's clock; the values written (`rotated_at`, `last_seen_at`, the
deadlines) are still the process's, as before. The two clocks are the same
host's or NTP-synchronised; a skew of seconds moves a deadline by seconds.

### Validation

`create_session` refuses a missing id, a token hash that is not 32 bytes, a
blank account, missing sealed tokens, and deadlines out of order;
`start_login` refuses hashes that are not 32 bytes, missing sealed values, a
landing that is not a same-origin path, and a sign-in longer than an hour;
`rotate_session` and `reissue_session` refuse a new hash that is not 32
bytes and missing times (a `NULL` grace would otherwise reissue at once).
All with `22023`. `end_account_sessions` with neither an account nor an
identity-provider session ends nothing. A forged session (a caller choosing
its own token) cannot act for anyone: its sealed tokens would have to be
sealed under the instance's key, which never reaches the database.

### The functions

As #527's:

- owned by the migration login (the tables' owner) and run as it;
- `SET search_path = pg_catalog, pg_temp`, every table name qualified, no
  dynamic SQL;
- executable by the runtime role only (`REVOKE ALL … FROM PUBLIC`, then a
  grant to `campbooks_app`).

The tables do not need the kernel-scope dance of `kernel.take_rate_limit`:
row-level security is enabled but not forced, so the owner (and so the
functions) is unaffected, while a role given privileges later by mistake
sees no row (no policy). The read functions are `LANGUAGE sql` with
parameter names that are no column's (in a SQL function a column wins over a
parameter of the same name); the ones that validate or branch are `plpgsql`.

### Prunes: bounded, the database's clock, re-checked

`bff.prune_sessions(batch)` and `bff.prune_logins(batch)` delete at most
`min(batch, 10000)` ended rows (`least(idle_expires_at, expires_at) <=
now()`, `expires_at <= now()`); the job calls each with 5 000 until a short
batch. The condition is repeated on the outer `DELETE`, so a session used
(its idle deadline moved) between choosing the batch and deleting it is kept
(read-committed re-evaluates the outer condition on the updated row, not the
subquery). The caller no longer supplies the time, so no caller can prune a
live session. `kernel.prune_rate_limits` gets the same re-check
(campbooks-cloud#213, item 5), in its own kernel migration, keeping its
signature, owner and grants (`CREATE OR REPLACE`).

### The coverage tests

- `every_kernel_table_has_row_level_security_or_a_reason` now lists schemas
  `kernel` and `bff`: a table there enables and forces row-level security
  with a policy, or grants the runtime role no privilege of any kind
  (`SELECT`, `INSERT`, `UPDATE`, `DELETE`, `TRUNCATE`, `REFERENCES`,
  `TRIGGER`), or is listed as instance-only with its reason (still only
  `kernel.instance_rows_from_workspaces`).
- `the_runtime_role_may_use_every_table` lists the three BFF tables as
  reachable through functions only, asserts that the runtime role holds no
  privilege on them, and pins the exact list of fifteen functions it may
  execute in schema `bff`.
- `every_definer_function_pins_its_path_and_is_granted` (new): every
  `SECURITY DEFINER` function of the database has exactly
  `search_path=pg_catalog, pg_temp` and is not executable by `PUBLIC`.
- The access matrix lists the three tables among those the runtime role may
  not read at all.

### Follow-ups from #527 (campbooks-cloud#213)

1. `cargo xtask arch`: the pinned path is exactly `pg_catalog, pg_temp`. A
   match followed by `,` (another entry, so `pg_temp` is not last), by `;`
   or by the end of the file is refused, and `pg_temp, pg_catalog` never
   matches; unit tests cover each.
2. `CAMPBOOKS_CHANGES__OPENS_WINDOW_SECONDS` must be between 1 and 86 400
   (`RateLimitsConfig::MAX_WINDOW_SECONDS`, the bound
   `kernel.take_rate_limit` enforces). The other configured windows were
   already bounded; no configured limit has a cost (every hit costs one).
5. The rate-limit prune race, above.

## Threat model

The boundary holds against SQL running as the runtime role, injected or
mistaken, on a dedicated runtime login (production). With the single-URL
`SET ROLE` setup or the owner's login, a query able to `RESET ROLE` escapes
it, as it escapes row-level security (`kernel::db::inspect_role` reports it).
What the runtime role can still do: start sign-ins of its own (harmless:
they need the browser cookie it chose), create a session with a token of its
choosing (it cannot seal tokens, so the session can do nothing), act on a
session whose cookie it holds (what that cookie's holder can do), and end one
account's sessions, which only signs that person out.

## Risks

- **Background work that stops.** The prune job and the two account
  handlers now go through the functions as the runtime role; each is tested
  that way. A missed caller would fail with permission denied (loudly)
  rather than see nothing; the caller list above and the BFF's end-to-end
  tests (which run the app as the runtime role) cover every path.
- **Deploy order.** The migration and the code ship together. A process of
  the previous build still running after the migration fails every BFF
  request (sign-in, session, API calls through the BFF: permission denied)
  and its worker's prune and account handlers fail and retry, until it is
  replaced. The server migrates on start, so a single-instance restart has
  no such window; replace the API and the worker together.
- **Sign-ins in flight** at the upgrade answer `invalid_state` once.
- **A superuser migration login** owns the functions, which then run as a
  superuser; their bodies are fixed statements on these tables with
  validated parameters and a pinned path.
- **Lock timeout.** The migration alters the three tables (`ACCESS
  EXCLUSIVE`); `lock_timeout = 5s` fails it (and the start) on a busy table
  rather than queueing every request; the next start tries again.

## Rollback

sqlx has no down migrations and refuses to start with applied migrations it
does not know. **Ops step: before redeploying an image older than this
change, run this once as the migration login.** It restores schema `bff` to
the shape of `20261020230000` and `kernel.prune_rate_limits` to its body of
`20261021060000`, and forgets both migrations, so the older image starts and
a later upgrade applies them again. Checked against a database migrated only
to the base: `pg_dump --schema-only` of schemas `bff` and `kernel` (columns,
constraints, indexes, row-level security, grants, functions) is identical,
the same `_sqlx_migrations`, and re-upgrading applies cleanly. Sign-ins in
flight are deleted (their nonce is sealed; the older image reads it as text);
sessions are kept.

```sql
-- Roll back campbooks-cloud#212 BEFORE redeploying an image older than it.
-- Run once, as the migration login (the tables' owner). Restores schema
-- `bff` to the shape of 20261020230000 (columns, grants, no row-level
-- security, no functions) and `kernel.prune_rate_limits` to its body of
-- 20261021060000, and forgets both migrations, so the older image starts
-- and a later upgrade applies them again. In-flight sign-ins are deleted
-- (their nonce is sealed; the older image reads it as text): one started in
-- the last ten minutes answers `invalid_state` and starts again. Sessions
-- are kept.
BEGIN;
SET LOCAL lock_timeout = '5s';
DROP FUNCTION bff.create_session(uuid, bytea, text, text, text, text, bytea, timestamptz,
                                 bytea, bytea, text, timestamptz, timestamptz, timestamptz,
                                 text);
DROP FUNCTION bff.find_session(bytea);
DROP FUNCTION bff.touch_session(bytea, timestamptz, timestamptz);
DROP FUNCTION bff.mark_session_delivered(bytea);
DROP FUNCTION bff.rotate_session(bytea, bytea, timestamptz);
DROP FUNCTION bff.reissue_session(bytea, bytea, timestamptz, timestamptz);
DROP FUNCTION bff.retired_session(bytea);
DROP FUNCTION bff.end_session(bytea);
DROP FUNCTION bff.lock_session_tokens(bytea);
DROP FUNCTION bff.store_session_tokens(bytea, bytea, timestamptz, bytea);
DROP FUNCTION bff.end_account_sessions(text, text);
DROP FUNCTION bff.prune_sessions(integer);
DROP FUNCTION bff.start_login(bytea, bytea, bytea, bytea, text, timestamptz, timestamptz, text);
DROP FUNCTION bff.take_login(bytea);
DROP FUNCTION bff.prune_logins(integer);
ALTER TABLE bff.sessions DISABLE ROW LEVEL SECURITY;
ALTER TABLE bff.login_transactions DISABLE ROW LEVEL SECURITY;
ALTER TABLE bff.retired_tokens DISABLE ROW LEVEL SECURITY;
GRANT SELECT, INSERT, UPDATE, DELETE
    ON bff.sessions, bff.login_transactions, bff.retired_tokens TO campbooks_app;
DELETE FROM bff.login_transactions;
ALTER TABLE bff.login_transactions ALTER COLUMN nonce TYPE text USING '';
CREATE OR REPLACE FUNCTION kernel.prune_rate_limits(batch integer)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, pg_temp
AS $$
DECLARE
    caller_scope text := current_setting('campbooks.kernel_scope', true);
    pruned bigint;
BEGIN
    PERFORM set_config('campbooks.kernel_scope', 'on', true);
    DELETE FROM kernel.rate_limits
     WHERE kernel.rate_limits.bucket IN (
         SELECT e.bucket FROM kernel.rate_limits e
          WHERE e.resets_at <= now()
          LIMIT greatest(coalesce(prune_rate_limits.batch, 0), 0));
    GET DIAGNOSTICS pruned = ROW_COUNT;
    PERFORM set_config('campbooks.kernel_scope', coalesce(caller_scope, ''), true);
    RETURN pruned;
END
$$;
DELETE FROM _sqlx_migrations WHERE version IN (20261021070000, 20261021070100);
COMMIT;
```

Every process must run the same side of the change: upgrade (or roll back)
the API and the worker together.
