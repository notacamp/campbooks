# Proposal

## Why

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/276

The owner, using Home: "on the home list of things I want to see dates and/or
age for each item", and "the behaviour of key nav and clicking is not the
same: when I key nav the email isn't expanded as it should".

- The spec already promised how long an item has waited, but no row showed
  it: only items with a due date said anything about time.
- A click on a row selects it and opens it (Scout's summary, the email); ↑/↓
  and J/K only selected it, and closed the one that was open, so reading with
  the keys took an extra Enter on every item.

## What Changes

- Every Home row says how long it has waited, from `since` (already in
  `attention.list`): "12 minutes ago", "5 hours ago", "Yesterday", "4 days
  ago", then the date ("2 Oct", "20 Dec 2025"), in the user's language
  (`Intl.RelativeTimeFormat`), with the full date and time on hover. It sits
  at the row's end, above the due date.
- ↑/↓ and J/K open the item they move onto, as a click does; Enter or Esc
  closes it. When the open item is done (or otherwise leaves the list), the
  item taking its place opens.

## Non-goals

- No server change: `since` and `due_on` are already sent.
- The age does not tick on its own while Home stays open; it is current at
  every refetch (each change to the list).
