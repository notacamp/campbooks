## Why

campbooks-cloud#290. Portuguese, Spanish and French strings are often much
longer than the English source: 488 short labels are at least 1.6 times
the English ("Inbox" → "Bandeja de entrada", "Syncing" → "Synchronisation
en cours"). Many `lib/ui` components cut their text with an ellipsis, so a
translated button, navigation item or tag can show half a word, and nothing
checks it. The parity check (`pnpm i18n:check`) proves every string exists,
not that it fits.

## What changes

- A "longest" pseudo-locale for stories: every string in its longest
  version (en, pt-PT, es-ES or fr), wrapped in an invisible marker so our
  copy can be told from demo content. It is in the Storybook locale toolbar.
- A CI check, `pnpm test:fit`: every story rendered in that pseudo-locale
  at 375px and 1280px, without play functions. It fails when marked copy is
  cut off by its box or an ancestor, runs off screen, a placeholder is wider
  than its field, or the page scrolls sideways.
- Every misfit it finds is fixed: a shorter, natural translation first;
  where a label can legitimately grow, the component wraps instead of
  cutting it.

## Capabilities

### New Capabilities

### Modified Capabilities

- `ui-design-system`: translated component text must fit where it is shown.

## Impact

- `clients/app/.storybook` (pseudo-locale), `clients/app/test/fit` (the
  check), `vitest.fit.config.ts`, CI `clients` job.
- Translation files of the modules and `lib/ui` where copy did not fit;
  component CSS in `lib/ui` where a label must wrap.
- No server, API, migration or data change.

## Non-goals

- Server-rendered text (emails, problem details): no fixed layout to fit.
- Demo or user content (names, subjects): it may truncate by design.
- Play-function states (menus opened by a click) beyond what stories
  render open by themselves.
