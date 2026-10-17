# Tasks

## 1. Home

- [x] 1.1 `rowChoices`: ranked actions in the server's order, a pair as one, the fixed actions when the key is missing; a question keeps Yes/No; no Still to pay twice
- [x] 1.2 `ItemActions` `picked`: the visor before the own actions, named; story and tests
- [x] 1.3 Remind in the Snooze dialog (due-date reminder first), Forward dialog found by name, Open attachment focuses the file
- [x] 1.4 Strings in en, pt-PT, es-ES and fr

- [x] 1.5 Server: the attachment fact counts files only (no image the body shows); the combo table forbids two follow-up dialogs
- [x] 1.6 Forward re-addresses only an untouched forward, goes nowhere when closed mid-start, never stays "Starting…"; a warning offers only its answers; Open attachment says when there is no file

## 2. Tests

- [x] 2.1 Model: fallback, ranked order, pair, question and still-to-pay
- [x] 2.2 Home: ranked render with the visor, fallback when missing, keys 1 to 3, the pair's button, Remind and Forward dialogs, Open attachment
- [x] 2.3 Laya decide-pass test compares the needs, on vs off (test-only)
- [x] 2.4 Server: files only (a logo is no attachment); no combo of two follow-ups
- [x] 2.5 Home: read-only narrowing, Before it's due first, a pair keeps the dialog's focus, re-address paths (PATCH and new), closed mid-start, no file to open, the email can't be read

## 3. Docs

- [x] 3.1 Recording scenario `home-ranked-actions` and the steps to run it
- [x] 3.2 CHANGELOG
- [x] 3.3 Gates green in CI
