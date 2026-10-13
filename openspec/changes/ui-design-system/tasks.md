# Tasks

## 1. Tokens and theming

- [x] 1.1 Port `tokens.css` (three layers, accent presets, `light-dark()`, density) to `lib/ui/tokens/`; verify with `tokens.test.ts` (listed tokens declared, colour roles use `light-dark()`, every accent has a preset)
- [x] 1.2 Map Tailwind 4 to the semantic tokens only (`@theme inline reference`, default theme wiped); verify utilities reference declared tokens
- [x] 1.3 `ThemeProvider`/`useTheme`, prefs keys, `public/theme-boot.js`; verify with unit tests (apply, persist, per-workspace accent, boot script)
- [x] 1.4 Self-host Funnel Sans and Funnel Display from npm

## 2. Shortcuts

- [x] 2.1 Key grammar, platform-aware formatting, `aria-keyshortcuts`; verify with `keys.test.ts`
- [x] 2.2 Registry with conflict detection and the default catalogue; verify the catalogue has no conflicts
- [x] 2.3 Dispatcher (sequences, typing guard, mod key) and `ShortcutProvider`/`useShortcut`; verify with unit tests

## 3. Components

- [x] 3.1 Primitives: Button, IconButton, Kbd/KbdHint, Avatar, Pill, Tag, Dot, Visor, Typography, Icon
- [x] 3.2 Feedback: Note, Banner, Toast + undo stack, EmptyState, Skeleton
- [x] 3.3 Rows and navigation: Item, ConversationRow, NavItem, Segmented, Chip, TabBar, ActionBar, Shell primitives, home queue pieces, Message, Composer
- [x] 3.4 Overlays: CommandMenu, ShortcutSheet (focus trap, Escape, focus return)
- [x] 3.5 `ui` and `shortcuts` i18n namespaces at parity in en, pt-PT, es-ES, fr

## 4. Storybook

- [x] 4.1 Storybook with theme × accent × density × locale toolbar; a story per component and variant
- [x] 4.2 Pattern stories: Home, Inbox, Phone home, Phone conversation, States, Themes
- [x] 4.3 Stories as tests with axe (`pnpm test-storybook`, light and dark) in the `clients` CI job

## 5. Guardrails and docs

- [x] 5.1 stylelint token-only rules, ESLint Tailwind and module rules, dependency-cruiser UI rules with fixtures
- [x] 5.2 Move the app shell and the `system` module to `lib/ui`
- [x] 5.3 `docs/design-system.md`, AGENTS.md "UI" section, clients README
- [x] 5.4 CHANGELOG updated; gates green; UI verified at 375px and desktop with a recording

## 6. Follow-ups (not in this change)

- [ ] 6.1 Scheduled, advisory visual regression run over the pattern stories
- [ ] 6.2 Module screens define shortcuts for their server operations, keyed by OpenAPI operationId (ADR 0013), through `ShortcutRegistry.define`
- [ ] 6.3 Workspace accent picker and user-editable bindings
