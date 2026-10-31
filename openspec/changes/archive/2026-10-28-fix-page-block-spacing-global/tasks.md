## 1. Fix

- [x] 1.1 `:global()` around `.tiptap`, `.react-renderer`, `.node-toggle`, `.ProseMirror-selectednode` in RichDocument.module.css
- [x] 1.2 Grep every `*.module.css` under clients for editor class names (only RichDocument)

## 2. Prove it

- [x] 2.1 Story `EveryBlockSpacing` measures the gaps in the real editor (runs in CI)
- [x] 2.2 Compiled CSS keeps `.tiptap` (checked in Storybook's served CSS and CI's build)
- [x] 2.3 Recording at 375 and desktop showing spaced paragraphs
