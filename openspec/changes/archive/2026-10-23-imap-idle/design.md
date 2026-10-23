# Design

## Context

An IMAP stream (#311) is read by `connectors.sync_stream`: one run signs in,
reads a bounded page over one session, logs out, and sets the stream's next
due time on its row of `connectors.sync_due`. The one-minute dispatcher
(`connectors.dispatch_due`) leases due rows across workspaces in the
maintenance scope and enqueues each run in its stream's workspace, with the
stream's unique and ordering key, so "sync now", ticks and pushes collapse.
Runs hold one slot of two token buckets in `connectors.provider_budgets`:
`imap:<host>:<workspace>` (2) and `imap:<host>` (4). Three refused sign-ins
in a row (`cursor.login_refusals`) set `needs_reauth`.

This change adds the sessions that stay: IDLE (RFC 2177) on the Inbox, held
by a pool in the processes that run sync jobs. IDLE never reads mail: it
only tells the existing engine when to run.

## Where the sessions live

| | Where IDLE sessions run |
|---|---|
| One job per session | a job holds a worker slot and its lease for hours; the queue's retries and timeouts are built for short work |
| A process role of its own (`idle`) | one more thing to deploy and probe; self-hosters run one container |
| **A task beside the job worker** (chosen) | like the tracked events relay (#321): started by `worker` and `all`, stopped with them, given the same grace |

The pool (`connectors::features::imap_idle`) is built from the module's
`Deps` and the process's `AppState`, and only when
`CAMPBOOKS_CONNECTORS__IMAP__ENABLED`, `…__IDLE_ENABLED` and the instance
secret are all set. It holds at most `…__IDLE_SESSIONS` sessions (default
50, per process). A session is one tokio task with one TLS connection and
no database connection: each database step (load, sync now, a refusal) is a
short transaction of its own.

## One holder per connection: the lease

| | How a connection gets one holder |
|---|---|
| A Postgres advisory lock held while idling | needs a database connection per session for hours: 50 sessions would hold 50 of the pool's connections |
| A slot of the `provider_budgets` bucket | approximate by design: the slot refills while the session is still open |
| **A lease row with an expiry** (chosen) | exact, survives a crash (it expires), no connection held |

`connectors.imap_idle_leases` (migration
`20261030020000_connectors__imap_idle_leases`), one row per IMAP connection
that may IDLE:

- `connection_id` (primary key), `workspace_id`, `stream_id`, with composite
  foreign keys `(connection_id, workspace_id)` to the connection and
  `(stream_id, connection_id)` to the stream (both cascading on delete; the
  migration adds the matching unique keys to `connections` and `streams`),
  so a row can never pair one workspace's connection with another's stream;
- `login_key` and `host_key`: HMAC-SHA256 (the instance's signer, contexts
  `connectors.imap_idle.login` and `connectors.imap_idle.host`) of the
  mailbox's lower-cased username with its host, and of the host; 32 hex
  characters each. They let the claim count sessions per login and per host
  without the table holding an address or a host name (a custom domain's
  host can name a person);
- `provider_host`: whether the host is a known provider (below);
- `offered`: whether the last word on it was an offer; a **withdrawn** row is
  kept, unheld and with its times, so a cooldown survives the next offer;
- `holder` (the holding process's random id, per start), `expires_at`,
  `considered_at` (when a claim last looked at it).

A row is **live** while `holder IS NOT NULL AND expires_at > now()`, and
**claimable** while `offered AND (expires_at IS NULL OR expires_at <= now())`
(a released row, an expired lease, or a cooldown that ended). Times are the database's
`now()`, the one clock every process shares.

- **Claim** (every minute, while the pool has room): one maintenance
  transaction takes a transaction advisory lock (one key for all claims, so
  two processes never count the same free room twice), reads the live rows'
  keys, reads up to 500 claimable rows whose login nobody holds, **longest
  unlooked-at first**, stamps every row it read (`considered_at`), chooses
  within the caps (below), and sets `holder` and `expires_at = now() + 180 s`
  on the chosen ones still claimable. The lock is held only for that short
  transaction. Rows passed over at their caps go to the back of the queue,
  so no number of them (a workspace could make many) hides the others: the
  window rotates through every claimable row within a few claims.
- **Renew** (every minute): one statement extends every row this process
  holds and is still live; a row it no longer gets back (taken, withdrawn,
  expired, deleted) stops its session at once.
- **Losing the database**: every renewal and claim is bounded (20 s, well
  within the minute between renewals), the time of a renewal is taken
  *before* it is asked (the database's `now()` for the new expiry is no
  earlier), and a watchdog beside the ticks stops every session once the last
  renewal that succeeded was asked for 120 s ago, whatever a tick is doing.
  So a process cut off from the database stops its sessions at most 120 s
  after its last good renewal, 60 s before its leases (180 s) can expire for
  others.
- **Release**: on a clean shutdown, `holder` and `expires_at` are cleared
  for every row the process holds (one statement), after its sessions said
  `DONE` and `LOGOUT` (5 s at most).
- **Give up**: a session that failed six times in a row clears its holder
  and sets `expires_at` 30 minutes ahead, a cooldown before any process
  claims it again; the stream is polled meanwhile.

### Reaching the lease rows across workspaces under row-level security

| | How the pool reads and claims other workspaces' rows |
|---|---|
| A privileged role or `BYPASSRLS` | never: the runtime role must not escape row-level security (`check_row_security`) |
| `SECURITY DEFINER` functions | narrow, but new definer grants on a module table, and a function per statement the claim needs |
| **The maintenance scope with the table's own policies** (chosen) | the stream dispatcher's pattern (#196, #207): reviewed in `architecture.toml` per file |

The table has row-level security enabled and forced, the
`workspace_isolation` policy every connectors table has, and two
maintenance policies: `maintenance_reads_idle_leases` (`FOR SELECT`) and
`maintenance_claims_idle_leases` (`FOR UPDATE`, `USING` and `WITH CHECK`),
both admitting only `campbooks.maintenance_scope = 'on'`
(`kernel::db::begin_maintenance`). The maintenance scope may neither insert
nor delete, and no kernel table admits it. A trigger keeps a row's
connection and workspace for good, and in the maintenance scope lets only
`holder`, `expires_at` and `considered_at` change (never what was offered,
nor whether it is). The maintenance
transactions live in one file, `features/imap_idle/lease.rs`, listed under
`[[access.maintenance_scope]]`. Grants: `SELECT, INSERT, UPDATE, DELETE` to
`campbooks_app`, nothing else.

Everything else the pool does happens in the connection's workspace
(`begin_for`): reading the stream, the server and the sealed password,
asking for a run, counting a refused password, withdrawing a row. The
maintenance scope never sees a password, an address or a host name: ids,
two keyed digests, a flag and times.

### Who decides that a connection may IDLE

The sync run, in its own workspace transaction, **offers** the row (insert
or update its keys) after a successful run when:

- `…__IDLE_ENABLED` is on;
- the server listed `IDLE` among its capabilities in that run (kept in the
  cursor, `idle`);
- and the stream changed within `…__IDLE_QUIET_DAYS` (14; its
  `last_change_at`).

Otherwise it **withdraws** the row (`offered = false`, holder cleared, times
kept): the holder's next renewal loses it and the session stops. A quiet
mailbox therefore drops IDLE at its next poll, and gets it back at the first
run that finds a change.

Access cut by the member or the provider ends a session within a renewal
period, not when the server next drops it:

- **`needs_reauth`** (`health::needs_reauth`, from a run, a session or a
  token refresh) withdraws the row with the health change, in the same
  transaction;
- **a new password** (`credentials::store_password`, a reconnect) withdraws
  the row: the session signed in with the old one ends, and the next run
  offers the row again (claimable once the old lease's time is up);
- **removal** deletes the row with the connection (the foreign keys
  cascade);
- **pausing** a stream, by any path (today only `needs_reauth` pauses), withdraws
  the row in the same transaction through a trigger on `connectors.streams`
  (`streams_paused_withdraw_idle`), so a future pause endpoint cannot forget
  it;
- and the session checks for itself: at every refresh, and whenever it asks
  for a run, it checks under `begin_for` that the stream is not paused, the
  connection does not need the member, the server still offers IDLE, the row
  is still offered and this process still holds it. If not, it ends IDLE
  (`DONE`), logs out, and withdraws the row (or, when only the lease was
  lost, leaves it).

The pool also withdraws a row itself when the server, once signed in, does
not offer `IDLE` or refuses the command.

## The session

On every (re)connect, in order:

1. In a workspace transaction: the stream (not paused), the connection
   (provider `imap`, not `needs_reauth`), its server and username, and its
   password opened from the sealed column; then the transaction ends.
2. `Session::open`, exactly as a sync run signs in: the host resolved again
   through the kernel's guard (`resolve_socket_with`, the mail ports, private
   addresses only where allowed), TLS or STARTTLS with the certificate
   checked, the same sign-in. The password is dropped as soon as the sign-in
   returns: the pool never keeps it.
3. `CAPABILITY`: no `IDLE` → withdraw, log out, end.
4. `EXAMINE INBOX` (read-only; IDLE needs a selected folder).
5. `IDLE`, wait for the server's `+`.
6. Wait. A response is framed by the session's bounded framer (`response`:
   the line and every literal bounded before a byte is read), at most 64 KiB
   one, at most 1,000 responses and 1 MiB per IDLE period; past either the
   session is dropped (`imap_idle_flood`) and the connection backs off.
   `EXISTS`, `EXPUNGE`, `FETCH` and `VANISHED` are changes; anything else
   (`* OK Still here`) is only counted; `* BYE` or a tagged answer ends the
   session.
7. Every `…__IDLE_REFRESH_MINUTES` (25): `DONE`, the tagged `OK` within the
   command timeout, `NOOP` (the keepalive: traffic both ways through any NAT,
   and a tagged answer that proves the server is alive), `IDLE` again. A
   server that answers none of it within `…__TIMEOUT_SECONDS` is dead: the
   session never outlives the refresh window plus that margin without a word
   from the server.

The wait itself reads the socket with no per-read timeout until the next
refresh (or the debounce's due time, below), and stops at once when the
pool asks (lease lost, shutdown); one read of a few KiB, cancel-safe, so a
stop between two responses leaves the session in step for `DONE` and
`LOGOUT`.

Nothing the server says is logged or kept: logs carry the workspace,
connection and stream ids, a stable code (`imap_idle_flood`,
`imap_timed_out`, `imap_login_refused`, …) and counts. A refused sign-in's
response code is logged only when it is one of RFC 3501's or RFC 5530's;
any other atom the server sent is logged as `other`.

### Changes ask for one run: the debounce

| | When a change is heard |
|---|---|
| Run on every change | a server repeating `EXISTS` is a sync storm |
| Trailing only (run once the server is quiet) | a busy mailbox might never run; new mail always waits the interval |
| **Leading, and one trailing per interval** (chosen) | the first change runs now; changes within `…__IDLE_DEBOUNCE_SECONDS` (30) after it fold into one more ask at the interval's end |

"Ask for a run" is `schedule::run_now` in the stream's workspace: the
stream's due row leased and `connectors.sync_stream` enqueued with its unique
and ordering key, as "sync now" does, so a run already pending absorbs it.
The trailing ask covers mail that arrived while a run was already past the
Inbox; a pending run absorbs it too. So a stream gets at most two asks per
interval whatever the server says, and at most one run is pending.

### Reconnecting

Any failure (unreachable, TLS, timeout, closed, flood, busy) waits
`min(5 s × 2^failures, 15 min)`, drawn uniformly from its upper half
(jitter: sessions dropped together by one server restart do not come back
together). A session that held IDLE before failing starts counting again
from one. Six failures in a row give the row up with its 30-minute cooldown.

- **A refused password** (`Refusal::ImapLoginRefused`) is counted on the
  stream's cursor exactly as a sync run's: both now go through one
  function, an atomic increment of `cursor.login_refusals` after locking the
  connection (removal's order). The third in a row, from runs or sessions in
  any mix, sets `needs_reauth` (`imap_login_refused`), pauses the streams,
  publishes `ConnectionNeedsReauth` once, and withdraws the row. A run that
  signs in resets the count, as before.
- **A busy server** (`UNAVAILABLE`, `INUSE`, `LIMIT`) backs off and is never
  counted as a refusal.
- **A host the guard refuses** (it now resolves to a private or metadata
  address) is a failure like any other: nothing is sent to it, and after six
  the row is given up.

## Polling with and without IDLE

Nothing changes for a stream without a live lease. While its lease is live,
a run that finds nothing new sets the next due time at least
`…__IDLE_POLL_SECONDS` (900) away; the stored interval keeps adapting as
before, so a stream that loses IDLE goes back to its usual cadence at once.
The safety poll still reads every folder: IDLE watches only the Inbox, so
Sent, Archive and the member's folders are seen within that time.

## Session caps

Sessions are what a mail server counts, and IDLE sessions are long: they
cannot draw from a refilling bucket, which would hand their slot back after
15 minutes while they still hold it. And #311's per-host cap (4 sessions
per host across all customers) protects a small or self-hosted server, but
on a big provider's shared host it would let four customers in at a time.

| | Per-host cap |
|---|---|
| One cap for every host (#311) | starves everyone on a big provider |
| No per-host cap, per-login and per-workspace caps only | a small server shared by several workspaces (a hosting company's host, a club's server) sees as many sessions as there are mailboxes |
| **A small cap for unknown hosts, a large one for known providers** (chosen) | protects small servers, lets big providers serve their customers, and still bounds what one instance sends a provider (its address's reputation) |

A host is a **known provider** when it is one of a short list of big
providers' IMAP hosts kept in the code (`imap::PROVIDER_HOSTS`: Gmail,
Outlook, Yahoo and AOL, iCloud, Fastmail, GMX and web.de, Yandex, Zoho,
mail.com, Mail.ru), matched exactly. Every other host, a small ISP of the
bundled ISPDB included, keeps the small hosts' caps: the ISPDB lists many
small providers that a provider cap would not protect. The list changes only
with a release: a member cannot make a host "known" by typing it.

The caps, all counted per IMAP host across every process:

| | small host (default) | known provider (default) |
|---|---|---|
| all sessions of the host (runs + IDLE) | `SESSIONS_PER_HOST` (4) | `PROVIDER_SESSIONS_PER_HOST` (200) |
| IDLE sessions of the host | half of that (2) | half of that (100) |
| one workspace's runs on the host | `WORKSPACE_SESSIONS_PER_HOST` (2) | the same (2) |
| one workspace's IDLE sessions on the host | half of its run share (1), at most `WORKSPACE_IDLE_SESSIONS_PER_HOST` | `WORKSPACE_IDLE_SESSIONS_PER_HOST` (10) |
| one mailbox login (username at host) | 1 IDLE + its stream's one run | the same |

- IDLE's caps are **exact**: counted from the live leases inside the claim
  lock.
- IDLE **counts against the runs** on small hosts: a run takes a slot only
  when the bucket holds one more token than the live IDLE sessions of the
  host (and of the workspace on the host). Since IDLE takes at most half,
  runs always keep the other half. With a share of 1, a workspace never
  IDLEs on a small host: it polls.
- On a known provider, IDLE counts against the host's total (same rule) but
  not against a workspace's run share: there the provider's own limits are
  per account, and one login holds at most one IDLE session.
- One login: the stream of a connection runs one at a time (its ordering
  key) and holds at most one IDLE session, whichever of the login's
  connections (in any workspace) holds it.
- One workspace, in one process's pool: at most
  `IDLE_SESSIONS_PER_WORKSPACE` (5) sessions, whatever the hosts. Host caps
  count host names: a workspace with a wildcard domain (`a1.`, `a2.`, …)
  would otherwise get a session per name and fill every pool. With the
  claim's rotation, the pools' room goes round the workspaces.

The run buckets stay approximate as #311 designed them (a run longer than a
quarter of the refill lets one more start); making runs exact would need
leases for runs too, a follow-up only if a server ever complains.

## Data ownership and events

Schema `connectors`. New table `connectors.imap_idle_leases` (above). The
stream's cursor gains `idle` (whether the server offers IDLE). No event is
added or changed: a session's asks are `connectors.sync_stream` runs, a
refused password ends as today's `ConnectionNeedsReauth`. No endpoint, no
scope, no AI.

## Tracked events

None added or changed. The pool is not an operation, a job, a schedule or
an event handler, so it needs no Coverage row; the runs it asks for are
`connectors.sync_stream` jobs, already covered.

## Settings

`CAMPBOOKS_CONNECTORS__IMAP__*` gains:

| Setting | Default | Bounds |
|---|---|---|
| `IDLE_ENABLED` | `false` | |
| `IDLE_SESSIONS` | 50 per process | 1 to 10,000 |
| `IDLE_SESSIONS_PER_WORKSPACE` | 5 per process | 1 to `IDLE_SESSIONS` |
| `IDLE_DEBOUNCE_SECONDS` | 30 | 1 to 3,600 |
| `IDLE_REFRESH_MINUTES` | 25 | 1 to 29 (RFC 2177) |
| `IDLE_QUIET_DAYS` | 14 | 1 to 365 |
| `IDLE_POLL_SECONDS` | 900 | `POLL__MIN_SECONDS` to `POLL__MAX_SECONDS` |
| `PROVIDER_SESSIONS_PER_HOST` | 200 | at least `SESSIONS_PER_HOST` |
| `WORKSPACE_IDLE_SESSIONS_PER_HOST` | 10 | at least 1 |

## Risks

- **A provider limits connections per source address.** 100 IDLE sessions
  from one instance to one provider host may be more than it likes; the
  provider cap is a setting, and a refused or busy connection only backs
  off.
- **Clock skew between processes.** Leases use the database's clock; a
  process stops its sessions 60 s before its leases can expire, on its own
  monotonic clock.
- **A server that never answers `DONE`.** The session is dropped after the
  command timeout and backs off; a run never waits for it.
- **The instance secret rotates.** Login and host keys change at each
  stream's next run (the row is offered again with new keys); until then a
  login could be held twice for at most one poll interval.

## Follow-ups

- Metrics: sessions held, changes heard, asks, flood drops (with the
  observability spec).
- Exact run caps with leases, if a server ever complains about the bucket's
  approximation.
- Keying hosts by the address they resolve to rather than by name (as #311's
  run caps do too): several names for one server count as several hosts.
  The per-workspace pool cap bounds what one workspace gets that way.
- IDLE (or NOTIFY) on Sent and Archive.
