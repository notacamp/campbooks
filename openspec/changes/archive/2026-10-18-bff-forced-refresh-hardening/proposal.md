## Why

The final re-review of the BFF's forced refresh (public #586,
campbooks-cloud#143) left four findings (campbooks-cloud#259):

- **A self-deadlock (L1).** If the stored token could not be opened in the
  refused branch, ending the session would wait on the refresh's own row
  lock.
- **Refused writes (L2).** A write refused while the refresh worked came back
  401 "Sign in again", and the app never retries, so the member had to act
  again.
- **Missing tests (L3)** for two requests refused together and for the
  identity provider being down. The latter was also not in the spec.
- **An inaccurate note** about `/bff/admin`.

## What Changes

- **L1:** in the refused branch, the stored token is opened outside
  `open_access`, and when it does not open, the refresh's transaction (and its
  row lock) is dropped before the session ends.
- **L2: the BFF holds small bodies and sends them again.** A request whose
  body is empty or of a known length of at most 1 MiB is read into memory, and
  after a forced refresh that works it is sent again, once, with the fresh
  token. This is safe:
  - the API refuses a token as invalid while authenticating, before any
    handler runs, so the refused attempt changed nothing;
  - the copy keeps the `Idempotency-Key`, so the idempotency layer still runs
    it at most once.

  A larger or streamed body is passed through and not sent again, as before.
  The alternative was the web transport retrying once on `invalid_token`. It
  was not chosen because the transport cannot tell a refused token from any
  other 401, would retry requests the BFF never refreshed for, and would need
  the same fix in the native shell.
- **L3:** tests for two requests refused together (one refresh, the second
  uses the stored token) and for the identity provider being down (503
  `idp_unavailable`, the session kept, the next request refreshing). The
  latter is now in the spec. The test identity provider gains a token
  endpoint that can be down and refreshes that can be slow.
- **`/bff/admin`:** it goes through the same path; the admin API answers 404,
  never `invalid_token`, so it never forces a refresh. This is stated in the
  spec and the code.

## Impact

- Spec: web-bff (API access through the BFF).
- Code: server `bff` (`forward`, `replayable`, `Inner::refresh`) and the
  kernel's `TestIdp` (test only).
- No migration.
