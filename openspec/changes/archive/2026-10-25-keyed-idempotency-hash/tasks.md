# Tasks

## 1. The keyed hash
- [x] 1.1 `idempotency::RequestHasher`: HMAC-SHA256 under `derive_bytes(secret, b"campbooks idempotency request v1")`, the previous key during a rotation, a random key by default; `RequestHash::matches` in constant time
- [x] 1.2 `AppState::request_hasher` + `with_request_hasher`; `Idempotency::new` takes it from the state; `claim_in` stores the current hash and compares with `matches`
- [x] 1.3 The server installs `RequestHasher::new` from `CAMPBOOKS_SECRET_KEY` (same `SecretKey::from_env` as the signer), else warns and keeps the per-process key; nothing new can stop startup
- [x] 1.4 `testing::request_hasher()` (fixed test key), installed by `testing::state` and `TestIdp::state`

## 2. Tests
- [x] 2.1 Unit: the hash is the HMAC under the derived key, not the plain SHA-256; per secret; the previous key's hash matches during a rotation, not after
- [x] 2.2 HTTP: an identical request replays; a changed body is 422; the stored hash is not the plain SHA-256 of the request
- [x] 2.3 HTTP: two instances with different secrets store different hashes for one request
- [x] 2.4 HTTP: a row holding the old plain hash answers 422 and is not run again, its row untouched
- [x] 2.5 Existing idempotency tests pass with the key

## 3. Finish
- [x] 3.1 OpenAPI, client types and tracking plan unchanged (CI's drift checks); no Coverage row added
- [x] 3.2 `deploy/README.md`: the rotation section says the secret keys idempotency hashes
- [x] 3.3 Changelog entry `changelog/unreleased/security/335-keyed-idempotency-hash.md`
- [x] 3.4 Gates green in CI
