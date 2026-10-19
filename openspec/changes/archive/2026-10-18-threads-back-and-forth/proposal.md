# Proposal

## Why

Your own mail already syncs and already sits in its threads, but the reader
does not show a conversation as one. Messages show newest first and yours
look like theirs apart from the avatar, every reply repeats the whole thread
as quoted text, a long thread is a long column of folded lines, and the
reply box sits after the oldest message, far from where you read. In the
list you cannot tell whether you wrote last.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/239
Approved visual proposal: "Threads and Sent", PR 1 (option A, lanes).

## What Changes

- Modules touched: `conversations` (server and client) and the design system
  (`lib/ui`). No new module, no cross-module dependency.
- **Lanes.** A message the inbox sent reads "You → Marta Silva" (and how many
  others) and is set in from the left on the sunken surface; theirs stay
  flush left. A `side` on `MessageCard` and `FoldedMessage`.
- **The middle of a long thread folds.** The newest two, the first, every
  unread one and any the member opened stay; each run of two or more of the
  rest becomes one "N more messages" button. Threads of four or fewer never
  fold. Order is unchanged: newest first.
- **Quoted history folds behind "Show quoted text"**, found from structure
  only. The sanitiser wraps a reply's trailing quote block (Gmail, Apple Mail,
  Thunderbird, Yahoo, Zoho, Outlook markup) in `<div data-cb-quoted="1">`; the
  client turns it into a `<details>` in the sandboxed frame. Plain text folds
  a trailing run of `>` lines.
- **The reply box is pinned to the bottom of the reader** from 48rem, the
  thread scrolling under it; phones keep the floating Reply and Done bar.
- **List rows say who wrote last**: `latest_from_you` on
  `ConversationSummary`, and "You:" before the snippet.

## Non-goals

- No Sent view (the proposal's PR 2, a separate change).
- No change to the order of messages or to which are open by default.
- No reading of words: no attribution-line detection ("On … wrote:") in any
  language.
- Outlook desktop (Word) replies, which mark their quote with no structure
  of its own, are not folded.
- A teammate's message in a shared inbox still reads "You" (see design).
