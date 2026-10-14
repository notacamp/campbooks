## ADDED Requirements

### Requirement: The search shortcut

The key `/` SHALL open search from every page inside a workspace, registered
as the shortcut `search.open`, shown in keyboard hints, the `?` sheet and the
⌘K menu. At 48rem and wider it SHALL open the search overlay; below 48rem it
SHALL open the results page. While the user types in a field, `/` SHALL be
text. On the results page `/` SHALL focus the page's field.

#### Scenario: Open from the inbox

- **WHEN** the user presses `/` in a stream on a desktop
- **THEN** the search overlay opens with its field focused and Esc closes it

#### Scenario: Typing a slash

- **WHEN** the user types `a/b` in the reply composer
- **THEN** no overlay opens

### Requirement: The search overlay

The overlay SHALL hold a field, the filters the server parsed as removable
chips under it, and instant rows: the best conversations by relevance with
the sender, time, subject and snippet, the words that matched marked. It SHALL
ask the server after a 120 ms pause, abort a superseded request, mark the
last word as a prefix while typing, and keep the previous rows on screen
until the next arrive. ↑ and ↓ (and Ctrl+J and Ctrl+K: plain J and K are
text while the field has focus) SHALL move the selection, ↵ SHALL open the
selected conversation in the inbox reader, ⌘↵ (Ctrl↵) SHALL open all results
for the query, and Esc SHALL close it.

#### Scenario: Instant results

- **WHEN** the user types `invoice from:ana`
- **THEN** a chip "From ana" is shown, rows with "Invoice" marked appear, and a request in flight is aborted by the next keystroke

#### Scenario: Open a row

- **WHEN** the user moves to the second row with ↓ and presses ↵
- **THEN** that conversation opens in the inbox reader on its stream and the overlay is closed

#### Scenario: All results

- **WHEN** the user presses ⌘↵ with `invoice` typed
- **THEN** the results page opens at `/w/:workspace/search?q=invoice`

#### Scenario: Remove a chip

- **WHEN** the user removes the chip "From ana"
- **THEN** `from:ana` is removed from the field's text and the search runs again

### Requirement: The command menu hands off

With a query typed, the ⌘K menu SHALL end with the row "Search conversations
for '<query>'", which closes the menu and opens the search overlay with the
query.

#### Scenario: From ⌘K

- **WHEN** the user presses ⌘K, types `lease` and chooses "Search conversations for 'lease'"
- **THEN** the search overlay opens with `lease` in its field

### Requirement: The results page

`/w/:workspace/search?q=` SHALL show the field with its chips, quick toggles
for Has attachment and Unread, a "Most relevant" toggle (off by default, so
the list is newest first), the conversation rows with the matched words
marked, and, from 64rem, the reader beside the list. A conversation SHALL open
at `/w/:workspace/search/:conversation?q=`. J and K SHALL move, ↵ SHALL open,
and E, S, U, M and Z SHALL act as in the inbox (Done archives and moves to the
next hit), each with its toast and Undo. More results SHALL load a page at a
time using the server's `next_cursor`, passed back unchanged.

#### Scenario: Newest first, then relevance

- **WHEN** the user opens `/w/:workspace/search?q=invoice` and turns on "Most relevant"
- **THEN** the first request asks for `sort=newest` and the second for `sort=relevance`

#### Scenario: Done from the results

- **WHEN** the user presses J then E on a hit and then Z
- **THEN** the conversation is archived with a toast offering Undo, and Z brings it back

#### Scenario: More results

- **WHEN** the user chooses "More results"
- **THEN** the next page is requested with the previous page's `next_cursor` unchanged

### Requirement: Phones

Below 48rem the top bar SHALL show a search button that opens the results
page full screen; chips SHALL wrap; opening a hit SHALL show the reader as its
own screen with Back returning to the results. No horizontal scrolling SHALL
appear at 375 px.

#### Scenario: Phone flow

- **WHEN** a user at 375 px taps Search, types `invoice` and taps a result
- **THEN** the reader fills the screen, and Back returns to the results with the query kept

### Requirement: Search states

With an empty query, search SHALL show the device's recent searches (newest
first, at most 8, each runnable with one click and all cleared by one button)
and the operator tips (`from:`, `has:attachment`, `is:unread`, `"exact phrase"`,
`-word`). When nothing matches it SHALL say so and suggest removing a chip when
there is one. When the response says `indexing` it SHALL say "Still indexing
older mail". Errors SHALL be plain: `query_too_long` asks for a shorter
search, `cursor_mismatch` offers to search again from the start, and any other
failure offers Try again. Storage failures SHALL be ignored.

#### Scenario: Recents

- **WHEN** the user opens all results for `lease` and later presses `/`
- **THEN** `lease` is listed under Recent searches, and "Clear" empties the list

#### Scenario: Still indexing

- **WHEN** a response has `indexing: true`
- **THEN** "Still indexing older mail" is shown above the rows

#### Scenario: Too long

- **WHEN** the server answers 400 `query_too_long`
- **THEN** the page says the search is too long, in the user's language

### Requirement: Highlighted text is never HTML

The UI SHALL render the API's `[{text, hit}]` segments as text, marking `hit`
segments with `<mark>`, and SHALL NOT insert any of it as HTML.

#### Scenario: Hostile subject

- **WHEN** a hit's title segment text is `<img src=x onerror=alert(1)>`
- **THEN** it is shown as that literal text and no element is created from it

### Requirement: Search copy

Every string of the search UI SHALL exist in English, European Portuguese,
Spanish from Spain and French, with the same keys.

#### Scenario: Portuguese

- **WHEN** the user's language is pt-PT
- **THEN** the overlay placeholder, tips, states and errors are in European Portuguese
