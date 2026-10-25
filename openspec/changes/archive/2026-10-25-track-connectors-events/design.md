# Design

How to add a tracked event is in `docs/tracking.md`; the decision is
ADR 0020. This change applies it to the connectors module
(`crates/modules/connectors/`).

## Data ownership and events

The module owns the schema `connectors`. Every snapshot is read from its
own rows (`connectors.connections`, `connectors.streams`,
`connectors.sends`) in the transaction of the change, by
`domain/tracked.rs`. Every event belongs to the workspace of the
connection. Events go to `kernel.tracked_events`; nothing else is written.
No domain event, endpoint, scope or AI role changes; new queries are
runtime `sqlx::query`/`query_as` (no `.sqlx` file changes).

What an event may hold: ids (the connection's, stream's, inbox's, send's,
and for a calendar change the schedule module's request, calendar and
event ids), the grantor's user id (pseudonymous), enums (provider,
capability, health and its reason, phase, how a change or send ended),
booleans, counts and times. Never the account's address or display name,
the provider's account subject, an IMAP host or username, a mailbox key, a
folder or label name, a subject, a provider message or event id, an etag,
a cursor, or a provider's error text. Reasons are the module's own closed
codes, mapped to enums; a code the plan does not name yet becomes
`other`.

## Events

All workspace events.

| Event | Fires when | Actor | Snapshot and properties |
| --- | --- | --- | --- |
| `connectors.connection_added` | a grant becomes a new connection: `connectors.connect_imap`, or the grant-intake port from `identity.finish_sign_in` (a connect through the front door) or `identity.adopt_held_grant` | `user`, `anonymous` | `connection` |
| `connectors.connection_reconnected` | a grant refreshes the member's existing connection of the same account (a reconnect, a consent that extends it, an IMAP password given again); its paused streams resume | `user`, `anonymous` | `connection` (after), `before_health`, `capabilities_added` (list), `mailboxes_added` |
| `connectors.connection_removed` | `connectors.disconnect` (`disconnected`), or `connectors.revoke_departed_grants` on `workspaces.member_left` (`member_left`), deletes it; one already gone tracks nothing | `user`, `system` | `connection` (as it stood), `cause`, `mail` |
| `connectors.connection_health_changed` | a sync run, a send or an IDLE session changes the connection's health or its reason: `needs_reauth` (the grant was revoked, refused run after run, its credentials do not open, an IMAP server refused the password too often), `throttled` (rate limited), `failing` (errors run after run), and back to `ok` after a good run; the same health and reason again tracks nothing; reconnecting is `connection_reconnected` | `system` | `connection` (after), `before` |
| `connectors.connection_sync_requested` | `connectors.sync_now` schedules the connection's streams to run now; with no stream to run it tracks nothing | `user`, `client`, `assistant`, `automation` | `connection`, `streams` |
| `connectors.stream_synced` | a run of `connectors.sync_stream` (mail or calendar) stored at least one batch of changes, or moved the stream's phase (the first sync done, a resync); a run that found nothing tracks nothing | `system` | `stream` (after), `provider`, `before_phase`, `changes`, `batches` |
| `connectors.outbound_changes_made` | a run made the stream's pending outbound changes (members' changes to conversations) at the provider and at least one ended; changes left pending for a later run are not counted | `system` | `stream_id`, `connection_id`, `provider`, `applied`, `superseded`, `skipped`, `failed` |
| `connectors.calendar_change_completed` | a member's calendar change ended: made, superseded, skipped or failed by the calendar stream's run, or ended at once by `connectors.record_calendar_change` (it does not stand, the connection is gone or waits for the member) | `system` | `request_id`, `calendar_id`, `event_id`, `stream_id`?, `change`?, `outcome`, `reason`? |
| `connectors.send_succeeded` | `connectors.send_message` finds the provider has the message (sent now, or an earlier attempt found by its lookup), in the transaction that marks it sent; one settled already tracks nothing | `system` | `send` (after) |
| `connectors.send_failed` | a send fails for good (refused, too large, not connected, the grant gone, no longer standing, out of attempts), in the transaction that marks it failed; a send settled already tracks nothing | `system` | `send` (after), `error` |

Records:

- `connectors.connection`: `id`, `provider`, `granted_by` (user id,
  pseudonymous), `capabilities` (list), `health`, `health_reason`?,
  `address_verified`, `mailboxes` (its `messages` streams), `created_at`,
  `health_changed_at`.
- `connectors.stream`: `id`, `connection_id`, `capability`, `inbox_id`?,
  `phase`, `paused`, `last_synced_at`?, `last_change_at`?.
- `connectors.send`: `id`, `inbox_id`, `stream_id`?, `provider`?, `state`,
  `attempts`, `sent_copy`?, `created_at`.

Enums: `connectors.provider` (google, zoho, imap), `connectors.capability`
(messages, calendar, files, bank), `connectors.health` (ok, needs_reauth,
throttled, failing), `connectors.health_reason` (grant_revoked,
access_refused, credentials_unreadable, unknown_data_centre,
imap_settings_missing, imap_login_refused, rate_limited, provider_errors,
other), `connectors.phase` (backfill, incremental, resync),
`connectors.removal_cause` (disconnected, member_left),
`connectors.mail_choice` (delete, keep), `connectors.send_state` (pending,
sent, failed), `connectors.sent_copy` (pending, appended, found, provider,
skipped, failed), `connectors.send_error` (unavailable, not_connected,
needs_reauth, too_large, rejected, unknown_outcome, other),
`connectors.calendar_change` (create, update, update_occurrence, delete,
respond), `connectors.calendar_outcome` (applied, superseded, skipped,
failed), `connectors.calendar_reason` (changed_at_provider, unverified,
not_connected, needs_reauth, rejected, not_found, unavailable,
api_not_enabled, too_many_attempts).

### Why summaries for runs

A stream runs every minute or so and a backfill reads thousands of
messages; a run's changes are bounded by its page, but the number of runs
and messages is not. So a run is one `stream_synced` with counts, tracked
only when it stored something or moved the phase, and its outbound
changes one `outbound_changes_made` with counts by end. A calendar change
is a member's single request, so it is one event per change.

### Where each is tracked

- **Connections.** `domain::grants::save` (the one seam every grant goes
  through) reads the existing connection's health before it is refreshed,
  and after the streams are created or resumed tracks `connection_added`
  or `connection_reconnected`, in the transaction that stores the
  credentials. `domain::removal::delete` takes the cause (none for a
  workspace's deletion), reads the snapshot after locking the connection
  and tracks `connection_removed` when the row was deleted.
- **Health.** `domain::health::set` (and `needs_reauth`, which calls it)
  locks the connection, reads its health before the update, and tracks
  `connection_health_changed` when the row changed; it takes the
  transaction and the state (the tracker) instead of a bare connection and
  the event bus. Its callers are the sync run (rate limited, failing, back
  to ok, needs_reauth), the send job and the IDLE pool (needs_reauth).
  The IDLE pool runs outside any request or job, so its events are
  `system` with the process's command.
- **Sync.** `SyncStream::apply` (mail) and `sync_calendars` (calendar)
  track `stream_synced` in the job's transaction, after the stream row is
  updated. `record_push` (Gmail, Zoho) and `record_imap_push` track
  `outbound_changes_made` in the transaction that records the outcome.
  The calendar run's `announce` tracks `calendar_change_completed` with
  each `connectors.calendar_change_completed` it publishes, as does
  `record_calendar_change` for the changes it ends at once.
- **Sends.** `finish_sent` tracks `send_succeeded` and `fail` tracks
  `send_failed`, each after its compare-and-set update succeeded.
- **Sync now.** `sync_now::execute` tracks `connection_sync_requested` in
  its transaction when it scheduled at least one stream.

## Coverage

Operations (`openapi/openapi.json`, tag `connectors`; the admin list is a
read):

| Operation | Events / exemption |
| --- | --- |
| `connectors.connect_imap` | `connection_added`, `connection_reconnected` |
| `connectors.disconnect` | `connection_removed` |
| `connectors.sync_now` | `connection_sync_requested` |
| `connectors.start_connection` | exempt: only starts a consent through the identity module's front door; nothing of connectors changes, and the connection it becomes is tracked as `connection_added` or `connection_reconnected` |

`connectors.discover_imap` is a POST that only reads (a body query: it
looks up a mailbox's servers), so it needs no row.

Other modules' operations that hand a grant to the module through the
grant-intake port list the connection events with their own:
`identity.finish_sign_in` and `identity.adopt_held_grant` gain
`connectors.connection_added` and `connectors.connection_reconnected`.

Jobs:

| Job | Events / exemption |
| --- | --- |
| `connectors.sync_stream` | `stream_synced`, `outbound_changes_made`, `calendar_change_completed`, `connection_health_changed` |
| `connectors.send_message` | `send_succeeded`, `send_failed`, `connection_health_changed` |
| `connectors.retag_bindings` | exempt: maintenance, no action of anyone: MACs of pending changes re-tagged after a key rotation |

Schedules:

| Schedule | Exemption |
| --- | --- |
| `connectors.dispatch_due` | maintenance, no action of anyone: only enqueues the runs that are due, which track |

Event handlers:

| Handler | Events / exemption |
| --- | --- |
| `connectors.revoke_departed_grants` | `connection_removed` |
| `connectors.record_calendar_change` | `calendar_change_completed` |
| `connectors.queue_send` | exempt: records the send conversations asked for (tracked there); its outcome is `send_succeeded` or `send_failed` |
| `connectors.request_changes` | exempt: records the changes conversations asked for (tracked there as the conversation's changes) for the stream's next run, which tracks `outbound_changes_made` |
| `connectors.apply_calendar_sync` | exempt: projection of the schedule module's calendar switched on or off (tracked there): which calendars the stream reads |
| `connectors.revoke_workspace_grants` | exempt: consequence of `workspaces.workspace_deleted` (tracked by workspaces): the deleted workspace's connections go with it |

## Notes

- `request_changes` records a change it can never make (an unsupported
  label) as `skipped` at once. That is not counted by
  `outbound_changes_made`, which counts what a run did at the provider;
  the member's change itself is tracked by conversations.
- Patch numbers and plan versions are taken in merge order; this one
  builds on plan version 5 (`0005-workspaces`).
