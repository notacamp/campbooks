# Design

How to add a tracked event is in `docs/tracking.md`; the decision is
ADR 0020. This change applies it to the `admin` module.

## Data ownership and events

Profiles live in `admin.profiles`, owned by `admin` and written only by
`campbooks_admin::cli` on the migration login (ADR 0015). Every snapshot is
read from that table in the transaction of the change: the profile before
the change by a new runtime `sqlx::query` (`SELECT … FOR UPDATE`), the
profile afterwards from what the existing queries return. No query the
module already has changes its text, so no `.sqlx` file changes. Events go
to `kernel.tracked_events` (the kernel's outbox); nothing else is written.
No domain event is published or consumed, no endpoint, scope or AI role
changes, and `admin.actions` gets exactly the rows it got before.

What an event may hold: ids, enums, booleans and timestamps. Never an
address, a person's name or the operator's note (only whether there is
one). The administrator is named by their Campbooks account id
(`pseudonymous`), and only when the profile's account id is one: a profile
can name the `sub` of another identity provider (`CAMPBOOKS_AUTH__ISSUER`),
which may be an address or a username, so an account id that is not a UUID
is left out of the snapshot.

**Scope.** Profiles belong to no workspace, so both events are
instance-wide. The CLI's transaction sets no workspace, and the tracker
writes an instance-wide event from it in the kernel scope for that one
insert, as for any instance-wide event.

## Actors

Both events are `system`, id `admin`: the kernel's context outside any
request or job (`TrackingContext::outside`) is `system` with the command's
name, which the server's `main` sets for every subcommand
(`tracking::set_command`, `admin` for `campbooks-server admin …`). There is
no `on_behalf_of` and no `operation` (a CLI command has no `operationId`);
the correlation is fresh. The envelope's `role` is `cli`: the admin
command builds its tracker from `CAMPBOOKS_TRACKING__*` with that role.
No kernel change is needed.

Who ran the command is not in the envelope: the actor types carry ids
only, and the shell user or the database login is not an account id. The
audit trail keeps the database login (`admin.actions.actor`, `via = 'cli'`)
as before.

With the sink `amqp`, the CLI only writes the outbox; a worker's relay
publishes the rows. With `none` (the default) nothing is written.

## Records (snapshots)

| Record | Fields |
| --- | --- |
| `admin.profile` | `account_id`?, `permissions` (list of `admin.permission`), `with_note`, `granted_at`, `updated_at`, `removed_at`? |

`?` marks an optional field. `account_id` is `pseudonymous`, present when
the profile's account id is a UUID (a Campbooks account id). `with_note`
says whether the operator wrote a note, never what. `removed_at` is set
when a revoke removed the whole profile: the snapshot is then the profile
as it is afterwards, with no permissions.

## Enums

`admin.permission`: `super`, `system_read`, `signups_review`, `users_read`,
`users_suspend`, `workspaces_read`, `jobs_read`, `jobs_retry`, the wire
names of `campbooks_kernel::admin::Permission` (`system.read`, …) in
`snake_case` as the plan requires. A stored name the kernel no longer knows
is left out of the lists.

## Events

Both are version 1, `live`, instance-wide, added in plan version 8 by this
change.

| Event | Fires when | Properties |
| --- | --- | --- |
| `admin.profile_granted` | `admin grant` (`cli::grant`) creates a profile or adds a permission it lacked, in the transaction that writes the profile and its audit entry. Permissions it already holds track nothing, with or without a new note | `profile` (afterwards), `before` (list of permissions it held; empty for a new profile), `granted` (list of permissions it gained), `new_profile` (bool) |
| `admin.profile_revoked` | `admin revoke` (`cli::revoke`) takes away at least one permission the profile held, or removes the whole profile, likewise. An account with no profile, or only permissions it does not hold, track nothing | `profile` (afterwards; `removed_at` and no permissions when removed), `before` (list of permissions it held), `revoked` (list of permissions it lost) |

`before` is there because the transition is what matters: what someone
could do before and after. No property repeats an envelope field.

## Coverage

The module serves no operation (in neither OpenAPI document) and registers
no job, schedule or handler, so it has no Coverage rows. The plan's
Coverage kinds (`operation`, `job`, `schedule`, `handler`) have none for a
CLI command, so the two commands are described here and in the events'
`fires_when`.

What else the module does, and why it tracks nothing:

| What | Why |
| --- | --- |
| `admin list` (`cli::list`) | a read |
| `admin log` (`cli::log`) | a read |
| `cli::grant` / `cli::revoke` writing `admin.actions` | the audit entry of the change these events track, in the same transaction |
| the audit port (`AdminAuditPort`, `admin::audit()`), appending a row to `admin.actions` per admin API request | the audit trail of admin requests: the change a request makes is tracked by the module that owns it, with the `admin` actor (`identity.account_suspended`, `workspaces.signup_request_approved`, `kernel.job_retried`, …); a read changes nothing |
| the profiles port (`AdminProfilesPort`) | a read on every admin request |

## Notes

- The audit trail still records a revoke of permissions the profile does
  not hold (it records the command), and a re-grant; only the tracked event
  is skipped, because nothing changed.
- A grant with an empty list of permissions (the command requires at least
  one) on an account with no profile creates an empty profile; it is
  tracked as `new_profile` with nothing `granted`.
- Patch numbers and plan versions are taken in merge order; this one builds
  on plan version 7 (`0007-assistant-attention-search-system`).
