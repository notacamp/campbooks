## Why
Production and staging already run Campbooks' own issuer (ADR 0014); Rauthy runs nowhere. The public repo still ships it: three compose services and a volume, two provisioning scripts with a test, and docs that walk a self-hoster through Rauthy's admin UI. A new self-hoster would start a service nothing uses, and `deploy/compose.yaml` still points `CAMPBOOKS_AUTH__ISSUER` at it, so they would not get the built-in sign-in at all. This is slice S5 of ADR 0014.

## What Changes
- `deploy/compose.yaml`: the `idp`, `idp-init` and `idp-provision` services and the `idp-data` volume go. `CAMPBOOKS_AUTH__ISSUER`, `CAMPBOOKS_AUTH__JWKS_URL` and the `CAMPBOOKS_BFF__*` lines that named Rauthy go, so a fresh stack uses the built-in issuer. `deploy/secrets/` is mounted read-only into the server so provider client secrets (`CAMPBOOKS_<PROVIDER>__CLIENT_SECRET_FILE`) have a place to live.
- `deploy/idp/` (`init.sh`, `provision.sh`) and `xtask/tests/idp_provision.rs` are deleted.
- `deploy/.env.example`, `deploy/README.md`, `clients/README.md`, `docs/architecture.md`: Rauthy sections and `CAMPBOOKS_IDP_*` settings removed; an "Upgrading from the Rauthy sign-in" section added; the local-development recipe uses the built-in issuer.
- Rust comments and test names that mention Rauthy are reworded. No behaviour change.
- `openspec/specs/upstream-sign-in` (Rauthy provider provisioning) is removed; the built-in sign-in specs already cover Google, Microsoft and Zoho.
- `openspec/decisions-needed.md`: the Rauthy-only questions are dropped.
- ADR 0004 is already marked superseded by 0014; unchanged.

## Non-goals
- No change to the BFF's or the resource server's support for an outside OpenID Connect issuer (`CAMPBOOKS_AUTH__ISSUER` naming another origin): it stays, generic and unprovisioned (see design).
- No migration of accounts that existed in Rauthy (their `sub` is not a Campbooks account id); the upgrade notes say so.
- No change to the hosted-cloud stack, DNS or volumes: those are operator tasks listed in the tracking issue.
- Archived OpenSpec changes are history and are not edited.

## Impact
Modules touched: none at runtime (comments only in `kernel`, `identity`, `server`). Self-hosters: see CHANGELOG. Tracking: https://github.com/notacamp/campbooks-cloud/issues/82
