# Tasks

## 1. Research

- [x] 1.1 Rauthy 0.36.2 provider admin API, callback URL, login page, linking and onboarding rules, and the 0.37.x differences, from the source and book at the tags; Google, Microsoft and Zoho discovery documents

## 2. Provisioning

- [x] 2.1 `deploy/idp/init.sh`: bootstrap the `campbooks` API key with `AuthProviders` read, create, update
- [x] 2.2 `deploy/idp/provision.sh`: providers from settings (secret files, Microsoft tenant, Zoho issuer), markers with fingerprint, adoption by issuer, signup-policy mapping, missing-rights message, no secret in output or argv; test seams `IDP_DATA_DIR` and `SERVER_SECRETS_DIR`
- [x] 2.3 `deploy/compose.yaml` (`idp-provision` environment and the read-only `secrets/` mount), `deploy/secrets/.gitignore`; `docker compose config -q`

## 3. Tests

- [x] 3.1 `xtask/tests/idp_provision.rs`: fake Rauthy admin API (wiremock) covering skip-when-unset, create once, idempotency, admin edits kept, re-apply on change, adoption, policy mapping and override, missing secret file, missing rights, secrets never in output

## 4. Docs

- [x] 4.1 `deploy/README.md` "Sign in with Google, Microsoft or Zoho" (redirect URL, console steps per provider, linking, policy table, rotation, removal), `.env.example`
- [x] 4.2 ADR 0004 findings, `openspec/decisions-needed.md` (one provider per account; Microsoft emails unverified), CHANGELOG
- [x] 4.3 Gates green
