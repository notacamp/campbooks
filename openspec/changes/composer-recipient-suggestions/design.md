# Design

## A new read rather than a wider `list_senders`

| Option | Verdict |
|---|---|
| Add sent mail and a `direction` flag to `list_senders` | Rejected. Its counts (`received`, `replied`) and its order (replied to most) answer "who writes to me", which the assistant and Forward's picker rely on. Mixing in people the caller wrote to changes both for them, and a flag that changes the meaning of the counts is a second operation hiding in the first. |
| Search the conversation list's `participants` | Rejected. Participants are a blob per message, not a column: the search would read every blob in the window. |
| A people table kept up to date at ingest | Rejected for now. A new table, a backfill and a purge path for a picker that a bounded read answers within the same cost as `list_senders`. Worth it once teammates and address books join. |
| `conversations.list_correspondents`, both directions, bounded | **Chosen.** Its own counts (`wrote`, `received`) and order (wrote most), the same bounds and the same safety facts as `list_senders`, reading only existing indexes. |

## Data

Owned by `conversations`, read only, in schema `conversations`. No migration,
no event published or consumed. Scope `conversations:read`; access
`conversations.inbox` read (ADR 0017), and only the inboxes the caller can
read (`visible_inboxes`): a client acting for nobody reads none.

- **Received:** the newest inbound mail (`direction = 'inbound'`, not spam,
  not trash) of those inboxes in the last 365 days, at most 5,000 messages,
  through `messages_inbound_recent_idx`; the person is `sender_address` with
  `sender->>'name'`.
- **Wrote:** the newest outbound mail of those inboxes in the last 365 days,
  at most 5,000 messages, through `messages_outbound_idx`; the people are the
  elements of `recipients->'to'` and `recipients->'cc'` (there is no Bcc on
  stored mail). A message that has someone in both To and Cc counts once.
- Addresses are lower-cased and trimmed; empty ones are skipped. The
  addresses of the inboxes read are left out, so the caller is never
  suggested to themselves.
- `name` is the newest non-empty display name seen for the address, sent or
  received.
- Words of `q` are split on spaces and commas (at most five), `%`, `_` and
  `\` escaped, and each must be in the address or one of the names, through
  `unaccent(lower(...))` `LIKE ... ESCAPE '\'`, as `list_senders` does.

## Safe to write to

The facts `list_senders` uses with `reachable` move into one domain helper
(`domain::reachable`): the SQL check for a sender with mail reported, under
review or found suspicious and not said safe (`messages_inbound_sender_idx`,
run row by row in ranking order after `OFFSET 0`), and `may_write_to` for the
no-reply address and the newest mail's authentication results. Both reads
use them. In `list_correspondents` the check applies only to people the
caller has not written to (`wrote = 0`): having written to someone is the
member's own decision, and the composer must find the person they emailed
yesterday even when that person's own mail failed a check. Rows beyond
`limit` are read (40 more) so those left out rarely shorten the answer.

## The composer

`RecipientField` stays data-agnostic in `lib/ui`: it takes `suggestions`,
`onQueryChange` and `loadingSuggestions`, and without them it is the plain
chip field it was. With them it follows the ARIA 1.2 combobox pattern
(`role="combobox"`, `aria-expanded`, `aria-controls`,
`aria-activedescendant`, `aria-autocomplete="list"`, a `listbox` of
`option`s): focus stays in the input, the keys are the input's own, and
nothing is added to the global shortcut registry. Esc closes the list and is
kept from the composer only while the list is open, so Esc with the list
closed still closes the composer.

The conversations module owns the data: `useRecipientSuggestions` keeps one
query text per field, debounced 200 ms, asks `useCorrespondents` (enabled
from two characters, keeping the previous answer while the next one loads)
and leaves out everyone already in To, Cc or Bcc. `DraftComposer` passes it
to `MessageComposer`, which hands each field its suggestions, so New message,
Reply, Reply all and Forward's composer all get them.

No AI is used.
