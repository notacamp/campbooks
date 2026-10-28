## Why

The message composer is a 96px plain-text box. Its fields are set at 13px, and
it only grows if you drag its corner. Formatting shows up as the marks it is
stored as (`**bold**`, `- `). The only links are bare `https://` addresses,
and strikethrough or a quote has to be typed by hand. The owner reported it on
2026-10-28: "lacking a lot of email features and a bit too compact to be easy
to use".

This is phase 1 of the composer rework, from the approved proposal
(https://claude.ai/artifact/BxLcKwjxF6gHbpfiWCeW8M): room to write, and
formatting you can see.

Tracking: https://github.com/notacamp/campbooks-cloud/issues/390 (epic #389).
Owner decision (2026-10-28): the editor shows the formatting itself, but the
draft keeps the same marks the server already renders, so no HTML ever comes
from the browser.

Modules touched:

- **`conversations` (server):** `domain/format.rs` learns two marks, links
  with words and strikethrough, plus backslash escapes. It renders them only
  in the member's own text. A plain-text rendering gives links as
  `words <url>`.
- **Client:** `lib/ui` `MessageComposer`, plus the conversations module's
  `DraftComposer`. TipTap is already a dependency (Pages), so nothing new
  comes in.

No new module, no cross-module dependency, no new operation or endpoint.

## What Changes

- **More room:** the composer's body starts at about ten lines, grows with
  the text, and is set at the body size (15px). The fields move up to the
  same size. The footer holds Send, Delete draft and the save state, and stays
  pinned to the bottom of the composer while the text scrolls.
- **Expand (⇧⌘E)** opens the same draft as a focus sheet over the page, about
  820px wide. Esc or Shrink puts it back. Expand is hidden below 48rem, where
  the composer is already the whole screen.
- **Formatting you can see:** a TipTap editor with a fixed schema, holding
  paragraphs, line breaks, bold, italic, strikethrough, links, bulleted and
  numbered lists, and quotes. The draft is still saved as marks, and
  reopening a draft shows its marks as formatting. Pasted content keeps only
  what the schema holds and drops everything else.
- **Toolbar:**
  - Buttons: Bold ⌘B, Italic ⌘I, Strikethrough ⇧⌘X, Link ⇧⌘K, Bullets ⇧⌘8,
    Numbers ⇧⌘7, Quote ⇧⌘9.
  - The toolbar stays one row: below 32rem of composer width, Strikethrough,
    Numbers and Quote fold into a More menu.
  - Every button's tooltip names its key.
- **Links:** Link asks for the words and the address. Pasting an address over
  selected words links them. Only `http:`, `https:` and `mailto:` are taken.
- **Server marks:**
  - `[words](url)` renders as a link, only for http/https/mailto addresses;
    anything else stays text.
  - `~~words~~` renders as strikethrough.
  - `\` escapes a mark character.
  - All three apply in the member's own text only. The quoted history keeps
    rendering bare links and nothing more.
  - The plain-text part writes a link as `words <url>` and drops escapes.

## Non-goals

- Attachments from the device (phase 2, #391), send later (#392),
  signatures (#393), and the From picker, mode switch, subject on replies and
  Scout menu (#394).
- The Reply dialog's draft box (`ReplyDraft`) and the reply box at the end of
  a chat (`Composer`) stay textareas.
- Headings, underline, text colour, images in the text, tables.
- HTML from the browser, in any form.
- Keeping a draft open while you read other mail (a minimised dock).

## Impact

- `crates/modules/conversations/src/domain/format.rs` and
  `features/release_send` (body and quote rendered separately; plain text
  through the new rendering).
- `clients/app/src/lib/ui/components/MessageComposer/*`: the editor, the
  marks ⇄ document conversion, the toolbar, Expand and the styles.
- `clients/app/src/lib/shortcuts/catalog.ts`: new `compose.*` keys.
- A shared fixture of marks cases, read by both the Rust and the TypeScript
  tests, so the two sides cannot drift apart.
- i18n in en, pt-PT, es-ES and fr.
