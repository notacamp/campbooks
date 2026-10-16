# Proposal

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/154

## Why

The sidebar shows two searches, one under the other:

- "Search or ask" (⌘K), a menu of commands that ends with "Search
  conversations for '<query>'" and an Ask row.
- "Search conversations" (`/`), an overlay with instant conversation hits.

They overlap: the first one hands the query to the second. A person has to
know which field does what before typing, and the menu has no hits of its own.

## What Changes

- One field in the sidebar, "Search or ask", hinting both ⌘K and `/`. The
  "Search conversations" sidebar item is removed.
- ⌘K and `/` open the same menu at 48rem and wider. Below 48rem `/` and the
  top bar's search button still open the results page, unchanged.
- As the user types, the menu lists a Conversations group first (instant hits
  with the matched words marked, the understood filters as removable chips under
  the input, "All results for '<query>'" last), then the matching commands, then
  Ask. One combobox, one cursor across every group: ↑ ↓ (and Ctrl+J, Ctrl+K) move,
  ↵ runs the active row (a hit opens in the inbox reader), ⌘↵ (Ctrl↵) opens all
  results, Esc closes. The footer hints the keys.
- Search states (still indexing, what was not understood, loading, nothing
  found, errors) move into the menu, inside the Conversations group. With an
  empty query the menu lists the device's recent searches above the commands.
  The operator tips stay on the results page only.
- Outside a workspace the menu is commands and Ask only.
- The search overlay is retired. Its conversation part is a section that
  `modules/search` supplies to the generic command menu (`useSection`), so
  `lib/ui` still does not know about search.

## Non-goals

- The results page, its field, its states and the phone flow are unchanged.
- Recent searches storage (per account and workspace, cleared on sign-out) is
  unchanged.

## Impact

- Client only: `lib/ui` CommandMenu and Shell, `modules/search`, the root and
  workspace routes. No API, module, event or data change.
