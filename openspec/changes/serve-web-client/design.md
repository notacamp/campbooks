# Design

## Context

`campbooks-server` composes the module routers into one axum `Router`, adds
`/openapi.json` and a problem+json 404 fallback, then wraps everything in the
middleware stack (request id, tracing, timeout, compression). The web client
(`clients/app`) is a Vite build: `index.html` plus content-hashed files under
`assets/`. In the browser it calls the API at same-origin `/bff/api/v1/...`
(`clients/app/src/lib/api/transport.ts`); in development the Vite proxy
rewrites that to `/api/v1/...`. See proposal.md for why this is needed now.

Data ownership: none. No module schema, no migration, no events published or
consumed. No new endpoint needs an OAuth scope: static files are public by
nature, and the passthrough reaches only routes that already declare their own
access rules.

## Goals / Non-Goals

**Goals:**

- One process, one port, one origin for UI and API.
- Keep the change in `campbooks-server` (composition), so module crates and
  `cargo xtask arch` are untouched.
- An unset `CAMPBOOKS_WEB_DIR` leaves the server byte-for-byte API-only.

**Non-Goals:**

- Sessions, OIDC, CSRF and token handling for `/bff` (issue #16).
- HTML templating or runtime config injection into `index.html`.

## Decisions

### Configuration lives in the kernel config, behaviour in the server

`web_dir: Option<PathBuf>` joins `campbooks_kernel::Config`, read from
`CAMPBOOKS_WEB_DIR` like every other setting. It is generic deployment config,
not domain knowledge, so the kernel stays domain-free. All serving code is in
`crates/server/src/web.rs`. At startup the server checks that the directory
holds an `index.html` and refuses to start otherwise: a typo in the path
should fail loudly, not serve 404s. An empty value counts as unset.

### Routing order

1. Module routes (`/api/v1/...`) and `/openapi.json`, as today.
2. `/bff` nests a copy of the module routes with its own problem+json 404
   fallback, so `/bff/api/v1/x` reaches the `/api/v1/x` handler and anything
   else under `/bff` is a 404. Giving the nested router its own fallback also
   stops `/bff/...` misses from reaching the SPA fallback with a stripped path.
3. The outer fallback. If no web directory is configured it is the existing
   problem+json 404. Otherwise it applies the rules below.

### Fallback rules (web client configured)

| Request | Answer |
| --- | --- |
| path is `/api`, `/api/*`, `/bff`, `/bff/*` or `/openapi.json` | 404 problem+json |
| method is not `GET`/`HEAD` | 404 problem+json |
| an existing file (`/` serves `index.html`) | the file |
| no file, path under `/assets/` | 404 problem+json |
| no file, `Accept` includes `text/html` | `index.html` (200) |
| no file, otherwise | 404 problem+json |

Requiring `text/html` in `Accept` limits the fallback to browser navigations.
A missing `/favicon.ico` or a `fetch()` with `Accept: */*` gets a real 404
instead of an HTML page that would be misparsed as JSON or an image. `/assets/`
never falls back because a stale hashed URL must fail visibly, not return HTML
with a JavaScript MIME expectation. Files are served with tower-http
`ServeDir`/`ServeFile`, which rejects path traversal, handles `HEAD`, ranges
and `If-Modified-Since`, and guesses MIME types.

Alternative considered: `ServeDir::fallback(ServeFile)`. It applies the
fallback to every miss, including `/assets/` and non-HTML requests, so it
cannot express the rules above.

### Caching

`/assets/*` files are named by content hash, so they get
`public, max-age=31536000, immutable`. Everything else the web client serves
(`index.html`, the SPA fallback, unhashed root files) gets `no-cache`: the
browser may store it but revalidates every time, so a deploy is visible on the
next load. Cache headers are only set on successful (2xx/3xx) responses.

### Security headers and CSP

A `SetResponseHeaderLayer` stack adds `X-Content-Type-Options: nosniff`,
`Referrer-Policy: strict-origin-when-cross-origin` and `X-Frame-Options: DENY`
to every response, API included.

HTML from the web client also gets:

```
default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:;
connect-src 'self'; object-src 'none'; base-uri 'none';
frame-ancestors 'none'; form-action 'self'
```

`style-src` has no `'unsafe-inline'`: the Vite build emits one external
stylesheet and no `<style>` blocks or `style=""` attributes in `index.html`,
and React applies `style` props through the CSSOM, which CSP does not govern.
Verified by loading the built client under this policy with no CSP violations
in the console. If a future dependency needs inline styles, relax `style-src`
in that change with the reason. The CSP is set only on HTML because it governs
documents; adding it to JSON or JS responses has no effect.

Email bodies will later need a sandboxed iframe with its own, stricter CSP
(AGENTS.md); that is out of scope here.

### Interim `/bff/api/*` passthrough

The passthrough is a placeholder with no auth semantics. It strips `/bff` and
dispatches to the same in-process routes; it adds no cookie, session or token.
The resulting guarantee is that `/bff/api/x` can never do more than `/api/x`,
so it opens nothing once the API starts requiring tokens: browser calls will
then get the same 401 a direct call gets until the BFF replaces this nest with
session lookup and token attachment. It is not listed in OpenAPI because it
mirrors documented routes rather than adding endpoints. The Vite dev proxy
rewrite (`/bff/api/` → `/api/`) mirrors it, so dev and prod behave alike.

### Image build

A `web` stage on `node:22-bookworm-slim` with `--platform=$BUILDPLATFORM`
(the output is platform-independent, so multi-arch builds do not run Node
under emulation) enables corepack, which installs the pnpm version pinned in
`clients/package.json` `packageManager`. Manifests and the lockfile are copied
first so `pnpm install --frozen-lockfile` is cached separately from sources;
then `pnpm --filter @campbooks/app build`. The repo's `pnpm-workspace.yaml`
supply-chain settings apply unchanged. The distroless runtime copies `dist/`
to `/app/web` and sets `CAMPBOOKS_WEB_DIR=/app/web`. `.dockerignore` lets
`clients/` in but keeps out `node_modules`, `dist`, coverage, Tauri build
output and recordings.

## Risks / Trade-offs

- [The passthrough is mistaken for the BFF] → doc comments in code, this
  design, and the spec requirement name it interim; issue #16 replaces it.
- [A new dependency starts injecting inline styles or scripts] → the CSP
  breaks visibly in the browser console; relax it deliberately in that change.
- [Image build gets slower] → the Node stage runs in parallel with the Rust
  stages under BuildKit and its install layer is cached on the lockfile.
- [Files outside `assets/` that are actually hashed get `no-cache`] → only a
  missed caching opportunity, never staleness.

## Migration Plan

None for operators. Existing self-hosters get the UI on their current port
after `docker compose pull && docker compose up -d`. Running the binary
outside the image without `CAMPBOOKS_WEB_DIR` keeps today's behaviour.
Rollback is redeploying the previous image.
