# Proposal

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/177

## Why

`inbox-panes-scroll` (#156) made the sidebar, list panes and readers scroll
on their own, but the owner still sees the whole page scroll on Home. Home
(HomeLayout, also the empty and first-run Home) renders its own `<main>`
straight into the shell, and that `<main>` does not scroll, so a busy Home
stretches the page and pushes the account menu below the window. A scan of
all 165 screen stories at 1440×900 found it is the only shell child that
doesn't scroll on its own.

The owner also asked for a scrollbar that fits the design system: now that
every pane scrolls, the browser's default bars show between panes.

## What Changes

- From 64rem every pane in the shell scrolls on its own: the sidebar, a list
  and any screen's `<main>` (a reader, Home), not only ReaderPane.
- HomeLayout's `<main>` takes the space beside the sidebar and scrolls, so
  its scrollbar sits at the window's edge; the content inside keeps its
  maximum width.
- Scrollbars are thin: a neutral thumb on no track, firmer while the pointer
  is over the pane, from new semantic tokens `--scrollbar-thumb` and
  `--scrollbar-thumb-hover` (light and dark) and size tokens
  `--scrollbar-size` and `--scrollbar-gap`, with a WebKit fallback for
  engines without `scrollbar-color`.
- Guard stories render Home with 40 needs at 1100×800 and 1440×900.

## Non-goals

- Below 64rem nothing changes: the page scrolls.
- No change to what Home shows.

## Impact

- Client only: `lib/ui` Shell and Queue (HomeLayout) CSS, HomeLayout markup
  (one wrapper), tokens, base styles and Home stories. No API, module, event
  or data change.
