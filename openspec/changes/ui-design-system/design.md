# Design

## Context

The client is one React 19 / Vite 8 / Tailwind 4 app (ADR 0003) whose web
build is served under a strict CSP (`script-src 'self'; style-src 'self'`,
see `serve-web-client`). `lib/` is business-agnostic and may not import
`modules/` or `app/`; modules import each other only through `index.ts`
(dependency-cruiser). The approved prototypes define the tokens, the
component CSS, the screens and the keyboard map.

No server code changes: there is no data ownership, no event, no endpoint and
no OAuth scope in this change. There is no AI use; the assistant's note and
visor are presentational, fed by future modules.

## Decisions

### Three token layers, resolved where they are declared

`tokens.css` keeps the prototype's layers. Semantic colours are declared on
`:root, [data-theme], [data-accent]` with `light-dark()`, so any element
carrying a theme or accent attribute recomputes them (the Themes board nests
six themes in one page). Size knobs are declared once on `:root` so that
`data-density="compact"` works on any element; only colour knobs follow the
theme. Signal and avatar hues are bound by `data-signal` / `data-avatar-hue`
attributes rather than global classes.

### Styling approach: CSS Modules reading tokens, Tailwind for layout

Components are styled with one CSS Module each, ported from the approved
`ui.css` with variants as data attributes (`data-variant="primary"`). The
prototype CSS relies on `color-mix()`, keyframes, hover-revealed hints and
token overrides between components (a primary button re-tints its keycaps by
setting `--kbd-*`); CSS Modules keep that fidelity and scope class names.
Tailwind is wired to the semantic tokens through
`@theme inline reference` with `--*: initial`, so utilities read tokens
directly and nothing else exists; modules use it for layout only.

Considered: Tailwind utilities inside components. Rejected: the hover/peer
reveals, keyframes and cross-component token overrides become long arbitrary
selectors, which is exactly what the guardrails forbid.

### Enforcement

- stylelint (`stylelint.config.js`) on `src/**/*.css` and `stories/**/*.css`:
  no hex, named colours, colour functions, `light-dark()`, `cubic-bezier()`
  or units other than `%`/`fr`/`deg` in declarations; z-index, font and
  opacity values must be `var()`. `src/lib/ui/tokens/` is exempt. Media
  queries may use the four `rem` breakpoints.
- ESLint (`eslint.config.js`): in class names and `cx()` calls, no arbitrary
  values, palette names or static duration/z-index utilities; in
  `src/modules/**`, layout utilities only; in `src/modules/**` and
  `src/app/**`, no `style` prop.
- dependency-cruiser: `ui-public-surface` (modules and app use
  `lib/ui/index.ts` only), `modules-have-no-stylesheets`,
  `shortcuts-below-ui`; proven by fixtures in the architecture test.
- Storybook stories run as Vitest browser tests with
  `parameters.a11y.test = "error"`, twice (light, dark).

### Theme runtime and no flash

`ThemeProvider` owns `{theme, accent, density}`, writes them to `<html>`
and to the prefs store (`theme`, `density`, `accent.<workspace>`,
`workspace`). The pref keys stay a closed union, extended with a template
literal for per-workspace accents. `public/theme-boot.js`, a classic script
loaded from `index.html`, applies the same attributes before first paint. It
is a file, not inline, because the CSP forbids inline scripts; a test runs it
in jsdom and checks it shares the storage prefix.

### Shortcut registry

`ShortcutRegistry` holds definitions `{id, keys, scope, group, label,
allowInInputs}` and a stack of handlers per id (the latest binding wins).
`define()` throws on duplicates, invalid bindings and conflicts: same keys,
or one binding a prefix of another's sequence, in scopes that can be live
together (`global` overlaps all; `list` and `item` overlap each other;
`compose` only itself and global). A single document keydown listener
matches chords and sequences (1.5 s window) against bound shortcuts only,
ignores text fields unless `allowInInputs`, and maps `mod` to ⌘ on Apple
platforms and Ctrl elsewhere. Labels are namespaced i18n keys
(`shortcuts:nav.inbox`) so modules can register their own; `KbdHint`,
`aria-keyshortcuts`, the sheet and the menu all format from the registry.
Ids follow ADR 0013: a server operation's shortcut is keyed by its OpenAPI
`operationId` (`conversations.archive`), a client-only command by an id whose
prefix is not a module name (`nav.inbox`). The registry stays client-side;
the server declares no keys.

The prototype's Inbox hinted 1–5 on the streams while the same screen offers
1/2 for a low-confidence choice; the conflict check rejects that pairing, so
the digits stay with the choice and streams are reached through the sidebar,
G then I and J/K.

### Accessibility by construction

Real buttons and links everywhere; `aria-current` on navigation and the open
conversation; `aria-pressed` on segments and chips; `aria-expanded` /
`aria-controls` on Item and "Why?"; overlays are `role="dialog"` with
`aria-modal`, a focus trap, Escape and focus return; toasts live in a polite
`status` region, failures are `alert`s. Keycaps are `aria-hidden` and the
control carries `aria-keyshortcuts`, except in the sheet where keys are read
out. Struck-through "undone" lines use the tertiary text colour so they keep
AA contrast. Motion stops under `prefers-reduced-motion`.

### i18n

Two bundled namespaces, `ui` and `shortcuts`, passed to `createI18n` through
a new `bundled` option so components never suspend on their labels. Both are
held to parity by `pnpm i18n:check`.

## Risks / Trade-offs

- [Storybook 10.6.1 is the first release supporting Vitest 5] → pinned
  exactly; it passes the repo's 7-day release-age policy.
- [The jsdom test run cannot see layout or contrast] → the browser story run
  covers rendering and axe; jsdom tests cover behaviour and coverage.
- [Tailwind arbitrary values still compile] → ESLint rejects them at the
  source; the generated CSS is never the check.

## Migration Plan

None for users or operators. Existing client code moves to tokens in this
change (the app shell and the `system` module).
