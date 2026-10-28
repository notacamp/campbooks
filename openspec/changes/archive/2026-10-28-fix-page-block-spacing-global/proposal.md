## Why

#695 (`fix-page-block-spacing`) shipped, but the owner saw no change in prod.
`RichDocument.module.css` is a CSS module, so Vite renames every class it
names, including the editor's own global ones: `.tiptap` became
`._tiptap_<hash>`, and likewise `.react-renderer`, `.node-toggle` and
`.ProseMirror-selectednode`. The editor's DOM keeps the plain names, so the
block gap, the first and last child resets, the margins of card blocks, the
`hr` reset and the selected-block outline never matched. Only the rules naming
no class (headings, lists, to-dos, callouts) applied. That was also the
original cause of paragraphs touching, not the reset order.

Found by campbooks-6a. Tracking:
https://github.com/notacamp/campbooks-cloud/issues/398 (reopened).

## What Changes

- Wrap the editor's class names in `:global()` in `RichDocument.module.css`,
  with a comment saying why. No other `*.module.css` under `clients/` names an
  editor class.
- A story with a `play` (`Screens/Pages/EveryBlockSpacing`) measures the gaps
  in the real editor in CI: 8px between paragraphs, 32px above an H1 and 12px
  under it, 12px between callouts and between cards, a flush first block, and
  a closed toggle that hides its content. A rule that stops matching fails it.
- The spec gains the two-paragraphs scenario.

Client only; no server, API or flag change.
