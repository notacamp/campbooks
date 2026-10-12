# AGENTS.md — rules for humans and agents

This file is the single source of truth for how work is done in this repo.
`CLAUDE.md` only imports it. Every rule names how it is enforced; a rule with
no enforcement is a convention you are still expected to follow.

## Read first

1. `docs/architecture.md` — the shape of the system (modules, crates, clients).
2. `docs/adr/` — decisions already taken. Don't re-litigate them in code; write
   a new ADR that supersedes one if you disagree.
3. `openspec/config.yaml` and `openspec/specs/` — the behaviour baseline.
4. `CONTRIBUTING.md` — branch, PR, changelog and release flow.

## 🔒 Public repository

This repo is public and source-available. **Nothing private lives here** — not
in code, docs, fixtures, or history: no secrets, IPs, hostnames of private
infrastructure, account IDs, or real people's names/emails. Hosted-cloud-only
behaviour, deploy pipelines and ops runbooks live in the private cloud repo.
Use `ENV` with neutral `example.com` defaults. *(gitleaks in CI.)*

## Spec-driven workflow

- **Specs before code.** Every behaviour change starts as an OpenSpec change
  (`openspec/changes/<name>/`: proposal, design, tasks, spec deltas). Use
  `/opsx:propose`, `/opsx:apply`, `/opsx:archive`. *(CI: `openspec validate --all --strict`.)*
- **One change = one branch = one PR.** The change records its tracking issue in
  `.openspec.yaml` (`issue: <url>`); never invent one.
- Unspecced behaviour and specs with no code behind them both block merge.
- Open questions for a human go in `openspec/decisions-needed.md`, one per
  heading, each with a recommended answer.

## Architecture rules (enforced)

- **Modular monolith, vertical slices.** A business domain is one crate under
  `crates/modules/<domain>`; inside it, each use case is a slice under
  `src/features/<feature>/` owning its handler, request/response types, queries
  and tests. No horizontal `controllers/`/`services/`/`repositories/` layers.
- **Modules don't import modules.** A module crate may depend on
  `campbooks-kernel` and `campbooks-ai` only. Cross-module needs go through
  domain events on the kernel's durable event bus (published in the same
  transaction as the change, ADR 0012), or an explicit allowlist entry in
  `architecture.toml` (reviewed like an ADR). *(CI: `cargo xtask arch`.)*
- **Kernel knows no domain.** `campbooks-kernel` depends on no internal crate;
  `campbooks-ai` depends only on the kernel; only `campbooks-server` composes
  modules. *(CI: `cargo xtask arch`.)*
- **Each module owns a Postgres schema** named after it. Migrations live in
  `migrations/` named `<timestamp>_<module>__<description>.sql` and touch only
  that module's schema. No cross-schema joins or foreign keys — reference other
  modules by ID. The kernel's infrastructure (event log, job queue) lives in
  schema `kernel`, migrated by `<timestamp>_kernel__<description>.sql`; modules
  never touch it. *(CI: `cargo xtask arch` checks migration names and the
  schemas each migration touches.)*
- **Background work is declared, named and owned.** Jobs, event handlers and
  schedules are registered in the module's `Module.background`, named
  `<module>.<name>`; events are `DomainEvent`s named `<module>.<event>` with
  an explicit `VERSION`. Job payloads hold ids, never secrets or message
  bodies, and `JobError` messages never carry secrets. *(Startup and
  `cargo xtask arch` reject misnamed or duplicate registrations.)*
- **Modules declare what they have** (ADR 0008). What a caller can *do* is an
  **action**, declared in the module's `Module.declarations` with the
  kernel's `Action` builder: named `<module>.<verb>`, with a scope, typed
  input and output (`ToSchema`), whether it mutates, optional undo and an
  optional default shortcut. The server generates its endpoint
  (`POST /api/v1/actions/<name>`), and `campbooks-server declarations`
  prints the catalogue clients and tools generate from, committed as
  `openapi/declarations.json`. Record types (`RecordType`,
  `<module>.<record>`, pointed at with `RecordRef`) and the events a module
  publishes (`.event::<E>()`) are declared there too. *(Startup and
  `cargo xtask arch` reject misnamed, duplicate or inconsistent declarations,
  and any action no test of its module calls by name; CI:
  `cargo xtask declarations --check`.)*
- **Every HTTP endpoint is in the OpenAPI document** (`utoipa`), which is
  generated to `openapi/openapi.json` and committed. *(CI: `cargo xtask openapi --check`.)*
- **Errors are RFC 9457 problem+json** via `kernel::Error`. Permission failures
  on reads return 404, not 403.
- **Clients mirror the server.** `clients/app/src/modules/<domain>` maps 1:1 to
  `crates/modules/<domain>`. Modules import each other only through `index.ts`;
  `lib/` never imports `modules/`. *(CI: dependency-cruiser.)*
- **Clients never hand-write API types**; they come from `openapi/openapi.json`.
  *(CI: `pnpm api:check`.)*

## Adding an action

1. In the feature slice (`crates/modules/<m>/src/features/<feature>/mod.rs`),
   define the input and output types (`Serialize`, `Deserialize`,
   `ToSchema`; give them names unique across the API) and the handler,
   `async fn(ActionContext, Input) -> Result<Output>`. Write through
   `ctx.tx()` and publish the action's events on that transaction.
2. Declare it next to the handler and list it in the module's
   `Declarations` (`.action(features::<feature>::action())`):
   ```rust
   pub fn action() -> ActionDecl {
       Action::<ArchiveInput, ArchiveOutput>::new("conversations.archive")
           .describe("Archive a conversation: it leaves the inbox until something new arrives.")
           .scope::<ConversationsWrite>() // one of the module's own scopes
           .undoable(unarchive)           // or .mutates() if it cannot be undone
           .shortcut("e")
           .handler(archive)
   }
   ```
   An undoable handler returns `Undoable<Output, U>`: `U` (ids and prior
   state, never secrets) is what `unarchive(ctx, U)` gets back.
3. Test it through its endpoint by name:
   `testing::call_action(&app, "conversations.archive", Some(&token), &input)`.
4. Run `cargo xtask openapi`, `cargo xtask declarations` and
   `pnpm --dir clients api:generate`, and commit the results. Add the label
   key (`<module>:actions.<verb>`) to the module's client i18n files when
   the client shows the action.

Never rename an action, record type or event: their names are wire contracts.

## Security rules

- **Tokens never reach browser JavaScript.** The web client talks to the API
  through the server's BFF with an httpOnly, `SameSite=Strict` session cookie.
  Native (Tauri) clients keep tokens in the OS keychain on the Rust side; the
  webview calls the API via Tauri commands. See ADR 0004.
- The API is an OAuth 2.1 resource server: validate the JWT (issuer, audience,
  expiry, JWKS) and check a scope on every handler. A scope is declared once
  as a `kernel::auth::ScopeName` type named `<module>:<action>` and listed in
  the module's `Module.scopes`. A handler requires it with the
  `kernel::auth::Scoped<S>` extractor and declares
  `security(("bearer" = ["<scope>"]))` in its `#[utoipa::path]`. A public
  endpoint says so with `security(())`. *(CI: `cargo xtask arch` fails on an
  operation with no explicit `security`, an unknown scope, or a misnamed
  scope; a server test checks that every scoped operation refuses anonymous
  calls.)*
- No `unsafe` (`#![forbid(unsafe_code)]` in every crate, workspace lint).
- Untrusted HTML (email bodies) is rendered only in a sandboxed iframe with a
  strict CSP; never `dangerouslySetInnerHTML` outside `lib/ui/safe-html`.

## Internationalization (enforced)

Campbooks ships in **English, European Portuguese, Spanish from Spain and
French** — locale tags `en`, `pt-PT`, `es-ES`, `fr`. Always the full tag:
a bare `pt` means Brazilian Portuguese to `Intl`/CLDR (dates, numbers,
plurals). Every string a user can see exists in all four before it ships.

- **Our UI:** every `i18n/` namespace has `en`, `pt-PT`, `es-ES` and `fr` files with
  exactly the same keys and no empty values. *(CI: `pnpm i18n:check`.)*
- **Our server:** messages a user reads (problem+json `title`/`detail`,
  emails, notifications) are rendered in the recipient's locale, from
  catalogues held to the same parity. *(Lands with the i18n foundation issue;
  until then, no user-facing server text.)*
- **Other people's screens we put in front of users** — the identity
  provider's login, sign-up, account pages and emails, and any future
  component that renders UI or mail to end users — count as ours. A component
  may only become a default once it covers all four languages; when it
  doesn't, contributing the missing translations upstream is part of adopting
  it, tracked as an issue that blocks shipping that component to users.
- **Translation quality:** machine-drafted translations are fine and need no
  separate human review, but they must read naturally, carry full diacritics
  (never ASCII-flattened text) and use the right variant: European
  Portuguese (utilizador, palavra-passe, ecrã, ficheiro) and Spanish from
  Spain. When contributing upstream, follow that project's policy and say
  plainly how the translations were produced.


- **Open-weight models only** in the shipped catalog. Every AI call goes through
  `campbooks-ai` by **role** (`decide`, `embed`, `complete`, optional
  `vision`), never by vendor. Providers speak the OpenAI-compatible HTTP API, so
  a self-hosted vLLM / llama.cpp / Ollama / TEI and a hosted provider serving
  open weights are interchangeable. Users and instance admins configure every
  role. See ADR 0005.
- AI output that changes user data is a **proposal** the user can undo, never a
  silent mutation.

## Code quality (all binary pass/fail in CI)

- Rust: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`
  (pedantic set in `Cargo.toml` `[workspace.lints]`), `cargo nextest run`,
  `cargo deny check` (advisories, licenses, bans, sources), `cargo machete`.
- TypeScript: `pnpm lint` (ESLint flat config, no `eslint-disable`),
  `pnpm typecheck` (strict + `noUncheckedIndexedAccess`), `pnpm test`,
  `pnpm deps:check`, `pnpm deadcode` (knip), `pnpm audit --prod`.
- Repo: gitleaks, Trivy on the built image, Conventional Commit PR title,
  changelog entry on `feat/` and `fix/` branches.

## Workflow

- Branch off `next` (the rewrite's trunk until cutover): `feat/<issue>-slug`,
  `fix/…`, `docs/…`, `refactor/…`, `chore/…`. PR title is a Conventional
  Commit; it becomes the squash commit. Commits must be signed.
- Tasks are tracked as GitHub issues in the private cloud repo; reference the
  issue URL in the PR body and in the change's `.openspec.yaml`.
- Update `CHANGELOG.md` `[Unreleased]` for user-visible changes.
- UI work: get a visual prototype approved before implementing it, then verify
  at **375px and desktop** before calling it done.
- **UI PRs carry a screen recording** (375px + desktop) of the change working,
  in a `## Recording` section of the PR body — `pnpm --dir clients record <scenario>`,
  then upload (maintainers: `upload-media`; contributors: drag into the PR).
  Demo data only. *(CI: Policy → recording check; `no-recording` label for
  non-visual changes, maintainers only.)*
- Never delete user data without explicit confirmation; never send email to a
  real person from a dev or test environment.
