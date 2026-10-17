# Tasks

## 1. Server
- [x] 1.1 Migration: `conversations.need_summaries` with row-level security; queue every inbox for the needs backfill
- [x] 1.2 `conversations.summarize_need`: fixed instruction, mail as JSON data, strict schema, bounded plain-text output, daily-limit and failure handling; tests
- [x] 1.3 Enqueue when a need opens (`NeedSettings::summaries`); the backfill queues the open needs without one; tests
- [x] 1.4 `need_summary` on `conversations.get` in the caller's language; event `conversations.need_summarised`; tests

## 2. Client
- [x] 2.1 `ItemDetail` `summary` slot (visor, heading, text only); story and test
- [x] 2.2 Home's opened item shows it above the email; strings in en, pt-PT, es-ES and fr; test
- [x] 2.3 Recording scenario

## 3. Ship
- [x] 3.1 OpenAPI and events catalogue regenerated, client types updated, changelog, archive
