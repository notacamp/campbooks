## MODIFIED Requirements

### Requirement: The search shortcut

The key `/` SHALL open search from every page inside a workspace, registered
as the shortcut `search.open`, shown in keyboard hints (beside ⌘K on the
sidebar's "Search or ask" field), the `?` sheet and the menu. At 48rem and
wider it SHALL open the same menu as ⌘K; below 48rem it SHALL open the results
page. While the user types in a field, `/` SHALL be text. On the results page
`/` SHALL focus the page's field.

#### Scenario: Open from the inbox

- **WHEN** the user presses `/` in a stream on a desktop
- **THEN** the "Search or ask" menu opens with its field focused and Esc closes it

#### Scenario: The same menu

- **WHEN** the user presses `/` and, after closing it, ⌘K
- **THEN** the same menu opens both times

#### Scenario: Typing a slash

- **WHEN** the user types `a/b` in the reply composer
- **THEN** no menu opens

## RENAMED Requirements

- FROM: `### Requirement: The search overlay`
- TO: `### Requirement: One menu to search and ask`

## MODIFIED Requirements

### Requirement: One menu to search and ask

The sidebar SHALL show one field, "Search or ask", and no separate
conversation search. Opening it (⌘K, or `/` from 48rem) SHALL show one menu: a
field, the filters the server parsed as removable chips under it, and one
combobox listing, in this order, a Conversations group, the matching commands
and the Ask row. Conversations SHALL be the best conversations by relevance
with the sender, time, subject and snippet, the words that matched marked, and
SHALL end with the row "All results for '<query>'" hinted ⌘↵. They SHALL be
asked of the server after a 120 ms pause, a superseded request aborted, the
last word marked as a prefix while typing, and the previous rows kept on
screen until the next arrive. There SHALL be one active row across all groups:
↑ and ↓ (and Ctrl+J and Ctrl+K: plain J and K are text while the field has
focus) SHALL move it, ↵ SHALL run it (a conversation opens in the inbox reader
and the query is remembered), ⌘↵ (Ctrl↵) SHALL open all results for the query
at `/w/:workspace/search?q=`, and Esc SHALL close the menu. Focus SHALL stay in
the field (`aria-activedescendant`; rows are not focusable). A footer SHALL
hint the keys. Outside a workspace the menu SHALL hold commands and Ask only.

#### Scenario: Instant results

- **WHEN** the user types `invoice from:ana`
- **THEN** a chip "From ana" is shown, a Conversations group with "Invoice" marked comes first, the commands and Ask follow, and a request in flight is aborted by the next keystroke

#### Scenario: One cursor

- **WHEN** the user moves down past the last conversation
- **THEN** the active row is the first matching command, and past the last command it is Ask

#### Scenario: Open a row

- **WHEN** the user moves to the second conversation with ↓ and presses ↵
- **THEN** that conversation opens in the inbox reader on its stream and the menu is closed

#### Scenario: All results

- **WHEN** the user presses ⌘↵ with `invoice` typed, or chooses "All results for 'invoice'"
- **THEN** the results page opens at `/w/:workspace/search?q=invoice`

#### Scenario: Remove a chip

- **WHEN** the user removes the chip "From ana"
- **THEN** `from:ana` is removed from the field's text, the search runs again and focus stays in the field

#### Scenario: Outside a workspace

- **WHEN** the menu is open on a page that is not in a workspace and `invoice` is typed
- **THEN** there is no Conversations group and Ask is offered

## REMOVED Requirements

### Requirement: The command menu hands off

**Reason**: The menu now lists conversations itself, so there is nothing to
hand off to.

**Migration**: Type in the "Search or ask" menu; the Conversations group and
"All results for '<query>'" replace the "Search conversations for '<query>'" row.

## MODIFIED Requirements

### Requirement: Search states

With an empty query, the menu SHALL list the device's recent searches (newest
first, at most 8, each a row that fills the field, with a row that clears them
all) above the commands; the results page SHALL show them and the operator tips
(`from:`, `has:attachment`, `is:unread`, `"exact phrase"`, `-word`). When
nothing matches it SHALL say so inside the Conversations group (commands and
Ask stay listed) and suggest removing a chip when there is one. While the first
rows load, a loading state SHALL show in the Conversations group only. When the
response says `indexing` it SHALL say "Still indexing older mail". Errors SHALL
be plain: `query_too_long` asks for a shorter search, `cursor_mismatch` offers
to search again from the start, and any other failure offers Try again. Recent
searches SHALL be stored only on the device, under a key scoped to the
signed-in account and workspace, SHALL NOT be read or written while the account
is unknown, and SHALL be removed on sign-out and whenever the session is found
to be over. Storage failures SHALL be ignored.

#### Scenario: Recents

- **WHEN** the user opens all results for `lease` and later presses `/`
- **THEN** `lease` is listed under Recent searches, choosing it fills the field, and the clear row empties the list

#### Scenario: A shared browser

- **WHEN** Ana's recent searches are stored and Ben signs in on the same browser, or Ana signs out
- **THEN** Ben sees none of them, and after sign-out none remain on the device

#### Scenario: Still indexing

- **WHEN** a response has `indexing: true`
- **THEN** "Still indexing older mail" is shown at the top of the Conversations group

#### Scenario: Nothing found

- **WHEN** nothing matches `zzz`
- **THEN** the Conversations group says so and the Ask row is still listed

#### Scenario: Too long

- **WHEN** the server answers 400 `query_too_long`
- **THEN** the menu says the search is too long, in the user's language

### Requirement: Search copy

Every string of the search UI SHALL exist in English, European Portuguese,
Spanish from Spain and French, with the same keys.

#### Scenario: Portuguese

- **WHEN** the user's language is pt-PT
- **THEN** the menu's group headings, hints, states and errors are in European Portuguese
