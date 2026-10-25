# Proposal

## Why

The idempotency layer (ADR 0013) keeps, for every keyed request, a hash of
its method, path, query and **whole body** in `kernel.idempotency`, so it
can tell an identical retry from a reused key. That hash is a plain,
unsalted SHA-256. Some bodies carry a secret, the mailbox password of
`connectors.connect_imap` first among them. Anyone holding only the database
(a stolen backup, a leaked dump, a read-only SQL injection) can rebuild the
rest of the request from the row and test password guesses against that hash
offline, at billions of guesses a second, for as long as the row lives. The
password itself is sealed under the instance secret; this hash is the only
place it can be attacked without that secret.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/335

Modules touched: the `kernel` only (`idempotency`, `AppState`, the test
helpers), and the server's startup wiring (`crates/server/src/main.rs`). No
new module, crate, endpoint, scope, event, job, schedule or cross-module
dependency. No migration.

## What Changes

- **The request hash is keyed.** It becomes HMAC-SHA256 over the same input
  (method, path, query, body, each separated as today) under a key derived
  from the instance secret (`CAMPBOOKS_SECRET_KEY`, HKDF-SHA256 with the
  label `campbooks idempotency request v1`). Without the instance secret the
  database alone no longer lets anyone test a guess.
- **Every process agrees.** The server installs the keyed hasher in
  `AppState` from the instance secret, like the rate limiter, the signer and
  the send binding, so a retry reaching another process (or the worker, for
  an invoked operation) still replays. During a secret rotation
  (`CAMPBOOKS_SECRET_KEY_PREVIOUS`) a row hashed under the previous key still
  matches. With no instance secret the server still starts, as today: the
  hasher takes a random key per process and says so at startup.
- **Compared in constant time.** The stored hash is compared with the
  request's in constant time.
- **Rows hashed before the change stop matching.** A retry, within the
  24-hour window, of a request first sent before the deploy is answered 422
  `idempotency_key_reused`, like any reused key: it is never run again.
  Those rows are neither deleted nor rewritten; they expire and the hourly
  prune deletes them, so the last plain hash is gone about 25 hours after
  the deploy.

## Non-goals

- A per-operation `secret_request` flag that skips or changes the hash for
  operations whose bodies hold secrets (weighed in the design; not needed
  once the hash is keyed).
- Deleting, nulling or rehashing rows already stored.
- Plain hashes already copied into database backups taken before the
  deploy: they age out with the backups' own retention.
- What the layer stores of responses (unchanged: `secret_once` answers are
  already withheld).
