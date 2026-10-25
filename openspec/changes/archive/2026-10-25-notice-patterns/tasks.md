# Tasks

## 1. Server

- [x] 1.1 Migration: `pattern` and `example_subject` on suggestions, `pattern` on answers, the groupings table with row-level security
- [x] 1.2 Settings `MIN_KIND_MESSAGES`, `MIN_KIND_DAYS`, `MODEL_SENDERS_PER_PASS`, `REGROUP_DAYS`, `MODEL_SUBJECTS`, with defaults
- [x] 1.3 Per-conversation read of frequent senders (runtime SQL) and the five steps in `analysis`
- [x] 1.4 The `complete` call: subjects only, strict schema, vetted phrases, cache, no transaction held
- [x] 1.5 Store, list, read and answer per pattern; `forget_inbox` clears the groupings
- [x] 1.6 Teach once: `sender_phrase`, default from the sender's kinds

## 2. Client

- [x] 2.1 Sentence with a real example subject in en, pt-PT, es-ES and fr
- [x] 2.2 "See the N emails" narrowed to the kind
- [x] 2.3 `sender_phrase` titled in teach once
- [x] 2.4 Stories for a template kind and a phrase kind

## 3. Tests

- [x] 3.1 Analysis: one kind, promotions ignored and shipping read, a bill blocks its kind, suspicious blocks all, mixed gets nothing, the model's words
- [x] 3.2 Pass: kinds stored with pattern and example, answered per kind, the model's words, refused model, the cache reused
- [x] 3.3 Teach once: mixed defaults to the template, the kept phrase, one kind stays the sender
- [x] 3.4 Client: sentence and search with a pattern

## 4. Release

- [x] 4.1 OpenAPI and client types updated
- [x] 4.2 Tracking plan: no new operation, job or event
- [x] 4.3 Changelog entry file
- [x] 4.4 Gates green in CI
- [x] 4.5 UI verified at 375px and desktop (recording)
