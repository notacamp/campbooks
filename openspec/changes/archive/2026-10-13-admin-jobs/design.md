# Design

## Data ownership and events

- `kernel.jobs` is the kernel's table and is "written only through the
  kernel's API". The new functions live in `campbooks_kernel::jobs::admin` and
  are the only SQL that reads it for administration. The `system` module
  serves the HTTP operations (it already owns instance operational views); it
  holds no SQL on `kernel.*`.
- No new table, no migration. `kernel.jobs` has no workspace policy (the queue
  is infrastructure), so nothing here uses `begin_admin` and no table joins
  the admin-read allowlist, which stays as it is: the pinned test still proves
  content tables cannot be read across workspaces.
- No domain events: retrying or discarding a job is an operator action,
  recorded by the admin audit trail (operation id and the job id as target).

## Operations

| operationId | route | permission |
| --- | --- | --- |
| `system.admin_jobs` | `GET /admin/v1/system/jobs` | `jobs.read` |
| `system.admin_failed_jobs` | `GET /admin/v1/system/jobs/failed?kind&queue&after&limit` | `jobs.read` |
| `system.admin_retry_job` | `POST /admin/v1/system/jobs/{id}/retry` | `jobs.retry` |
| `system.admin_retry_jobs` | `POST /admin/v1/system/jobs/retry` `{kind, queue?}` | `jobs.retry` |
| `system.admin_discard_job` | `DELETE /admin/v1/system/jobs/{id}` | `jobs.retry` |

- **Summary**: one row per (queue, kind) with unfinished or failed work:
  ready (pending and due), scheduled (pending for later: delayed, or waiting
  out a retry backoff), running, running past its lease (a worker died; the
  next upkeep reclaims it), failed, and the age of the oldest ready job.
  Succeeded jobs are pruned and not counted.
- **Failed list**: `id, queue, kind, workspace_id, attempts, max_attempts,
  last_error, created_at, finished_at`, newest failure first. The cursor is
  `<finished_at micros>.<id>` (opaque to clients); `limit` 1–200, default 50.
  `last_error` is already truncated and secret-free by the queue's contract.
- **Metadata only**: the SQL never selects `payload`, so no response type can
  carry it. A test inserts a payload with a marker string and asserts it never
  appears.
- **Retry one**: `UPDATE … WHERE id = $1 AND state = 'failed'` to
  `pending, run_at = now(), attempts = 0, finished_at = NULL`, then
  `pg_notify` so an idle worker wakes. The statement itself matches only
  failed rows, so a job a worker has claimed, or that finished, cannot be
  touched, however the call races with a claim (a claim updates only `pending`
  rows; both take the row lock). Outcome `applied`, or `unchanged` when the
  job is no longer failed (so a retried-twice job is a no-op success), `404`
  when the id never existed. A failed job whose `(kind, unique_key)` is now
  held by another unfinished job is `superseded`: re-running it would
  duplicate that work, and the partial unique index would refuse it.
- **Retry all of a kind**: the same update over at most 1000 failed jobs of
  the kind (optionally one queue), oldest first, `FOR UPDATE SKIP LOCKED`,
  skipping unique-key conflicts (also among the batch itself). Returns
  `{requeued, more}`; the UI says so and the operator can repeat the call.
- **Discard**: `DELETE … WHERE id = $1 AND state = 'failed'`; `unchanged` when
  it was not failed or is already gone (so repeating is a no-op success).
- `last_error` is kept on retry (a job that fails again overwrites it), so a
  retried job still tells what went wrong last time while it waits.

## Client

- Admin → "Jobs" (`/admin/jobs`), contributed by `modules/system`
  (`JOBS_ADMIN_SECTION`, one entry in `ADMIN_SECTIONS`).
- Summary list: one row per kind, counts in words, "Show failures" (filters
  the list below) and "Retry all failed" (inline confirmation naming the count
  and kind). Failed list: filter by kind, rows with the error in the new
  `CodeBlock` (monospace, wrapped, Copy button with an announced result),
  "Retry", and "Discard" with an inline confirmation ("deleted for good");
  "Show more" pages by cursor. Every action says what happened in a banner
  (including `unchanged` and `superseded`), failures say so too.
- Empty, loading and error states; 375 px and desktop; light and dark;
  strings in en, pt-PT, es-ES, fr; Storybook stories for the screen states and
  for `CodeBlock`.

## Alternatives considered

1. **Copy the old Rails app's approach: mount a generic jobs dashboard**
   (Mission Control) behind an admin check. It shows payloads and arguments,
   has its own UI and auth, and would need either a second web app or
   embedding another product's UI. Rejected: payloads are exactly what must
   not leave the queue, and the dashboard would sit outside the design system,
   the audit trail and the permission model. It was not copied by reflex; the
   one thing kept from it is the idea of retry/discard by kind.
2. **Let the `system` module query `kernel.jobs` directly.** Simplest, no
   kernel change. Rejected: the kernel owns its tables and an `xtask arch`
   rule exists to keep other modules out; every future module with an admin
   view of the queue would repeat the SQL, and the "never select payload" and
   "failed only" guarantees would be per-handler convention instead of a
   property of one reviewed API. The cost of the chosen design is a small API
   in the kernel, which stays domain-free (kinds and queues are opaque
   strings).
3. **A new `jobs` module with its own schema** (a read model of the queue).
   Rejected: it would duplicate state that already has one owner and would lag
   it; the queue is small enough to aggregate on demand (partial indexes cover
   the state filters) and the summary is refreshed every 15 seconds by the UI
   only while the page is open.
4. **Retry by flipping state in the UI via a generic "update job" endpoint.**
   Rejected: a general mutation would let a client change attempts, run time
   or state; the narrow verbs (retry, retry kind, discard) can only do what an
   operator means.

Self-host weight: nothing new to run. The old Rails app is not copied except
for the retry/discard vocabulary.

## Risks and trade-offs

- Summary counts scan `kernel.jobs` excluding `succeeded`; succeeded rows are
  pruned by `PruneJobs`, so the scan stays small. If a deployment keeps them
  for a long time the query is still bounded by an index-free aggregate over
  one table; revisit with a partial index if it shows up in traces.
- `last_error` can name a provider or an id; it is shown to administrators
  with `jobs.read` only and is already specified as secret-free.
