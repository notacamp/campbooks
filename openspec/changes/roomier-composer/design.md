## Context

A draft's `body` is plain text with marks, and the server is the only thing
that turns marks into HTML (`domain/format.rs`, escape first and then a fixed
set of tags). The client never sends HTML. This change keeps that contract
and changes how the marks are edited.

## Decisions

### 1. The editor shows formatting, but the draft stays marks

We looked at three options:

- **(a)** Keep the textarea and add a preview.
- **(b)** A rich editor that serialises to marks.
- **(c)** Store HTML from the browser.

The owner chose (b). Option (c) would put a sanitiser on every send path and
make the client an HTML author, which goes against the security-first rule.

The editor is TipTap, with its own **fixed schema**. It is not the Pages
extension set, and it does not use `StarterKit`'s defaults blindly:

- `Document`, `Paragraph`, `Text`, `HardBreak`
- `Bold`, `Italic`, `Strike`, `Link` (autolink on, `openOnClick` off,
  `isAllowedLink`-style protocol check: http/https/mailto)
- `BulletList`, `OrderedList`, `ListItem`, `Blockquote`
- `History` (undo and redo, ⌘Z and ⇧⌘Z)

Anything else pasted in, such as headings, images, tables or code, is dropped
by the schema; a heading's text becomes a paragraph.

The conversion lives in `MessageComposer/marks.ts` as two pure functions:

- `toMarks(doc)`, from the document to text.
- `fromMarks(text)`, from text to the document.

The editor holds the document. `onBodyChange` gets `toMarks(doc)`, so the save
path (`useDraftEditor`, partial updates by version) does not change. When the
draft arrives from the server, `fromMarks(body)` is set once at mount. After
that, the server's echo is compared to `toMarks` of the current document and
only replaces it when they differ, so the cursor never jumps on a save.

Lists nest only one level. Deeper nesting, which the server has no mark for,
is flattened.

### 2. Escapes

Typing a literal `*`, `~`, `[`, `]` or `\` in the editor must not turn into a
mark, so `toMarks` writes a `\` before them. At the start of a line it also
escapes a `-`, a `>` or a run of digits before `. `, written as `\-`, `\>` and
`1\.`.

`format.rs` reads `\` followed by one of those characters as the character
itself. A `\` before anything else stays as it is, so old drafts holding a
backslash still read the same.

### 3. Links with words, own text only

The new link syntax is `[words](url)`. The URL must start with `http://`,
`https://` or `mailto:`, and contain no whitespace, `<`, `>` or `"`. If it
doesn't, the whole thing renders as escaped text.

The words are rendered with the inline rules (bold and italic inside a link
are fine), and a link never nests inside another link.

The quoted history comes from someone else's mail. If it went through the same
rules, a sender's own text could become a link whose words hide where it
goes. So `release_send` now renders the two parts on their own:

```
to_html(body, Marks::Own) + to_html(quoted, Marks::Quoted)
```

wrapped in the one `<div>` as before. `Marks::Quoted` keeps today's set (bare
links, emphasis, lists, quotes) and adds no `[words](url)` and no `~~`. The
plain-text part becomes `to_plain(body) + "\n\n" + quoted`:

- `to_plain` writes `words <url>` for a link and drops the escapes.
- The other marks stay as typed, which is how plain-text mail is read anyway.

### 4. Shared fixture

`crates/modules/conversations/tests/fixtures/marks.json` holds a list of
cases:

```
{ "doc": <ProseMirror JSON>, "marks": "…", "html": "<div>…</div>", "plain": "…" }
```

- The Rust tests check `to_html(marks, Own) == html` and
  `to_plain(marks) == plain`.
- The vitest suite checks `toMarks(doc) == marks` and
  `fromMarks(marks) == doc`.

The cases cover each mark, the escapes, nesting, a link with emphasis, a
refused `javascript:` link, a list followed by a paragraph, a quote holding a
list, and an old draft with a stray backslash.

### 5. Room, Expand and the pinned footer

The body's minimum height is `15.5em` (about ten lines at 15px), and it has no
fixed maximum. The reader's reply slot already caps the whole composer at 60%
of the window and scrolls inside it (inbox-ui, "The reply slot"). The footer is
`position: sticky; bottom: 0` inside that scroller.

Expand keeps the same `MessageComposer` mounted. Inline, its parts sit in a
plain element, so nothing announces a dialog that isn't there. Expanded,
they sit in a native `<dialog>` opened with `showModal()`, which puts them in
the top layer, makes the rest of the page inert and gives the sheet its
dialog role.

The parts move between the two, but the TipTap editor is one instance for
the life of the composer, and `EditorContent` keeps the editor's view while
it moves. So the cursor and the undo history stay. The focus and resize
listeners re-attach on each move.

The app's `Dialog` would swallow the composer's keys, which is why the sheet
is a native dialog. Esc and the native `cancel` shrink the sheet, as does a
click on its backdrop.

The link form is a row under the toolbar, not a popup, so it shows over the
sheet as well. The More menu opens when a ResizeObserver finds the composer
narrower than 32rem; it is the shared `OverflowActions` component, which
container queries could not drive.

The body text uses the body token `--text-md` with `--leading-relaxed`, since
the design system has no 15px step.

### 6. Shortcuts

New `compose.*` entries in `lib/shortcuts/catalog.ts` (scope `compose`,
`allowInInputs`):

- `compose.strike` ⇧⌘X
- `compose.link` ⇧⌘K
- `compose.bullets` ⇧⌘8
- `compose.numbers` ⇧⌘7
- `compose.quote` ⇧⌘9
- `compose.expand` ⇧⌘E

⌘K stays the command palette. TipTap's own key bindings for these marks are
turned off, so each key has exactly one handler, the registry's. The
existing `compose.bold` and `compose.italic` keep ⌘B and ⌘I.

## Data, events, scopes, AI

- No data change: drafts keep `body` as text.
- No new operation, endpoint or OAuth scope.
- No AI.
- No tracked events added or changed, and no Coverage rows (ADR 0020): there
  is no new operation, job, schedule or handler.

## Risks

- **Round-trip drift between the TypeScript and Rust sides.** The shared
  fixture guards against it, and both suites fail on it.
- **A draft written before this change containing `[x](y)`** now renders as a
  link. This is accepted: before, it was literally that text in the member's
  own body.
- **Bundle size.** TipTap is already in the Pages chunk. The composer imports
  only the extensions listed above, so the conversations chunk grows by that
  core.
