## Context

The UI is designed and approved in Decision 4 of the archived `search` change
(`openspec/changes/archive/2026-10-13-search/design.md`, "Decision 4: API and
UI"). This change builds it; this file records only what building it decided.

It does not copy a pattern from the old Rails app on `main`: that app had no
`/` overlay, and its results were server-rendered pages.

## Decisions

### D1. Where the pieces live

- `lib/ui` gets the business-agnostic parts: `Highlight` (segments to text and
  `<mark>`), `SearchField` (field, chips, clear, shortcut hint), `ConversationRow`
  taking nodes for subject and snippet (so a highlighted row is the inbox row,
  not a fork) and an `option` mode for listboxes, a public `Overlay`, and the
  ⌘K `onSearch` row.
- A new client module `search` (mirrors `crates/modules/search`) owns the
  query hooks, the overlay, the results screen, the route and the recent
  searches. Modules have no stylesheets, so its layout is Tailwind over the
  `lib/ui` parts.
- `conversations` exports its reader, `useInboxActions` and the reply
  composer through `index.ts`; `search` imports them only from there.

### D2. The overlay is mounted by the app shell

`/` must work on every workspace page, so the root layout mounts
`SearchOverlay` beside the ⌘K menu and the `?` sheet and owns its open state
(the same way). On a narrow viewport (below `md`) `/` navigates to the results
page instead: a phone has no use for a floating panel over a keyboard.

### D3. Actions on hits reuse the inbox's, through summaries

A hit carries what a row needs, but the inbox actions take a
`ConversationSummary`. The results screen reads the page's summaries with
`conversations.list?ids=` (the same batch read Home uses) and uses them for
the actions and for the live flags (unread, starred) on each row, so Done, S
and U show at once and Undo (Z) works unchanged.

### D4. Chips come from `query.filters`; removing one edits the text

The API returns the parsed filters, not their source positions. Removing a
chip removes the matching token from the query text (case-insensitively,
quotes and a leading `-` allowed) and searches again; the server stays the
only parser. Quick toggles for "Has attachment" and "Unread" append or remove
`has:attachment` and `is:unread`; the other chips (From, Stream, Date) are
typed, with the tips showing how.

### D5. Paging treats the cursor as opaque

`next_cursor` is passed back verbatim as `cursor` with the same query and
order, and never parsed or built. A `cursor_mismatch` answer shows "These
results are out of date" with a Search again button that starts over from the
first page.

### D6. The device remembers queries, not mail

Recent searches (the last 8 query strings) are stored through
`lib/storage/prefs` (try/catch inside), cleared by a button. A query is saved
when it is opened as all results or when a result is opened from it. Only text
the member typed is stored, never a result.

Queries are private, so the key is scoped: `searches.<account id>.<workspace
id>` (a template-literal pref key, the account id is the session's `sub`).
Until the session says who is signed in nothing is read or written, so the
next person on a shared browser never sees the previous one's queries. Every
`searches.*` entry is removed (`forgetPrefs("searches.")`, the only code that
touches Web Storage stays in `prefs.ts`) on sign-out (`signOut`,
`signOutAndLeave`) and whenever the session query finds no one signed in
(an ended session, a 401 leading to sign-in). `prefs.ts`'s rule against user
data now has this one documented exception.

### D7. Instant results never flicker

Requests are debounced by 120 ms, ask for `sort=relevance&prefix=true` and a
small page, and keep the previous data (`keepPreviousData`) while the next
loads; a superseded request is aborted by its signal. The results page asks
for `sort=newest` unless the member toggles "Most relevant", and never sets
`prefix`.

### D8. Deviations from Decision 4

- **J and K in the overlay.** Focus stays in the field (the combobox pattern),
  where J and K are text. The overlay moves with ↑ ↓ and Ctrl+J / Ctrl+K; plain
  J and K move on the results page, where the list has focus.
- ↵ on an instant row opens the conversation in the **inbox** reader on its
  own stream (`/w/:w/inbox/:stream/:id`), as designed. A hit whose stream is
  unknown opens in Primary.
- The results page's field filters (From, Stream, Date) are chips parsed from
  what is typed, not pickers: pickers need the suggestions endpoint
  (`search-suggestions`, #110). Has attachment and Unread are one-click toggles.

## Alternatives considered

1. **Fork the inbox row for search.** Simple to style, but two rows drift (the
   unread dot, star, count, need chips). Rejected: `ConversationRow` accepts
   nodes and gains one `option` flag instead.
2. **Make the instant panel a section of the ⌘K menu.** Already rejected in
   Decision 4(a); kept as is.
3. **Fetch each hit's summary on demand** (`conversations.get`) instead of one
   batch read per page. One request per row, and nothing for rows never acted
   on. Rejected: the batch read is one request per page and gives live flags.
4. **Parse the query on the client to remove chips.** A second parser that
   would disagree with the server. Rejected: the token edit in D4 is local and
   the server decides what the result is.
5. **Keep recent searches on the server.** Syncs between devices but adds a
   table, a scope and retention rules for text that is private. Rejected for
   now: device-only is the approved design.

## Data ownership, events, scopes

None added. The client reads `search.query` (`search:read`) and
`conversations.list` (`conversations:read`), and writes only through the
existing conversation operations.

## Risks / Trade-offs

- Removing a chip by text edit can miss an exotic spelling; the chip then stays
  and the member edits the field. Covered by tests for quotes, case and `-`.
- A hit's summary may be missing for a conversation the batch read leaves out
  (access changed meanwhile); its actions are disabled and the row still opens.
