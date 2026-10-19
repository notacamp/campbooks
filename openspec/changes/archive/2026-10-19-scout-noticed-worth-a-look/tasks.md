# Tasks

## 1. Engine and server

- [x] 1.1 `worth_a_look` action and `highlight` slot in the engine; refused with anything that hides mail
- [x] 1.2 `RuleAction::WorthALook`; stored as a `taught` answer on arrival, over existing mail and on undo
- [x] 1.3 Need reason and question `worth_a_look`; raised only when nothing else is; cleared as `opened` once read
- [x] 1.4 `mark_read` / `mark_unread` work the need out again
- [x] 1.5 Attention reason `worth_a_look`, listed after Pay; `opened` is not in Done today
- [x] 1.6 Migrations (conversations and attention checks, validated in their own migrations)
- [x] 1.7 Seen files no correction for a `worth_a_look` need

## 2. Client

- [x] 2.1 Scout noticed after the first group; four buttons, one style, Something else… with the visor
- [x] 2.2 Confirmation asks what should happen first; Scout's guess, Highlight them, Newsletters, Mark read, own words
- [x] 2.3 `ChoiceCards` tag; `signal-look` token, Pill label
- [x] 2.4 Worth a look group last; Seen and Snooze only
- [x] 2.5 Strings in en, pt-PT, es-ES and fr, with diacritics
- [x] 2.6 Recording scenario `smart-rule-offers` updated

## 3. Tests

- [x] 3.1 Engine: slot, validation, evaluate
- [x] 3.2 Needs: raised last, cleared when opened, Seen
- [x] 3.3 A notice answered with another goal; opening clears one item only
- [x] 3.4 Components: confirmation goals, free text, Home order, Worth a look row, ChoiceCards tag, model

## 4. Release

- [x] 4.1 OpenAPI, events and contract updated; client types updated
- [x] 4.2 CHANGELOG updated
- [ ] 4.3 Gates green in CI
- [ ] 4.4 UI verified at 375px and desktop (recording)
