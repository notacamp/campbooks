# Proposal

## Why

Suspending an account (ADR 0015) and "sign out everywhere" end the
account's issuer sessions and refresh tokens, but the access tokens already
issued are JWTs the API checks without asking anyone: they keep working
until they expire, up to ten minutes later. For a person who just found a
stolen session, or an administrator who just locked an account out, ten
minutes is too long. This is the last item of campbooks-cloud#122.

Modules touched: the kernel (`auth`: the verifier, `Principal`, a new
port; `changes`: the stream), `identity` (the port's implementation, the
revocation epoch, minting) and the server (wiring). No new cross-module
dependency: identity implements a kernel port the server registers, as it
already does for the client directory.

## What Changes

- **A revocation epoch per account.** New column
  `identity.accounts.tokens_not_before`. Suspending and signing out
  everywhere set it to the revocation's instant, in the transaction that ends
  the sessions. Unsuspending leaves it: the old tokens stay dead and the
  person signs in again for new ones.
- **The API refuses revoked tokens at once.** `Principal` gains
  `issued_at` (the token's `iat`). A new kernel port,
  `auth::TokenRevocation`, answers where an account stands (suspended or
  not, its epoch); identity implements it and the server wires it into the
  in-process issuer's verifier. Every person's token of that issuer is
  checked on every verification: refused when the account is missing or
  suspended, when the token has no `iat`, or when its `iat` is not in a
  later second than the epoch. The answer is the same 401 `invalid_token`
  a forged token gets.
  Every path that verifies a token (`Scoped`, `Unbound`, `Workspace`,
  `Admin`, the idempotency layer, the request locale, the web BFF's
  proxied calls) goes through it.
- **The issuer mints nothing for a revoked sign-in, and a fresh one works
  at once.** The token endpoint refuses (`invalid_grant`) a code or refresh
  token whose sign-in is not newer than the epoch, stamps a newer sign-in's
  token minted in the revocation's own second with the next second as
  `iat`, and reads the account under a share lock while it mints, so no
  token falls on the wrong side of a revocation that is committing.
- **Open change streams end.** A stream asks the same port again whenever it
  resolves membership (at least once a heartbeat) and sends `revoked` when
  the token no longer stands.
- **Cleanup.** The root `.gitleaksignore` goes: its one entry names a
  commit that never reached `next`.

## Non-goals

- Tokens of an outside identity provider (`CAMPBOOKS_AUTH__ISSUER`): their
  subjects are not identity accounts, so nothing is wired and nothing
  changes. That provider revokes its own tokens.
- Workspace clients' `client_credentials` tokens: their standing is already
  resolved on every request by the client directory (ADR 0017).
- Narrower revocations (removing one way in, revoking one app's consent,
  `/oauth2/revoke` of one refresh token, ending one web session): they end
  sessions and refresh tokens as today; their access tokens still run out
  their ten minutes. A per-session epoch would cover them and is not built.
- No cache of the account's standing, and no cross-process invalidation
  channel.
- No change to token lifetimes, to suspension's other effects, or to what
  the web client does with a 401.

## Impact

- Migration `20261020060000_identity__tokens_not_before.sql` (one nullable
  column; no rewrite, no backfill).
- Kernel: `auth::{TokenRevocation, TokenRevocationPort, TokenStanding}`,
  `TokenVerifier::with_revocation` and `TokenVerifier::still_stands`,
  `Principal::issued_at`, the change stream's membership check.
- Identity: `token_revocation()`, `accounts::revoke_tokens`, suspension and
  sign out everywhere, the token endpoint's minting.
- Server: wiring in `with_composed_ports`.
- One primary-key read of `identity.accounts` per request that carries a
  person's token from the in-process issuer.
- CHANGELOG; no OpenAPI or client change.
