# Campbooks design system — "Linear register"

The visual language for the `/web` React app. Approved 2026-09-20. Tokens live in
[`web/tokens.css`](./tokens.css); this file is the rules that go with them. If a
value isn't here or in a token, don't invent it in a component — add a token.

> **Layout vs. finish are separate decisions.** This document is the *finish* (the design
> system); it dresses any layout. The chosen *layout* (locked 2026-09-21) is the
> **assistant-first shell**: Today · Inbox · Books · Calendar, with Scout as a persistent
> spine and Skim as an auto-curated mode — full spec in [`specs/core-ux.md`](./specs/core-ux.md).
> The five-place model and the earlier Track issue-list are retired; the tokens are unchanged.

Interactive references (private artifacts, demo data):
- Core UX (the locked model): https://claude.ai/artifact/KSbgiWXxBqDfscYgqAtjd2
- Design system (this): https://claude.ai/artifact/8EZ5vdKS5mB7gdwoxKnCNV
- Earlier layout studies (A Lookout / D Track): https://claude.ai/artifact/7JJhAct5LtWJx3DnLZNfa1 · https://claude.ai/artifact/Xminow4PyfvkQayH5GpsVA

## North star

The interface should feel engineered and disappear into the task: quiet, fast, precise.
Dark by default. Colour is spent on **meaning**, never decoration. Craft comes from
restraint — layered neutrals, fine borders, opacity-based hierarchy — not from effects.

## The seven rules

1. **Elevation by light, not shadow.** Inline surfaces separate with a 1px hairline
   (`--cb-line`) plus a faint top highlight (`--cb-hl`) — use `.cb-raised`. Only true
   overlays (menus, dialogs, toasts) cast a real shadow — use `.cb-overlay`. Never put a
   drop shadow on a card or row.
   - **Atmospheric ground, glass surfaces (dark).** The app ground is not flat: `.bg-ground`
     layers a subtle place-neutral radial glow (indigo→violet, viewport-fixed) over `--cb-bg`,
     so the shell reads atmospheric rather than paper-flat. In dark theme the raised surfaces
     `--cb-s1/--cb-s2` are **translucent** (an alpha in the OKLCH token) so that ambient glow
     shows through as glass; overlays (`--cb-s2`) sit a touch more opaque for legibility. This
     is the only place opacity enters a surface — it does **not** loosen rule 4 (signals still
     never tint a surface). Light theme keeps solid, near-white surfaces. Applied once at the
     app root; components keep using `bg-surface-1/2` and the `.cb-raised`/`.cb-overlay` helpers
     unchanged.
2. **Text is four opacities of one near-ink**, not different colours: `--cb-t1` primary,
   `--cb-t2` body, `--cb-t3` meta/labels, `--cb-t4` disabled/shortcuts. Hierarchy = which
   opacity, never a hue.
3. **One accent for chrome.** A single cool indigo (`--cb-accent`) carries selection,
   focus rings and links, and nothing else. It maps to shadcn `--primary` and `--ring`.
4. **Five signals for meaning only.** `now / people / paper / money / time` appear **only**
   as small dots and label pills, and only say *which place* or *what state*. They never
   fill a surface, never tint a row background beyond `--cb-sel`, never become "series 4".
   Every signal ships with a word (a label), never colour alone.
5. **Small radii, tight space.** Radii `--cb-r1..4` (6/8/12/16). Space on a 4px grid,
   mostly 8. Controls are 6–8px; cards 12–16px.
6. **Type: Funnel, quiet.** `Funnel Sans` runs the UI (400/500/600/650); `Funnel Display`
   appears only on large numerals and page-level headings. Base 13–15px. Tracking tightens
   as size grows. Numerals are `font-variant-numeric: tabular-nums` wherever they align.
   No display face on buttons, labels or data. Fonts are self-hosted (SIL OFL), no CDN.
7. **Motion conveys state, briefly.** 120–160ms, `--cb-ease` (ease-out expo, no bounce).
   State change / selection / reveal only — never decorative, never a page-load sequence.
   Everything honours `prefers-reduced-motion`.

## The brand: eyes + visor

Scout is a lookout; the mark is a **black-glass visor with two white eyes** (the "oo" in
`campbooks`). Ship it as one `<Visor>` component (inline SVG/CSS, theme-independent — it's
always dark glass). States drive the eyes: `watching` (idle blink + gaze-follow),
`reading`, `thinking`, `found`, `happy` (all-clear), `asleep` (**AI paused — eyes close on
every surface**). Keep it small in a tool: 18–26px in chrome, larger only in empty states.
Respect reduced-motion (no blink/gaze).

## Colour discipline (the one test)

If a pixel is a signal hue, it must name a place or a state. If a pixel is the accent, it
must be selection, focus or a link. Everything else is a neutral surface or a text opacity.
Semantic status (`--cb-danger`, the meter steps) is separate from the accent and from the
signals. Wide-gamut OKLCH throughout; P3 screens get the full chroma.

## Components (shadcn/ui, reskinned)

Keep shadcn's behaviour and a11y; change only the finish, via `tokens.css`. Because tokens
map onto shadcn's variables, most components inherit the look for free. Conventions:

- **Buttons:** primary = accent (`--primary`); secondary = surface + hairline; ghost =
  transparent + hover; danger = danger-tint → solid on hover. Sizes 26/32/38px.
- **Feedback, not toasts:** every mutation confirms **in place or in the dock/peek** with an
  **Undo** (TanStack Query optimistic update; Undo calls the inverse mutation). Do not
  install Sonner. This enforces the "undo for everything" rule.
- **Labels vs pills:** `label` = neutral chip with a signal dot (identity); `pill` = tinted
  status (state). Both tiny (11–12px).
- **Avatars:** person = round, service/company = square, both from the signal hue.
- **Menus / dialogs / command menu / tooltips:** overlays, `.cb-overlay`, `--cb-s2`.
- **Inputs:** inset (`--cb-s3`) + hairline; focus = accent ring + 3px accent-tint glow.
- **Data:** meters use the `--cb-m-*` steps (CVD-validated) with a 2px surface gap between
  segments and a legend; never rely on colour alone.

Add ours where shadcn has no equivalent: `Visor`, `Signal` (dot/label/pill), `StatusGlyph`
(open ring / half / done-check in a signal colour), and the shell pieces in `specs/core-ux.md`.

## The shell (assistant-first)

One assistant over familiar tools. **Sidebar (≈220px) | main surface**, with **Scout as a
persistent bar** pinned to the bottom of every surface (⌘K) plus an answer popover. Full spec
and the four surfaces live in [`specs/core-ux.md`](./specs/core-ux.md); in brief:

- **Sidebar:** workspace mark, four nav items with a count badge — **Today · Inbox · Books ·
  Calendar** — Scout status ("· Mistral, EU") + the user at the foot. Collapses to a bottom
  tab bar < 680px.
- **Today** — the assistant's desk: a finite, ranked "needs you" worklist that *empties*, then
  "Coming up" deadlines, then a quiet "handled since…" line. Not an infinite feed.
- **Inbox** — one unified, familiar mailbox, AI-sorted **Important / Everything**, each thread
  annotated in plain language + a reading pane with Scout's "what this is". Documents inline.
- **Books** — cash line, the reconciliation lines that need you, reconciled this month, the loan.
- **Calendar** — events + the deadlines Scout tracks.
- **Scout spine** — the bottom bar asks/acts on every surface; Today's brief is its voice.
- **Skim = a mode, never a chore.** A one-at-a-time focus flow launched over any *auto-curated*
  set (Today's worklist; the Inbox long tail): keyboard `1–n`, a progress bar, ends "cleared".
  You never assemble a deck — Scout maintains it.

## Accessibility

WCAG 2.1 AA. Visible focus (accent ring) on every control. State never by colour alone
(signals carry a word; status carries a glyph). Full keyboard path. Motion-forward pieces
degrade to instant under `prefers-reduced-motion`. Body copy 65–75ch; data may run denser.

## Integration & guards

- `web/globals.css` (owned by the scaffold session) must `@import "./tokens.css";`.
- App defaults to **dark** (`next-themes` `defaultTheme="dark"`, class strategy → `.dark`).
- Lint guard (add to the scaffold's ESLint/stylelint): fail the build on a raw hex in
  `web/src/**`, on a hard-coded signal hue name in a component, and on default shadcn radii.
  Everything visual flows from `tokens.css`.
- Storybook stories are the visual contract for each reskinned component.

## Ownership (multi-session)

`campbooks-b4` owns this design layer (`tokens.css`, `DESIGN.md`, `specs/*`, component skins,
the shell + surface specs). The `/web` scaffold session owns plumbing (Vite/React/TS, shadcn install,
router, TanStack Query, the `/api/app` client, `globals.css`). `/api/app` (token-bearer
auth) is owned by `campbooks-e1`. Coordinate before editing across these boundaries.
