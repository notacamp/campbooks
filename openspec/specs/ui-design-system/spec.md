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

### Requirement: Translated copy fits
Every string from the UI catalogues SHALL be shown whole in every shipped
locale, at phone (375px) and desktop (1280px) widths: never cut off by an
ellipsis, a clamp or a clipping box, never off screen, and a placeholder
never wider than its field; a screen SHALL NOT scroll sideways. A
translation SHALL be shortened (staying natural) before a component is
changed; a component whose label can grow SHALL wrap it rather than cut it.
User and demo content (names, subjects, addresses) MAY truncate. Every story
SHALL pass this in the "longest" pseudo-locale, where each string is its
longest translation (CI: `pnpm test:fit`).

#### Scenario: A long Spanish label
- **WHEN** the locale is `es-ES` and the navigation shows the inbox
- **THEN** "Bandeja de entrada" is shown whole, with no ellipsis

#### Scenario: Demo content that truncates
- **WHEN** a row shows a sender name longer than its column
- **THEN** the name may end in an ellipsis and the check does not report it

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

### Requirement: Date and time pickers
`lib/ui` SHALL provide `DatePicker` and `TimePicker`, and the app SHALL use
them for every date and time it asks for, never a native date or time input.

`DatePicker` SHALL be a field the height of a text field showing the chosen
day in the member's language ("Mon, Oct 26", with the year when it is not
this year's) or a placeholder, that opens a month grid beside it; or
(`inline`) the grid alone in place. The grid SHALL show the month and year
with previous and next month buttons, the locale's weekday initials starting
on the locale's first day of the week (Monday where the platform does not
say), and six weeks of days: today ringed, the chosen day filled with the
accent, days outside the month muted, days before `min` or after `max`
disabled. It SHALL follow the ARIA grid pattern: each day a button named in
full ("Monday 26 October 2026"), the chosen day's cell `aria-selected`, one
day in the tab order. Arrows SHALL move a day or a week, Page Up and Page Down
a month (with Shift a year), Home and End to the week's start and end, Enter
or Space SHALL pick, and Escape SHALL close and give the focus back to the
field. A day SHALL be sent once, when it is picked, never while months are
browsed. Today and, when `clearable`, Clear SHALL sit under the grid.

`TimePicker` SHALL be an ARIA 1.2 combobox: a text field showing the time in
the member's language (9:30 AM in English, 09:30 in Portuguese, Spanish and
French) and a list of the day's times every `step` minutes (15 by default)
that opens on focus, a click or Down, with the field's time active and in
view. Up and Down SHALL move the active time, Enter SHALL take it, Escape
SHALL close the list and nothing else. A typed time SHALL be taken on Enter
or on leaving the field when it reads as a time ("9", "930", "9:30", "21:15",
"9.30", "21h15", "9pm", "9:30 am"); anything else SHALL go back to the last
time. With `min`, earlier times SHALL not be listed and an earlier typed time
SHALL not be taken.

Both SHALL carry a label, help and an error like the other fields, and their
own words (Previous month, Next month, Today, Clear, Choose a date, Choose a
time) in English, European Portuguese, Spanish from Spain and French.

#### Scenario: A day from the keyboard
- **WHEN** Ana opens a date field on 26 October, presses Right, Down and Enter
- **THEN** the grid moves to 27 October, then 3 November (showing November), and the field takes 3 November once, closing with the focus on it

#### Scenario: Nothing in the past
- **WHEN** a date field has `min` today and Ana opens it
- **THEN** yesterday is disabled, the previous month cannot be shown, and the arrow keys stop at today

#### Scenario: A typed time
- **WHEN** Ana types "2115" in a time field and leaves it
- **THEN** the field takes 21:15 and shows it as "9:15 PM" in English, "21:15" in Portuguese

#### Scenario: An end after its start
- **WHEN** an event starts at 10:00 and Ana opens its end time
- **THEN** the list starts at 10:00, and typing "9:00" leaves the end as it was

#### Scenario: Escape in a dialog
- **WHEN** Ana presses Escape with a time list open in the new event dialog
- **THEN** the list closes and the dialog stays open

### Requirement: The design system is a shared package
The design system SHALL live in the package `@notacamp/ui` (`clients/ui`):
the tokens and themes, the generic components with their stories and tests,
the keyboard shortcut engine (`@notacamp/ui/shortcuts`) and their strings in
every shipped locale. The package SHALL import nothing from an app; a
component that names a Campbooks concept SHALL stay in the app, built from
the package's components. The app SHALL use the package from its source
through `~/lib/ui` and `~/lib/shortcuts`, which add its own components, its
shortcut catalogue, its preferences store and its strings. Themes SHALL be
token sets: an app SHALL be able to give the components its own look by
supplying its own values for the package's tokens, without changing a
component. The package SHALL be published to GitHub Packages from a
`ui-v<version>` tag on `next` whose version is the package's.

#### Scenario: The package reaching into the app
- **WHEN** a file in `clients/ui` imports from `~/` or from the app
- **THEN** `pnpm lint` and `pnpm deps:check` fail

#### Scenario: Campbooks without a package build
- **WHEN** the app is started, tested or built with nothing built in `clients/ui`
- **THEN** it uses the package's source, and its screens, strings, shortcuts and saved theme are the same as before the package existed

#### Scenario: Another app's look
- **WHEN** an app imports its own token set after the package's `tokens.css`
- **THEN** every component draws with that app's values, and no component is changed

#### Scenario: A tag that is not the version
- **WHEN** `ui-v0.2.0` is pushed while the package's version is 0.1.0, or the tagged commit is not on `next`
- **THEN** the publish workflow fails before building and nothing is published
