# Tasks

## 1. Engine

- [x] 1.1 `Condition::MemberNotInTo`, `Action::WaitOnAddressed`, `Slot::Wait`, `Facts.member_not_in_to`, `Outcome.wait`; validation (the action needs the condition; the condition alone is too broad); suspicious or impersonated mail never waits
- [x] 1.2 Unit tests: holds and fails closed, own slot, stable JSON, validation

## 2. Rules and needs

- [x] 2.1 Rule condition and action on the wire; engine mapping; the facts on arrival, for existing mail and in the strategy
- [x] 2.2 `rules::waits::answers`: the single person in `To`, stored as taught `next_move` and `awaited`; reverted like any answer
- [x] 2.3 `needs::waiting` reads a taught move; `moved_on` counts it; the wait is `sure`, basis `taught`
- [x] 2.4 Unit tests of the need: a taught wait, a taught reply first, every guard

## 3. The offer

- [x] 3.1 `TaughtAction::Waiting`, `RuleSuggestions.wait_days`; the count, the match with the member's own answer, dismissal reused
- [x] 3.2 HTTP tests: the second answer offers, the first does not, Undo cancels, Ask me each time dismisses, the rule waits on Ana and sends nothing, a taught reply comes first

## 4. Client

- [x] 4.1 `AheadAction` `waiting`; the offer in Home and the reader; the rule sentence; four locales with accents
- [x] 4.2 Unit tests of the offer (Yes, always; Ask me each time; Undo)
- [x] 4.3 Home screen story and a play-less twin for the recording

## 5. Close

- [x] 5.1 OpenAPI and client types regenerated, changelog entry file
- [x] 5.2 Gates green; the recording left to the maintainer (`## Recording` in the PR)
- [x] 5.3 Change archived into `openspec/specs/`
