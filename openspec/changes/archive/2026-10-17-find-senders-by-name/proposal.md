# Proposal

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/206

## Why

In Scout's questions, "Whose mail should always reach you?" lists the ten
senders replied to most. To pick anyone else, the member has to type the
person's full address or a domain into "Someone else". The owner: "it
doesn't make sense for me to have to know email addresses by heart, it
should work as a filter box." "What never needs you?" has the same field.

`conversations.list_senders` already takes `q`: words that every sender's
address, display name or domain contains, ignoring case and accents. The
questions never use it.

## What Changes

- On both sender views, the address field becomes a filter box, "Find
  someone", above the sender cards. Typing filters the cards straight
  away, and from two characters it asks `list_senders?q=` (debounced), so
  a name or part of an address finds people beyond the first ten.
- What the member ticked stays listed while they filter, so a filter never
  hides an answer.
- A full address or a domain that matches no one in their mail shows as its
  own card ("No mail from them yet", or "Everyone at <domain>"); ticking it
  adds it, as the old field did. A free-mail domain still says why it can't
  be a rule.
- With text in the box, Enter ticks the first match and clears the box,
  and Esc clears the box (with the box empty they still mean Next and Not
  now). No match says so, and says that an address or a domain can still
  be added.
- "What never needs you?" no longer lists your most-replied people, because
  they are the opposite of what it asks. Only the senders you find in the
  box show there.

## Non-goals

- No API change: `list_senders` keeps its limit of ten and its window.
- Teach Scout's "who is …?" fallback keeps its address field. It already
  offers the people matching the phrase.
- No new design-system component. The box is the existing `SearchField`,
  and the cards are the existing `CheckCards`.

## Impact

- Client only: the `assistant` module (Questions, a new SenderFilter,
  `useSenders` gains an `enabled` option, i18n en/pt-PT/es-ES/fr), its
  tests, a Storybook screen and a `find-senders` recording. `useDebounced`
  moves from the search module to `lib/hooks` so both modules share it. No
  module, event, scope or dependency changes.
