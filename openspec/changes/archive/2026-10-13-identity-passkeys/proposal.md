# Proposal

## Why

ADR 0014 makes passkeys a way in, on a par with every owned mailbox, and
allows accounts with a passkey alone. Slices S1a, S1b and S2 built the
issuer, Google, Microsoft and Zoho sign-in, the sign-in screens, and
Settings → Ways to sign in. Nobody can sign in with a passkey yet. A
self-hosted instance with no provider app configured, or someone whose mail
is on IMAP, has no way in at all.

This change is **slice S3** (campbooks-cloud#80, parent #75):

- a WebAuthn relying party in `identity`;
- adding, renaming and removing a passkey while signed in, with step-up;
- signing in with a passkey on `/sign-in`, from the browser's autofill
  (conditional UI) or its passkey dialog (`P`);
- creating an account with a passkey alone, under the signup policy.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/80

**Modules touched**
- **identity**:
  - the relying party (`webauthn/`, with `coset` + `ciborium` + `aws-lc-rs`);
  - `identity.passkeys` and `identity.webauthn_challenges` tables;
  - the ceremony endpoints `/auth/passkey/{sign-in,sign-up}[/options]`;
  - the API operations `identity.list_passkeys`,
    `identity.start_passkey_registration`, `identity.register_passkey`,
    `identity.rename_passkey` and `identity.remove_passkey`;
  - `amr` from the session in ID tokens, and `passkey` among the ways in a
    sign-in request lists.
- **kernel**: `Principal.auth_time`, read from the access token. It carries
  the step-up rule shared with S2 and S4. Done in this change's first commit,
  already on `next` through #458.
- **server**: `/bff/login?max_age=` for step-up. Already on `next` through
  #458.
- **clients/app**:
  - passkeys on the sign-in screen;
  - a new `/sign-in/passkey` screen;
  - the Passkeys section of Settings → Ways to sign in, plugged into
    `WAY_IN_SECTIONS`.

No module or cross-module dependency is added. Two crates are added, both
approved in the S1a design: `coset` 0.4.2 and `ciborium` 0.2.2, Apache-2.0.

## What Changes

- **Register, while signed in.** `POST /api/v1/identity/passkeys/registrations`
  (`identity:write`, step-up) returns WebAuthn creation options: a
  discoverable credential, user verification required, attestation `none`,
  EdDSA/ES256/RS256, the account's passkeys excluded.
  `POST /api/v1/identity/passkeys` verifies the new credential against that
  single-use, five-minute challenge and stores it.
- **Sign in.** `POST /auth/passkey/sign-in/options` (CSRF) returns request
  options for any of the site's passkeys, bound to the browser by a `Strict`
  ceremony cookie. `POST /auth/passkey/sign-in` verifies the assertion,
  starts the issuer session (`method = passkey`, `amr` = `hwk`|`swk` +
  `user`) and completes the waiting authorization request.
- **Sign up with a passkey alone.** `POST /auth/passkey/sign-up/options`
  (display name) and `POST /auth/passkey/sign-up` create an account with no
  provider and no address. Refused on an invite-only instance
  (`signup_invite_only`).
- **Rename and remove.** Removing (step-up) is refused when it is the
  account's last way in (`last_way_in`, through `ways_in::lock_and_count`).
  It ends every session signed in with that passkey and revokes their
  refresh tokens. Renaming needs no step-up.
- **Sign-in request.** `GET /auth/requests/{id}` lists `passkey` after the
  providers when the instance can offer passkeys (https, or `localhost`),
  and says whether a passkey may create an account (`passkey_sign_up`).
- **Screens.** On `/sign-in`, the passkey action runs the ceremony in place
  (`P`). A "Saved passkeys" field offers them through autofill where the
  browser supports conditional UI, and a link leads to the passkey-only
  sign-up. `/sign-in/passkey` asks one question (what to call you), then
  creates. Settings → Ways to sign in gains a Passkeys section: the list,
  Add (`N P`), Rename, Remove, and step-up explained.
- **Events.** `identity.way_in_added` and `identity.way_in_removed` with
  `method = passkey`; `identity.account_created` with `provider =
  passkey`; `identity.signed_in` with `method = passkey`.

## Non-goals

- Attestation trust: no FIDO Metadata Service, no AAGUID allow-list, no
  "authenticator model" in the UI.
- Passkeys as a second factor, or a "passkey required" policy (`amr` is
  recorded, so that can come later as a check).
- Native-app passkeys (Tauri uses the system browser on the same origin,
  slice S4).
- Recovery when the last passkey is lost. The last way in can't be removed,
  and adding a provider way in is the backup. Operator-assisted recovery is
  a follow-up.
- Accepting an email invitation with a passkey-only account (the workspaces
  rule needs a verified address; the S1a design's follow-up).
- WebAuthn extensions (`credProps`, `prf`, `largeBlob`) and `hints`.
