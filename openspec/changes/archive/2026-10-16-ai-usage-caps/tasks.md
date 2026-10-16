# Tasks

## 1. AI gateway

- [x] 1.1 `campbooks_ai::usage`: `Scope` (task-local; workspace or instance, feature, pool), `Meter` port (`admit`, `settle`), `Admission`, `Outcome`; `Gateway::with_meter`, `is_metered`
- [x] 1.2 `chat`, `embed`, `decide` go through one metering step; errors `OverDailyCap`, `Unattributed`, `Metering`; tokens from chat, embeddings and the both `decide` protocols when reported
- [x] 1.3 The answers check runs under the instance scope
- [x] 1.4 `CAMPBOOKS_AI__DAILY_CALLS_PER_WORKSPACE` (unset 5,000, `0` none, blank unset)
- [x] 1.5 Tests: unattributed calls refused for every method with nothing sent; admitted calls settled with tokens and failures; refused and failing meters send nothing; instance scope; an unmetered gateway needs no scope

## 2. system module

- [x] 2.1 Migration `system__ai_usage`: `ai_usage_days`, `ai_usage`, row-level security, admin reads, grants
- [x] 2.2 `AiUsageMeter` (cap, clock): atomic admission, refusal count and the once-a-day log line, settle
- [x] 2.3 `system.admin_ai_usage` (`GET /admin/v1/system/ai-usage`, `system.read`)
- [x] 2.4 Tests: counts per workspace, role, feature and day; the cap; concurrent admission; the day rolls over; two workspaces; no cap; the admin read and its 404s

## 3. conversations

- [x] 3.1 The `decide` pass runs in its workspace's scope; over the cap it stores nothing; a metering failure retries
- [x] 3.2 Tests: the first messages get model answers, the rest no model pass, no failed job, no re-queue

## 4. Server

- [x] 4.1 Compose the gateway with the meter; the same meter in the system module's deps
- [x] 4.2 Tests: the composed gateway is metered; end to end, cap reached, next day resumes, admin read; the admin-read policy allowlist

## 5. Wrap-up

- [x] 5.1 `openapi/admin.json`, `.sqlx`, `deploy/.env.example`, CHANGELOG
- [x] 5.2 Gates green, terminal recording, archive this change
