# Tasks

## 1. The package

- [x] 1.1 `clients/ui` (`@notacamp/ui`, version 0.1.0) in the pnpm workspace; tokens, theme, styles, internal, safe-html, the `ui` strings and the generic components moved with `git mv`
- [x] 1.2 The shortcut engine as `@notacamp/ui/shortcuts`: any string as an id, `UI_SHORTCUTS` and their labels; Campbooks' catalogue and labels stay in `lib/shortcuts`, which includes `UI_SHORTCUTS` unchanged (registry test)
- [x] 1.3 `ThemeProvider` prefs adapter (`localStoragePrefs` by default); the app passes `lib/storage`; the boot script test moves to the app
- [x] 1.4 The package's message bundles, `Locale` and `Messages`; the app's components' strings in the app's part of `ui`, merged
- [x] 1.5 ESLint, stylelint and TypeScript configs for the package; no import from an app

## 2. Campbooks on the package

- [x] 2.1 `~/lib/ui` and `~/lib/shortcuts` re-export the package with the app's components, prefs and strings; kept components import the package
- [x] 2.2 `tsconfig.app.json` paths, Vite aliases, Vitest (tests and coverage), Storybook, the fit check, Tailwind sources, i18n check, dependency-cruiser, knip and the Dockerfile point at `clients/ui`

## 3. Publishing

- [x] 3.1 Library build (ES modules, `ui.css`, declarations, stylesheets) and README (install, CSS, prefs, strings, shortcuts, a consumer's own token set, `minimumReleaseAgeExclude`)
- [x] 3.2 CI runs the published build on every pull request; `publish-ui.yml` publishes a `ui-v*` tag on `next` to GitHub Packages

## 4. Done

- [x] 4.1 No changelog entry (a refactor, nothing a user notices); gates green; UI verified at 375px and desktop in Storybook (recording)
