# Tasks

- [x] 1.1 `reply/prompt.rs`: `MAX_TOKENS` 4096 for suggestions and drafts, with the reason; the prompt test checks it
- [x] 1.2 `ChatResponse::finish_kind` in the gateway, with a unit test; compile uses it
- [x] 1.3 Reply's warning on an answer that does not parse: `reason` (`empty` / `truncated` / `not_json`), `finish_reason`, `completion_tokens`, `content_chars`
