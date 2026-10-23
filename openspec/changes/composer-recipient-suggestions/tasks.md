# Tasks

## 1. Conversations: who the caller exchanges mail with

- [ ] 1.1 `domain::reachable`: the flagged-sender SQL check and `may_write_to`, moved out of `list_senders` (its tests unchanged)
- [ ] 1.2 `features/list_correspondents`: query, bounds, both directions, own addresses left out, ranking, safety for the inbound-only, `invalid_correspondents_query` in the four catalogues
- [ ] 1.3 Route and operation (`conversations:read`, `conversations.inbox` read)
- [ ] 1.4 HTTP tests: name and domain, accents, written-to only, ranking, no-reply and reported left out unless written to, another member's inbox, bounds, the caller's own address, literal `%` and `_`

## 2. Composer

- [ ] 2.1 `RecipientField` as a combobox when given suggestions (listbox, keys, click, Esc kept only while open, already added left out); story and tests
- [ ] 2.2 `useCorrespondents` and `useRecipientSuggestions` (debounced, per field, excluding To, Cc and Bcc)
- [ ] 2.3 `MessageComposer` takes the suggestions; `DraftComposer` passes them (new message, reply, reply all, forward)
- [ ] 2.4 Strings in en, pt-PT, es-ES and fr (diacritics checked)

## 3. Close

- [ ] 3.1 OpenAPI regenerated
- [ ] 3.2 Client types regenerated
- [ ] 3.3 Changelog entry file in `changelog/unreleased/`
- [ ] 3.4 Gates green (CI)
- [ ] 3.5 Recording at 375px and desktop: typing in To, picking with the keyboard, then in Cc
- [ ] 3.6 `openspec archive composer-recipient-suggestions --yes`
