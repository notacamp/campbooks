# Design

## Context

ADR 0014 and the S1a design (`openspec/changes/archive/2026-10-13-campbooks-identity-s1a/design.md`,
"Passkeys") settle the outline: relying-party id = host of
`CAMPBOOKS_PUBLIC_URL`, registration while signed in with step-up,
discoverable credentials through conditional UI plus a "Use a passkey"
button, passkey-only sign-up under the signup policy, attestation `none`,
EdDSA/ES256/RS256 verified with `aws-lc-rs` and the key parsed with `coset`,
a sign count that goes backwards audited rather than refused. This document
decides what that outline leaves open, and how.

What `next` had before this change: the `passkeys` and
`webauthn_challenges` tables, `sessions.passkey_id` and `sessions.amr`
(migration `identity__passkeys`), `Principal.auth_time`,
`domain::step_up::require_recent`, `domain::ways_in::lock_and_count`, and
`/bff/login?max_age`. These landed in this branch's first commit so slice S2
could build on them, and reached `next` with #458. The sign-in screen
(#455) showed a passkey button that linked to `/auth/start/passkey`, a route
that never existed.

## Data ownership

Schema `identity`, no row-level security (a person's, not a workspace's):

```text
passkeys            id · account_id → accounts (cascade) · credential_id bytea UNIQUE
                    (16..1023 bytes) · public_key (COSE_Key as registered) · alg
                    (-8|-7|-257) · sign_count (never moves back) · aaguid ·
                    transports text[] · backup_eligible · backup_state · name
                    (1..64) · created_at · last_used_at
webauthn_challenges challenge_hash PK (SHA-256) · purpose (register|sign_in|sign_up) ·
                    account_id (register) · browser_hash (sign_in|sign_up) ·
                    user_handle + display_name (sign_up) · request_id · return_to ·
                    created_at · expires_at (5 min); a CHECK per purpose
sessions            + passkey_id → passkeys (set null) · amr text[] default {fed}
```

`identity.prune` deletes expired challenges with the other expired sign-in
state. A passkey's public key is not a secret. No secret is added.

## Events

Published, instance-scoped, payloads with ids and method names only:

- `identity.way_in_added` {account, method: `passkey`, provider: null,
  via: `passkey` | `sign_up`};
- `identity.way_in_removed` {account, method: `passkey`, provider: null};
- `identity.account_created` {account, provider: `passkey`};
- `identity.signed_in` {account, method: `passkey`, provider: null}.

Nothing is consumed.

## Endpoints and scopes

| Operation | Path | Security |
| --- | --- | --- |
| `identity.start_passkey_sign_in` | `POST /auth/passkey/sign-in/options` | public, CSRF header + same origin, rate-limited per address |
| `identity.finish_passkey_sign_in` | `POST /auth/passkey/sign-in` | the same, plus the ceremony cookie |
| `identity.start_passkey_sign_up` | `POST /auth/passkey/sign-up/options` | public, CSRF, rate-limited; signup policy |
| `identity.finish_passkey_sign_up` | `POST /auth/passkey/sign-up` | the same, plus the ceremony cookie |
| `identity.list_passkeys` | `GET /api/v1/identity/passkeys` | `identity:read` |
| `identity.start_passkey_registration` | `POST /api/v1/identity/passkeys/registrations` | `identity:write`, step-up |
| `identity.register_passkey` | `POST /api/v1/identity/passkeys` | `identity:write`, step-up |
| `identity.rename_passkey` | `PATCH /api/v1/identity/passkeys/{id}` | `identity:write` |
| `identity.remove_passkey` | `DELETE /api/v1/identity/passkeys/{id}` | `identity:write`, step-up |

None is opened to automation or the assistant (`x-campbooks` defaults). A
passkey of another account is 404. The `/auth/*` actions answer 404 when the
issuer doesn't run here, or when the public URL can't be a relying party
(plain http on a host other than `localhost`, or an IP address).

## The relying party (`identity::webauthn`)

Pure functions, no database:

- **Options** in WebAuthn L3's JSON forms, so the client converts base64url
  and nothing else. The user handle is the account id's 16 bytes, which
  carry no personal data. Timeout five minutes.
- **Registration** (§7.1):
  - `type` = `public-key` and `id` = `rawId`;
  - client data `type` = `webauthn.create`, `challenge` (constant-time),
    `origin` = the public origin exactly, `crossOrigin` not true, no
    `topOrigin`;
  - the attestation object is a CBOR map with `fmt`, `attStmt`, `authData`
    and nothing after it;
  - `fmt` is one of the registered formats, and `none` needs an empty
    statement;
  - `rpIdHash`, UP and UV set, BS only with BE, AT present, the credential
    id equal to `rawId` and at most 1023 bytes;
  - the COSE key: `alg` stated and one of the three, matching its key type
    and curve, no private parameter, RSA at least 2048 bits, P-256
    coordinates uncompressed.
- **Assertion** (§7.2):
  - the same client-data checks with `webauthn.get`;
  - the authenticator data without attested credential data;
  - BE unchanged since registration;
  - the signature over `authData || SHA-256(clientDataJSON)`, verified with
    `aws-lc-rs`: `ED25519`, `ECDSA_P256_SHA256_ASN1`,
    `RSA_PKCS1_2048_8192_SHA256`.
  - The caller checks that `userHandle` is the passkey's account, and
    compares the counter.
- Every refusal is a labelled `Refusal`. Endpoints answer one generic
  `passkey_not_accepted` and log only the label (never the credential,
  challenge or client data).

## Ceremonies

- **Challenges** are 256-bit random values (`random_token`), stored as
  SHA-256, looked up by the hash of the challenge the client data carries.
  `take` deletes the row whatever happens next, so one attempt spends it.
  It also has to match purpose, expiry, and the account (register) or the
  browser's ceremony cookie (sign-in, sign-up).
- **The ceremony cookie** `__Host-campbooks_webauthn` is HttpOnly,
  `SameSite=Strict` and lasts five minutes. Like the S1a cookies, it is
  dropped to `campbooks_webauthn` over plain http on localhost. It is
  cleared after a sign-in or sign-up.
- **Sign-in** finds the passkey by credential id (`FOR UPDATE`; a disabled
  account's passkeys are not found), checks the user handle, verifies the
  assertion and records the use. The counter only moves forward. A reported
  counter at or below the stored one, when either is non-zero, is audited
  as `passkey_sign_count_regressed` and allowed (synced passkeys report 0).
  It then starts the issuer session with `amr` and goes on exactly as a
  provider sign-in does (`continue_after_sign_in`).
- **Sign-up**:
  - the options step checks the policy, cleans the name (1–64 printable
    characters) and picks the account id;
  - the finish step checks the policy again, verifies, then creates the
    account (no address, `primary_email_verified = false`), the passkey
    (named "Passkey") and the session in one transaction.
- **Registration** binds the challenge to the account. Both calls need
  step-up. A credential id already registered, by anyone, is 409
  `passkey_already_registered`.
- **Removal** follows `remove_way_in` (S2):
  - `lock_and_count`, then 404 unless it is the caller's, then 409
    `last_way_in`;
  - every session signed in with it ends (`end_signed_in_with_passkey`) and
    its refresh families are revoked;
  - it is deleted, audited and announced.

## `amr`

ID tokens report what the session recorded:

- a provider session: `["fed"]`, the column's default, so S1a/S2 sessions
  are unchanged;
- a passkey session: `["swk", "user"]` for a backup-eligible (synced)
  passkey, `["hwk", "user"]` for a device-bound one.

The token endpoint reads it from `grant.session_id`. A family whose session
is gone was revoked with it, so the `fed` fallback is never relied on for a
passkey.

## The client

- **`webauthn.ts`** converts the JSON forms by hand: base64url ↔ bytes, and
  the `PublicKeyCredential` fields, so the conversion doesn't depend on
  `parseCreationOptionsFromJSON` and `toJSON`, which only recent browsers
  have. It maps browser rejections:
  - `NotAllowedError` / `AbortError` → cancelled;
  - `InvalidStateError` → exists;
  - `NotSupportedError` / `SecurityError` → unsupported.
- **`/sign-in`**:
  - when the server lists `passkey` and the browser has passkeys, "Use a
    passkey" (`P`) runs the dialog ceremony in place. Shown only then.
  - where conditional mediation is available, a "Saved passkeys" field
    (`autocomplete="username webauthn"`) starts an autofill ceremony on
    load. The dialog aborts it first, and autofill restarts after a refused
    or cancelled attempt.
  - "New here? Create an account with a passkey" when `passkey_sign_up`.
  - Errors are explained in place: cancelled (neutral), not accepted,
    expired, rate-limited, failed.
- **`/sign-in/passkey`** is a one-question `FocusPage`: name, Create
  (Enter), Sign in another way (Esc). It explains its outcomes: steps,
  cancelled, exists, expired, invite-only, an unsupported browser.
- **The Passkeys section** is `PasskeysSection` in `WAY_IN_SECTIONS`, S2's
  extension point:
  - the list shows synced or one device, added and last used, with Rename
    in place and Remove behind `InlineConfirm`;
  - Add a passkey (`N P`, a sequence like `N W`) names it after the device
    ("Passkey on Mac");
  - step-up offers "Sign in again" (`/bff/login?max_age=600&return_to=`);
  - `last_way_in` is explained in place.

## Decisions and alternatives considered

The old Rails app on `main` had passkeys (the `webauthn` gem) as a **second
factor**:
- non-discoverable, user verification discouraged;
- the challenge in the cookie session;
- reauthentication to remove.

Copied on the merits: attestation `none`, `excludeCredentials`, a fresh
sign-in to change them. Not copied: second-factor-only and optional user
verification (here a passkey is a first factor, so UV is required), and the
session-held challenge (below).

### 1. Where the relying party comes from

- **A. Hand-rolled on `coset` + `ciborium` + `aws-lc-rs` (chosen).** It is
  the narrow profile of the S1a design: three algorithms, no attestation
  trust, no extensions. A software authenticator in the tests drives every
  check of §7.1/§7.2, and Chrome's virtual authenticator drives the
  recording.
- **B. `webauthn-rs` 0.5.** The best-reviewed RP in Rust, but
  `webauthn-rs-core` needs OpenSSL (banned in `deny.toml`) and is MPL-2.0.
- **C. `webauthn_rp` 0.3.** It pulls RustCrypto `rsa` 0.9 (RUSTSEC-2023-0071)
  and `ed25519-dalek` 2 next to `aws-lc-rs`, and has a small user base.

### 2. Where registration runs

- **A. The API, with the access token's `auth_time` (chosen).** Adding a way
  in is an account operation like S2's `start_link`. It shares the step-up
  rule (`require_recent`) and is reachable from native apps with their own
  tokens. The challenge is bound to the account.
- **B. The issuer's `/auth/passkey/register/*` with the issuer cookie.** It
  would know `auth_time` directly from the session, but a second step-up
  mechanism next to S2's, and the web BFF's session would not be the
  authority any more.

### 3. How a challenge is kept

- **A. A row per ceremony, deleted when taken (chosen).** It is single-use
  by construction, survives restarts and several API processes, is bound to
  an account or a browser, and pruned with the other sign-in state.
- **B. A stateless, HMAC-signed challenge** (expiry and binding inside the
  token). No table, but it isn't single-use without a spent-list, which is
  a table again.
- **C. In memory.** It breaks with more than one API process and on every
  restart.

### 4. Attestation formats other than `none`

- **A. Accept the registered formats, verify none of their statements,
  record the format (chosen).** We ask for `none`, and browsers usually
  strip attestation. When an authenticator still sends `packed` (some
  security keys) nothing here depends on provenance, which §7.1's note on
  self-attestation allows. Unknown formats are refused.
- **B. Refuse anything but `none`.** Stricter-looking, but it locks out
  hardware keys that always attest, for no security gain since we trust no
  attestation anyway.

### 5. A counter that goes backwards

- **A. Audit and allow, keep the highest counter (chosen, per the S1a
  design).** Synced passkeys report 0, and some authenticators reset.
- **B. Refuse the sign-in.** That locks the person out on a false positive,
  with no recovery path.

### 6. Conditional UI on a screen with no username field

- **A. A labelled "Saved passkeys" field where conditional mediation is
  available (chosen).** Browsers offer passkeys only through a field's
  autofill, and the sign-in screen has no address field.
- **B. No field, the button only.** Simpler, but it loses autofill, which
  ADR 0014 asks for.
- **C. An address field to fill in.** That invites matching by email, which
  this design never does.

### 7. Where passkey-only sign-up lives

- **A. Its own one-question screen, `/sign-in/passkey` (chosen).** It asks
  only what a passkey can't tell us (a name) and creates in one step.
- **B. Reusing `/sign-in/new` (pending sign-up).** Its pending row and copy
  are about a provider identity that already exists, and there is no
  provider round trip here to make one.

### 8. Signup policy for a passkey alone

`invite_only` refuses it at both steps: an invitation is matched by a
verified address, which a passkey never proves. `open`, `code` and
`approval` allow it, and their gate stays at workspace creation, as for
providers (S1a).

## Security posture

- Fail closed: every malformed or unverifiable credential is
  `passkey_not_accepted`; a missing or foreign ceremony is
  `passkey_expired`; nothing is created before verification succeeds.
- Phishing resistance comes from the exact origin and `rpIdHash` checks, and
  `crossOrigin`/`topOrigin` are refused, so the screens can't be framed.
- Single use: challenges are taken before verification. A replayed
  assertion finds no ceremony.
- Binding: sign-in and sign-up ceremonies are bound to the browser's
  `Strict` cookie. Registration is bound to the account and needs step-up.
- Rate limits: the kernel limiter (S4) caps 30 ceremony calls per minute
  per client address on the four `/auth/passkey/*` endpoints; at most ten
  challenges waiting per address and ten registrations per account (per
  five minutes, a challenge's lifetime); at most 30 passkey-only accounts an
  hour instance-wide.
- Bounded storage (security review of #465): the ceremony endpoints read at
  most 16 KiB of body; a `request` id must be at most 64 URL-safe characters
  and name a waiting authorization request before anything is stored; the
  migration `identity__passkey_limits` adds length CHECKs on every
  free-form column of `webauthn_challenges` and on `passkeys.public_key` and
  `transports`.
- Key and encoding checks: an Ed25519 key must be a canonical point not of
  small order (libsodium's list) that `aws-lc` parses, and a P-256 key a point
  `aws-lc` accepts (on the curve); credential ids are at least 16 bytes; the
  attestation object is strict CBOR (no duplicate keys, definite lengths,
  shortest heads) and the credential public key CTAP2-canonical (§6.5.1.1).
- One ceremony cookie per browser, reused by every ceremony it starts: the
  autofill ceremony and the dialog's keep their own challenges instead of the
  second orphaning the first.
- Logs and events hold no credential, challenge, client data or address,
  only passkey ids and refusal labels.
- Tests cover every refusal path, in Rust (RP and endpoints) and in the
  client.
- Left for the security review (#57):
  - review of the hand-rolled verification against the WebAuthn L3 test
    vectors and a second browser (Safari/Firefox) by hand;
  - per-credential rate limits on sign-in;
  - notifying the person, or refusing, on a counter regression of a
    device-bound passkey;
  - recovery when the last passkey is lost;
  - an operator view of `passkey_sign_count_regressed`.

## Risks

- **Own WebAuthn code.** The surface is narrow, and every check has a
  refusing test. `webauthn-rs` stays the fallback if its OpenSSL dependency
  goes.
- **A passkey-only account has no address.** It can't accept an email
  invitation yet (S1a follow-up), so the sign-up screen says a provider can
  be added later.
