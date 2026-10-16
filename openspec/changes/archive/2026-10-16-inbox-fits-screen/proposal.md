# Proposal

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/139

## Why

The inbox can be wider or taller than the screen on desktop:

- Between 64rem (1024px) and about 1180px the reader wraps under the list.
  The shell is a wrapping flex row and the reader's flex-basis is
  `--reader-min` (480px). When the sidebar and the list leave less room than
  that, the reader drops to a full-width row of its own. The inbox-ui spec
  already says the panes sit side by side from 64rem.
- A long unbroken string in the reader (a sender address, a subject or URL
  with no spaces, a long word in the assistant's note) makes the reader wider
  instead of wrapping, so the whole page scrolls sideways.

## What Changes

- From 64rem the shell's panes never wrap: the reader takes the width the
  sidebar and list leave, at least 376px at 1024px (the width it already
  handles on phones).
- Text in the reader breaks long unbroken strings, so no content can make
  the page wider than the screen.
- A guard story renders the reader with long unbroken text and checks that
  the page fits the viewport at 375px, 1100px and 1440px.

## Non-goals

- No new layout: list and reader stay as approved (fixed list, reader beside
  it, separate screens below 64rem).
- No compact sidebar rail for 64rem to 80rem. If 376px proves tight, that is
  a separate design proposal.
- Email bodies are untouched: the sandboxed frame already keeps wide
  newsletters inside it.

## Impact

- Client only: `lib/ui` Shell (layout CSS) and the inbox screen stories. No
  API, module, event or data change.
