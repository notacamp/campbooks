# Proposal

## Why

Background work (mail sync, the AI read, event deliveries) runs through the
kernel's job queue, and a job that runs out of attempts is kept as `failed`
for inspection. Today the only way to see or act on one is SQL on the
database. An operator needs to see whether the queue is healthy, why jobs
gave up, and to put them back on the queue once the cause is fixed, from the
admin dashboard (epic notacamp/campbooks-cloud#95).

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/99.
Foundation: ADR 0015 and the `admin-foundations` change.

## What Changes

- **Kernel** `jobs::admin`: functions taking an `AdminGrant` — queue summary
  (counts per queue and kind), failed jobs (cursor paging, kind and queue
  filters), retry one, retry all failed of a kind (bounded), discard one.
  None of them reads a job's payload.
- **system** admin operations under `/admin/v1/system/jobs`:
  `system.admin_jobs`, `system.admin_failed_jobs` (`jobs.read`);
  `system.admin_retry_job`, `system.admin_retry_jobs`,
  `system.admin_discard_job` (`jobs.retry`).
- **Client**: Admin → Jobs: the queue by kind, the failed list with each
  error (monospace, wrapped, copyable), retry, discard with a confirmation,
  and "retry all" per kind with a confirmation; a `CodeBlock` design-system
  component and a `--font-mono` token.

## Non-goals

- Job payloads, in any form, now or later: the admin API never returns them.
- Enqueuing, cancelling or reprioritising jobs, pausing queues, or editing
  schedules.
- Per-workspace views of the queue, history of succeeded jobs (they are
  pruned), job traces or logs (observability owns those).
- A separate `jobs.discard` permission: discarding is `jobs.retry`.
