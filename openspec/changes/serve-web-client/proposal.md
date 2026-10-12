# Proposal

## Why

ADR 0006 promises one image that serves both the API and the web UI, but
`campbooks-server` only answers `/api/*` and `/openapi.json` today. Without a
second web server in front of it, a self-hoster or a staging deploy gets an
API with no UI, and the web client's same-origin calls to `/bff/api/v1/...`
have nothing to answer them outside the Vite dev proxy.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/16. This
change covers the static-serving part of that issue and an interim `/bff/api/*`
passthrough; the real BFF (sessions, OIDC code flow, token attachment) is a
later change.

## What Changes

- New optional setting `CAMPBOOKS_WEB_DIR`. When set, `campbooks-server` serves
  the built web client from that directory on the same port as the API, with a
  single-page-app fallback to `index.html` for browser navigations. When unset,
  the server stays API-only, exactly as today.
- Caching rules: content-hashed files under `/assets/` are cached for a year
  and marked immutable; `index.html` and other unhashed files are revalidated
  on every load.
- Security headers on every response (`nosniff`, a strict referrer policy,
  framing denied) and a strict Content-Security-Policy on HTML.
- `/api/*`, `/bff/*` and `/openapi.json` never fall back to the web client:
  unknown paths there stay `application/problem+json` 404s.
- Interim `/bff/api/*` passthrough: forwards to the same API routes with the
  `/bff` prefix removed. It attaches no credentials and adds no session, so it
  can never grant more than calling `/api/*` directly. It mirrors the Vite dev
  proxy and is replaced by the BFF in issue #16.
- The release image gains a Node build stage that builds `clients/app` and
  ships the result at `/app/web` with `CAMPBOOKS_WEB_DIR=/app/web` set, so the
  published image serves the UI out of the box.

## Capabilities

### New Capabilities

- `web-delivery`: how one server process delivers the web client next to the
  API: static files, SPA fallback, caching, security headers, reserved API
  paths, and the interim BFF passthrough.

### Modified Capabilities

None. The `system` endpoints are unchanged.

## Impact

- Modules: none. Static serving and the passthrough are composition concerns
  and live in `campbooks-server`; the kernel gains one optional config field.
  No new module, no cross-module dependency, no `architecture.toml` entry.
- Dependencies: tower-http's `fs` and `set-header` features (already a
  dependency).
- OpenAPI: unchanged. `/bff/api/*` mirrors documented `/api/*` routes and is
  not a separate API; static files are not API endpoints.
- Image: one more build stage (Node 22 + pnpm from `packageManager`); the
  runtime image stays distroless and non-root. Self-hosters get the UI on the
  same port (8080) after `docker compose pull`, with no config change.

## Non-goals

- The BFF itself: sessions, the OIDC authorization-code flow, CSRF protection
  and token attachment (issue #16, ADR 0004).
- Serving the UI from a CDN or a separate container.
- Pre-compressed (`.br`/`.gz`) assets; responses keep using on-the-fly
  compression.
- Any change to the desktop or mobile (Tauri) clients.
