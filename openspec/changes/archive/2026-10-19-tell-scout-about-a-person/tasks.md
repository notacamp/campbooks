# Tasks

## 1. Storage

- [x] 1.1 Migration `20261023070000_conversations__person_notes`: the table, its checks and unique key, RLS enabled and forced with the workspace policy, granted to the runtime role
- [x] 1.2 `forget_member` deletes a departing member's notes; `purge_workspace` deletes the workspace's

## 2. API

- [x] 2.1 `conversations.person_note` in the access catalogue, owned by its member, with its facts
- [x] 2.2 `list_person_notes`, `set_person_note`, `delete_person_note`, every statement keyed by the caller
- [x] 2.3 `plain_address` and `kept_char` moved to `domain::person`, shared with the reply code
- [x] 2.4 Error messages in en, pt-PT, es-ES and fr

## 3. Drafting

- [x] 3.1 `write_reply` looks up the caller's note for a trusted person answered and sends it as `my_note_about_them`
- [x] 3.2 The instruction says how to use it

## 4. Client

- [x] 4.1 `NoteField` in `lib/ui`, with a story and a test
- [x] 4.2 The reply dialog's line and inline form, "Rewrite with this"
- [x] 4.3 The people card on Scout's memory, Edit and Remove with Undo
- [x] 4.4 Translations in four locales; a recording scenario

## 5. Tests

- [x] 5.1 HTTP tests: keep, change, forget; another member's note is 404; limits; an app acting for nobody; names; a departing member; nothing in the event log or the queue
- [x] 5.2 Reply tests: the caller's note only, none for a distrusted message, none for suggestions, a hostile note stays data
- [x] 5.3 The access matrix seeds a note
- [x] 5.4 Component tests: the dialog's line and form, the memory card

## 6. Close

- [x] 6.1 OpenAPI regenerated
- [x] 6.2 Client types regenerated
- [x] 6.3 Changelog entry file
- [x] 6.4 Gates green
- [ ] 6.5 UI verified at 375px and desktop (the lead records it)
