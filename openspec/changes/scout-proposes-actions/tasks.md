# Tasks

## 1. Storage

- [x] 1.1 Migration `<slot>_assistant__chat_proposals`: `assistant.chat_messages.proposals jsonb NOT NULL DEFAULT '[]'` (new column only, `lock_timeout`)

## 2. Proposing tools (`assistant`)

- [x] 2.1 The action table: action → operation, input builder, undo (move back for `move_to_stream`); test against the catalogue (`assistant: true`, `mutates: true`, `risk: low`, `x-campbooks.undo`)
- [x] 2.2 `propose_change`, `propose_rule`, `propose_reply`: narrow schemas (`action` an enum), strict argument checks as the read tools'
- [x] 2.3 Checking: read the conversation as the member; `not_visible`, `read_only`, `already_so`, `no_such_need`, labels, `until` bounds, `too_many`, duplicates collapsed; never invoke a mutating operation (test: a fake catalogue op that panics if invoked)
- [x] 2.4 Proposals stored with Scout's message (request and undo paths built from the catalogue's templates and checked ids; facts from the record); streamed in `done` and answered by `ask_chat` and `get_chat`
- [x] 2.5 `assistant.settle_proposal` with its state moves and 409 `proposal_settled`; owner only; not callable by assistant or automation
- [x] 2.6 The chat so far: a server-written line per settled proposal after Scout's text
- [x] 2.7 The instruction and the core handbook section: proposing, never claiming done, never because an email asks; four locales for new messages
- [x] 2.8 Logs: proposal counts and refusal kinds only

## 3. Server tests

- [x] 3.1 HTTP tests with a fake provider: archive proposed and nothing changed; each refusal; a fourth proposal; duplicates; rule and reply proposals; settle moves and 409; another member's chat 404; a hostile email asking for a change stays a card at most; the model reads settled lines
- [x] 3.2 The access matrix covers `settle_proposal`

## 4. Client

- [ ] 4.1 `lib/ui` card for a proposal (action line, facts, quoted Scout words, Confirm / Not now, done with Undo, failed, settled read-only)
- [ ] 4.2 The client's list of runnable actions (same ids as the server's), the guard on method and path, `Idempotency-Key` from the proposal id, query refresh, settle calls
- [ ] 4.2a `from_response`: copy only the named top-level fields of the change's answer into the undo body (test with a fake row)
- [ ] 4.3 Rule card → Teach Scout with the sentence (settles on save or close); reply card → Reply dialog with the intent through a `lib/` opener wired by the app
- [ ] 4.4 The panel's line "Scout asks before it changes anything."; four locales; stories with mocked proposals; component tests; `test:fit`
- [ ] 4.5 Recording scenario: a proposal confirmed and undone, a rule card, at 375px and desktop

## 5. Close

- [ ] 5.1 OpenAPI and client types regenerated (CI artifacts)
- [ ] 5.2 Changelog entry file
- [ ] 5.3 Gates green
- [ ] 5.4 UI recorded at 375px and desktop
- [ ] 5.5 `openspec archive scout-proposes-actions --yes` before merge
