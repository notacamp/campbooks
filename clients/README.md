# Campbooks clients

One React + TypeScript UI (`app/`) shipped to the web and, through [Tauri 2](https://tauri.app),
to desktop (macOS, Windows, Linux), iOS, Android and tablets. See `../docs/architecture.md`
("Clients") and `../AGENTS.md` for the rules this code follows.

## Prerequisites

- Node.js 22 (`nvm use` reads `.nvmrc`) and pnpm 11 (`corepack enable` picks the version
  pinned in `package.json`).
- Desktop only: Rust (the toolchain pinned in `../rust-toolchain.toml`) and the
  [Tauri system prerequisites](https://tauri.app/start/prerequisites/) for your OS.
- Mobile: see [`app/src-tauri/MOBILE.md`](app/src-tauri/MOBILE.md).

```sh
pnpm --dir clients install
```

Dependencies are pinned to exact versions. `pnpm-workspace.yaml` refuses releases younger
than 7 days, provenance downgrades and git/tarball sub-dependencies, and blocks every install
script that isn't explicitly allowed.

## Web development

```sh
cargo run -p campbooks-server        # the API on http://localhost:8080 (repo root)
pnpm --dir clients dev               # the UI on http://localhost:5173
```

The web client only ever calls same-origin `/bff/api/v1/...` with `credentials: "same-origin"`;
it never sees a token. In local development the Vite dev server stands in for the BFF: it
proxies `/bff/api/...` to `http://localhost:8080/api/...` (override with `CAMPBOOKS_API_URL`)
**without any session layer**. The real BFF (httpOnly `SameSite=Strict` session cookie + OIDC
code flow, ADR 0004) is served by `campbooks-server` and lands in a later task.

Without a running API the home page shows a "can't reach the API" state instead of crashing.

## Screen recordings for PRs

UI pull requests include a screen recording at 375px and desktop width.

```sh
pnpm --dir clients --filter @campbooks/app exec playwright install chromium   # once
pnpm --dir clients dev                                # the UI on http://localhost:5173
pnpm --dir clients record home                        # [--base-url http://localhost:5173]
```

This writes `app/recordings/out/<scenario>-375.webm` and `<scenario>-desktop.webm`
(gitignored). Maintainers upload with `upload-media <file>`; contributors drag the files into
the PR's "## Recording" section.

A scenario is `app/recordings/scenarios/<name>.ts` exporting a default async function that
drives Playwright's `Page`; use `pause(ms)` from `../context.ts` to linger on key states:

```ts
import { pause, type ScenarioContext } from "../context.ts";

export default async function home({ page }: ScenarioContext): Promise<void> {
  await page.goto("/");
  await pause(1500);
}
```

Recordings must show demo data only, never a real person's mail or documents.

## Desktop development

```sh
cd clients/app
pnpm tauri dev                       # starts Vite, then the native window
pnpm tauri build                     # release bundle for the current OS
```

Inside Tauri the UI never uses `fetch` for the API: `src/lib/api` hands each request to the
Rust command `api_request` (`src-tauri/src/api.rs`), which forwards it to `CAMPBOOKS_API_URL`
(default `http://localhost:8080`) and will attach the token from the OS keychain (ADR 0004).
`src-tauri/` is its own Cargo workspace, so GUI system libraries never enter server CI.

## Quality gates

All of these run in CI and must pass. Run them from the repo root with `--dir clients`, or
from `clients/`:

| Command                                     | What it checks                                                                       |
| ------------------------------------------- | ------------------------------------------------------------------------------------ |
| `pnpm lint`                                 | ESLint (typescript-eslint strict + stylistic type-checked, React, hooks, a11y, ...)  |
| `pnpm typecheck`                            | `tsc -b` with `strict`, `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`     |
| `pnpm test`                                 | Vitest + Testing Library; >= 80% coverage of `src/lib` and `src/modules`             |
| `pnpm deps:check`                           | dependency-cruiser architecture rules (`app/.dependency-cruiser.cjs`)                |
| `pnpm deadcode`                             | knip: unused files, exports and dependencies                                         |
| `pnpm format:check`                         | Prettier (`pnpm format` to fix)                                                      |
| `pnpm build`                                | type-check + production build to `app/dist`                                          |
| `pnpm audit --prod`                         | known vulnerabilities in shipped dependencies                                        |
| `pnpm api:check`                            | `src/lib/api/schema.gen.ts` is up to date with `openapi/openapi.json`                |
| `cargo clippy --all-targets -- -D warnings` | the Tauri shell (run inside `app/src-tauri`, plus `cargo fmt --check`, `cargo test`) |

Rules worth knowing before you write code:

- `eslint-disable` comments are ignored and reported. Fix the code, or change the rule in
  `eslint.config.js` where it gets reviewed.
- `dangerouslySetInnerHTML` is banned outside `src/lib/ui/safe-html/`; `localStorage` /
  `sessionStorage` outside `src/lib/storage/` (UI preferences only, never credentials).
- Modules import each other only through `modules/<name>/index.ts`; `lib/` never imports
  `modules/` or `app/`; only `lib/api` may import the generated schema.
- API types are generated, never hand-written: `pnpm api:generate` after the server's OpenAPI
  document changes (`cargo xtask openapi` at the repo root).

## Layout

```
app/
  src/app/        shell: main.tsx, router.tsx (the one router), providers, module manifest, routes/
  src/lib/        business-agnostic toolbox: api (typed client + transport), i18n, platform, storage
  src/modules/    one folder per server module (crates/modules/<name>), public surface = index.ts
  test/           test helpers, setup, and the architecture (dependency-cruiser) test
  src-tauri/      Tauri 2 shell (desktop + mobile)
```

Each module ships its strings in `modules/<name>/i18n/{en,pt,es,fr}.json`; they become the
i18next namespace `<name>`, downloaded the first time one of the module's screens renders.
The shell's own strings (`src/app/i18n/`) are bundled.
