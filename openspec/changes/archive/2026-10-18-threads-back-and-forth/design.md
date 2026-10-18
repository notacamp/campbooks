# Design

## Data

Everything lives in the `conversations` schema; no migration. The list
queries (`conversations.list` and the single summary every action returns)
read the newest message's `direction` beside its sender and snippet, so
`latest_from_you` is the same "newest" `latest_from` and `snippet` use. No
event changes. `conversations.list` and `conversations.get_message_body`
keep their scope, `conversations:read`. No AI is involved.

## Quoted history: found on the sanitiser's output

Detection runs after ammonia, on the sanitiser's final output
(`domain::sanitize::quotes`), not on the incoming tree:

- That output is html5ever's serialisation: every non-void element closed,
  attribute values double-quoted with `<` and `>` escaped, no comments and no
  raw-text elements. A single pass over its bytes reads the real tree
  without a DOM crate (ammonia's `rcdom` is behind an unstable cfg).
- What it reads must survive ammonia. `class` already does. `type="cite"` on
  a `blockquote` and Outlook's two ids on a `div` are allowed with exactly
  those values (`add_tag_attribute_values`), so nothing else about `type` or
  `id` gets through.
- Marking last means nothing after it can move or drop the mark, and the mark
  is never in the input to anything that filters: ammonia drops every
  `data-` attribute, so a sender cannot write `data-cb-quoted` (or hide their
  words behind it). The client reads only the attribute, never a class.

The block is the first quote-shaped element in document order, extended to a
Thunderbird `moz-cite-prefix` or Zoho `zmail_extra_hr` right before it, or for
Outlook to the end of its parent. It is folded only when visible text comes
before it (a bare forward is shown whole), nothing visible (text or an
image) after it, a Gmail, Yahoo or Zoho wrapper holds no visible text outside
its blockquotes once the first has started (an inline reply), and its parent
can hold a `div` (so the browser's parser keeps the wrapper and the reader's
`details` where they are). Anything else fails open: the message shows
whole. Being the first quote also keeps inline replies whole: a quote
followed by an answer is never the trailing one.

## Folding in the frame

The client parses the sanitised string with `DOMParser` (an inert document),
moves the marked block into `<details data-cb-quoted><summary>…`, and hands
the result to the frame. `details` toggles with no script, so the sandbox
keeps no `allow-scripts`. The summary carries "Show quoted text" and "Hide
quoted text"; a constructed style sheet of the app's own (adopted before the
message's `@media` rules) shows the right one. The frame's existing
`ResizeObserver` on its root re-measures when the block opens.

## Plain text

A pure function (`splitQuoted`) folds the trailing run of lines starting with
`>` (blank lines among them included) when words of the message's own come
first. No attribution line is looked for.

## The middle of a thread

`threadLayout(messages, shown)` in `model.ts` returns the thread newest first
as messages and "more" entries. "N more messages" adds its ids to the
reader's own state, so they show as folded lines; the reader is keyed by
conversation, so the state resets when another one opens.

## The pinned reply area

`ReaderFooter` (lib/ui `Shell`) is a sticky element after `ReaderBody`, the
last child of the reader pane: it stays at the bottom of the pane (from 64rem
the pane scrolls; below, the window does) and the thread scrolls under it. It
holds three slots: the composer, the desktop prompt ("Reply to <name>", R) and
the phone bar (Reply and Done). While the composer slot renders anything, CSS
(`:empty`) hides the prompt and the phone bar, so a draft reopened without a
press also takes the bar. The composer slot grows up to
`--reader-footer-max` (60dvh) and scrolls inside itself. On phones it sits
above the tab bar where the floating bar did. The header keeps Reply all and
Forward; Reply moved to the pinned prompt on desktop.

## Lanes

`MessageCard` and `FoldedMessage` take `side: "theirs" | "yours"` and an
`addressee` line. Yours read from component tokens: `--message-yours-indent`
(`--space-5`, `--space-10` from 48rem) and `--message-yours-bg`
(`--surface-sunken`). HTML bodies keep the card's full width.

Inboxes have one member today. When inbox sharing lands
(campbooks-cloud#84), an outbound message a teammate sent should show their
name instead of "You"; out of scope here.
