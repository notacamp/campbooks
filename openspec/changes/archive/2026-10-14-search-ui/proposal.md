## Why

`search.query` (change `search`, merged) answers a query with conversation
rows, highlighted segments and the parsed filters, but nothing in the app calls
it. Members still scroll streams to find mail. Slice 2 of the search module
(#40) puts search in reach of one key: `/` anywhere in a workspace, ⌘K handing
its query over, and a results page where the inbox's shortcuts keep working.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/108.

Modules touched:

- **`search` (client module)**: the overlay, the results page, the route and
  the recent searches. It mirrors `crates/modules/search`.
- **`conversations` (client module)**: exports what the results page reuses
  (the reader, the inbox actions and the reply composer) and lets the reader
  say where Back goes.
- **`lib/ui`**: `Highlight`, `SearchField`, a highlight-ready `ConversationRow`,
  a public `Overlay`, and the "Search conversations for" row in the ⌘K menu.
- **App shell**: mounts the overlay, the phone's search button, the route.

No server change: the design is Decision 4 of the archived `search` change.

## What Changes

- **`/` opens search** from anywhere in a workspace (`search.open`): an
  overlay with a field, the parsed filters as removable chips, instant rows
  with highlighted subject and snippet, ↑ ↓ and J K to move, ↵ to open in the
  inbox reader, ⌘↵ for all results, Esc to close.
- **⌘K hands off**: with a query typed, a last row offers "Search
  conversations for '<query>'".
- **A results page** at `/w/:workspace/search?q=`: newest first with a "Most
  relevant" toggle, the reader beside the list on wide screens, and the
  inbox's J/K, ↵, E, S, U and Z working on it.
- **Phones**: the top bar gets a search button that opens the results page
  full screen; chips wrap; the reader is its own screen.
- **States**: recent searches on the device, operator tips, no results,
  "Still indexing older mail", and plain errors for `query_too_long`,
  `cursor_mismatch` and anything else.
- **Copy** in en, pt-PT, es-ES and fr.

## Non-goals

- Filter suggestions and `from:`/`to:` autocomplete (`search-suggestions`, #110).
- Semantic search (`search-semantic`, #109).
- Searching anything but conversations (the API returns only those today).
- Saving searches on the server, or syncing recent searches between devices.
- Any server change.

## Impact

Client only. New shortcut `search.open` (`/`) in the `?` sheet and the ⌘K menu.
New device preference key `searches` (recent searches; the only Web Storage
write, through `lib/storage/prefs`).
