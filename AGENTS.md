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
  schema `kernel`, migrated by `<timestamp>_kernel__<description>.sql`, and the
  server's web BFF keeps its sessions in schema `bff`
  (`<timestamp>_bff__<description>.sql`); modules never touch either.
  *(CI: `cargo xtask arch` checks migration names and the schemas each
  migration touches.)*
- **Background work is declared, named and owned.** Jobs, event handlers and
  schedules are registered in the module's `Module.background`, named
  `<module>.<name>`; events are `DomainEvent`s named `<module>.<event>` with
  an explicit `VERSION`. Job payloads hold ids, never secrets or message
  bodies, and `JobError` messages never carry secrets. *(Startup and
  `cargo xtask arch` reject misnamed or duplicate registrations.)*
- **Every HTTP endpoint is in the OpenAPI document** (`utoipa`), which is
  generated to `openapi/openapi.json` and committed. *(CI: `cargo xtask openapi --check`.)*
- **The OpenAPI document is the operation catalogue (ADR 0013); there is no
  other action registry.** Every operation declares `operation_id =
  "<module>.<verb>[_<noun>]"` and an `x-campbooks` extension built from
  `kernel::Operation`; automation and the assistant call it by id through
  `kernel::operations::invoke`. Published events are declared with
  `Background::publishes` and listed in `openapi/events.json`. *(CI: `cargo
  xtask arch` checks ids, metadata, `mutates` vs method and undo targets;
  `cargo xtask events --check`.)*
- **Errors are RFC 9457 problem+json** via `kernel::Error`. Permission failures
  on reads return 404, not 403.
- **Clients mirror the server.** `clients/app/src/modules/<domain>` maps 1:1 to
  `crates/modules/<domain>` (a multi-word `snake_case` module is kebab-case
  on the client, as its file-name lint requires). Modules import each other only through `index.ts`;
  `lib/` never imports `modules/`. *(CI: dependency-cruiser.)*
- **Clients never hand-write API types**; they come from `openapi/openapi.json`.
  *(CI: `pnpm api:check`.)*

## Security rules

- **Tokens never reach browser JavaScript.** The web client talks to the API
  through the server's BFF with an httpOnly, `SameSite=Strict` session cookie;
  it sends `X-Campbooks-CSRF: 1` on every request, and the BFF refuses unsafe
  methods without it or from another origin. BFF operations (`/bff/*` only)
  declare `security(())` or the scope-less cookie scheme
  `security(("session" = []))`; never log tokens, codes, cookies or session
  ids.
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
- **One access model (ADR 0017).** Every resource type is declared once in
  its module's `Module.resources`: its owner, the owner's actions, and who
  else may do what (workspace role, container role, client tokens). Every
  handler acting in a workspace or for an account asks
  `kernel::authz::require` / `require_type` / `require_account` before it
  reads or changes anything. Anyone who may not read a resource gets 404,
  never 403. Every operation declares `x-campbooks.access` (`Operation::access`).
  Module code queries workspace tables only in `begin_for` / a job's
  `ctx.tx()`. *(CI: `cargo xtask arch` checks the declarations, the calls,
  pool queries and consumer-less events; exceptions live in
  `architecture.toml` `[access]`. The server's access matrix tests check
  every resource type and operation against every kind of caller.)*
- No `unsafe` (`#![forbid(unsafe_code)]` in every crate, workspace lint).
- **Telemetry carries no personal data or secrets.** Span fields, log fields
  and metric attributes never hold message content, prompts, tokens, cookies,
  codes or email addresses, and metric attributes never hold ids (route
  templates, names and small enums only). See `kernel::telemetry`. Error
  reports and the app's forwarded telemetry carry ids and error kinds only
  (`kernel::redact::error_kind`, `server::error_reports`,
  `system::client_telemetry`). *(CI: the server's telemetry privacy test,
  its error report tests and the client telemetry scrubber tests.)*
- **Personal data stays out of the queue and the event log** (campbooks-cloud#65).
  Events are kept 90 days and finished jobs up to 30, longer than a member's
  or a workspace's data once its deletion is asked for. So event payloads,
  job payloads and job errors (`kernel.jobs.last_error`) hold ids, codes,
  counts and blob references, never addresses, names, subjects or what
  someone wrote: that travels as a blob, which the purges delete. A job's
  error says what failed with ids and status codes (`redact::sqlx`,
  `redact::json` for errors that quote data, in an error enum's message too).
  *(CI: `cargo xtask arch` lists, each with its reason in
  `xtask/src/payloads.rs`: every event payload field that can hold free text,
  by its path (`EVENT_FIELDS_ALLOWED`); every job payload field
  (`JOB_PAYLOAD_FIELDS`); and every value a job error is built from, per file
  with its number of uses (`JOB_ERROR_VALUES`). A job it cannot read, or a
  `JobError` built where its message cannot be read, fails. Known limit: a
  count cannot catch a swap (one use of an expression removed and another use
  of the same expression, holding something else, added in the same file);
  review job errors in diffs as well.)*
- Untrusted HTML (email bodies) is rendered only in a sandboxed iframe with a
  strict CSP; never `dangerouslySetInnerHTML` outside `@notacamp/ui`'s
  `safe-html` (`clients/ui/src/safe-html`).

## Tracked events (enforced)

The record of every action (ADR 0020): who did it, in what context, and what
the records it touched looked like. Defined in the tracking plan,
`tracking/tracking-plan.xlsx`; how to work with it: `docs/tracking.md`.

- **Every new mutating operation, job, schedule and event handler gets a
  Coverage row**: the events it tracks, or an exemption with its reason.
  *(CI: `cargo run -p campbooks-tracking-plan -- check` fails on a row
  naming an unknown operation, on rows with both or neither; the server's
  coverage test fails on an unknown job, schedule or handler and lists what
  has no row, which campbooks-cloud#332 makes a failure.)*
- **Events are added through the plan**, in the workbook or with a YAML
  patch (`cargo run -p campbooks-tracking-plan -- apply <patch>`), then
  generated; never write a `TrackedEvent` or edit `tracking.gen.rs` or
  `tracking/tracking-plan.json` by hand. A `live` event is tracked outside
  tests. *(CI: the check fails on stale generated files, a hand-written
  `TrackedEvent`, and a live event nothing tracks.)*
- **No personal data.** Properties have a type from a closed list (no free
  text; a `code` declares its pattern) and a classification; `personal` and
  `content` are rejected. Names, addresses, subjects and bodies never appear,
  in the body or the envelope. *(CI: the plan check; `cargo xtask arch`
  audits `kernel.jobs.origin`, `JOB_ORIGIN_FIELDS`.)*
- **Snapshots come from the module's own rows in the same transaction**:
  `state.tracker.track(&mut tx, &event)` (a job's `ctx.track`) in the
  transaction of the change, so a rollback leaves no event. The kernel adds
  the envelope (actor, on behalf of, operation, correlation); callers never
  set it. *(Review; the kernel refuses an event of another workspace.)*

## Internationalization (enforced)

Campbooks ships in **English, European Portuguese, Spanish from Spain and
French** — locale tags `en`, `pt-PT`, `es-ES`, `fr`. Always the full tag:
a bare `pt` means Brazilian Portuguese to `Intl`/CLDR (dates, numbers,
plurals). Every string a user can see exists in all four before it ships.

- **Our UI:** every `i18n/` namespace has `en`, `pt-PT`, `es-ES` and `fr` files with
  exactly the same keys and no empty values. *(CI: `pnpm i18n:check`.)*
- **Our server:** messages a user reads (problem+json `title`/`detail`,
  emails, notifications) are rendered in the recipient's locale from the
  crate's catalogue (`crates/<crate>/i18n/<locale>.json`, the client's
  format, declared with `kernel::catalogue!`), never from string literals.
  Errors name a `Reason` (`MESSAGES.reason("code")`, a stable code plus the
  message `errors.<code>`); handlers that render text take `Locale`; jobs
  carry the recipient's `Locale` in their payload. Clients match on the
  problem's `code`, never on `title` or `detail`. *(CI: `cargo xtask
  i18n` — same keys, plural forms and placeholders in all four, no empty
  values; each catalogue's unit test runs the same check.)*
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


- **Open-weight models by default.** The shipped catalog and defaults are
  open-weight; an instance may point `decide` at a hosted decision model (Jev)
  by configuration, with content sent only after the admin opts in (ADR 0018).
  Every AI call goes through `campbooks-ai` by **role** (`decide`, `embed`,
  `complete`, optional `vision`), never by vendor. Providers speak the
  OpenAI-compatible HTTP API (or Laya's decision protocol for `decide`), so a
  self-hosted vLLM / llama.cpp / Ollama / TEI and a hosted provider are
  interchangeable. Users and instance admins configure every role. See ADR 0005.
- **No content rules.** Deciding what an email means is a model's job (ADR
  0018). No deterministic code reads a message's subject, body or attachment
  names to decide anything: no phrase lists, keyword lists or regular
  expressions over content, in any language, not even as a fallback.
  Deterministic code reads structured facts only (headers, addresses,
  authentication results, media types) and may only say "no"; what it cannot
  settle that way is the model's, or stays undecided. Text hygiene (HTML to
  text, dropping quoted history) is not a decision, and neither is a rule a
  member taught matching the words or subject pattern the member chose
  (campbooks-cloud#125): that is the member's decision, applied as given. *(CI: the facts module's
  source test, `campbooks_ai::decide::signals::facts`.)*
- AI output that changes user data is a **proposal** the user can undo, never a
  silent mutation.

## UI (enforced)

The design system is the package `@notacamp/ui` (`clients/ui`: tokens,
themes, generic components, the keyboard shortcut engine), shared with the
other Not A Camp apps, plus, in the app, `clients/app/src/lib/ui` (its door,
and the components that name a Campbooks concept) and `lib/shortcuts`
(Campbooks' shortcut catalogue). How it works, how to add a token or theme a
workspace: `docs/design-system.md`.

- **The package knows no app.** Nothing in `clients/ui` imports Campbooks
  code; a component that names a Campbooks concept stays in the app.
  *(CI: the package's ESLint `no-restricted-imports`; dependency-cruiser
  `design-system-knows-no-app`.)*
- **Tokens only.** Colour, size, duration and easing values live in
  `clients/ui/src/tokens/`. Component CSS (one CSS Module per component) reads
  tokens with `var()`. *(CI: stylelint in `pnpm lint` rejects hex/named
  colours, colour functions, `light-dark()`, `cubic-bezier()`, units other
  than `%`/`fr`/`deg`, and literal z-index, font and opacity values outside
  the tokens.)*
- **Tailwind is mapped to semantic tokens only**; its default palette and
  scales are wiped. *(CI: ESLint rejects arbitrary values like `p-[13px]`,
  palette classes like `bg-blue-500` and static `duration-*`/`z-*` classes in
  `className` and `cx()`.)*
- **Modules compose `lib/ui`; they don't style.** In `src/modules/**`,
  Tailwind is for layout only (flex, grid, gap, padding, width) and there are
  no `style` props or stylesheets. A pattern that needs visual styling becomes
  a `lib/ui` component first. *(CI: ESLint layout-only and no-style rules;
  dependency-cruiser `modules-have-no-stylesheets`.)*
- **One door.** Modules and the app shell import UI only from
  `~/lib/ui` (its `index.ts`), never `@notacamp/ui` directly. *(CI:
  dependency-cruiser `ui-public-surface`, `design-system-through-lib-ui`.)*
- **Every component has a story and a test**, with every variant and state.
  Stories run as tests with axe, in light and in dark; a violation fails.
  *(CI: `pnpm test-storybook`.)*
- **Accessible by construction:** native buttons and links, state in ARIA
  (`aria-current`, `aria-pressed`, `aria-expanded`), focus ring from tokens,
  dialogs that trap focus and close on Escape, WCAG AA contrast in both
  themes, motion off under `prefers-reduced-motion`.
- **Keyboard first, and visible.** Every action that has a shortcut shows
  it, from the registry (`shortcut` prop / `KbdHint`), never hand-typed keys.
  New shortcuts are defined in the registry with a namespaced label; the
  conflict check must pass. *(CI: `registry.test.ts`.)*
- **Light and dark are equal; no blue default.** The default accent is ink
  (no hue); themes change only tokens.
- **UI copy:** sentence case, plain words, no em dashes, in all four locales.

## Code quality (all binary pass/fail in CI)

- Rust: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`
  (pedantic set in `Cargo.toml` `[workspace.lints]`), `cargo nextest run`,
  `cargo deny check` (advisories, licenses, bans, sources), `cargo machete`.
- TypeScript: `pnpm lint` (ESLint flat config, no `eslint-disable`),
  `pnpm typecheck` (strict + `noUncheckedIndexedAccess`), `pnpm test`,
  `pnpm deps:check`, `pnpm deadcode` (knip), `pnpm audit --prod`.
- Repo: gitleaks, Trivy on the built image, Conventional Commit PR title,
  a changelog entry file on feat and fix PRs (by branch or title),
  `CHANGELOG.md` untouched and entries kept outside a release (only a release
  deletes them).

## Workflow

- Branch off `next` (the rewrite's trunk until cutover): `feat/<issue>-slug`,
  `fix/…`, `docs/…`, `refactor/…`, `chore/…`. PR title is a Conventional
  Commit; it becomes the squash commit. Commits must be signed.
- Tasks are tracked as GitHub issues in the private cloud repo; reference the
  issue URL in the PR body and in the change's `.openspec.yaml`.
- One task = one worktree with its own branch and test database; never
  migrate the shared dev database from a worktree; keep up with `next` by
  merging, never rebasing a pushed branch. See `docs/parallel-work.md`.
- Scaffold modules and slices with `cargo xtask new-module` /
  `cargo xtask new-feature` rather than copying by hand. *(CI:
  `cargo xtask scaffold-check` keeps their output passing the gates.)*
- User-visible changes get a changelog entry *file*:
  `.github/scripts/changelog.sh new <kind> <issue>-<slug>`, then write it
  (`changelog/README.md`). Never edit `CHANGELOG.md`: only a release writes it,
  and the Policy check fails any other PR that does. *(CI: Policy → changelog
  checks.)*
- UI work: get a visual prototype approved before implementing it, then verify
  at **375px and desktop** before calling it done.
- **UI PRs carry a screen recording** (375px + desktop) of the change working,
  in a `## Recording` section of the PR body — `pnpm --dir clients record <scenario>`,
  then upload (maintainers: `upload-media`; contributors: drag into the PR).
  Demo data only. *(CI: Policy → recording check; `no-recording` label for
  non-visual changes, maintainers only.)*
- Never delete user data without explicit confirmation; never send email to a
  real person from a dev or test environment.
