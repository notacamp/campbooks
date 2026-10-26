# @notacamp/ui

The Not A Camp design system: three-layer design tokens and themes, accessible React components
(each with its story and test), a keyboard shortcut engine whose hints can never drift from their
bindings, and the components' strings in English, European Portuguese, Spanish from Spain and
French. Campbooks is built on it; any Not A Camp app can be.

The rules it follows (token-only styling, accessible by construction, keyboard first) are in the
repository's `AGENTS.md` → UI and `docs/design-system.md`.

## Install

It is published to GitHub Packages. Tell your package manager where `@notacamp` packages come
from, in the app's `.npmrc`:

```ini
@notacamp:registry=https://npm.pkg.github.com
//npm.pkg.github.com/:_authToken=${NODE_AUTH_TOKEN}
```

`NODE_AUTH_TOKEN` is a token that can read the organisation's packages (in GitHub Actions, the
job's `GITHUB_TOKEN` with `packages: read`). Then:

```sh
pnpm add @notacamp/ui react react-dom i18next react-i18next
```

With pnpm's supply-chain settings, a release younger than `minimumReleaseAge` cannot be installed.
Our own packages are reviewed where they are built, so exclude them in `pnpm-workspace.yaml`:

```yaml
minimumReleaseAgeExclude:
  - "@notacamp/*"
```

## Set up

Import the stylesheets once, at the app's entry, in this order:

```ts
import "@notacamp/ui/fonts.css"; // Funnel Sans and Funnel Display, self-hosted
import "@notacamp/ui/tokens.css"; // the default token set (or yours, see Themes)
import "@notacamp/ui/base.css"; // document defaults, read only tokens
import "@notacamp/ui/ui.css"; // every component's styles
```

With Tailwind 4, map its utilities onto the tokens (its default palette and scales are wiped):

```css
@import "tailwindcss";
@import "@notacamp/ui/tailwind-theme.css";
```

Give i18next the package's namespaces, `ui` and `shortcuts`, bundled, so components never wait for
their own labels. Merge your strings into them to add or change any (the `visor` and `commandMenu`
strings name Campbooks and Scout; an app replaces them with its own):

```ts
import { UI_LOCALES, UI_NAMESPACES } from "@notacamp/ui";
import i18next from "i18next";
import { initReactI18next } from "react-i18next";

await i18next.use(initReactI18next).init({
  lng: "en",
  fallbackLng: "en",
  supportedLngs: UI_LOCALES, // "en", "pt-PT", "es-ES", "fr"
  resources: Object.fromEntries(
    UI_LOCALES.map((locale) => [
      locale,
      {
        ui: { ...UI_NAMESPACES.ui[locale], ...myUiStrings[locale] },
        shortcuts: { ...UI_NAMESPACES.shortcuts[locale], ...myShortcutLabels[locale] },
      },
    ]),
  ),
});
```

Wrap the app in the theme and the shortcut registry:

```tsx
import { ThemeProvider, UndoProvider, localStoragePrefs } from "@notacamp/ui";
import { ShortcutProvider, ShortcutRegistry, UI_SHORTCUTS } from "@notacamp/ui/shortcuts";

const shortcuts = new ShortcutRegistry([...UI_SHORTCUTS, ...myShortcuts]);
const prefs = localStoragePrefs("myapp.pref.");

<I18nextProvider i18n={i18next}>
  <ShortcutProvider registry={shortcuts}>
    <ThemeProvider workspaceId={workspace.id} prefs={prefs}>
      <UndoProvider>{app}</UndoProvider>
    </ThemeProvider>
  </ShortcutProvider>
</I18nextProvider>;
```

### Preferences

`ThemeProvider` remembers the theme, the density and each workspace's accent on the device through
a prefs adapter, `{ read(key), write(key, value) }`. The default is `localStoragePrefs()`
(`localStorage` under `ui.pref.`, blocked storage reading as empty). Pass your own to keep every
preference in one store, or to apply the saved theme before first paint from a boot script that
reads the same keys (`theme`, `density`, `workspace`, `accent.<workspace id>`). Never store
credentials or user data in Web Storage.

### Shortcuts

`@notacamp/ui/shortcuts` is the engine: `ShortcutRegistry` (definitions, conflict checks, bound
handlers), `ShortcutProvider` (one key listener), `useShortcut`, and the hooks the hints, the `?`
sheet (`ShortcutSheet`) and the ⌘K menu (`CommandMenu`) read. Your app owns its catalogue: define
your shortcuts with a namespaced label (`"shortcuts:nav.inbox"`) next to `UI_SHORTCUTS`, the few
the components use (lists, ⌘K, `?`, Escape, Z for undo). A shortcut id is any string.

## Themes

A theme is a token set: components never use raw values, only the CSS custom properties in
`tokens.css`. To give an app its own look, import your token set after `tokens.css` (or instead of
it, declaring every token) and override the properties you change:

```css
/* my-theme.css, imported after @notacamp/ui/tokens.css */
:root,
[data-theme],
[data-accent] {
  --neutral-h: 60;
  --neutral-c: 0.004;
  --radius-md: 0.25rem;
}
```

Semantic colours are defined once for light and dark with `light-dark()`; keep yours that way, and
keep `--accent` against `--on-accent` at 6:1 or more. `data-theme` (`system`, `light`, `dark`),
`data-accent` and `data-density` on `<html>` (or any element) switch between them at runtime.

## Developing

The package lives in the Campbooks repository (`clients/ui`). Campbooks uses it from source, so
there is nothing to build while working on it: its tests run in the app's Vitest
(`pnpm --dir clients test`) and its stories in the app's Storybook (`pnpm --dir clients
storybook`). `pnpm --dir clients --filter @notacamp/ui build` makes the published build in `dist/`
(CI runs it on every pull request). It imports nothing from any app.

To release: bump `version` in `package.json` in a pull request, and once it is merged, tag that
commit of `next` with `ui-v<version>`. `.github/workflows/publish-ui.yml` publishes it.
