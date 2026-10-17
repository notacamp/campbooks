# Tasks

## 1. Layout

- [x] 1.1 Shell: from 64rem any `<main>` directly in the shell scrolls on its own, like the sidebar and list panes
- [x] 1.2 HomeLayout: an outer `<main>` that fills beside the sidebar and scrolls; the content keeps its maximum width

## 2. Scrollbar

- [x] 2.1 Semantic tokens `--scrollbar-thumb`, `--scrollbar-thumb-hover` (light-dark) and size tokens `--scrollbar-size`, `--scrollbar-gap`, listed in tokens.ts
- [x] 2.2 Base styles: thin scrollbars from the tokens, firmer on hover, WebKit fallback

## 3. Guard

- [x] 3.1 Home story with 40 needs whose play test checks Home scrolls inside its pane, the page is no taller than the window and the sidebar fills it, at 1100×800 and 1440×900
- [x] 3.2 Every screen story scanned at 1440×900: no shell child other than the panes

## 4. Finish

- [x] 4.1 OpenAPI and client types unchanged (no API change)
- [x] 4.2 CHANGELOG updated
- [x] 4.3 Gates green
- [x] 4.4 UI verified at 375px and desktop, light and dark, with a recording
