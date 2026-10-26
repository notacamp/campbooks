# Extract the design system into @notacamp/ui

Client only: `clients/app/src/lib/ui` and `lib/shortcuts` become the pnpm
workspace package `@notacamp/ui` in `clients/ui`, which Campbooks consumes.
No server module is touched. Tracking:
https://github.com/notacamp/campbooks-cloud/issues/371.

## Why

A second Not A Camp app (Applicamp) needs the same design system. Copying
it would fork the tokens, the components and the shortcut engine, and the
two would drift. One package, published to GitHub Packages and used by
Campbooks from source, keeps one design system with one set of rules.

## What Changes

- New package `clients/ui` (`@notacamp/ui`): the tokens (`tokens.css`,
  `tailwind-theme.css`, `tokens.ts`), `theme/`, the base and font
  stylesheets, `internal/`, `safe-html/`, the `ui` strings, the shortcut
  engine (`@notacamp/ui/shortcuts`) with its few shortcuts (`UI_SHORTCUTS`)
  and their labels, and every generic component with its story and test,
  moved with history. It imports nothing from the app (ESLint
  `no-restricted-imports`, dependency-cruiser `design-system-knows-no-app`).
- Components that name a Campbooks concept stay in
  `clients/app/src/lib/ui/components` and import the package's primitives:
  ConversationRow, Message, MessageComposer, Composer, ReplyDraft,
  RecipientField, Attachments, Chat, Queue, TaskRow, Offer, HowDecided,
  RulePreview, ThinkingTrace, ProviderMark, WorkspaceMenu. Their strings
  move to the app's part of the `ui` namespace. The Visor moves with the
  primitives, which draw it.
- Three seams: the shortcut engine takes any string as an id and Campbooks
  keeps its catalogue (`lib/shortcuts/catalog.ts`) and labels;
  `ThemeProvider` takes a prefs adapter (`{ read, write }`, by default
  `localStorage`) and Campbooks passes `lib/storage`; the package exports
  its message bundles, `Locale` and `Messages`, and the app merges its
  strings into the same namespaces.
- `~/lib/ui` re-exports the package and the app's components, so no import
  in the app changes. `tsconfig.app.json` paths, the Vite aliases, Vitest,
  Storybook and the fit check use the package's source: nothing is built
  for Campbooks, the package's tests count toward the app's coverage gate,
  and its stories run in the app's Storybook.
- The published build (`vite build` in library mode, declarations, the
  stylesheets) runs in CI on every pull request, and
  `.github/workflows/publish-ui.yml` publishes it to GitHub Packages from a
  `ui-v<version>` tag on `next` with the job's `GITHUB_TOKEN`. First
  version 0.1.0.

## Non-goals

- No change a Campbooks user can see: same components, same strings, same
  shortcuts, same theme and the same saved preferences.
- No Storybook in the package (it stays in Campbooks), no second theme yet
  (Applicamp ships its own token set), no change to the token values.
- No change to the app's imports outside `lib/ui` and `lib/shortcuts`.
