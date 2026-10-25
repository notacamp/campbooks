# Design

How to add a tracked event and a Coverage row is in `docs/tracking.md`; the
decision is ADR 0020. This change makes Coverage required and closes the
rows still missing on `next`.

## The gate

Two checks read the Coverage sheet, and both now fail on a missing row:

- **The plan tool** (`cargo run -p campbooks-tracking-plan -- check`, in CI
  and runnable without building the server) reads the operations of
  `openapi/openapi.json` and `openapi/admin.json`. A mutating operation
  (`x-campbooks.mutates`) with no row is a problem, naming the operation and
  the fix. `check::operations` returns problems only; the report loses its
  `warnings` field, which had no other use.
- **The server's coverage test** (`crates/server/src/tracking_tests.rs`)
  builds the real registry and catalogue, so it also sees jobs, schedules
  and event handlers, the kernel's own maintenance included. It fails on a
  row naming something the server does not compose (as before) and on
  anything composed with no row, listing each as `<kind> \`<name>\``.
  `REQUIRE_FULL_COVERAGE` goes: there is nothing left to switch.

There is no baseline, allow-list or ratchet: the owner's decision. A new
operation, job, schedule or handler lands with its row in the same PR.

## The policy step

"Tracking plan changes come through a patch" (`.github/workflows/policy.yml`,
PRs only, `if: always()` like the others): when the PR's own changes
(`PR_RANGE`, computed by the first step) touch
`tracking/tracking-plan.xlsx` or `tracking/tracking-plan.json`, they must
also add or modify a file under `tracking/patches/`. Otherwise the step
fails with a plain message pointing to `docs/tracking.md`. A hand edit of
the workbook is still allowed by the tool; this step only asks that the
change be recorded as a patch, so the plan's history reads from
`tracking/patches/` and reviewers see the rows that changed as text.

## Data ownership and events

The address-check events belong to `connectors` (schema `connectors`). The
snapshot is the existing `connectors.connection` record, read by
`domain/tracked.rs` from `connectors.connections` in the transaction of the
change: ids, provider, the granting member's user id, capabilities,
health, `address_verified`, the mailbox count and times. Never the address,
the code, the nonce, a host or a username. No domain event, endpoint,
scope, AI role or migration changes; the queries are the existing runtime
ones.

### Events added (plan version 11)

| Event | Fires when | Snapshot and properties |
| --- | --- | --- |
| `connectors.address_check_requested` v1 | `connectors.check_address` starts a new check of an IMAP mailbox whose address is not verified, in the transaction that starts it. A check under way is answered as it is and tracks nothing; a refused ask (not needed, needs reauth, IMAP off, the day's five checks spent) tracks nothing. | `connection` (`connectors.connection`) |
| `connectors.address_check_send_completed` v1 | `connectors.send_address_check` marks the check's message sent (the runs look for it from now on) or ends the check unverified because it could not be sent, in the job's transaction. A check sent, ended or replaced meanwhile, a connection gone, or an address already verified tracks nothing; a busy server tried again later is not an event. | `connection` (`connectors.connection`), `outcome` (`connectors.address_check_send_outcome`: `sent`, `failed`), `reason` (`connectors.address_check_reason`, when failed) |

Actors: `address_check_requested` is `user`, `client`, `assistant` or
`automation` (the operation takes `connectors:write`, like
`connectors.sync_now`); `address_check_send_completed` is `system`, the job,
on behalf of whoever started the check (a connect or a retry).

`connectors.address_check_reason` values are the job's own closed codes:
`provider_not_configured`, `address_changed`, `needs_reauth`,
`not_connected`, `address_invalid`, `address_refused`,
`smtp_login_refused`, `smtp_unavailable`, and `other` for a code the plan
does not name yet. Never the server's reply.

The check's end in the stream's runs (the code found and the address
verified, or not found within the window) is not tracked here; it is a
follow-up on `connectors.sync_stream`, whose row exists.

### Coverage rows added

| Kind | Name | Events or exemption |
| --- | --- | --- |
| operation | `connectors.check_address` | `connectors.address_check_requested` |
| job | `connectors.send_address_check` | `connectors.address_check_send_completed` |
| schedule | `kernel.prune_jobs` | maintenance, no action of anyone: finished jobs past their retention deleted |
| schedule | `kernel.prune_events` | maintenance, no action of anyone: domain events past their retention deleted, never one still being delivered |
| schedule | `kernel.prune_tracked_events` | maintenance, no action of anyone: tracked events already published deleted from the outbox |
| schedule | `kernel.sweep_blobs` | maintenance, no action of anyone: blobs no record references and abandoned uploads deleted after their grace period |
| schedule | `kernel.prune_idempotency` | maintenance, no action of anyone: expired idempotency keys deleted |
| schedule | `kernel.prune_rate_limits` | maintenance, no action of anyone: rate-limit counters whose window ended deleted |
| schedule | `kernel.prune_changes` | maintenance, no action of anyone: change-feed entries past their retention deleted |
| schedule | `kernel.sweep_purged_workspaces` | maintenance: what reached the kernel's tables of a purged workspace after its purge deleted; the purge is tracked as `workspaces.workspace_purged` |
| schedule | `kernel.dispatch_rekeys` | maintenance, no action of anyone: only enqueues the modules' re-tag jobs while a previous key is set, which have their own rows |
| job | `kernel.sequence_changes` | projection: numbers a workspace's change-feed entries for clients to follow; the changes themselves are tracked by the modules that make them |

The list is the server coverage test's own output with the gate on; any
other row it lists in CI is added to the patch and to this table.

## Risks

- A PR open while this merges, adding an operation or a job without a row,
  goes red once it merges `next`. That is the gate working; its message
  says what to add.
