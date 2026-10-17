# Proposal

## Why

The review of notacamp/campbooks#527 (campbooks-cloud#210) found that the
web BFF's tables, `bff.sessions`, `bff.login_transactions` and
`bff.retired_tokens`, have no row-level security and grant the runtime role
`SELECT, INSERT, UPDATE, DELETE` (campbooks-cloud#212). Any transaction of
the runtime role could list or end anyone's web sessions, read pending
sign-ins, or remove entries from the retired-token list. Only the BFF's
`store.rs` touches them today, so this needs a code mistake or injected
SQL, but these are the web sign-in's credentials. The issue also asks that
nothing secret be stored in plain text, and that #527's coverage test reach
the `bff` schema.

Three cheap follow-ups of the #527 review that touch the same code
(campbooks-cloud#213, items 1, 2 and 5) come along.

Modules touched: the server's web BFF (`crates/server/src/bff`, schema
`bff`), the kernel (the change feed's configuration check, a migration
redefining `kernel.prune_rate_limits`), the server's tests and `xtask` (the
pinned `search_path` rule). No new module and no new cross-module
dependency.

## What Changes

- **The runtime role holds no privilege on the BFF's tables.** It reaches
  them through fifteen `SECURITY DEFINER` functions in schema `bff`, one per
  operation the BFF needs (create, find, touch, mark delivered, rotate,
  reissue, find a retired token's session, end, lock and store the tokens of
  a refresh, end one account's sessions, start and take a sign-in, and the
  two prunes). Each runs as the tables' owner with a pinned `search_path`,
  validates its input, and acts on the one session the presented cookie's
  token hash names, the one sign-in a state hash names, or one account.
  Only the prunes reach across accounts, and they delete ended rows only (by
  the database's clock), at most 10 000 per call, re-checking each row they
  delete. Row-level security is enabled (with no policy) on the three
  tables, so a role given privileges later by mistake sees nothing.
- **Nothing secret in plain text.** A started sign-in's nonce, the one
  plain value, is now sealed like its PKCE verifier. Cookies, previous and
  retired tokens, the state and the login cookie stay SHA-256 hashes; the
  access, refresh and ID tokens stay sealed.
- **Coverage.** The server's test now covers schemas `kernel` and `bff`:
  every table enables and forces row-level security with a policy, grants
  the runtime role no privilege at all, or is listed with its reason. A new
  test checks that every `SECURITY DEFINER` function pins its path and is
  not executable by `PUBLIC`.
- **Follow-ups (campbooks-cloud#213).** `cargo xtask arch` refuses a pinned
  path with more entries or `pg_temp` not last (1);
  `CAMPBOOKS_CHANGES__OPENS_WINDOW_SECONDS` must be at most a day (2); the
  rate-limit prune re-checks the window it deletes (5).
- **Migrations** `20261021070000_bff__sign_in_tables_behind_functions.sql`
  and `20261021070100_kernel__rate_limit_prune_recheck.sql`, each one
  transaction with a lock timeout.

## Non-goals

- Hashing or sealing what is not a credential: the account id (`subject`),
  the identity provider's session id (matched by back-channel logout, which
  needs a signed logout token), the name and email shown in the app, the
  user agent, the landing path and the scopes.
- The other items of campbooks-cloud#213 (the idempotency key space per
  app, a structural `secret_once` guard, the self-hosting wording, views in
  the coverage test).
- A separate database role for the BFF.

## Impact

- **No one is signed out.** Every session is kept; their cookies and tokens
  were already hashed or sealed. Sign-ins in flight at the upgrade (started
  in the last ten minutes) are deleted and answer `invalid_state` once; the
  person signs in again.
- The upgrade must replace every process at once: a process of the previous
  build fails every BFF request (permission denied) until replaced.
- An instance configured with `CAMPBOOKS_CHANGES__OPENS_WINDOW_SECONDS` over
  86 400 now refuses to start, naming the setting (every stream open failed
  with such a value already).
