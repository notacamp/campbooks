# Tasks

## 1. Measure
- [x] 1.1 Four arms over 50 fictional emails; report in `crates/ai/evals/complete/2026-10-28-read-budget.md`

## 2. Gateway
- [x] 2.1 `ChatRequest.reasoning_effort`, `ProviderConfig.reasoning`, `accepts_reasoning`; the request's effort wins when the provider takes it
- [x] 2.2 `ChatResponse::ran_out_of_room`, `ChatRequest::with_more_room`, `Gateway::chat_with_room`
- [x] 2.3 Gateway tests: sent, wins over the role, dropped for a provider that does not take it, retry with more room

## 3. Read and grouping
- [x] 3.1 `read_message` and the grouping: low effort, 8192 tokens, `chat_with_room`
- [x] 3.2 Other 4096 callers checked and listed in design.md

## 4. The note says so
- [x] 4.1 `conversations.get` carries `need_read_failed`; test in the read tests
- [x] 4.2 Reader line `scout.readFailed` in en, pt-PT, es-ES, fr; unit test and Storybook stories

## 5. Finish
- [x] 5.1 OpenAPI and client types updated for `need_read_failed`
- [x] 5.2 Tracking plan unchanged (no event, no Coverage row)
- [x] 5.3 `deploy/.env.example` documents `CAMPBOOKS_AI__<ROLE>__REASONING`
- [x] 5.4 Changelog entry file
- [x] 5.5 Change archived; baseline specs updated
