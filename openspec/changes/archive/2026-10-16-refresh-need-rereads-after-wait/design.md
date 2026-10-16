## Context
Follow-up to `harden-security-alert-auth` (archived 2026-10-16), whose design
describes the first-need check (it never waits outside the job) and the
`conversations.refresh_need` follow-up job.

## Decisions

**The job is the one place that waits.** When it gets to the check it holds no
needs row and no message lock (plain reads so far), so it may wait: it takes the
conversation `FOR KEY SHARE` and the message `FOR SHARE` blocking, under a 5 s
`lock_timeout` inside a savepoint, and a timeout makes it retry later. The
previous timeout is put back with a transaction-local `set_config(…, true)`,
not a session-wide `RESET`.

**The wait is bounded, not cycle-free.** While it waits for the message it holds
the conversation `FOR KEY SHARE`, which a request taking that conversation
`FOR UPDATE` waits on, so a longer cycle (for example with a rule run over
several conversations) is possible. The deadlock detector or the lock timeout
breaks it and the job retries. Waiting means a long-held lock does not use up
its ten attempts.

**Once it holds both locks, the job works the refresh out again.** Its reads
(`live_conversation`, the needs row, the candidate, what happened since) were
made before it waited, and under READ COMMITTED the writer it waited for may have
archived the conversation, or another refresh may have stored the first need.
So the job runs the refresh again, once, in the same transaction, and this
second pass does not wait: every read is fresh, and it may pick a newer message
than the one the job holds. The job's own locks pass at once; a message someone
else holds comes back busy, and the job rolls back and retries with fresh reads.
So the second pass can neither wait nor ask again, and never stores a need from
reads made before a wait. An archived conversation stores no open Reply need, and an existing
need is read under its row lock rather than overwritten from a stale view.

**Real order of each caller of `needs::refresh`:**
- `apply_changes`: messages, then the conversation's recount, then the needs
  row (and, for a first need, the check);
- flags: the conversation `FOR UPDATE`, then messages, then the check;
- rules (`carry_out` updates messages before it refreshes): the conversation,
  then messages, then the check;
- `judge` and `reopen`: the conversation, then the needs row;
- the decide pass and the backfill: the needs row (none yet), then the check.

The check never waits outside the job, so none of these gains a wait edge.
