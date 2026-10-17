# Tasks

## 1. Compile

- [x] 1.1 `prompt.rs`: `MAX_TOKENS` 4096, with the reason
- [x] 1.2 `ChatResponse::json` reads one Markdown code fence; unit tests (plain, fenced, prose, truncated, other info strings)
- [x] 1.3 An answer that does not parse is 503 `compile_failed`; the OpenAPI description says so
- [x] 1.4 The `compile` log line: `finish_reason`, `completion_tokens`, `content_chars`; a warning with `empty` / `truncated` / `not_json`
- [x] 1.5 Tests: prose, empty and truncated answers are 503; a fenced answer is read; "my accountants at northtax or atelier conseils" gives two `who` questions

## 2. Teach Scout

- [x] 2.1 "Build a rule yourself" next to the privacy line, with no model call; "Tell Scout in a sentence instead"
- [x] 2.2 The builder's lead for the chosen path; `unclear` and `unsupported` offer the builder
- [x] 2.3 The builder's "It's from" condition is the sender filter; one rule card per ticked sender
- [x] 2.4 Strings in en, pt-PT, es-ES and fr
- [x] 2.5 Tests: the builder without a model call, two senders give two rules, no sender says so, `unclear` offers the builder
- [x] 2.6 Story `Screens/Scout's memory/Teach by hand` and recording scenario `teach-by-hand`

## 3. Wrap-up

- [x] 3.1 OpenAPI and client types updated (one description)
- [x] 3.2 CHANGELOG updated
- [x] 3.3 Gates green in CI
- [x] 3.4 Recording scenario ready; the maintainer records it at 375px and desktop
