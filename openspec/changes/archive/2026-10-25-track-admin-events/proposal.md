# Track the admin module's changes

Module: `admin` (instance administrators' profiles, changed only by the
server's `campbooks-server admin grant|revoke` command), with the server's
admin CLI (`crates/server/src/admin_cli.rs`) and the tracking plan
(`tracking/`). Tracking:
https://github.com/notacamp/campbooks-cloud/issues/331, part of epic
https://github.com/notacamp/campbooks-cloud/issues/319.

## Why

ADR 0020 asks for a record of every action: who did it, in what context,
and what the records it touched looked like then. The most security-relevant
change on an instance is someone gaining or losing admin powers, and it is
not tracked yet: an operator runs `campbooks-server admin grant` or
`revoke`, and only the module's own audit trail (`admin.actions`) knows.
The module serves no routes and runs no jobs, schedules or handlers, so it
has no Coverage rows; its two changes are CLI commands, and they need events
before full coverage becomes a CI failure with campbooks-cloud#332.

## What Changes

- Tracking plan version 8 (`tracking/patches/0008-admin.yaml`): 2 new live
  instance-wide events, `admin.profile_granted` and `admin.profile_revoked`,
  the snapshot record `admin.profile` and the enum `admin.permission`.
- `campbooks_admin::cli::grant` and `revoke` take the process's `Tracker`
  and track the change with `Tracker::track` in the transaction that writes
  the profile and its audit entry, after the change, from the profile read
  in that transaction. A grant that adds no permission to an existing
  profile, and a revoke that takes none away (no profile, or permissions it
  does not hold), track nothing.
- The server's `admin` command builds its tracker from
  `CAMPBOOKS_TRACKING__*` with the role `cli`. The actor is the kernel's
  context outside any request or job: `system` with the command's name
  (`admin`), set by the server's `main`.
- Tests grant and revoke through the CLI functions and read the events back
  from `kernel.tracked_events`, the no-ops included.

No new endpoint, scope, migration, dependency or domain event. No query the
module already has changes, and the audit trail records what it recorded
before.

## Non-goals

- The other modules (#325–#330) and the hard coverage gate (#332).
- `admin list` and `admin log`: reads.
- The admin audit trail (`AdminAuditPort`, `admin.actions`): the record of
  every admin API request, whose changes the owning modules track with the
  `admin` actor (`identity.account_suspended`, `kernel.job_retried`, …).
  Tracking its rows would track each of them twice.
- Telling apart which operator ran the command: the envelope carries no
  database login or shell user (the audit trail keeps the database login).
- Any personal data or content in events: no address, no person's name, no
  note the operator wrote, and no account id that is not a Campbooks
  account id (another identity provider's `sub` may be an address).
