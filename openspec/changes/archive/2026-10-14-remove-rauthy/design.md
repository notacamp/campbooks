## Context
ADR 0014 made Campbooks its own identity provider and listed "Rauthy is removed: the `idp`, `idp-init` and `idp-provision` services, `deploy/idp/`, its settings and its docs" as slice S5. The built-in issuer runs when `CAMPBOOKS_AUTH__ISSUER` is unset (or names the public origin) and a secret key exists; otherwise the identity module is off and the BFF and the resource server follow whatever issuer is configured. Production and staging have been on the built-in issuer since cloud #94 and #104.

## Decision: is an outside OIDC issuer still a supported self-host option?
**Decision: the generic mechanism stays; everything Rauthy-specific goes.**

- Kept: `CAMPBOOKS_AUTH__ISSUER` / `JWKS_URL` / `AUDIENCE` for the resource server, and the BFF's external-issuer path (`CAMPBOOKS_BFF__CLIENT_SECRET[_FILE]`, `IDP_INTERNAL_URL`). They are plain OIDC and OAuth 2.1 (discovery, JWKS, authorization code with PKCE, refresh, end-session); no code names Rauthy. The verifier tests and the `TestIdp` fake that exercise the external path are the proof it works and stay, with their comments made vendor-neutral.
- Removed: provisioning of scopes and clients, sign-in provider registration, scope mirroring, Rauthy-specific compose services, secrets layout and runbooks. Campbooks documents the external option in one paragraph as "advanced, you provision it yourself", and says what is off while it is used (ways in, apps, mailbox connection, since those live in the built-in issuer).
- The compose file no longer sets the external-issuer variables, so a default install gets the built-in issuer.

## Alternatives considered
1. **Remove the external-issuer path from the code too** (BFF external branch, `Deps` early return, verifier remote keys). Smallest long-term surface and one way to run. Rejected for this change: the resource server's remote-JWKS verifier is also how the API validates tokens in tests and could serve a future split deployment; removing it is a refactor across `kernel::auth`, `bff/config.rs` and `identity/deps.rs`, files that open PRs (#479, #480) are changing now, and it would break anyone who already points at another issuer with no gain for the deploy cleanup. It can follow as its own change if nobody uses it.
2. **Keep Rauthy as an optional compose profile.** Preserves a path for self-hosters who like it. Rejected: it keeps the provisioning scripts, the fake-Rauthy test, the scope mirroring and a second source of truth for accounts alive, for a path the owner no longer runs anywhere; ADR 0014 already rejected working around Rauthy (two consents, no upstream tokens).
3. **Chosen: remove the Rauthy deploy, scripts, tests and docs; keep the generic external-issuer code, documented as unprovisioned.** Least risk, no behaviour change, matches ADR 0014's consequences (three fewer services, one fewer volume to back up).

This does not copy a pattern from the old Rails app on `main`.

## Upgrade path (self-hosters)
- New `compose.yaml`: `docker compose up -d --remove-orphans` stops and removes the three Rauthy containers. The `idp-data` volume is left in place for the operator to remove. An old compose file kept as is still works (it still names Rauthy), it is simply no longer shipped.
- Accounts created in Rauthy have Rauthy ids as `idp_subject`; the built-in issuer issues Campbooks account ids, so those people get a new account on first sign-in. Documented in the README and CHANGELOG. There is no migration in this change (non-goal).
- `CAMPBOOKS_IDP_*` settings are ignored; provider apps move to `CAMPBOOKS_<PROVIDER>__*`.

## Data ownership, events, scopes
None: no schema, event, endpoint or scope changes.
