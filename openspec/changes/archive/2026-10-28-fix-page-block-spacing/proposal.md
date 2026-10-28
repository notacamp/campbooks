## Why

The owner found the default look of blocks on a page poor (2026-10-28):
headings sit right on the line under them, and callouts and other blocks
run into each other. An audit of every block on one page found more:

- The block gap (`.tiptap > * + *`) was overridden by later `margin: 0`
  resets of the same specificity, so paragraphs, lists, code, quotes and
  callouts touched, and headings had no space below them.
- Bullet and numbered lists had no markers: the CSS reset strips
  `list-style` and the document never put it back.
- A closed toggle still showed what is inside: the editor wraps the
  content one level deeper than the hide rule looked.
- Every to-do was a 44px touch target on desktop too, so a to-do list was
  twice as loose as a list, and the checkbox sat 4px above its text.
- Callouts, cards and code blocks stacked with almost no gap.

Tracking: https://github.com/notacamp/campbooks-cloud/issues/398 (epic #363).

## What Changes

- One spacing rhythm for a page's blocks, at the top level and inside
  callouts, quotes, toggles and list items: 8px between blocks, 12px around
  boxed blocks (callouts, quotes, code, cards, images, tables), more above a
  heading than below it (32 / 24 / 20px above H1 / H2 / H3, 12px under H1
  and H2), a heading straight under a heading kept close, and nothing pushed
  off the top or bottom edge of the page or of a box.
- Bullet and numbered lists show their markers, a different one per level.
- A closed toggle shows only its first line; the arrow is centred on it.
- To-dos are as close as list items with a pointer, the checkbox on the
  first line of text; on touch screens each stays a full finger tall.
- A Storybook page with every block (`Screens/Pages/EveryBlock*Idle`) to
  check the spacing against.

Client only (`RichDocument` in `clients/app/src/lib/ui`); no server, API or
schema change.

## Impact

- `clients/app/src/lib/ui/components/RichDocument/RichDocument.module.css`
- `clients/app/test/demo/pages.ts`, `clients/app/stories/screens/Pages.stories.tsx`
