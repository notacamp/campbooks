# Proposal

## Why

New Zoho mail shows up minutes after other mail clients get it. Zoho has no
push and no change log, so a mailbox is polled, and all of the delay is the
wait for the next poll. In prod a quiet mail stream is synced every 180 s,
not the 120 s cap set by `CAMPBOOKS_CONNECTORS__POLL__MAX_SECONDS`.

The dispatcher ticks once a minute. A run sets its next due time to
`now + interval`, and `now` is a second or two after the tick, so the due
time lands just after a tick and the stream waits a whole extra minute for
the next. Gmail and the calendars are polled through the same schedule and
lose the same minute.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/409.

Modules touched: `connectors` only. No new module, dependency, crate,
migration, event, operation or job.

## What Changes

- **A poll is made due a tick's slack early.** The adaptive next poll of a
  mail or calendar stream is `interval - 15 s` after the run (`TICK_SLACK`),
  so the tick that the interval reaches takes it. A wait that must be kept
  whole (a request budget, a provider's Retry-After, an IDLE or notification
  safety poll) is never cut short.
- **Zoho mail is polled at most a minute apart.** New setting
  `CAMPBOOKS_CONNECTORS__ZOHO__POLL_MAX_SECONDS` (default 60, 30 to 14400)
  caps a Zoho mailbox's interval, whatever the global maximum. It is not
  tied to the global poll settings. A missing value uses the default; an
  unusable one logs a warning at startup, uses 60, and never stops the
  server. Calendars are not capped.

Not in scope: near-instant delivery (an IMAP IDLE session for Zoho OAuth
mailboxes).
