# Proposal

## Why

An IMAP mailbox (#311) is polled: a quiet one is read every few hours at
most, and new mail waits for the next run, up to the polling interval. Most
IMAP servers offer IDLE (RFC 2177): a signed-in session on the Inbox that
the server tells, as it happens, that mail arrived, was deleted or changed
flags. With it, new mail reaches the inbox within seconds and quiet
mailboxes cost fewer polls. A session open for hours is a different thing
from a run that signs in, reads and leaves, though: it must belong to one
process at a time, survive restarts and network trouble, never let a
hostile server drive the sync engine, and never starve the small servers
the session caps of #311 protect.

Reviewing those caps for long sessions also showed a fault in #311: one
global cap of four sessions per host, shared by every customer, would
starve everyone on a big provider (`imap.mail.yahoo.com`,
`imap.mail.me.com`) once a few dozen of its mailboxes are connected. This
change fixes it in the same design.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/318
(epic https://github.com/notacamp/campbooks-cloud/issues/308).

Modules touched: `connectors` (the IDLE pool, its lease table and the
session caps) and the server (it runs the pool in the process role that
runs sync jobs, `worker`, and in the combined `all`). No new module, no new
crate, no new cross-module dependency. One new `[[access.maintenance_scope]]`
entry in `architecture.toml` (the lease claim works across workspaces like
the stream dispatcher, through the lease table's own maintenance policies).

## What Changes

- **IDLE on the Inbox.** Each process that runs sync keeps a bounded pool of
  IDLE sessions (`CAMPBOOKS_CONNECTORS__IMAP__IDLE_SESSIONS`, at most
  `…__IDLE_SESSIONS_PER_WORKSPACE` of them for one workspace), behind its own
  flag `CAMPBOOKS_CONNECTORS__IMAP__IDLE_ENABLED` (off), on top of
  `CAMPBOOKS_CONNECTORS__IMAP__ENABLED` (off).
- **One holder per connection**, through a lease row renewed every minute
  that expires after three, released on a clean shutdown; a session whose
  lease is lost stops, and a watchdog stops them all before their leases can
  expire when the database stops answering. No database connection is held
  while a session waits.
- **Cutting access ends the session**: `needs_reauth`, a new password and
  removal withdraw the lease at once; a session re-checks at every refresh
  and on every change that it may still IDLE (not paused, not needing the
  member).
- **What the server reports in IDLE** (`EXISTS`, `EXPUNGE`, `FETCH`,
  `VANISHED`) asks the stream's existing schedule for a run now, at most once
  per `CAMPBOOKS_CONNECTORS__IMAP__IDLE_DEBOUNCE_SECONDS` (30): a server
  repeating `EXISTS` never causes more runs. IDLE is renewed every
  `CAMPBOOKS_CONNECTORS__IMAP__IDLE_REFRESH_MINUTES` (25, RFC 2177's limit
  is 29) with a `NOOP` between, which also proves the server still answers.
- **Reconnecting** backs off exponentially with jitter, goes through the
  host guard, the mail ports and the TLS rules of #310 every time, and opens
  the sealed password every time (never kept past one sign-in). A refused
  password counts with the sync runs' refusals (three in a row:
  `needs_reauth`); a busy server only backs off.
- **Polling stays.** A connection without a lease, a server without IDLE,
  and a mailbox quiet for `CAMPBOOKS_CONNECTORS__IMAP__IDLE_QUIET_DAYS` (14)
  are polled as today. While IDLE is held the stream's next poll is at least
  `CAMPBOOKS_CONNECTORS__IMAP__IDLE_POLL_SECONDS` (900) away: a safety poll
  that also reads the folders IDLE does not watch.
- **Session caps for long sessions**: one IDLE session per mailbox login;
  on a host Campbooks does not know as a big provider, IDLE takes at most
  half of the host's sessions and of a workspace's share of it, and the runs
  keep the rest; on a known provider (one of a short list of big providers' IMAP hosts,
  matched exactly) a
  workspace holds at most `CAMPBOOKS_CONNECTORS__IMAP__WORKSPACE_IDLE_SESSIONS_PER_HOST`
  (10).
- **#311's run caps revisited**: the per-host cap of
  `CAMPBOOKS_CONNECTORS__IMAP__SESSIONS_PER_HOST` (4) now applies only to
  hosts that are not known providers; a known provider's host takes up to
  `CAMPBOOKS_CONNECTORS__IMAP__PROVIDER_SESSIONS_PER_HOST` (200) sessions
  across all customers, IDLE included. The per-workspace share (2 runs) is
  unchanged everywhere.
- **A hostile server in IDLE** is read only through the bounded framer of
  #311, with caps on the responses and bytes of one IDLE period: past them
  the session is dropped and backs off.

## Non-goals

- IDLE on folders other than the Inbox, and NOTIFY (RFC 5465): the safety
  poll reads them.
- A shared IDLE service apart from the job worker, or holding sessions
  across process restarts.
- Exact run caps: the run buckets stay approximate as #311 designed them;
  only IDLE's caps are exact.
- Metrics for the pool (sessions held, wakes): a follow-up, with the
  observability spec.
- Turning IMAP or IDLE on by default: the connect screen (#314) does that.
