# ui-design-system Specification

## Purpose
The client's design system: design tokens and themes, the shared component
library, the keyboard shortcut system, and the rules that keep every screen
on them.

## Requirements

### Requirement: Three-layer design tokens
The client SHALL define its visual values as tokens in three layers:
primitives (raw hues and scales), semantic roles defined once for light and
dark with `light-dark()`, and component knobs built from semantic tokens.
Components SHALL read only semantic and component tokens.

#### Scenario: A colour role exists for both themes
- **WHEN** a semantic colour token such as `--surface-base` is declared
- **THEN** its value uses `light-dark()` with a light and a dark value

#### Scenario: Utilities map to tokens only
- **WHEN** a class such as `bg-surface-raised` or `p-3` is compiled
- **THEN** it reads the corresponding token, and default palette classes such as `bg-blue-500` generate no CSS

### Requirement: Runtime theming
The client SHALL apply the theme (light, dark or system), the workspace
accent (business, personal or forest; business by default, with no hue) and
the density (comfortable or compact) as attributes on the document root,
remember them on the device (the accent per workspace), and apply the saved
values before the first paint.

#### Scenario: Default theme
- **WHEN** nothing has been saved
- **THEN** the root carries `data-theme="system"`, `data-accent="business"` and `data-density="comfortable"`

#### Scenario: Switching workspace
- **WHEN** the user switches to a workspace whose saved accent is forest
- **THEN** the root's `data-accent` becomes `forest` without changing the theme or density

#### Scenario: Reload
- **WHEN** a user who chose the dark theme reloads the web app
- **THEN** the boot script sets `data-theme="dark"` before the app's code runs, from a same-origin script allowed by the CSP

### Requirement: Token-only styling is enforced
CI SHALL fail when CSS outside the token files contains a colour, size,
duration or easing literal, when a class name uses a Tailwind arbitrary
value or a palette colour, when a module uses an inline style or a
non-layout utility, or when code outside the design system imports anything
but its public index.

#### Scenario: A literal colour in a component
- **WHEN** a component stylesheet declares `color: #123456`
- **THEN** `pnpm lint` fails

#### Scenario: A module reaching into a component
- **WHEN** a module imports a file under `src/lib/ui/` other than `index.ts`
- **THEN** `pnpm deps:check` fails

### Requirement: Accessible components
Every component SHALL use native interactive elements, expose its state
(`aria-current`, `aria-pressed`, `aria-expanded`), show a focus ring from
tokens, and stop decorative motion when reduced motion is requested.
Overlays SHALL be modal dialogs that trap focus, close on Escape and return
focus. Every story SHALL pass axe in the light and the dark theme.

#### Scenario: A story with an accessibility violation
- **WHEN** a story renders text below WCAG AA contrast
- **THEN** `pnpm test-storybook` fails

#### Scenario: Closing the command menu
- **WHEN** the command menu is open and the user presses Escape
- **THEN** the menu closes and focus returns to the element that had it

### Requirement: Undoable actions
The component library SHALL provide an undo stack: every action pushed to it
is confirmed in a polite live region with an Undo button, Z undoes the most
recent action not yet undone, and the history stays available for the
session with Redo.

#### Scenario: Undo with the keyboard
- **WHEN** the user marks two items done and presses Z twice
- **THEN** both items are restored, newest first

### Requirement: One shortcut registry
Keyboard shortcuts SHALL be declared once in a registry with an id, default
keys (single keys, modifier chords with `mod` meaning ⌘ on Apple platforms
and Ctrl elsewhere, and sequences such as G then I), a scope and a
translated label. Keyboard hints, `aria-keyshortcuts`, the `?` sheet and the
⌘K menu SHALL read keys from the registry. Defining a shortcut that shares
keys, or a sequence prefix, with another in an overlapping scope SHALL fail.

#### Scenario: Conflicting definition
- **WHEN** a shortcut bound to `g` is defined next to `g i` in the global scope
- **THEN** the registry rejects it as a prefix conflict

#### Scenario: Typing in a field
- **WHEN** the user types `e` in a text field
- **THEN** the "Done" shortcut does not run, while `mod+k` still opens the command menu

#### Scenario: Hint follows the binding
- **WHEN** a button names the shortcut `command.open`
- **THEN** it shows ⌘ K on Apple platforms, Ctrl K elsewhere, and exposes the same keys in `aria-keyshortcuts`

### Requirement: Translated component text
Every string a component owns (Undo, Why?, Keyboard shortcuts, shortcut
labels and the like) SHALL come from the `ui` or `shortcuts` i18n namespace,
present in en, pt-PT, es-ES and fr with the same keys.

#### Scenario: Portuguese user
- **WHEN** the locale is `pt-PT` and the shortcut sheet is open
- **THEN** its title is "Atalhos de teclado" and "Inbox" reads "Caixa de entrada"

### Requirement: Clamp
`lib/ui` SHALL provide `Clamp`: long content held to `--clamp-height` with a
fade (`--clamp-fade-height`) and a link button that shows it all in place and
folds it back, saying which with `aria-expanded`. The button SHALL show only
when the content is taller than the clamp.

#### Scenario: A long email on Home
- **WHEN** an opened item's email is taller than the clamp
- **THEN** it ends in a fade with "Show the whole email", and pressing it shows all of it with "Show less"

### Requirement: Item summary
`ItemDetail` SHALL accept a `summary` (a model's plain sentences) and a
`summaryLabel`, and draw them as a region named by the label, marked with the
visor, after the reasons and above the email, as text only. It SHALL draw
nothing without a summary.

#### Scenario: A summary above the email
- **WHEN** an opened item has a summary and an email
- **THEN** the summary shows under its heading before the email, and without a summary only the email shows
