# Tasks

## 1. Decide

- [x] 1.1 Laya and OpenAI-compatible parsers: a catalogue answer missing or not fitting its type is unanswered; triage stays strict
- [x] 1.2 Gateway drops unanswered questions; the model strategy reads answers by id (an unanswered question abstains)
- [x] 1.3 Tests: parser tests on both protocols; decide pass with the catalogue on and a missing, "NaN" and out-of-range action answer decides triage exactly as with it off, stores no rating for that action, and succeeds (Laya and OpenAI)

## 2. Labelling

- [x] 2.1 `eval actions` asks Open attachment only for items with an attachment
- [x] 2.2 `labelling export --set actions` lists each conversation's newest message that raised a need; test asserts it

## 3. Docs

- [x] 3.1 CHANGELOG (Fixed)
- [x] 3.2 Gates green in CI
