# Design

## Context

`crates/kernel/src/idempotency.rs` sits in front of every module route. For
a mutating operation called by an authenticated caller with an
`Idempotency-Key`, it stores a row in `kernel.idempotency` (owned by the
kernel, under per-caller row-level security) holding the caller, the key,
the operation id, a `request_hash` and, once settled, the response. A later
request with the same key and caller is a replay when the operation id and
the hash agree, and a 422 `idempotency_key_reused` otherwise.

Until now `request_hash` was
`SHA-256(method ‖ 0 ‖ path ‖ 0 ‖ query ‖ 0 ‖ body)`. Method, path and query
are guessable from the operation id stored beside it, so the hash is
effectively a fast, unsalted hash of the body.

## Which operations carry a secret in their body

Only requests that pass through the layer **and** carry an
`Idempotency-Key` **and** a valid bearer token (or an in-process principal)
get a row. Unauthenticated requests (sign-in, sign-up, the passkey
ceremonies, the BFF's login) are passed through untouched and are never
hashed. Within that set:

| Operation | Secret in the body | Weight |
|---|---|---|
| `connectors.connect_imap` | `password`, the mailbox's (often app) password | **High**: low entropy, high value; the reason for this change |
| `identity.token` (`/oauth2/token`, in the catalogue as `secret_once`) | `code`, `code_verifier`, `refresh_token`, `client_secret` (form body), when a client also sends a bearer token and a key | Medium: high-entropy values, but a refresh token or client secret is a standing credential |
| `workspaces.create_workspace` | `signup_code` (policy `code`); codes from `CAMPBOOKS_SIGNUP__CODES` are chosen by the operator and may be short | Medium: guessable when operators pick weak codes |
| `workspaces.accept_invitation` | `token`, the invitation link's token | Low: 256-bit random, single use, already stored as SHA-256 by its module |
| `identity.register_passkey` | a WebAuthn attestation (public key, signed challenge) | None: public material |
| Mail and notes (`conversations.create_draft`, `update_draft`, `send_draft`, replies, person notes, the assistant's chat) | private content, not credentials | A plain hash let a database reader confirm a guessed short body ("Yes", "OK, see you Friday"); keyed, it does not |

`connectors.discover_imap` (an address), `connectors.start_connection` and
`identity.start_link` (their secrets are in the answer, already withheld)
carry no secret in their bodies.

## Decision: an HMAC under a key derived from the instance secret

`request_hash` becomes `HMAC-SHA256(K, same input)` with
`K = HKDF-SHA256(instance secret, info = "campbooks idempotency request v1")`
(`crypto::derive_bytes`, the derivation the send binding, the signer and the
rate limiter already use, each under its own label). The key lives only in
process memory; the database never holds it.

- **The type.** `idempotency::RequestHasher` holds the current key and,
  during a rotation, the previous one (`SecretKey::previous_key`), as
  `binding::Binding` does. It hashes with the current key; a stored hash
  matches when it equals the current key's hash or the previous key's, each
  compared in constant time (`crypto::same_hash`). The previous key's hash
  is only computed while a previous key is set.
- **How it is passed.** A field of `AppState` (`request_hasher`, set with
  `with_request_hasher`), which `Idempotency::new(&state, …)` reads, the same
  way the layer already takes the database, the token verifier and the
  ports. `operations::compose` is the only place the layer is built, so the
  server, `testing::state`, the test identity provider's state and every
  module's testkit get a hasher without further wiring. No global.
- **Defaults and startup.** `AppState::new` installs `RequestHasher::default()`,
  a random key per process (as `RateLimits::default` and `Signer::default`
  do). The server installs `RequestHasher::new(&secret)` when
  `CAMPBOOKS_SECRET_KEY[_FILE]` is set, reading it with the same
  `SecretKey::from_env` the signer and the binding already call at startup,
  so no configuration that starts today fails to start. With no secret it
  logs a warning and keeps the per-process key: a retry that reaches another
  process, or arrives after a restart, gets 422 instead of a replay. That
  never runs anything twice, and production always has the secret (sends and
  connector credentials need it already).
- **Tests** get a fixed key from `testing::request_hasher()`, installed by
  `testing::state` and `TestIdp::state`, so two states over one database in
  one test agree, like `testing::binding()`.

## Alternatives

1. **A random salt per row, still SHA-256 (or a slow hash).** A per-row salt
   defeats precomputation but not a targeted offline guess: the salt sits
   in the row. A slow hash (Argon2) on every keyed request, up to 10 MiB,
   would cost CPU on the hot path and still be guessable for a weak
   password. Rejected.
2. **Keep no hash for operations whose bodies hold secrets**
   (a `secret_request` flag on `Operation`). The layer could then not tell
   an identical retry from a reused key, so it would have to refuse every
   reuse (409) or replay blindly. Either is a behaviour change for those
   operations, and it relies on every author flagging every secret field.
   See below.
3. **Reuse the `Signer`** (`state.signer.sign(context, input)`). It is an
   HMAC under a derived key already in `AppState`, but its key is shared with
   cursors and resume tokens, separated only by context, and its tags are
   handed to clients. A dedicated derived key keeps the uses apart and lets
   either be rotated or relabelled without the other. Rejected for a
   dedicated key; the cost is one more field.
4. **Hash only a canonical subset of the body** (drop fields declared
   secret). Needs per-operation schemas in the layer and the same flagging
   discipline as 2. Rejected.

**Recommendation: the keyed HMAC (this change).** It removes the offline
attack for every operation at once, whatever its body, with no per-operation
discipline and no change in behaviour.

### The optional `secret_request` flag

Once the hash is keyed, a database-only attacker learns nothing from it. An
attacker who also holds the instance secret can open the sealed IMAP
password directly (`connectors` seals it under the same secret), so a flag
that also drops the hash for those operations protects nothing further. Not
built. If a later operation stores a secret somewhere that is *not* sealed
under the instance secret, revisit.

## Rows stored before the deploy

- **What happens.** A row's `request_hash` holds the old plain SHA-256. A
  retry of that request with the same key computes the HMAC, which differs,
  so `claim_in` returns `Claim::Mismatch` and the layer answers **422**
  `idempotency_key_reused`. The mismatch check comes before any state check
  and returns at once: the row is not taken over (only an *expired* row is
  replaced, `ON CONFLICT … WHERE expires_at <= now()`, exactly as today), its
  stored response is not touched, and the handler is never called. So an
  in-window retry of a request made before the deploy is refused, never run
  twice. A client that gets that 422 checks the outcome and retries with a
  new key, as for any reused key.
- **Rows are not deleted or rewritten.** Deleting an unexpired row would free
  its key, and an in-window retry would then run the operation a second
  time.
- **When the plain hashes are gone.** A row expires 24 hours
  (`RETENTION`) after its first request, and `kernel.prune_idempotency`
  deletes expired rows every hour. Every row hashed the old way was claimed
  before the deploy, so the last one expires within 24 hours of the deploy
  and is deleted at the next prune: **about 25 hours after the deploy**,
  provided the worker runs the prune (an expired row reused by its own
  caller is overwritten even sooner). The window is a day, not days, so no
  extra step (nulling the old hashes while keeping the rows) is needed;
  `request_hash` is `NOT NULL` and nulling it would need a migration for no
  gain.
- **Backups.** Database backups taken before the deploy still hold the plain
  hashes, for as long as those backups are kept. Out of scope; noted for
  operators.

## Data, events, scopes, tracking

- Data: `kernel.idempotency`, owned by the kernel. Schema unchanged; what
  `request_hash` holds changes (32 bytes, as before).
- No event published or consumed; no endpoint, so no new scope.
- No tracked event added or changed, and no Coverage row: no new operation,
  job, schedule or handler.

## Risks

- **A process without the instance secret** keys its hashes randomly, so a
  retry served by another process (or after a restart) answers 422. Safe
  (never a double run), logged at startup.
- **A rotation finished too early.** Once `CAMPBOOKS_SECRET_KEY_PREVIOUS` is
  removed, rows hashed under it answer 422 for the rest of their 24 hours.
  Safe; rotations keep the previous key for 30 days.
- **Timing.** The comparison was a plain `!=` on bytes read for the caller's
  own key; it is now constant time, which removes any timing signal about
  the stored hash.
