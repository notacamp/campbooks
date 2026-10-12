# Tasks

## 1. Configuration

- [x] 1.1 Add optional `web_dir` (`CAMPBOOKS_WEB_DIR`) to the kernel `Config` and the test config; verify with a config unit test that it defaults to unset and parses a path
- [x] 1.2 Refuse to start when the configured directory has no `index.html`; verify with a unit test of the startup check

## 2. Serving the web client

- [x] 2.1 Add `crates/server/src/web.rs` serving files from the directory with the fallback rules in design.md; verify with server tests for `/`, a deep link, an existing asset, a missing asset, a non-HTML miss and `/api/v1/nope` while serving
- [x] 2.2 Set `Cache-Control` (immutable for `/assets/`, `no-cache` otherwise) and the CSP on HTML; verify the headers in the same tests
- [x] 2.3 Add the security-header layers to every response; verify on an API response and on `/`
- [x] 2.4 Keep the API-only behaviour when the directory is unset; verify `/` is a problem+json 404

## 3. Interim BFF passthrough

- [x] 3.1 Nest the module routes under `/bff` with their own problem+json fallback, documented as a placeholder with no auth; verify `/bff/api/v1/system/version` equals `/api/v1/system/version` and `/bff/api/v1/nope` and `/bff/nope` are problem+json 404s

## 4. Image

- [x] 4.1 Add a Node build stage to the `Dockerfile` and ship `clients/app/dist` at `/app/web` with `CAMPBOOKS_WEB_DIR` set; update `.dockerignore`; verify `docker build` succeeds and the container serves `/`, a deep link, `/api/v1/system/health` and `/bff/api/v1/system/version` with the expected headers
- [x] 4.2 Update `deploy/README.md` and `docs/architecture.md` to say the image serves the UI on the same port; verify the docs match the container behaviour

## 5. Wrap-up

- [x] 5.1 OpenAPI regenerated (`cargo xtask openapi --check` passes; no change expected) and client types regenerated (`pnpm api:check`; no change expected)
- [x] 5.2 CHANGELOG `[Unreleased]` updated
- [x] 5.3 Gates green: fmt, clippy, tests, `cargo xtask arch`, sqlx prepare check, `cargo deny`, `cargo machete`, `openspec validate --all --strict`
- [x] 5.4 UI verified at 375px and desktop served by the container, with no CSP violations in the console
