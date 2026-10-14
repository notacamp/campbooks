# Tasks

## 1. Kernel

- [x] 1.1 `kernel::jobs::admin`: `summary`, `failed` (cursor, filters), `retry`, `retry_kind` (bounded), `discard`; all take an `AdminGrant`, none selects `payload`
- [x] 1.2 Tests: counts by queue and kind; paging and filters; retry touches only failed jobs and is idempotent; a retried job runs again in the worker; discard only deletes failed jobs; unique-key conflicts are skipped; retries racing three workers never run a job twice

## 2. system module

- [x] 2.1 `features/admin_jobs` (`system.admin_jobs`, `system.admin_failed_jobs`, `jobs.read`)
- [x] 2.2 `features/admin_job_actions` (`system.admin_retry_job`, `system.admin_retry_jobs`, `system.admin_discard_job`, `jobs.retry`)
- [x] 2.3 Tests through the admin router: 404 for members and for the wrong permission, 200 with the right one, payload never present, audit trail records the operation and job id, mutations idempotent
- [x] 2.4 `openapi/admin.json`, `.sqlx`

## 3. Client

- [x] 3.1 `CodeBlock` in `lib/ui` (+ `--font-mono`), story and test
- [x] 3.2 `modules/system`: Jobs panel (summary, failed list, retry, discard, retry all), nav entry, four locales
- [x] 3.3 Storybook screen stories, component tests, recording scenario `admin-jobs`

## 4. Wrap-up

- [x] 4.1 CHANGELOG, archive this change
