# Tasks

- [x] 1.1 Remove `idp`, `idp-init`, `idp-provision`, `idp-data` and the Rauthy env from `deploy/compose.yaml`; mount `deploy/secrets/` into the server; validate with `docker compose config -q`
- [x] 1.2 Delete `deploy/idp/` and `xtask/tests/idp_provision.rs`; fix `server/init.sh`, `secrets/.gitignore`
- [x] 1.3 `deploy/.env.example`, `deploy/README.md` (incl. upgrade section), `clients/README.md`, `docs/architecture.md`, `openspec/decisions-needed.md`
- [x] 1.4 Reword Rauthy comments and test names in `kernel`, `identity`, `server`
- [x] 1.5 Spec deltas: remove `upstream-sign-in`; add the stack requirement to `identity-authorization-server`
- [x] 1.6 CHANGELOG (what to remove, what happens to accounts)
- [x] 1.7 Gates green: fmt, clippy, tests, `cargo xtask arch`, openapi check, deny, machete, OpenSpec validate
- [x] 1.8 Archive the change (`openspec archive remove-rauthy --yes`)
