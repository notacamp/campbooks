# Tasks

## 1. AI gateway

- [x] 1.1 `ExternalContent`, `Location`, `AiConfig::internal_hosts`, `provider_for_content`; ignore role sections without base URL and model
- [x] 1.2 `Error::ExternalContentRefused` (+ `ai_external_content_refused` in four locales); gate `chat`, `embed`, `decide`
- [x] 1.3 `Error::Status` without body; serde errors described by kind; health reports location and content flag
- [x] 1.4 Tests: classification, opt-in, refusal before any request, bodies never shown

## 2. Kernel

- [x] 2.1 `redact` (json, sqlx, error); internal-error log and `JobError::from(sqlx::Error)` use it
- [x] 2.2 `CAMPBOOKS_EVENTS__RETENTION_DAYS`, `CAMPBOOKS_JOBS__FAILED_RETENTION_DAYS`; `kernel.prune_events`; failed jobs in `kernel.prune_jobs`
- [x] 2.3 `jobs::purge_workspace`; migration `kernel__retention` (indexes)
- [x] 2.4 Tests: pruning keeps undelivered events, purge keeps unfinished jobs, `last_error` without content

## 3. Modules

- [x] 3.1 conversations: no model pass when `decide` is refused (rules sort); test
- [x] 3.2 connectors: mail reader errors by kind; test
- [x] 3.3 workspaces: purge deletes events and finished jobs; test
- [x] 3.4 system: overview `location` and `content_allowed`; test

## 4. Server and clients

- [x] 4.1 `no-store` middleware for `/api/`, `/bff/`, `/admin/`; test
- [x] 4.2 Redaction integration test with log capture (provider body, Postgres DETAIL and message)
- [x] 4.3 Admin → System shows where each role is and whether it gets mail (four locales); tests
- [x] 4.4 `pnpm record` refuses non-local base URLs without `--allow-remote`; tests

## 5. Docs and gates

- [x] 5.1 `deploy/.env.example`, `docs/architecture.md`, ADR 0012 amendment, clients README
- [x] 5.2 OpenAPI regenerated, client types regenerated, `.sqlx` regenerated
- [x] 5.3 CHANGELOG updated
- [x] 5.4 Gates green; UI verified at 375px and desktop
