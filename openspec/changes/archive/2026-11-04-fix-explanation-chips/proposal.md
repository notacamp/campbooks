## Why

On desktop, a point in Scout's explanation that cites two or more messages
became unreadable: its text stacked one letter per line and the chips spilled
into the next column. Each point was a single flex row that never wraps, with
the text first and then every chip. The chips couldn't shrink
(`flex: none; white-space: nowrap`), but the text could
(`flex: 1 1 0; min-width: 0; overflow-wrap: anywhere`). In a half-width
column, two chips are wider than the column when one of them names a sender
only by a long address, so the text was squeezed to almost nothing. Your own
messages made this worse: their chip showed the inbox's full address, while
the reader calls them "You".

Tracking: https://github.com/notacamp/campbooks-cloud/issues/422 (follow-up
to #407).

## What Changes

- A point's chips sit on their own row under its text and wrap there. The
  text keeps the column's full width and breaks only between words. A chip
  longer than the column shortens with "…" and shows its full label on hover.
- The card puts its groups in two columns only when it is at least 40rem
  wide (a container query on the card, not the window). In the reader beside
  the inbox list the card is about 35rem wide, so the columns were about
  15rem each; there it now uses one column.
- A chip for a message the inbox sent says "You", as the reader does. A
  sender known only by an address is named by the part before the @. The
  "newer message" line uses the same short name.
- The `Explanation/ManySources` story shows points citing several messages,
  one of them from a long address, and a reader test covers both labels and
  the chips row.

Client only; no server, API or flag change.
