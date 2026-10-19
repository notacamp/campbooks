## Why

campbooks-cloud#272. Since campbooks-cloud#271, Scout's reply drafts follow
how the member wrote to the person before. That guesses from mail, and mail
does not say that Marta is the member's landlord, that she is always written
to formally and in Portuguese, or that Rui is a friend who gets "tu". The
owner asked for one control and only one: the member tells Scout about a
person in their own words, and Scout's drafts to that person follow it.

## What changes

- A private note per member per person, keyed by the person's address:
  who they are and how to write to them, 1 to 500 characters.
- Three operations in the conversations module:
  `conversations.list_person_notes`, `conversations.set_person_note` (an
  upsert by address, the address in the body) and
  `conversations.delete_person_note`. Each answers only the caller's own
  notes; anyone else gets 404.
- `conversations.write_reply` sends the caller's note about the person
  answered to the model as data (`my_note_about_them`), for tone, form of
  address, language and formality, never as a fact. A distrusted message
  (spam, suspicious, under review, failed sender checks) gets no note.
- The reply dialog gets one quiet line under the quote: "Tell Scout about
  Marta", or "Scout knows: …" with Edit, opening the note's field in
  place, and "Rewrite with this" once a note is saved while a draft shows.
- Scout's memory gets a card, "People you told Scout about", after "Your
  rules": each note with Edit and Remove (with Undo).
- The workspace's purge and a departing member's clean-up delete the notes.

## Capabilities

### New Capabilities

### Modified Capabilities

- `conversations`: person notes and their operations; what a written reply
  sends to the drafting model; what a departing member leaves behind.
- `memory-ui`: the people card.
- `home-ui`: the reply dialog's line about the person.

## Impact

- Modules: `conversations` only (`features/person_notes`, `features/reply`,
  `domain/person`, `domain/resources`, `forget_member`, `purge_workspace`).
  No new cross-module dependency. The assistant client module shows the
  conversations module's list through the same slot as the rules list.
- Migration `20261023070000_conversations__person_notes` (a new table).
- OpenAPI: three operations, four schemas; client types regenerated.
- Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/272

## Non-goals

- Notes shared with teammates.
- Turning "Teach Scout" sentences into notes.
- A formality picker or any other control per person.
- Notes steering likely replies (`conversations.suggest_replies`).
- Reading the note anywhere but the member's own written replies.
