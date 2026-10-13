# Tasks

## 1. Shared groundwork (first commit; on `next` through #458)

- [x] 1.1 Kernel `Principal.auth_time` from the token; test for users and clients
- [x] 1.2 `domain::step_up::require_recent` (10 minutes, `step_up_required`) and `domain::ways_in::lock_and_count` (providers + passkeys)
- [x] 1.3 BFF `/bff/login?max_age=` forwarded to `/oauth2/authorize`; test
- [x] 1.4 Migration `identity__passkeys` (passkeys, webauthn_challenges, sessions.passkey_id/amr); events `way_in_added`/`way_in_removed` shared with S2

## 2. Relying party

- [x] 2.1 `coset` 0.4.2 + `ciborium` 0.2.2 (Apache-2.0); `cargo deny` passes
- [x] 2.2 `webauthn/`: authenticator data, COSE keys (EdDSA, ES256, RS256), options in WebAuthn JSON, registration (§7.1) and assertion (§7.2) verification
- [x] 2.3 A software authenticator (Ed25519, P-256, RSA) and a refusing test per check

## 3. Ceremonies and API

- [x] 3.1 `domain::passkeys`: storage, single-use challenges bound to an account or a browser, pruning
- [x] 3.2 `/auth/passkey/sign-in[/options]`: ceremony cookie, CSRF, rate limit, user handle, counter audit, `amr` sessions
- [x] 3.3 `/auth/passkey/sign-up[/options]`: account with a passkey alone, signup policy at both steps
- [x] 3.4 `identity.list_passkeys`, `start_passkey_registration`, `register_passkey`, `rename_passkey`, `remove_passkey` (step-up, `last_way_in`, sessions ended)
- [x] 3.5 `amr` in ID tokens from the session; `passkey` and `passkey_sign_up` in the sign-in request
- [x] 3.6 End-to-end tests through the issuer: sign-up, sign-in, every algorithm, step-up, last way in, single use, browser binding, refusals, invite-only, counters, uniqueness

## 4. Web client

- [x] 4.1 `webauthn.ts` (JSON ↔ browser types, rejections explained)
- [x] 4.2 `/sign-in`: Use a passkey (`P`) in place, autofill field, sign-up link, errors
- [x] 4.3 `/sign-in/passkey`: one question, then create
- [x] 4.4 `PasskeysSection` in `WAY_IN_SECTIONS`: list, Add (`N P`), Rename, Remove, step-up
- [x] 4.5 en / pt-PT / es-ES / fr; Storybook stories for every state; unit tests

## 5. Docs and gates

- [x] 5.1 OpenAPI and events regenerated, client types regenerated
- [x] 5.2 `deploy/README.md` → Passkeys; CHANGELOG
- [x] 5.3 Gates green; recording at 375px and desktop with Chrome's virtual authenticator; vhs clip of the relying-party tests
