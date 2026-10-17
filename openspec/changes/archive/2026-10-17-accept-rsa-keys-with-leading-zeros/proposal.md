# Proposal

## Why

Signing in with Zoho, and connecting a Zoho mailbox, always fail with "The
provider didn't let you in". The server logs `the provider's ID token is
invalid` for every Zoho callback since Zoho went live (14, 16 and 17 October
in production).

Every Zoho data centre (eu, com, in, com.au, jp, sa, com.cn) publishes its
RSA signing key with the modulus `n` encoded with a leading zero byte (257
bytes for a 2048-bit key). The kernel builds keys with `jsonwebtoken`'s
`DecodingKey::from_jwk`, which keeps those bytes as published, and the RSA
backend (aws-lc-rs) refuses a component that starts with a zero byte when it
verifies. Every Zoho signature therefore fails to verify. Google publishes
minimal encodings, so Google sign-in is unaffected.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/221

## What Changes

- When the kernel loads an RSA key from a JWKS, it drops leading zero bytes
  from `n` and `e` before building the key. The number is the same, so no
  check gets weaker: signature, `kid`, algorithm, issuer, audience, expiry
  and nonce are verified as before.

## Non-goals

- No change to which keys are usable (signing RSA/RS256 and Ed25519 only)
  or to any provider's flow.
- Saying which ID token check failed in the sign-in log (the kernel reports
  one opaque `invalid_token`) is a separate follow-up.

## Impact

- `crates/kernel` (`auth::jwks::usable_key`). Applies to every JWKS the
  kernel reads: upstream providers' ID tokens and an external IdP's access
  tokens.
- No setting, API, schema or UI change.
