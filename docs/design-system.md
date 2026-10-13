# Design system

The client's look, feel and keyboard behaviour live in two business-agnostic
libraries under `clients/app/src/lib/`:

- `lib/ui`: tokens, themes, components (public surface: `lib/ui/index.ts`).
- `lib/shortcuts`: the shortcut registry, the key listener and hooks.

Modules build screens by composing `lib/ui` components and use Tailwind for
layout only. The rules and how CI enforces them are in AGENTS.md → UI.

## Tokens: three layers

`lib/ui/tokens/tokens.css` is the only file with colour, size and duration
values.

| Layer         | What                                                            | Examples                                                                                                           | Who reads it                   |
| ------------- | --------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------ | ------------------------------ |
| 1. Primitives | Raw hues, chroma and scales                                     | `--accent-h`, `--neutral-c`, `--signal-pay-h`, `--space-3`, `--radius-md`, `--duration-fast`                       | Only the other two layers      |
| 2. Semantic   | Roles, each defined once for light and dark with `light-dark()` | `--surface-raised`, `--text-secondary`, `--border-subtle`, `--accent`, `--on-accent`, `--signal-reply`, `--danger` | Components, Tailwind utilities |
| 3. Component  | Per-component knobs                                             | `--btn-height`, `--row-pad-y`, `--kbd-bg`, `--sidebar-width`                                                       | Components                     |

A theme is three attributes, on `<html>` or any element:

- `data-theme`: `system` (default), `light` or `dark`. It picks the half of
  every `light-dark()`.
- `data-accent`: `business` (default, ink: no hue at all), `personal`
  (terracotta) or `forest`. A preset sets the accent hue, its lightness in
  each theme and the faint tint of every grey.
- `data-density`: `comfortable` (default) or `compact`.

Semantic tokens are declared on `:root, [data-theme], [data-accent]`, so they
are recomputed wherever a theme or accent attribute appears (the Themes
story nests six themes on one page). Size knobs are declared once on `:root`
so `data-density` works on any element.

`tokens.ts` lists the semantic and component token names for typed use
(`cssVar("surface-raised")`); `tokens.test.ts` fails if a name there is not
declared in the CSS.

### Add a token

1. Add the value to the right layer in `tokens.css`. A new colour role goes
   in layer 2 with a light and a dark value; never put a literal in a
   component.
2. If modules need it as a utility, map it in `tokens/tailwind-theme.css`
   (for example `--background-color-<name>: var(--<name>)`).
3. If it is semantic or component level, add its name to `tokens.ts`.
4. Check contrast in both themes: `pnpm test-storybook` runs axe in light and
   in dark.

### Theme a workspace

Add a preset next to `[data-accent="personal"]` in `tokens.css` that sets
`--accent-h`, `--accent-c`, the four `--accent-*-l-*` lightnesses, the two
`--on-accent-l-*` values and the neutral tint (`--neutral-h`,
`--neutral-c`). Add its name to `ACCENTS` in `tokens.ts`, to the
`allowed.accent` list in `public/theme-boot.js`, and a tile to the
Patterns/Themes story. Keep `--accent` against `--on-accent` at 6:1 or more.

At runtime, `ThemeProvider` (from `~/lib/ui`) owns the three attributes and
remembers them on the device; the accent is remembered per workspace
(`workspaceId` prop). `useTheme()` returns the current values and
`setTheme`, `setAccent`, `setDensity`. `public/theme-boot.js` applies the
saved values before first paint; it is a file rather than an inline script
because the web CSP allows same-origin scripts only.

## Tailwind

`tokens/tailwind-theme.css` wipes Tailwind's default theme (`--*: initial`)
and maps utilities onto the semantic tokens with `@theme inline reference`:

- colours: `bg-surface-{base,raised,sunken,overlay,hover,selected}`,
  `bg-accent`, `bg-accent-soft`, `text-{primary,secondary,tertiary,accent,danger}`,
  `border-{subtle,strong}`
- space (`p-*`, `gap-*`, `w-*`...): `0`, `0-5`, `1`, `1-5`, `2`, `3`, `4`,
  `5`, `6`, `8`, `10`, `12`, `hit`
- type: `text-{xs..3xl}`, `font-{sans,display}`, `font-{regular,medium,semibold,bold}`
- radius, `max-w-{measure,content,list,sidebar,aside,overlay}`,
  breakpoints `sm` 40rem, `md` 48rem, `lg` 64rem, `xl` 80rem

`bg-blue-500`, `p-7` or `rounded-2xl` generate nothing; arbitrary values
like `p-[13px]` are rejected by ESLint.

## Components

One folder per component family in `lib/ui/components/<Name>/`: the
component, a CSS Module that reads only tokens (variants are data
attributes, e.g. `data-variant="primary"`), a story and a test. Fonts are
Funnel Sans and Funnel Display, self-hosted from `@fontsource-variable/*`
(SIL OFL 1.1).

| Component                                                                                                                             | Notes                                                                                                                                              |
| ------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| Button, ButtonLink                                                                                                                    | primary, secondary, ghost, danger, link; `md` follows density, `lg` is the 44px touch size; `shortcut` shows the keys and sets `aria-keyshortcuts` |
| IconButton                                                                                                                            | required `label`; tooltip includes the keys                                                                                                        |
| Kbd, KbdHint                                                                                                                          | keycaps read from the registry (`shortcut`) or one-off `keys`; hidden on touch-only devices                                                        |
| Avatar                                                                                                                                | 8 low-chroma identity hues from a hash of the name; services are squircles                                                                         |
| Pill, Tag, Dot                                                                                                                        | signals (review, reply, decide, do, pay, sort) carry meaning only                                                                                  |
| Visor                                                                                                                                 | the assistant's face: idle, reading, thinking, unsure, happy, alert, asleep, plus the emotions dizzy, starry and hearts; sm to xl                  |
| Note                                                                                                                                  | "where things stand": linked records, confidence, a Why? disclosure, the low-confidence choice                                                     |
| Banner, EmptyState, Skeleton, Toast                                                                                                   | states; `UndoProvider` + `useUndoToast()` is the undo stack (Z)                                                                                    |
| Item, ConversationRow, NavItem, Segmented, Chip, TabBar, ActionBar                                                                    | rows and navigation                                                                                                                                |
| Shell, Sidebar, ListPane, ReaderPane, PaneHeader, HomeLayout, PageHero, QueueGroup, AsideSection, Line, Figure, MessageCard, Composer | layout and screen pieces                                                                                                                           |
| CommandMenu (⌘K), ShortcutSheet (?)                                                                                                   | modal dialogs that read the shortcut registry                                                                                                      |
| TextField, Select, ChoiceCards                                                                                                        | labelled form controls with help and errors; ChoiceCards are native radios with a sentence each                                                    |
| FocusPage, Brand                                                                                                                      | a centred page that asks one thing (signing in, each first-run step); can preview an accent with `accent`                                          |
| Popover, PopoverSection                                                                                                               | a small non-modal panel on its trigger (the account menu); Escape and outside clicks close it                                                      |
| WorkspaceMenu, WorkspaceMark, AccountButton, PhoneHeader                                                                              | the workspace switcher (⌘⇧W) with each workspace in its own accent, who is signed in, the phone's top bar                                          |
| List, ListRow, InlineConfirm                                                                                                          | rows of people or invitations; risky actions confirm in place, not in a modal                                                                      |
| Fact, FactList                                                                                                                        | a description list of label and value (the admin System panel), with an optional status dot: success, danger or off                                |
| Heading, Text, VisuallyHidden, Icon                                                                                                   | typography and icons for modules                                                                                                                   |

## Keyboard shortcuts

`lib/shortcuts` has one registry. A definition is

```ts
{ id: "nav.inbox", keys: "g i", scope: "global", group: "navigation", label: "shortcuts:nav.inbox" }
```

- **Keys**: `"e"`, `"?"`, `"mod+k"` (⌘K on Apple platforms, Ctrl+K
  elsewhere), `"mod+shift+w"`, named keys (`"enter"`, `"escape"`), and
  sequences separated by a space (`"g i"`: G, then I within 1.5 s).
- **Scope**: `global`, `list`, `item` or `compose`. Two shortcuts conflict
  when they share keys, or one starts the other's sequence, in scopes that
  can be live together (`global` overlaps all; `list` and `item` overlap).
  `define()` throws on a conflict and a test keeps the defaults clean.
- **Label**: a namespaced i18n key, so a module can register shortcuts with
  its own strings.
- **Typing**: keys typed in inputs, textareas and contenteditable are text,
  unless the definition sets `allowInInputs` (Escape, ⌘K, ⌘↵).

Screens bind behaviour with `useShortcut(id, handler, { enabled })`; the most
recent binding wins and unbinds on unmount. `KbdHint`, `aria-keyshortcuts`,
the `?` sheet and the ⌘K menu format keys from the registry, so a hint cannot
drift from its binding. The menu lists every shortcut that currently has a
handler, plus the screen's own commands.

The defaults are in `lib/shortcuts/catalog.ts`: G then H/I/S/T/D/M/, to go
places, J/K/↵ in lists, R/E/L/H/P/1/2 on the selected item, ⌘J/⌘↵ while
writing, ⌘K, Z (undo), ⌘⇧W (switch workspace), ? and Esc.

**Ids (ADR 0013).** The registry is entirely client-side: the server never
declares keys or labels. A shortcut's id is either

- the **OpenAPI `operationId`** of the server operation it runs,
  `<module>.<verb>[_<noun>]` (e.g. `conversations.archive` → `e`), so one id
  names the operation in the API, the command menu and the hint; or
- a **client-only command id** for things that are not server operations
  (`nav.inbox` → `g i`, `item.done`, `undo`). Its prefix must not be a module
  name, so the two kinds never collide (`registry.test.ts` checks the
  defaults against the modules in `docs/domain-map.md`).

A module's screen defines its operation shortcuts with
`registry.define([...])` (with labels in its own i18n namespace) and binds
them with `useShortcut`. Nothing waits for, or reads, a server-side
declarations file.

## Storybook

```sh
pnpm --dir clients storybook          # http://localhost:6006
pnpm --dir clients build-storybook    # static build in app/storybook-static
pnpm --dir clients test-storybook     # every story as a test, with axe, light then dark
```

The toolbar switches theme, accent, density and locale. Stories live next
to their component (`Components/*`); the approved screens are in
`clients/app/stories/patterns/` (`Patterns/Home`, `Inbox`, `Phone home`,
`Phone conversation`, `States`, `Themes`) with fictional demo data. Every
story runs in headless Chromium in CI; an axe violation fails it
(`parameters.a11y.test = "error"`).

Record the patterns for a PR with
`pnpm --dir clients record design-system --base-url http://localhost:6006`
while Storybook runs.
