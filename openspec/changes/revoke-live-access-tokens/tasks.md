# Tasks

## 1. Kernel

- [x] 1.1 `Principal::issued_at` from `iat`; every principal built in code says `None` or the token's
- [x] 1.2 `auth::TokenRevocation` port, `TokenStanding`, `TokenVerifier::with_revocation` / `still_stands`; `verify` refuses a person's token that no longer stands with `invalid_token`; unit tests of the rule (seconds, missing `iat`, suspended, missing account, clients untouched, no port)
- [x] 1.3 Change stream: `check_access` asks `still_stands` first and ends with `revoked`

## 2. Identity

- [x] 2.1 Migration `identity.accounts.tokens_not_before`
- [x] 2.2 `accounts::revoke_tokens` (epoch after the row lock, never backwards); suspension and sign out everywhere call it in their transactions
- [x] 2.3 Token endpoint: profile read under `FOR SHARE` with the epoch; `invalid_grant` for a sign-in not newer than it; `iat` stamped to the next second for a newer sign-in in the epoch's second
- [x] 2.4 `token_revocation()` port implementation (one primary-key read)

## 3. Server

- [x] 3.1 Wire the port into the in-process issuer's verifier in `with_composed_ports`
- [x] 3.2 End-to-end tests: suspend (scoped, admin, idempotency replay), unsuspend, sign out everywhere, the same-second edge, a code from before the revocation, client credentials, an outside issuer, the BFF's proxied calls, the change stream

## 4. Close

- [x] 4.1 Delete `.gitleaksignore`
- [x] 4.2 CHANGELOG; docs comments that said access tokens "live out their ten minutes"
- [x] 4.3 Gates green (no OpenAPI or client change); terminal recording
- [x] 4.4 Archive this change
