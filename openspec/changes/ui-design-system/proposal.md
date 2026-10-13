# Proposal

## Why

Campbooks Next has no visual language yet: the client ships a placeholder
token block and raw Tailwind classes. Every module about to land (mail,
schedule, tasks, documents, money) needs the same controls, the same
keyboard behaviour and the same themes, and the owner approved the visual
prototypes for them in round 2 of the design review (issue #23).

Without one design system now, each module would invent its own buttons,
colours and shortcuts, and retheming (light/dark, a workspace accent) would
mean editing every screen.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/23.

## What Changes

- Design tokens in three layers (primitives, semantic via `light-dark()`,
  component) with three workspace accent presets (business = ink with no hue,
  personal = terracotta, forest), light/dark/system themes and two densities,
  under `clients/app/src/lib/ui/tokens/`.
- Tailwind 4 wired to the semantic tokens only; its default palette and scales
  are removed, so a palette class generates nothing.
- A theme runtime: provider and hook that put `data-theme`, `data-accent` and
  `data-density` on `<html>`, remembered per device (accent per workspace),
  applied before first paint by a CSP-compatible boot script.
- Components in `lib/ui/components/`, each with token-only CSS, a story and a
  test: Button, IconButton, Kbd/KbdHint, Avatar, Pill, Tag, Dot, Visor, Note,
  Banner, Toast with an undo stack, EmptyState, Skeleton, Segmented, Chip,
  Item, ConversationRow, NavItem, Shell layout primitives, TabBar,
  ActionBar, Composer, Message, the home queue pieces, Typography, Icon,
  CommandMenu (⌘K) and ShortcutSheet (?).
- A shortcut system in `lib/shortcuts/`: one registry (id, default keys
  including sequences, scope, label, handler), conflict detection, typing
  guard, platform-aware labels; keyboard hints, the sheet and the command
  menu all read it.
- Self-hosted Funnel Sans and Funnel Display (SIL OFL 1.1) from npm.
- Storybook with a theme × accent × density × locale toolbar, stories for
  every component and the approved screens (Patterns/Home, Inbox, Phone home,
  Phone conversation, States, Themes), run as tests with axe in CI, in light
  and in dark.
- Guardrails in CI: stylelint rejects colour, size, duration and easing
  literals outside the tokens; ESLint rejects Tailwind arbitrary values,
  palette classes and inline styles in modules; dependency-cruiser makes
  `lib/ui/index.ts` the only door into the design system.
- `ui` and `shortcuts` i18n namespaces in en, pt-PT, es-ES and fr.

## Capabilities

### New Capabilities

- `ui-design-system`: tokens and theming, the component library and its
  accessibility guarantees, the keyboard shortcut system, and the rules that
  keep screens on the system.

### Modified Capabilities

None.

## Impact

- Modules: none on the server. Client-side, `lib/ui` and `lib/shortcuts` are
  new business-agnostic libraries; the `system` module and the app shell
  move to them. No new module, no cross-module dependency, no
  `architecture.toml` entry.
- Dependencies (client): `@fontsource-variable/funnel-sans` and
  `@fontsource-variable/funnel-display` (shipped); `storybook`,
  `@storybook/react-vite`, `@storybook/addon-a11y`, `@storybook/addon-vitest`,
  `@vitest/browser`, `@vitest/browser-playwright` and `stylelint` (dev only).
- CI: the `clients` job installs Chromium and runs `pnpm test-storybook`.
- Web CSP: unchanged. The boot script is a same-origin file, not inline.

## Non-goals

- Real screens wired to the API: the Home and Inbox patterns are stories with
  fictional demo data; the mail, schedule and money modules build them later.
- Shortcuts for server operations: the registry accepts ids that are OpenAPI
  operationIds (ADR 0013); module screens define them when they land. The
  registry is
  shaped for it, the declarations come with those modules.
- User-editable key bindings and a workspace accent picker UI.
- Swipe gestures on touch screens (the swiped row is shown, not gestured).
- Visual regression testing (left as an advisory follow-up).
- A Tauri-specific window chrome.
