## ADDED Requirements

### Requirement: IMAP push with IDLE
When `CAMPBOOKS_CONNECTORS__IMAP__ENABLED` and
`CAMPBOOKS_CONNECTORS__IMAP__IDLE_ENABLED` are both on (both default off),
each process that runs sync jobs SHALL hold at most
`CAMPBOOKS_CONNECTORS__IMAP__IDLE_SESSIONS` (default 50) IDLE sessions
(RFC 2177) on the Inbox of IMAP mailboxes, and SHALL hold no database
connection while a session waits. An `EXISTS`, `EXPUNGE`, `FETCH` or
`VANISHED` the server sends in IDLE SHALL ask the stream's schedule for a run
now, collapsing into a run already pending, at most once per
`CAMPBOOKS_CONNECTORS__IMAP__IDLE_DEBOUNCE_SECONDS` (default 30) per stream,
with the changes heard during that interval folded into one more ask at its
end. A session SHALL end IDLE (`DONE`), send `NOOP` and start IDLE again
every `CAMPBOOKS_CONNECTORS__IMAP__IDLE_REFRESH_MINUTES` (default 25, at
most 29).

#### Scenario: New mail during IDLE
- **WHEN** a message arrives in the Inbox of a mailbox whose session is in IDLE
- **THEN** exactly one run of that mailbox's stream is asked for, and that run announces the message

#### Scenario: A burst of new mail
- **WHEN** the server sends many `EXISTS` within one debounce interval
- **THEN** one run is asked for within the interval and one run is pending

#### Scenario: IDLE is renewed
- **WHEN** a session has been in IDLE for the refresh window
- **THEN** it sends `DONE`, `NOOP` and `IDLE` again on the same connection

#### Scenario: IDLE is off by default
- **WHEN** an instance reads IMAP mailboxes with `CAMPBOOKS_CONNECTORS__IMAP__IDLE_ENABLED` unset
- **THEN** no IDLE session is opened and every mailbox is polled

### Requirement: One IDLE holder per connection
An IMAP connection SHALL be held in IDLE by at most one process at a time,
through a lease renewed about every minute that expires after three
minutes. A session whose lease is lost, expired or withdrawn SHALL stop;
a process that cannot renew its leases for two minutes SHALL stop every
session it holds; a process that shuts down cleanly SHALL release its
leases.

#### Scenario: Two processes
- **WHEN** two processes claim IDLE sessions for the same connection
- **THEN** only one holds its lease and only one session signs in

#### Scenario: A lost lease
- **WHEN** a session's lease expires or is taken while it is in IDLE
- **THEN** the session stops at the holder's next renewal and another process may claim the connection

#### Scenario: Shutdown
- **WHEN** a process holding IDLE sessions shuts down cleanly
- **THEN** its sessions end and its leases are released for another process to claim

### Requirement: IDLE sessions reconnect safely
An IDLE session that ends with an error SHALL reconnect after an
exponential backoff with jitter, and each reconnect SHALL resolve the host
through the kernel's guard, use the mail ports and TLS rules of
`imap-connection`, and open the sealed password again, keeping it no longer
than the sign-in. A password the server refuses SHALL count towards the
same three consecutive refusals as sync runs; a server answering
`UNAVAILABLE`, `INUSE` or `LIMIT` SHALL only back off. After six failures
in a row the lease SHALL be given up for a cooldown, the stream still
polled.

#### Scenario: The host now resolves to a private address
- **WHEN** a session reconnects and the host's name now resolves to an address the guard refuses
- **THEN** nothing connects to it, and the lease is given up after six failures

#### Scenario: Password changed at the provider during IDLE
- **WHEN** two sync runs have had the password refused and a session's reconnect is refused too
- **THEN** the connection's health becomes `needs_reauth`, `ConnectionNeedsReauth` is published once, and the session ends

### Requirement: Polling stays beside IDLE
A stream SHALL be offered for IDLE only after a run whose server listed
`IDLE`, and only while it changed within
`CAMPBOOKS_CONNECTORS__IMAP__IDLE_QUIET_DAYS` (default 14); otherwise its
offer SHALL be withdrawn and the stream polled as before. While its lease is
held, a run SHALL set the stream's next poll no sooner than
`CAMPBOOKS_CONNECTORS__IMAP__IDLE_POLL_SECONDS` (default 900), a safety poll
that reads every folder.

#### Scenario: A server without IDLE
- **WHEN** a mailbox's server does not list `IDLE`
- **THEN** no session is claimed for it and its stream keeps its adaptive polling

#### Scenario: A quiet mailbox
- **WHEN** a mailbox held in IDLE has not changed for the quiet period
- **THEN** its next run withdraws the offer, its session stops, and it is polled

#### Scenario: Safety poll
- **WHEN** a run of a stream held in IDLE finds nothing new
- **THEN** the stream's next run is due no sooner than the IDLE poll interval

### Requirement: A server in IDLE is read within bounds
Everything an IMAP server sends during IDLE SHALL be read through the
session's bounded framer, at most 64 KiB a response, 1,000 responses and
1 MiB per IDLE period; past those the session SHALL be dropped and back off.
A session that gets no tagged answer within the command timeout of a
refresh SHALL be dropped. Logs SHALL carry ids, stable codes and counts,
never what the server said.

#### Scenario: A server repeating EXISTS
- **WHEN** the server sends `EXISTS` without end during IDLE
- **THEN** the session is dropped past the cap, at most one run is asked for, and the connection backs off

### Requirement: IDLE leases are isolated by workspace
The lease table SHALL have row-level security enabled and forced: a
workspace's transaction SHALL see only that workspace's leases, the lease
claim SHALL work across workspaces only in the maintenance scope, through
read and update policies of its own, and SHALL change only a lease's
holder and times.

#### Scenario: Another workspace
- **WHEN** a transaction of another workspace, or one with no workspace, reads the leases
- **THEN** it sees none of the workspace's leases

## MODIFIED Requirements

### Requirement: IMAP sessions are bounded and failures are classified
A sync run SHALL use one session, through the host guard and TLS of
`imap-connection`. Sessions SHALL be capped per IMAP host across all
processes, IDLE sessions included: a host that is not a fixed IMAP host of
the bundled ISPDB SHALL see at most
`CAMPBOOKS_CONNECTORS__IMAP__SESSIONS_PER_HOST` (default 4) sessions, and a
known provider's host at most
`CAMPBOOKS_CONNECTORS__IMAP__PROVIDER_SESSIONS_PER_HOST` (default 200). Of a
host's sessions, IDLE SHALL hold at most half; one workspace's runs SHALL
hold at most `CAMPBOOKS_CONNECTORS__IMAP__WORKSPACE_SESSIONS_PER_HOST`
(default 2); one workspace's IDLE sessions SHALL be at most
`CAMPBOOKS_CONNECTORS__IMAP__WORKSPACE_IDLE_SESSIONS_PER_HOST` (default 10)
and, on a host that is not a known provider, at most half its run share,
counted against it. One mailbox login SHALL hold at most one IDLE session.
Every answer of the server, the sign-in's included, SHALL be read within
bounds set before it is read, never sized by the server's own word. A login
refused in three consecutive sign-ins, by sync runs or IDLE sessions, SHALL
set the connection's health to `needs_reauth`; a server answering
`UNAVAILABLE`, `INUSE` or `LIMIT`, or refusing the connection, SHALL only
slow that stream down.

#### Scenario: Password changed at the provider
- **WHEN** the IMAP server refuses the stored password in three consecutive runs
- **THEN** the connection's health becomes `needs_reauth`, its streams pause, and `ConnectionNeedsReauth` is published once

#### Scenario: Busy server
- **WHEN** the server answers `[UNAVAILABLE]` at login
- **THEN** the stream is retried later and its health is not `needs_reauth`

#### Scenario: Many mailboxes on one provider
- **WHEN** five streams on the same IMAP host, not a known provider, are due at once
- **THEN** at most four run at the same time and the fifth runs at a later dispatch

#### Scenario: Many mailboxes on a known provider
- **WHEN** five streams on a known provider's IMAP host are due at once
- **THEN** all five run, each workspace within its share

#### Scenario: One workspace's share of a provider
- **WHEN** a workspace already holds two sessions to an IMAP host
- **THEN** its next stream on that host waits, and other workspaces' streams on it still run

#### Scenario: IDLE counts against a small host's sessions
- **WHEN** two IDLE sessions are open to a small host and two runs hold its other sessions
- **THEN** a third run on that host waits for the next dispatch

#### Scenario: One IDLE session per mailbox login
- **WHEN** the same mailbox login is connected in two workspaces and both may IDLE
- **THEN** one connection holds an IDLE session and the other is polled

#### Scenario: A workspace's IDLE share of a small host
- **WHEN** a workspace has two mailboxes on a small host whose run share is two
- **THEN** one of them is held in IDLE and the other is polled

#### Scenario: A huge answer to a sign-in
- **WHEN** the server answers a sign-in by announcing a literal larger than the sign-in allows
- **THEN** the sign-in is refused before any of it is read
